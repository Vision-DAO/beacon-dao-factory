#![feature(iterator_try_collect)]
#![feature(async_closure)]
#![feature(let_chains)]

mod cli;
mod net;

#[macro_use]
extern crate convertable_errors;

use net::contract;
use std::env;

#[actix::main]
async fn main() {
    let mut args = env::args();

    // Show usage if no args are provided
    if args.len() == 1 {
        cli::usage(&mut args);
    }

    // Will throw an error if not enough args were provided
    let conf = cli::Context::try_from(args).unwrap();

    match conf.cmd {
        cli::Command::New(ctx) => {
            let addr = contract::deploy(ctx).await.unwrap();

            // No need to print extra output
            println!("{addr}");
        }
        cli::Command::List(ctx) => {
            // Print out each deployed contract's address on a separate line
            println!("{}", contract::list(ctx).await.unwrap().join("\n"));
        }
    };

    // Stop any IPFS processes running in the background
    if let Some(mut ipfs_handle) = conf.ipfs_handle {
        ipfs_handle.kill().expect("failed to stop IPFS process");
    }
}
