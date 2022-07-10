use ipfs_api::Error as IpfsError;
use serde_json::Error as SerializationError;
use std::{io::Error as IoError, error::Error as StdError, fmt};
use hex::FromHexError;
use web3::{contract::deploy::Error as DeployError, error::Error as Web3Error, ethabi::Error as ContractError};

use super::super::cli::ParseError;

// Generate From<T> bindings for these variant types. Comes from a crate
// I made. Could also just be replaced with an error library that is better
// supported, but this approach has less magic going on.
convertable_error! {
    /// An error that occurred while interfacing with the Ethereum network.
    #[derive(Debug)]
    pub enum Error {
        // Usually comes from the command-line module
        (Conf(ParseError), [(ParseError, Self::Conf)]),

        // Web3 module errors
        (Web3(Web3Error), [(Web3Error, Self::Web3)]),
        (Contract(ContractError), [(ContractError, Self::Contract)]),
        (Deploy(DeployError), [(DeployError, Self::Deploy)]),

        // File-related errors
        (Io(Box<dyn StdError>), [(IoError, |e| Self::Io(Box::new(e)))]),
        (Serialization(Box<dyn StdError>), [(SerializationError, |e| Self::Serialization(Box::new(e))), (FromHexError, |e| Self::Serialization(Box::new(e)))]),
        (Ipfs(IpfsError), [(IpfsError, Self::Ipfs)]),
        (InvalidInput),
    }
}

impl fmt::Display for Error {
    fn fmt(&self, w: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Conf(e) => write!(w, "configuration error: {e}"),
            Self::Web3(e) => write!(w, "web3 error: {e}"),
            Self::Contract(e) => write!(w, "contract error: {e}"),
            Self::Deploy(e) => write!(w, "deployment error: {e}"),
            Self::Io(e) => write!(w, "IO error: {e}"),
            Self::Serialization(e) => write!(w, "serialization error: {e}"),
            Self::Ipfs(e) => write!(w, "ipfs network error: {e}"),
            Self::InvalidInput => write!(w, "the inputted file could not be parsed properly"),
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Conf(e) => Some(e),
            Self::Web3(e) => Some(e),
            Self::Io(e) => Some(e.as_ref()),
            Self::Contract(e) => Some(e),
            Self::Deploy(e) => Some(e),
            Self::Serialization(e) => Some(e.as_ref()),
            Self::Ipfs(e) => Some(e),
            Self::InvalidInput => None,
        }
    }
}
