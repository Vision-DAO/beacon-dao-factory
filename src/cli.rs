use ipfs_api::{IpfsClient, TryFromUri};
use itertools::Itertools;
use log::debug;
use std::{
	collections::HashMap,
	convert::TryFrom,
	env::{self, Args},
	error::Error as StdError,
	fmt,
	fs::{File, OpenOptions},
	io::{stderr, BufRead, BufReader, Write},
	process,
	process::{Child, Command as ProcCommand, Stdio},
	sync::mpsc,
	thread,
};

const CLI_NAME: &str = "./daowiz";
const PRIVATE_KEY_ARG: &str = "DEPLOYMENT_PRIVATE_KEY";

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
\t--eth-chain-id (required) - a flag specifying the Ethereum blockchain to \
interact with
\t--contracts-dir (required) - a flag specifying the path to a directory \
containing the built Beacon DAO contracts that will be used for deploying the \
Beacon DAO";

/// Required args to the command-line application.
pub(crate) struct Context {
	pub(crate) cmd: Command,
}

#[derive(Default)]
struct ContextBuilder {
	cmd: Option<CommandBuilder>,

	eth_uri: Option<String>,
	eth_chain_id: Option<String>,
	ipfs_uri: Option<String>,
	contracts_dir: Option<String>,
	private_key: Option<String>,

	files: HashMap<String, (Option<File>, Option<File>)>,
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
	pub(crate) eth_chain_id: u64,
	pub(crate) contracts_dir: String,

	// Handles to all of the specified modules
	pub(crate) modules: Vec<(File, File)>,

	// IPFS Node that might be running in the background if no proxy URL was
	// provided
	pub(crate) ipfs: IpfsClient,
	pub(crate) ipfs_handle: Option<Child>,
}

/// Configuration variables necessary for executing the `list` command.
pub struct ListContext {
	pub(crate) private_key: String,
	pub(crate) eth_uri: String,
	pub(crate) eth_chain_id: u64,
	pub(crate) contracts_dir: String,
}

impl TryFrom<ContextBuilder> for Command {
	type Error = ParseError;

	/// Unwraps fields from a configuration, returning an error if a required
	/// field was not specified. Uses defaults for relevant fields.
	fn try_from(mut v: ContextBuilder) -> Result<Self, Self::Error> {
		match v.cmd {
			Some(CommandBuilder::New) => Ok(Self::New(Box::new(NewContext {
				private_key: v.private_key.ok_or(ParseError::MissingPrivateKey)?,
				eth_uri: v.eth_uri.ok_or(ParseError::MissingRpcUrlETH)?,
				eth_chain_id: v
					.eth_chain_id
					.ok_or(ParseError::MissingChainId)?
					.parse()
					.map_err(|_| ParseError::MissingChainId)?,
				contracts_dir: v.contracts_dir.ok_or(ParseError::MissingContractsSrc)?,
				// Transform paths into file contents, bubbling IO errors
				modules: v
					.files
					.drain()
					.filter_map(
						|(_, tup): (String, (Option<File>, Option<File>))| match tup {
							(Some(a), Some(b)) => Some((a, b)),
							_ => None,
						},
					)
					.collect(),

				// Spawn an IPFS node if the user didn't specify a host
				ipfs_handle: if v.ipfs_uri.is_none() {
					let (tx, rx) = mpsc::channel();

					log::debug!("starting IPFS daemon");

					thread::spawn(move || {
						let mut cmd = ProcCommand::new("ipfs")
							.arg("daemon")
							.stdout(Stdio::piped())
							.stderr(Stdio::piped())
							.spawn()
							.map_err(|e| ParseError::MiscError(Box::new(e)))
							.unwrap();

						let out = cmd.stdout.take().unwrap();
						let reader = BufReader::new(out);
						let mut lines = reader.lines().map(Result::unwrap);

						for l in lines.by_ref() {
							debug!("{l}");

							if l.contains("API server listening") {
								tx.send(cmd).unwrap();
								break;
							}
						}

						loop {
							lines.next();
						}
					});

					Some(rx.recv().map_err(|e| ParseError::MiscError(Box::new(e)))?)
				} else {
					None
				},
				ipfs: {
					IpfsClient::from_str(
						v.ipfs_uri
							.as_deref()
							.unwrap_or_else(|| DEFAULT_IPFS_GATEWAY),
					)
					.map_err(|e| ParseError::MiscError(Box::new(e)))?
				},
			}))),
			Some(CommandBuilder::List) => Ok(Self::List(ListContext {
				private_key: v.private_key.ok_or(ParseError::MissingPrivateKey)?,
				eth_uri: v.eth_uri.ok_or(ParseError::MissingRpcUrlETH)?,
				eth_chain_id: v
					.eth_chain_id
					.ok_or(ParseError::MissingChainId)?
					.parse()
					.map_err(|_| ParseError::MissingChainId)?,
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
	MissingChainId,
	MiscError(Box<dyn StdError>),
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
			Self::MissingChainId => write!(fmt, "config error: command requires a --eth-chain-id"),
		}
	}
}

impl StdError for ParseError {}

/// Gets the configuration of the command-line client from the command-line
/// args.
impl TryFrom<Args> for Context {
	type Error = ParseError;

	fn try_from(mut v: Args) -> Result<Self, Self::Error> {
		let mut builder = ContextBuilder {
			// new, or ls should be the first arg after the program name, which
			// is already extracted
			cmd: v.nth(1).and_then(|cmd| match cmd.as_str() {
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
				"--eth-chain-id" => builder.eth_chain_id = Some(v),
				"--ipfs-rpc-uri" => builder.ipfs_uri = Some(v),
				"--contracts-dir" => builder.contracts_dir = Some(v),

				// Open non-flag args that end with .wasm as modules
				_ => {
					for fname in [k, v] {
						// Get slot storing js loader and wasm module
						let stripped = fname
							.trim_end_matches(".wasm")
							.trim_end_matches(".js")
							.trim_end_matches("_bg");

						if let Ok(f) = OpenOptions::new().read(true).open(&fname) {
							// Set the slot to the default
							if !builder.files.contains_key(stripped) {
								builder.files.insert(stripped.to_owned(), (None, None));
							}

							// Sort encountered files by loader, or module type
							if fname.ends_with(".wasm") {
								builder
									.files
									.get_mut(stripped)
									.ok_or(ParseError::MissingContractsSrc)?
									.1 = Some(f);
							} else if fname.ends_with(".js") {
								builder
									.files
									.get_mut(stripped)
									.ok_or(ParseError::MissingContractsSrc)?
									.0 = Some(f);
							}
						}
					}
				}
			}
		}

		// Private key is required for all commands
		builder.private_key = env::var(PRIVATE_KEY_ARG).ok();

		Ok(Context {
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
				"{} a.wasm a.js b.wasm b.js ... {USAGE}\n",
				args.next().unwrap_or_else(|| CLI_NAME.to_owned())
			)
			.as_bytes(),
		)
		.unwrap();

	process::exit(0x0100);
}
