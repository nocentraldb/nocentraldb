//! Running the TypeScript schema with Node.js, to get its description.
//!
//! Node.js 24 strips TypeScript types natively, so the schema module runs as
//! it is, with the SDK the app depends on. The SDK's `nc.model` keeps a
//! function that describes the model under a hidden key; the script below
//! calls it and prints the description as JSON, which the core reads as a
//! `Draft`.

use std::io::ErrorKind;
use std::path::Path;
use std::process::Command;

use ncdb_core::model_lock::Draft;

/// Imports the schema module and prints the description of its `model`.
const SCRIPT: &str = r#"
const { pathToFileURL } = await import("node:url");
const fail = (message) => {
  process.stderr.write(`${message}\n`);
  process.exit(3);
};
let schema;
try {
  schema = await import(pathToFileURL(process.argv[1]).href);
} catch (error) {
  fail(`cannot be run: ${error?.message ?? error}`);
}
const describe = schema.model?.[Symbol.for("nocentraldb.model.describe")];
if (typeof describe !== "function") {
  fail("exports no model; export the model as `export const model = nc.model({ ... })`.");
}
try {
  process.stdout.write(JSON.stringify(describe()));
} catch (error) {
  fail(error?.message ?? String(error));
}
"#;

/// The oldest Node.js major version that strips TypeScript types natively,
/// as RFC 0005's schema module needs.
const NODE_MAJOR: u32 = 24;

/// Refuses a missing `node`, or one older than [`NODE_MAJOR`].
fn check_node() -> Result<(), String> {
    let output = Command::new("node")
        .arg("--version")
        .output()
        .map_err(|error| match error.kind() {
            ErrorKind::NotFound => {
                "ncdb runs the schema with Node.js 24 or later, and found no `node` on PATH."
                    .to_owned()
            }
            _ => format!("cannot start Node.js: {error}"),
        })?;
    let version = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let major = version
        .strip_prefix('v')
        .and_then(|v| v.split('.').next())
        .and_then(|m| m.parse::<u32>().ok());
    match major {
        Some(major) if major >= NODE_MAJOR => Ok(()),
        _ => Err(format!(
            "ncdb runs the schema with Node.js {NODE_MAJOR} or later, and the `node` on PATH is {}.",
            if version.is_empty() {
                "of an unknown version"
            } else {
                &version
            }
        )),
    }
}

/// The description of the schema module at `path`, or the message to show
/// after the module's name.
pub fn describe(path: &Path) -> Result<Draft, String> {
    if !path.is_file() {
        return Err(
            "no such file; the schema module is ./model.ts unless --model names another.".into(),
        );
    }
    let absolute = path
        .canonicalize()
        .map_err(|error| format!("cannot be opened: {error}"))?;
    check_node()?;
    let output = Command::new("node")
        .arg("--input-type=module")
        .arg("--eval")
        .arg(SCRIPT)
        .arg(absolute)
        .output()
        .map_err(|error| match error.kind() {
            ErrorKind::NotFound => {
                "ncdb runs the schema with Node.js 24 or later, and found no `node` on PATH."
                    .to_owned()
            }
            _ => format!("cannot start Node.js: {error}"),
        })?;
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr);
        return Err(message.trim_end().to_owned());
    }
    let text = String::from_utf8_lossy(&output.stdout);
    Draft::from_json(&text).map_err(|error| {
        format!("the SDK described the model in a form this ncdb cannot read: {error}")
    })
}
