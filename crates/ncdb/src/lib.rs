//! `ncdb`, the NoCentralDB command line: `ncdb model build` and
//! `ncdb model check` (RFC 0005, `docs/rfc/0005-schema-and-model-lock.md`).
//!
//! The schema is a TypeScript module, so `ncdb` runs it with Node.js
//! ([`schema`]) to get its description. Everything else, building the lock
//! and the additive-change rule, is the core's (`ncdb_core::model_lock`), so
//! the build and the check on the device apply the same rules. [`report`]
//! words what a build changed.

mod args;
mod model;
pub mod report;
mod schema;

use args::{Command, USAGE};

/// Runs `ncdb` with its arguments, and returns the exit code: 0 on
/// success, 1 when the lock is refused, out of date or cannot be built, 2
/// on a usage error.
pub fn run(args: &[String]) -> u8 {
    match args::parse(args) {
        Ok(Command::Help) => {
            print!("{USAGE}");
            0
        }
        Ok(Command::Model { check, paths }) => model::run(check, &paths),
        Err(message) => {
            eprint!("ncdb: {message}\n{USAGE}");
            2
        }
    }
}
