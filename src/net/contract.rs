use serde::Deserialize;
use std::{fs::OpenOptions, io::Read, str::FromStr};
use web3::{
    api::Web3,
    contract::Contract,
    error::Error as Web3Error,
    transports::Http,
    types::{H256, U256},
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
}

/// Deploys an instance of the Beacon DAO using the details specified by the
/// context.
pub async fn deploy(
    NewContext {
        private_key,
        eth_uri,
        contracts_dir,
        modules,
        ipfs,
    }: NewContext,
) -> Result<String, Error> {
    // Wrapper for the API using the specified URL
    let web3 = Web3::new(Http::new(eth_uri.as_ref())?);

    // Load contract source code
    let mut src = Vec::new();

    let mut f = OpenOptions::new()
        .read(true)
        .open(format!("{contracts_dir}/contracts/Idea.sol/Idea.json"))?;
    f.read(&mut src)?;

    // Extract the bytecode from the compiled contract
    let parsed: DeployableContract = serde_json::from_slice(src.as_slice())?;

    // Deploy the metadata required for the contract, including specified
    // payloads
    let meta = deploy_metadata(&ipfs, DEFAULT_NAME, DEFAULT_DESCRIPTION, modules).await?;

    // Deploy an instance of the contract form the specified address
    Ok(Contract::deploy(web3.eth(), src.as_slice())?
        .confirmations(0)
        .execute(
            parsed
                .bytecode
                .strip_prefix("0x")
                .ok_or(Error::InvalidInput)?,
            (
                DEFAULT_NAME.to_owned(),
                DEFAULT_SYMBOL.to_owned(),
                DEFAULT_SUPPLY.to_string(),
                meta.cid_string,
            ),
            web3.parity_accounts()
                .new_account_from_secret(
                    &H256::from_str(private_key.as_ref()).map_err(|_| Web3Error::Internal)?,
                    "",
                )
                .await?,
        )
        .await?
        .address()
        .to_string())
}
