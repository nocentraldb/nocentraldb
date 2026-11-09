//! `ncdb model build` and `ncdb model check`.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use ncdb_core::model_lock::{ModelId, ModelLock, build};

use crate::args::Paths;
use crate::report;
use crate::schema;

/// Runs `ncdb model build`, or `ncdb model check` when `check` is set.
pub fn run(check: bool, paths: &Paths) -> u8 {
    match build_or_check(check, paths) {
        Ok(message) => {
            print!("{message}");
            0
        }
        Err(message) => {
            eprint!("{message}");
            1
        }
    }
}

fn build_or_check(check: bool, paths: &Paths) -> Result<String, String> {
    let lock_name = paths.lock.display().to_string();
    let model_name = paths.model.display().to_string();
    let draft =
        schema::describe(&paths.model).map_err(|message| format!("{model_name}: {message}\n"))?;
    let previous_text = read(&paths.lock).map_err(|message| format!("{lock_name}: {message}\n"))?;
    let previous = match &previous_text {
        None if check => return Err(format!("{lock_name}: missing; run ncdb model build.\n")),
        None => None,
        Some(text) => Some(parse(text).map_err(|message| format!("{lock_name}: {message}\n"))?),
    };
    let built = build(&draft, previous.as_ref(), ModelId::random)
        .map_err(|refusals| report::refused(&lock_name, previous.as_ref(), &refusals))?;
    let next = built.lock;
    let text = next.to_canonical_json();
    if previous_text.as_deref() == Some(text.as_str()) {
        return Ok(format!(
            "{lock_name}: up to date (model version {})\n",
            next.model.version
        ));
    }
    let changes = report::changes(previous.as_ref(), &next);
    if check {
        let would = match previous.as_ref() {
            Some(previous) if previous == &next => format!(
                "rewrite it in its canonical form (model version stays {})\n",
                next.model.version
            ),
            _ => changes,
        };
        return Err(format!(
            "{lock_name}: out of date; run ncdb model build. It would write {would}"
        ));
    }
    write(&paths.lock, &text).map_err(|message| format!("{lock_name}: {message}\n"))?;
    Ok(format!("{lock_name}: {changes}"))
}

/// The lock's text, or `None` when there is none yet.
fn read(path: &Path) -> Result<Option<String>, String> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("cannot be read: {error}")),
    }
}

/// The previous lock, which must be one `ncdb model build` wrote: a lock
/// whose hash does not match its content was edited by hand, and building
/// on it would number the next version wrongly.
fn parse(text: &str) -> Result<ModelLock, String> {
    let lock = ModelLock::from_json(text).map_err(|error| format!("cannot be read: {error}"))?;
    if lock.compute_hash() != lock.model.hash {
        return Err(
            "its model.hash does not match its content, so it was edited by hand. \
                    Restore it from version control, and change the schema instead."
                .into(),
        );
    }
    Ok(lock)
}

/// Writes the lock through a temporary file next to it, so a failed write
/// never leaves half a lock.
fn write(path: &Path, text: &str) -> Result<(), String> {
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(".tmp");
    fs::write(&temporary, text)
        .and_then(|()| fs::rename(&temporary, path))
        .map_err(|error| format!("cannot be written: {error}"))
}
