use ipfs_api::{IpfsClient, TryFromUri};
use itertools::Itertools;
use std::{
    convert::TryFrom,
    env::{self, Args},
    error::Error,
    fmt,
    fs::{File, OpenOptions},
    io::{stderr, Write},
    path::PathBuf,
    process,
    process::{Child, Command as ProcCommand},
};

const CLI_NAME: &str = "./daowiz";
const PRIVATE_KEY_ARG: &str = "DEPLOYMENT_KEY";

/// The assumed IPFS URL, by default an in-process instance.
const DEFAULT_IPFS_GATEWAY: &str = "http://127.0.0.1:5001/";

/// Instructions for how to use the program.
const USAGE: &str = " - creates a new Vision Beacon DAO with the specified \
default modules
\tDEPLOYMENT_KEY (required) - an environment var specifying the ethereum \
private key to use for deploying the DAO
\t--eth-rpc-uri (required) - a flag specifying the http url of an EVM-\
compatible node that daowiz will deploy the Beacon DAO to
\t--ipfs-rpc-uri (optional) - a flag specifying the http url of an IPFS node \
that daowiz will deploy Beacon DAO metadata to. Uses an in-process IPFS node by \
default
\t--contracts-dir (required) - a flag specifying the path to a directory \
containing the built Beacon DAO contracts that will be used for deploying the \
Beacon DAO";

/// Required args to the command-line application.
pub struct Context {
    pub(crate) cmd: Command,
    pub(crate) ipfs_handle: Option<Child>,
}

#[derive(Default)]
struct ContextBuilder {
    cmd: Option<CommandBuilder>,

    eth_uri: Option<String>,
    ipfs_uri: Option<String>,
    contracts_dir: Option<String>,
    private_key: Option<String>,

    files: Vec<(PathBuf, File)>,
}

/// Command-specific configuration options.
pub enum Command {
    New(Box<NewContext>),
    List(ListContext),
}

/// Configuration variables necessary for executing the `new` command.
pub struct NewContext {
    pub(crate) private_key: String,
    pub(crate) eth_uri: String,
    pub(crate) contracts_dir: String,

    // Handles to all of the specified modules
    pub(crate) modules: Vec<(PathBuf, File)>,

    // IPFS Node that might be running in the background if no proxy URL was
    // provided
    pub(crate) ipfs: IpfsClient,
}

/// Configuration variables necessary for executing the `list` command.
pub struct ListContext {
    pub(crate) private_key: String,
    pub(crate) eth_uri: String,
    pub(crate) contracts_dir: String,
}

impl TryFrom<ContextBuilder> for Command {
    type Error = ParseError;

    /// Unwraps fields from a configuration, returning an error if a required
    /// field was not specified. Uses defaults for relevant fields.
    fn try_from(v: ContextBuilder) -> Result<Self, Self::Error> {
        match v.cmd {
            Some(CommandBuilder::New) => Ok(Self::New(Box::new(NewContext {
                private_key: v.private_key.ok_or(ParseError::MissingPrivateKey)?,
                eth_uri: v.eth_uri.ok_or(ParseError::MissingRpcUrlETH)?,
                contracts_dir: v.contracts_dir.ok_or(ParseError::MissingContractsSrc)?,
                // Transform paths into file contents, bubbling IO errors
                modules: v.files,

                // Spawn an IPFS node if the user didn't specify a host
                ipfs: IpfsClient::build_with_base_uri(
                    v.ipfs_uri
                        .unwrap_or_else(|| DEFAULT_IPFS_GATEWAY.to_owned())
                        .parse()
                        .map_err(|e| ParseError::MiscError(Box::new(e)))?,
                ),
            }))),
            Some(CommandBuilder::List) => Ok(Self::List(ListContext {
                private_key: v.private_key.ok_or(ParseError::MissingPrivateKey)?,
                eth_uri: v.eth_uri.ok_or(ParseError::MissingRpcUrlETH)?,
                contracts_dir: v.contracts_dir.ok_or(ParseError::MissingContractsSrc)?,
            })),
            None => Err(ParseError::MissingCommand),
        }
    }
}

enum CommandBuilder {
    New,
    List,
}

/// An error encountered while parsing CLI args.
#[derive(Debug)]
pub enum ParseError {
    MissingCommand,
    MissingPrivateKey,
    MissingRpcUrlETH,
    MissingContractsSrc,
    MiscError(Box<dyn Error>),
}

impl fmt::Display for ParseError {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingCommand => write!(fmt, "parse error: no command specified"),
            Self::MissingPrivateKey => write!(
                fmt,
                "config error: no {} environment variable provided",
                PRIVATE_KEY_ARG
            ),
            Self::MissingRpcUrlETH => write!(fmt, "config error: command requires a --eth-rpc-uri"),
            Self::MissingContractsSrc => {
                write!(fmt, "config error: command requires a --contracts-dir")
            }
            Self::MiscError(e) => write!(fmt, "error: {e}"),
        }
    }
}

impl Error for ParseError {}

/// Gets the configuration of the command-line client from the command-line
/// args.
impl TryFrom<Args> for Context {
    type Error = ParseError;

    fn try_from(mut v: Args) -> Result<Self, Self::Error> {
        let mut builder = ContextBuilder {
            // new, or ls should be the first arg after the program name, which
            // is already extracted
            cmd: v.next().and_then(|cmd| match cmd.as_str() {
                "new" => Some(CommandBuilder::New),
                "list" => Some(CommandBuilder::List),
                _ => None,
            }),
            ..Default::default()
        };

        // Parse flags
        for (k, v) in v.into_iter().tuples() {
            match k.as_str() {
                "--eth-rpc-uri" => builder.eth_uri = Some(v),
                "--ipfs-rpc-uri" => builder.ipfs_uri = Some(v),
                "--contracts-dir" => builder.contracts_dir = Some(v),

                // Open non-flag args that end with .wasm as modules
                fname => {
                    if fname.ends_with(".wasm") {
                        if let Ok(f) = OpenOptions::new().read(true).open(fname) {
                            builder
                                .files
                                .push((PathBuf::from(fname).with_extension(".js"), f));
                        }
                    }
                }
            }
        }

        // Private key is required for all commands
        builder.private_key = env::var(PRIVATE_KEY_ARG).ok();

        Ok(Context {
            ipfs_handle: if builder.ipfs_uri.is_none() {
                Some(
                    ProcCommand::new("ipfs")
                        .spawn()
                        .map_err(|e| ParseError::MiscError(Box::new(e)))?,
                )
            } else {
                None
            },
            cmd: Command::try_from(builder)?,
        })
    }
}

/// Prints the usage of the program to stderr.
pub fn usage(args: &mut Args) {
    // Log the program usage, exit with 1
    let mut handle = stderr().lock();
    handle
        .write_all(
            format!(
                "{} a.wasm b.wasm ... {USAGE}\n",
                args.next().unwrap_or_else(|| CLI_NAME.to_owned())
            )
            .as_bytes(),
        )
        .unwrap();

    process::exit(0x0100);
}
