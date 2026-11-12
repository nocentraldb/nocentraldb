//! `tasks`: an example app of NoCentralDB (NOC-237). A task list that two
//! devices share through a relay, built on the public Rust SDK only.
//!
//! Each run opens the device's database, does one thing and, when it wrote
//! something and a relay is given, waits until the relay holds it, so the
//! other device can receive it. See README.md for a walk-through.

mod args;
mod commands;
mod wait;

use std::process::ExitCode;

use nocentraldb::{Db, OpenOptions};

use args::{Args, Command, USAGE};

/// The lock that `ncdb model build` writes from model.ts.
const LOCK: &str = include_str!("../model.lock.json");

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args = match args::parse(&args) {
        Ok(Args {
            command: Command::Help,
            ..
        }) => {
            print!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Ok(args) => args,
        Err(message) => {
            eprint!("tasks: {message}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("a runtime");
    match runtime.block_on(run(args)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("tasks: {message}");
            ExitCode::FAILURE
        }
    }
}

/// Opens the database the arguments name and runs the command.
async fn run(args: Args) -> Result<(), String> {
    let mut options = OpenOptions::new(LOCK).path(&args.data);
    if let Some(key_file) = &args.key_file {
        options = options.key_file(key_file);
    }
    if let Some(relay) = &args.relay {
        options = options.relay(relay);
    }
    let db = Db::open(options).await.map_err(|e| e.to_string())?;
    let syncs = args.relay.is_some();
    commands::run(&db, args.command, syncs).await
}
