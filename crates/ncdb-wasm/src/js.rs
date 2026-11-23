//! The export to JavaScript, and the host the bin hands in.

use std::io;
use std::path::{Path, PathBuf};

use js_sys::{Array, Object, Reflect};
use ncdb::host::{Host, NodeRun};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    /// The bin's host: Node.js's own `node` and file system calls, each
    /// synchronous. A call that fails throws the Node.js error.
    pub type JsHost;

    #[wasm_bindgen(method, catch)]
    fn node(this: &JsHost, args: Array) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(method, js_name = isFile)]
    fn is_file(this: &JsHost, path: &str) -> bool;
    #[wasm_bindgen(method, catch)]
    fn canonicalize(this: &JsHost, path: &str) -> Result<String, JsValue>;
    #[wasm_bindgen(method, catch)]
    fn read(this: &JsHost, path: &str) -> Result<String, JsValue>;
    #[wasm_bindgen(method, catch)]
    fn write(this: &JsHost, path: &str, text: &str) -> Result<(), JsValue>;
    #[wasm_bindgen(method, catch)]
    fn rename(this: &JsHost, from: &str, to: &str) -> Result<(), JsValue>;
}

fn text(value: &JsValue, key: &str) -> Option<String> {
    Reflect::get(value, &key.into()).ok()?.as_string()
}

/// A Node.js error as an I/O error: its `code` gives the kind, so a missing
/// file reads as `NotFound`, and its `message` the reason.
fn error(value: JsValue) -> io::Error {
    let kind = match text(&value, "code").as_deref() {
        Some("ENOENT") => io::ErrorKind::NotFound,
        Some("EACCES" | "EPERM") => io::ErrorKind::PermissionDenied,
        _ => io::ErrorKind::Other,
    };
    let message = text(&value, "message")
        .or_else(|| value.as_string())
        .unwrap_or_else(|| "unknown error".into());
    io::Error::new(kind, message)
}

fn path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

impl Host for JsHost {
    fn node(&mut self, args: &[String]) -> io::Result<NodeRun> {
        let array: Array = args.iter().map(|a| JsValue::from_str(a)).collect();
        let run = JsHost::node(self, array).map_err(error)?;
        Ok(NodeRun {
            success: Reflect::get(&run, &"success".into())
                .map(|s| s.is_truthy())
                .unwrap_or(false),
            stdout: text(&run, "stdout").unwrap_or_default(),
            stderr: text(&run, "stderr").unwrap_or_default(),
        })
    }

    fn is_file(&mut self, p: &Path) -> bool {
        JsHost::is_file(self, &path(p))
    }

    fn canonicalize(&mut self, p: &Path) -> io::Result<PathBuf> {
        JsHost::canonicalize(self, &path(p))
            .map(PathBuf::from)
            .map_err(error)
    }

    fn read(&mut self, p: &Path) -> io::Result<String> {
        JsHost::read(self, &path(p)).map_err(error)
    }

    fn write(&mut self, p: &Path, text: &str) -> io::Result<()> {
        JsHost::write(self, &path(p), text).map_err(error)
    }

    fn rename(&mut self, from: &Path, to: &Path) -> io::Result<()> {
        JsHost::rename(self, &path(from), &path(to)).map_err(error)
    }
}

/// Runs `ncdb` with `args` on `host`, and returns `{ code, stdout, stderr }`
/// for the bin to print.
#[wasm_bindgen]
pub fn run(args: Vec<String>, mut host: JsHost) -> Object {
    let outcome = ncdb::run_on(&mut host, &args);
    let result = Object::new();
    let _ = Reflect::set(&result, &"code".into(), &outcome.code.into());
    let _ = Reflect::set(&result, &"stdout".into(), &outcome.stdout.into());
    let _ = Reflect::set(&result, &"stderr".into(), &outcome.stderr.into());
    result
}
