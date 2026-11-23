//! `ncdb` for `@nocentraldb/sdk`'s bin (NOC-415; Carsten, 2026-11-23,
//! NOC-443, A): the command line of crates/ncdb, compiled to wasm and run
//! on a host the bin's Node.js script provides
//! (packages/sdk/src/ncdbCli.ts). The arguments, the schema run, the lock
//! and every message are crates/ncdb's, so the bin says and writes what the
//! native `ncdb` does.

#[cfg(target_arch = "wasm32")]
mod js;

#[cfg(target_arch = "wasm32")]
pub use js::run;
