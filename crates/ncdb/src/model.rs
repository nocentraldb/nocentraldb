//! `ncdb model build` and `ncdb model check`.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use ncdb_core::model_lock::{ModelId, ModelLock, build};

use crate::args::Paths;
use crate::report;
use crate::schema;
use crate::texts;

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
        None if check => return Err(format!("{}\n", texts::lock_missing(&lock_name))),
        None => None,
        Some(text) => Some(parse(text).map_err(|message| format!("{lock_name}: {message}\n"))?),
    };
    let built = build(&draft, previous.as_ref(), ModelId::random)
        .map_err(|refusals| report::refused(&lock_name, previous.as_ref(), &refusals))?;
    let next = built.lock;
    let text = next.to_canonical_json();
    if previous_text.as_deref() == Some(text.as_str()) {
        return Ok(format!(
            "{}\n",
            texts::up_to_date(&lock_name, next.model.version)
        ));
    }
    let changes = report::changes(previous.as_ref(), &next);
    if check {
        let would = match previous.as_ref() {
            Some(previous) if previous == &next => {
                format!("{}\n", texts::rewrite_canonical(next.model.version))
            }
            _ => changes,
        };
        return Err(texts::out_of_date(&lock_name, would));
    }
    write(&paths.lock, &text).map_err(|message| format!("{lock_name}: {message}\n"))?;
    Ok(format!("{lock_name}: {changes}"))
}

/// The lock's text, or `None` when there is none yet.
fn read(path: &Path) -> Result<Option<String>, String> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(texts::cannot_be_read(error)),
    }
}

/// The previous lock, which must be one `ncdb model build` wrote: a lock
/// whose hash does not match its content was edited by hand, and building
/// on it would number the next version wrongly.
fn parse(text: &str) -> Result<ModelLock, String> {
    let lock = ModelLock::from_json(text).map_err(texts::cannot_be_read)?;
    if lock.compute_hash() != lock.model.hash {
        return Err(texts::HAND_EDITED.into());
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
        .map_err(texts::cannot_be_written)
}
