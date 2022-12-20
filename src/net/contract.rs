use futures::stream::{self, StreamExt};
use secp256k1::SecretKey;
use serde::Deserialize;
use serde_json::Value;
use std::{fs::OpenOptions, io::BufReader, str::FromStr};
use web3::{
	api::Web3,
	contract::{Contract, Options},
	error::Error as Web3Error,
	signing::SecretKeyRef,
	transports::Http,
	types::{Address, BlockId, BlockNumber, Bytes, Transaction, TransactionReceipt, H256, U256},
};

use super::{
	super::cli::{ListContext, NewContext},
	error::Error,
	payload::deploy_metadata,
};

/// Details of the Beacon DAO
const DEFAULT_NAME: &str = "Vision DAO";
const DEFAULT_DESCRIPTION: &str =
	"The Vision DAO is a DAO that governs the Beacon DAO layer of the Vision ecosystem.";
const DEFAULT_SYMBOL: &str = "VIS";

// 1_000_000 * 10^18
const DEFAULT_SUPPLY: U256 = U256([2003764205206896640, 54210, 0, 0]);

/// A JSON object that can be deployed as a contract by having a specified bytecode.
#[derive(Deserialize)]
struct DeployableContract {
	bytecode: String,
	abi: Value,
}

/// Gets the bytecode of the Idea.sol contract in the specified contracts dir.
/// Returns the raw source of the contract, and the bytecode.
fn with_contract(contracts_dir: String) -> Result<(Vec<u8>, DeployableContract), Error> {
	let f = OpenOptions::new()
		.read(true)
		.open(format!("{contracts_dir}/contracts/Idea.sol/Idea.json"))?;
	let src_reader = BufReader::new(f);

	let parsed: DeployableContract = serde_json::from_reader(src_reader)?;
	let src = serde_json::to_vec(&parsed.abi)?;

	// Extract the bytecode from the compiled contract
	Ok((src, parsed))
}

/// Deploys an instance of the Beacon DAO using the details specified by the
/// context.
pub async fn deploy(ctx: Box<NewContext>) -> Result<Address, Error> {
	let NewContext {
		private_key,
		eth_uri,
		eth_chain_id,
		contracts_dir,
		modules,
		ipfs,
		..
	} = *ctx;

	let secret_key =
		SecretKey::from_str(private_key.as_str()).map_err(|e| Error::Serialization(Box::new(e)))?;
	let ref_key = SecretKeyRef::new(&secret_key);

	// Wrapper for the API using the specified URL
	let web3 = Web3::new(Http::new(eth_uri.as_ref())?);

	log::debug!("connected to web3 API: {eth_uri}");

	// Load the source of the Idea.sol contract for deployment
	let (src, DeployableContract { abi: _, bytecode }) = with_contract(contracts_dir)?;

	log::debug!("loaded contract bytecode: {:?}", bytecode);
	log::debug!("deploying metadata to IPFS");

	// Deploy the metadata required for the contract, including specified
	// payloads
	let meta = deploy_metadata(&ipfs, DEFAULT_NAME, DEFAULT_DESCRIPTION, modules).await?;

	log::info!("deployed metadata at: {:?}", meta);

	// Deploy an instance of the contract form the specified address
	Ok(Contract::deploy(web3.eth(), src.as_slice())?
		.confirmations(2)
		.options(Options::with(|opt| {
			opt.gas = Some(4_000_000.into());
			opt.gas_price = Some(2_000_000_000.into());
		}))
		.sign_with_key_and_execute(
			bytecode.strip_prefix("0x").ok_or(Error::InvalidInput)?,
			(
				DEFAULT_NAME.to_owned(),
				DEFAULT_SYMBOL.to_owned(),
				DEFAULT_SUPPLY,
				meta.cid_string,
			),
			ref_key,
			Some(eth_chain_id),
		)
		.await?
		.address())
}

/// Gets a list of the addresses of contracts deployed using the context
/// information.
pub async fn list(
	ListContext {
		eth_uri,
		contracts_dir,
		private_key,
		eth_chain_id: _,
	}: ListContext,
) -> Result<Vec<String>, Error> {
	// Wrapper for the API using the specified URL
	let web3 = Web3::new(Http::new(eth_uri.as_ref())?);

	// Compare the bytecode of contracts deployed to the address with contracts
	// located in contracts_dir
	let (
		_,
		DeployableContract {
			abi: _,
			bytecode: bc_hex,
		},
	) = with_contract(contracts_dir)?;
	let bytecode = Bytes(hex::decode(bc_hex)?);

	// Fetch transactions
	let sender = web3
		.parity_accounts()
		.new_account_from_secret(
			&H256::from_str(private_key.as_ref()).map_err(|_| Web3Error::Internal)?,
			"",
		)
		.await?;

	let mut deployed = Vec::new();
	let until = web3.eth().block_number().await?.as_u64();

	// Iterate through blocks and look for transactions from the sender that
	// create a contract, until the sender's balance is 0
	for i in until..=0 {
		if let Some(txs) = web3
			.eth()
			.block_with_txs(BlockId::Number(BlockNumber::Number(i.into())))
			.await?
			.map(|block| block.transactions)
		{
			let web3 = &web3;

			// Look for transctions from me that have records containing the
			// address of contracts deployed (receipts)
			let receipts = stream::iter(txs.into_iter())
				.then(async move |tx| {
					web3.eth()
						.transaction_receipt(tx.hash)
						.await
						.map(|v| v.map(|v| (tx, v)))
				})
				.filter_map(async move |v| v.ok())
				.filter_map(async move |v| v)
				.collect::<Vec<(Transaction, TransactionReceipt)>>()
				.await;

			for (tx, receipt) in receipts {
				// Check if the transaction deploys an instance of Idea contract
				// if so, record the recipient, which is the created contract
				if let Some(contract_addr) = receipt.contract_address && receipt.from == sender && tx.input == bytecode {
                    deployed.push(contract_addr.to_string());
                }
			}

			continue;
		}

		break;
	}

	Ok(deployed)
}
