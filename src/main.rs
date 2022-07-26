#![feature(iterator_try_collect)]
#![feature(async_closure)]
#![feature(let_chains)]
#![feature(try_blocks)]

mod cli;
mod net;

#[macro_use]
extern crate convertable_errors;

use dotenv::dotenv;
use net::{contract, error::Error};
use std::{env, process::Child};

async fn run_cli(args: env::Args) -> (Option<Child>, Result<(), Error>) {
	// Will throw an error if not enough args were provided
	let conf = cli::Context::try_from(args).unwrap();
	let mut handle = None;

	match conf.cmd {
		cli::Command::New(mut ctx) => {
			handle = ctx.ipfs_handle.take();
			let addr = contract::deploy(ctx).await.unwrap();

			// No need to print extra output
			println!("{addr}");
		}
		cli::Command::List(ctx) => {
			// Print out each deployed contract's address on a separate line
			println!("{}", contract::list(ctx).await.unwrap().join("\n"));
		}
	};

	(handle, Ok(()))
}

#[actix::main]
async fn main() -> Result<(), Error> {
	env_logger::init();
	dotenv().ok();

	let mut args = env::args();

	// Show usage if no args are provided
	if args.len() == 1 {
		cli::usage(&mut args);
	}

	let (ctx, res) = run_cli(args).await;

	// Stop any IPFS processes running in the background
	if let Some(mut ipfs_handle) = ctx {
		ipfs_handle.kill().expect("failed to stop IPFS process");
	}

	res
}
