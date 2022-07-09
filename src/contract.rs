use super::cli::{Command, ParseError};
use serde::Deserialize;
use serde_json::Error as SerializationError;
use std::{
    error::Error,
    fmt,
    fs::OpenOptions,
    io::{Error as IoError, Read},
};
use web3::{
    api::Web3,
    contract::{deploy::Error as DeployError, Contract},
    error::Error as Web3Error,
    ethabi::Error as ContractError,
    transports::Http,
};

/// Details of the Beacon DAO
const DEFAULT_NAME: &str = "Vision DAO";
const DEFAULT_SYMBOL: &str = "VIS";
const DEFAULT_SUPPLY: u64 = 1_000_000_000_000_000_000_000_000;

/// A JSON object that can be deployed as a contract by having a specified bytecode.
#[derive(Deserialize)]
struct DeployableContract {
    bytecode: String,
}

// Generate From<T> bindings for these variant types. This comes from a crate
// I made. It works moderately well, but this could be cleaned up with a
// proc macro to prevent mixing syntax.
convertable_error! {
    /// An error that occurred while interfacing with the Ethereum network.
    #[derive(Debug)]
    pub enum EthError {
        (ConfError(ParseError), [(ParseError, Self::ConfError)]),
        (Web3Error(Web3Error), [(Web3Error, Self::Web3Error)]),
        (ContractError(ContractError), [(ContractError, Self::ContractError)]),
        (DeployError(DeployError), [(DeployError, Self::DeployError)]),
        (IoError(IoError), [(IoError, Self::IoError)]),
        (SerializationError(SerializationError), [(SerializationError, Self::SerializationError)]),
    }
}

impl fmt::Display for EthError {
    fn fmt(&self, w: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConfError(e) => write!(w, "configuration error: {e}"),
            Self::Web3Error(e) => write!(w, "web3 error: {e}"),
            Self::ContractError(e) => write!(w, "contract error: {e}"),
            Self::DeployError(e) => write!(w, "deployment error: {e}"),
            Self::IoError(e) => write!(w, "IO error: {e}"),
            Self::SerializationError(e) => write!(w, "serialization error: {e}"),
        }
    }
}

impl Error for EthError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ConfError(e) => Some(e),
            Self::Web3Error(e) => Some(e),
            Self::IoError(e) => Some(e),
            Self::ContractError(e) => Some(e),
            Self::DeployError(e) => Some(e),
            Self::SerializationError(e) => Some(e),
        }
    }
}

/// Deploys an instance of the Beacon DAO using the details specified by the
/// context.
pub async fn deploy(
    Command::New {
        private_key,
        eth_uri,
        ipfs_uri,
        contracts_dir,
        modules,
    }: &Command,
) -> Result<String, EthError> {
    // Wrapper for the API using the specified URL
    let web3 = Web3::new(Http::new(eth_uri)?);

    // Load contract source code
    let mut src = Vec::new();

    let f = OpenOptions::new()
        .read(true)
        .open(format!("{contracts_dir}/contracts/Idea.sol/Idea.json"))?;
    f.read(&mut src);

    // Extract the bytecode from the compiled contract
    let parsed: DeployableContract = serde_json::from_slice(src.as_slice())?;

    // Deploy an instance of the contract form the specified address
    Ok(Contract::deploy(web3.eth(), src.as_slice())?
        .confirmations(0)
        .execute(parsed.bytecode[2..], (DEFAULT_NAME, DEFAULT_SYMBOL, DEFAULT_SUPPLY.into()))
        .await?
        .address()
        .to_string())
}
