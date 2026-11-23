//! `ncdb`, the NoCentralDB command line: `ncdb model build` and
//! `ncdb model check` (RFC 0005, `docs/rfc/0005-schema-and-model-lock.md`).
//!
//! The schema is a TypeScript module, so `ncdb` runs it with Node.js
//! ([`schema`]) to get its description. Everything else, building the lock
//! and the additive-change rule, is the core's (`ncdb_core::model_lock`), so
//! the build and the check on the device apply the same rules. [`report`]
//! words what a build changed. Everything `ncdb` reads, writes or runs goes
//! through a [`host::Host`], so `@nocentraldb/sdk`'s `ncdb` bin runs this
//! same code compiled to wasm (NOC-415).

mod args;
pub mod host;
mod model;
pub mod report;
mod schema;
#[rustfmt::skip]
mod texts;

use args::{Command, USAGE};
use host::Host;

/// What a run printed, and its exit code: 0 on success, 1 when the lock is
/// refused, out of date or cannot be built, 2 on a usage error.
#[derive(Debug, PartialEq, Eq)]
pub struct Outcome {
    pub code: u8,
    pub stdout: String,
    pub stderr: String,
}

impl Outcome {
    fn out(code: u8, stdout: String) -> Self {
        Outcome {
            code,
            stdout,
            stderr: String::new(),
        }
    }

    fn err(code: u8, stderr: String) -> Self {
        Outcome {
            code,
            stdout: String::new(),
            stderr,
        }
    }
}

/// Runs `ncdb` with its arguments on `host`.
pub fn run_on(host: &mut dyn Host, args: &[String]) -> Outcome {
    match args::parse(args) {
        Ok(Command::Help) => Outcome::out(0, USAGE.into()),
        Ok(Command::Model { check, paths }) => model::run(host, check, &paths),
        Err(message) => Outcome::err(2, format!("ncdb: {message}\n{USAGE}")),
    }
}

/// Runs `ncdb` with its arguments on this machine, prints what it says,
/// and returns the exit code.
#[cfg(not(target_arch = "wasm32"))]
pub fn run(args: &[String]) -> u8 {
    let outcome = run_on(&mut host::System, args);
    print!("{}", outcome.stdout);
    eprint!("{}", outcome.stderr);
    outcome.code
}
