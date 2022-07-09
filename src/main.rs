mod cli;
mod contract;

#[macro_use]
extern crate convertable_errors;

use std::env;

fn main() {
    let mut args = env::args();

    // Show usage if no args are provided
    if args.len() == 1 {
        cli::usage(&mut args);
    }

    // Will throw an error if not enough args were provided
    let conf = cli::Context::try_from(args)
        .unwrap();

    match conf.cmd {
        cli::Command::New{ .. } => contract::deploy(&conf.cmd),
    };
}
