use ipfs_api::Error as IpfsError;
use serde_json::Error as SerializationError;
use std::{io::Error as IoError, error::Error as StdError, fmt};
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
        (ConfError(ParseError), [(ParseError, Self::ConfError)]),

        // Web3 module errors
        (Web3Error(Web3Error), [(Web3Error, Self::Web3Error)]),
        (ContractError(ContractError), [(ContractError, Self::ContractError)]),
        (DeployError(DeployError), [(DeployError, Self::DeployError)]),

        // File-related errors
        (IoError(Box<dyn StdError>), [(IoError, |e| Self::IoError(Box::new(e)))]),
        (SerializationError(SerializationError), [(SerializationError, Self::SerializationError)]),
        (IpfsError(IpfsError), [(IpfsError, Self::IpfsError)]),
        (InvalidInput),
    }
}

impl fmt::Display for Error {
    fn fmt(&self, w: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConfError(e) => write!(w, "configuration error: {e}"),
            Self::Web3Error(e) => write!(w, "web3 error: {e}"),
            Self::ContractError(e) => write!(w, "contract error: {e}"),
            Self::DeployError(e) => write!(w, "deployment error: {e}"),
            Self::IoError(e) => write!(w, "IO error: {e}"),
            Self::SerializationError(e) => write!(w, "serialization error: {e}"),
            Self::IpfsError(e) => write!(w, "ipfs network error: {e}"),
            Self::InvalidInput => write!(w, "the inputted file could not be parsed properly"),
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::ConfError(e) => Some(e),
            Self::Web3Error(e) => Some(e),
            Self::IoError(e) => Some(e.as_ref()),
            Self::ContractError(e) => Some(e),
            Self::DeployError(e) => Some(e),
            Self::SerializationError(e) => Some(e),
            Self::IpfsError(e) => Some(e),
            Self::InvalidInput => None,
        }
    }
}
