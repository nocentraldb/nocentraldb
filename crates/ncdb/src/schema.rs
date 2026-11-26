//! Running the TypeScript schema with Node.js, to get its description.
//!
//! Node.js 24 strips TypeScript types natively, so the schema module runs as
//! it is, with the SDK the app depends on. The SDK's `nc.model` keeps a
//! function that describes the model under a hidden key; the script below
//! calls it and prints the description as JSON, which the core reads as a
//! `Draft`.

use std::io::ErrorKind;
use std::path::Path;

use ncdb_core::model_lock::Draft;

use crate::host::Host;
use crate::texts;

/// Imports the schema module and prints the description of its `model`.
/// Its arguments: the module's path, then its two messages, from
/// texts/cli/, the first with `{error}` where the exception goes.
const SCRIPT: &str = r#"
const { pathToFileURL } = await import("node:url");
const [, path, cannotBeRun, exportsNoModel] = process.argv;
const fail = (message) => {
  process.stderr.write(`${message}\n`);
  process.exit(3);
};
let schema;
try {
  schema = await import(pathToFileURL(path).href);
} catch (error) {
  fail(cannotBeRun.replace("{error}", () => String(error?.message ?? error)));
}
const describe = schema.model?.[Symbol.for("nocentraldb.model.describe")];
if (typeof describe !== "function") {
  fail(exportsNoModel);
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

/// Why `node` did not start.
fn not_started(error: std::io::Error) -> String {
    match error.kind() {
        ErrorKind::NotFound => texts::node_not_found(NODE_MAJOR),
        _ => texts::node_not_started(error),
    }
}

/// Refuses a missing `node`, or one older than [`NODE_MAJOR`].
fn check_node(host: &mut dyn Host) -> Result<(), String> {
    let output = host.node(&["--version".into()]).map_err(not_started)?;
    let version = output.stdout.trim().to_owned();
    let major = version
        .strip_prefix('v')
        .and_then(|v| v.split('.').next())
        .and_then(|m| m.parse::<u32>().ok());
    match major {
        Some(major) if major >= NODE_MAJOR => Ok(()),
        _ => Err(texts::node_too_old(
            NODE_MAJOR,
            if version.is_empty() {
                texts::NODE_VERSION_UNKNOWN
            } else {
                &version
            },
        )),
    }
}

/// The description of the schema module at `path`, or the message to show
/// after the module's name.
pub fn describe(host: &mut dyn Host, path: &Path) -> Result<Draft, String> {
    if !host.is_file(path) {
        return Err(texts::NO_SCHEMA_FILE.into());
    }
    let absolute = host.canonicalize(path).map_err(texts::cannot_be_opened)?;
    check_node(host)?;
    let output = host
        .node(&[
            "--input-type=module".into(),
            "--eval".into(),
            SCRIPT.into(),
            absolute.to_string_lossy().into_owned(),
            texts::schema_cannot_be_run("{error}"),
            texts::SCHEMA_EXPORTS_NO_MODEL.into(),
        ])
        .map_err(not_started)?;
    if !output.success {
        return Err(output.stderr.trim_end().to_owned());
    }
    Draft::from_json(&output.stdout).map_err(texts::description_unreadable)
}
