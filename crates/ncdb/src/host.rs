//! What `ncdb` needs from the machine it runs on: Node.js, to run the
//! schema, and the files of the schema and the lock.
//!
//! The native binary runs on [`System`]. `@nocentraldb/sdk`'s `ncdb` bin
//! runs the same code compiled to wasm, on a host its Node.js script
//! provides (crate `ncdb-wasm`, NOC-415), so both read the arguments, run
//! the schema, build the lock and word every message the same way.

use std::io;
use std::path::{Path, PathBuf};

/// What a Node.js run printed, and whether it exited with 0.
pub struct NodeRun {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

/// The machine `ncdb` runs on.
pub trait Host {
    /// Runs Node.js with `args`. An error of kind `NotFound` means there is
    /// no Node.js to run.
    fn node(&mut self, args: &[String]) -> io::Result<NodeRun>;
    /// Whether `path` names a file.
    fn is_file(&mut self, path: &Path) -> bool;
    /// `path` made absolute, with every link resolved.
    fn canonicalize(&mut self, path: &Path) -> io::Result<PathBuf>;
    /// The text of the file at `path`; an error of kind `NotFound` when
    /// there is none.
    fn read(&mut self, path: &Path) -> io::Result<String>;
    /// Writes `text` into the file at `path`, replacing what it held.
    fn write(&mut self, path: &Path, text: &str) -> io::Result<()>;
    /// Renames the file `from` to `to`, replacing `to` at once.
    fn rename(&mut self, from: &Path, to: &Path) -> io::Result<()>;
}

/// The native machine: `node` from PATH and the file system.
#[cfg(not(target_arch = "wasm32"))]
pub struct System;

#[cfg(not(target_arch = "wasm32"))]
impl Host for System {
    fn node(&mut self, args: &[String]) -> io::Result<NodeRun> {
        let output = std::process::Command::new("node").args(args).output()?;
        Ok(NodeRun {
            success: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }

    fn is_file(&mut self, path: &Path) -> bool {
        path.is_file()
    }

    fn canonicalize(&mut self, path: &Path) -> io::Result<PathBuf> {
        path.canonicalize()
    }

    fn read(&mut self, path: &Path) -> io::Result<String> {
        std::fs::read_to_string(path)
    }

    fn write(&mut self, path: &Path, text: &str) -> io::Result<()> {
        std::fs::write(path, text)
    }

    fn rename(&mut self, from: &Path, to: &Path) -> io::Result<()> {
        std::fs::rename(from, to)
    }
}
