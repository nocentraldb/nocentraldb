//! The view in a browser, end to end (NOC-174; RFC 0006, "The browser" and
//! "The spike"): the harness the Playwright test in
//! `scripts/browser-view/` drives in a dedicated Web Worker.
//!
//! Built for wasm32, it exports `open`, `fold`, `read`, `close` and
//! `sqlite` to JavaScript; the worker calls them one by one, so a test can
//! close the view, reopen it, reload the page and read again. Run
//! natively, `main` takes the same steps on a database file and prints
//! what `read` and `sqlite` return, which the test compares with what the
//! browser returned: the same fold code, the same view (principle 7).
//!
//! The changes are those of the view's tests: the doubles of
//! `tests/support/member.rs` stand in for keys, since the browser's crypto
//! runs in the wasm32 gate already.

#[path = "../tests/support/member.rs"]
mod member;
#[path = "../tests/support/view.rs"]
mod view_support;

use std::collections::BTreeSet;

use ncdb_core::basic::{BasicValue, Kind, Write};
use ncdb_core::cbor::{self, Value};
use ncdb_core::change::Operation;
use ncdb_core::clock::Hlc;
use ncdb_core::field::FieldType;
use ncdb_core::field_op;
use ncdb_core::ids::{ChangeId, ObjectId, OperationId};
use ncdb_core::log::Appended;
use ncdb_core::view::{Error, FieldState, View};

use member::*;
use view_support::*;

/// How many changes with tasks the space holds after its key envelope.
const CHANGES: u8 = 3;
/// How many tasks each of them creates.
const TASKS: u8 = 10;
/// The page size `read` pages the index of tasks by rank with, so that a
/// read crosses several cursors.
const PAGE: usize = 7;

/// A view and the doubles' checks it receives with.
struct Session {
    view: View,
    control: Control,
}

impl Session {
    fn open(path: &std::path::Path) -> Result<Session, Error> {
        Ok(Session {
            view: View::open(path, &lock())?,
            control: Control::default(),
        })
    }

    /// Receives the key envelope and the changes with tasks; returns how
    /// many of them were new.
    fn fold(&mut self) -> Result<u32, Error> {
        let mut new = 0;
        for frame in frames() {
            if let Appended::New(_) = self.view.receive(&frame, &mut self.control)? {
                new += 1;
            }
        }
        Ok(new)
    }

    /// What the view holds, as JSON: the tasks in the order of the index by
    /// rank, read page by page, each with its fields.
    fn read(&self) -> Result<String, Error> {
        let mut tasks = Vec::new();
        let mut cursor = None;
        loop {
            let page = self.view.page(TASK, BY_RANK, &[], cursor.as_ref(), PAGE)?;
            for (space, object) in &page.rows {
                let row = self.view.get(*space, TASK, *object)?;
                let fields: serde_json::Map<String, serde_json::Value> = row
                    .iter()
                    .flat_map(|row| &row.fields)
                    .map(|(name, state)| (name.clone(), field(name, state)))
                    .collect();
                tasks.push(serde_json::json!({ "object": hex(&object.0), "fields": fields }));
            }
            match page.next {
                Some(next) => cursor = Some(next),
                None => break,
            }
        }
        let spaces: Vec<String> = self.view.spaces().map(|s| hex(&s.0)).collect();
        Ok(serde_json::json!({ "spaces": spaces, "tasks": tasks }).to_string())
    }
}

/// A field of a task as the app reads it, and its state in its canonical
/// encoding: equal states, equal bytes (RFC 0004). Values are rendered as
/// canonical CBOR, since `Debug` redacts them.
fn field(name: &str, state: &FieldState) -> serde_json::Value {
    let FieldState::Basic(state) = state else {
        return serde_json::Value::Null;
    };
    let kind = if name == RANK {
        Kind::Integer
    } else {
        Kind::Text
    };
    let field_type = BasicValue { kind };
    let read = field_type.read(state);
    serde_json::json!({
        "state": hex(&field_type.state_bytes(state)),
        "value": read.value.map(|entry| hex(&cbor::encode(&entry.value))),
    })
}

/// The SQLite release this build carries, as JSON.
fn sqlite() -> Result<String, Error> {
    let conn = rusqlite::Connection::open_in_memory()?;
    let (version, source_id): (String, String) =
        conn.query_row("SELECT sqlite_version(), sqlite_source_id()", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;
    Ok(serde_json::json!({ "version": version, "sourceId": source_id }).to_string())
}

/// The journal mode of the database at `path`, as SQLite reports it.
fn journal_mode(path: &std::path::Path) -> Result<String, Error> {
    let conn = rusqlite::Connection::open(path)?;
    Ok(conn.query_row("PRAGMA journal_mode", [], |row| row.get(0))?)
}

/// The founder's key envelope, then the changes with tasks.
fn frames() -> Vec<Vec<u8>> {
    let mut frames = vec![envelope(FOUNDER, 1, 0)];
    for change in 0..CHANGES {
        let operations = (0..TASKS).flat_map(|n| task(change * TASKS + n)).collect();
        frames.push(sealed(FOUNDER, 2 + u64::from(change), 0, operations));
    }
    frames
}

/// Task `n`: created, with a title and a rank that orders the tasks
/// differently from their ids.
fn task(n: u8) -> Vec<Operation> {
    let object = ObjectId([n; 16]);
    let rank = u64::from(n.wrapping_mul(37) % 101);
    vec![
        field_op::create(object, TASK),
        set(TITLE, Kind::Text, object, Value::Text(format!("task {n}"))),
        set(RANK, Kind::Integer, object, Value::Unsigned(rank)),
    ]
}

fn set(field: &str, kind: Kind, object: ObjectId, value: Value) -> Operation {
    let write = Write {
        id: OperationId {
            change: ChangeId([0; 32]),
            position: 0,
        },
        author: member(FOUNDER),
        stamp: Hlc(0),
        value,
        replaces: BTreeSet::new(),
    };
    field_op::encode(&BasicValue { kind }, &write, object, TASK, field)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Natively: open, fold, close, reopen and read a database file, and print
/// what the browser test compares with.
#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = TempDb::new("browser-view");
    let mut session = Session::open(&db.0)?;
    session.fold()?;
    drop(session);
    let session = Session::open(&db.0)?;
    let read = session.read()?;
    drop(session);
    println!(
        "{}",
        serde_json::json!({
            "read": serde_json::from_str::<serde_json::Value>(&read)?,
            "sqlite": serde_json::from_str::<serde_json::Value>(&sqlite()?)?,
            "journalMode": journal_mode(&db.0)?,
        })
    );
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn main() {}

/// The exports the worker calls.
#[cfg(target_arch = "wasm32")]
mod exports {
    use std::cell::RefCell;
    use std::path::Path;

    use ncdb_core::view::browser::{self, Storage};
    use wasm_bindgen::prelude::*;

    use super::Session;

    thread_local! {
        static SESSION: RefCell<Option<Session>> = const { RefCell::new(None) };
    }

    fn js(error: impl std::fmt::Display) -> JsError {
        JsError::new(&error.to_string())
    }

    fn with<T>(f: impl FnOnce(&mut Session) -> Result<T, JsError>) -> Result<T, JsError> {
        SESSION.with_borrow_mut(|session| f(session.as_mut().ok_or(js("no view is open"))?))
    }

    /// Installs the browser's VFS and opens the view at `path`; resolves
    /// to where it lives, `"opfs"` or `"memory"`.
    #[wasm_bindgen]
    pub async fn open(path: String) -> Result<String, JsError> {
        let storage = browser::install().await.map_err(js)?;
        let session = Session::open(Path::new(&path)).map_err(js)?;
        SESSION.with_borrow_mut(|open| *open = Some(session));
        Ok(match storage {
            Storage::Opfs => "opfs",
            Storage::Memory => "memory",
        }
        .to_owned())
    }

    /// Receives the changes; returns how many were new.
    #[wasm_bindgen]
    pub fn fold() -> Result<u32, JsError> {
        with(|session| session.fold().map_err(js))
    }

    /// What the view holds, as JSON.
    #[wasm_bindgen]
    pub fn read() -> Result<String, JsError> {
        with(|session| session.read().map_err(js))
    }

    /// Closes the view: its connection ends.
    #[wasm_bindgen]
    pub fn close() {
        SESSION.with_borrow_mut(|open| *open = None);
    }

    /// The SQLite release of this build, as JSON.
    #[wasm_bindgen]
    pub fn sqlite() -> Result<String, JsError> {
        super::sqlite().map_err(js)
    }

    /// The journal mode of the closed database at `path`.
    #[wasm_bindgen(js_name = journalMode)]
    pub fn journal_mode(path: String) -> Result<String, JsError> {
        super::journal_mode(Path::new(&path)).map_err(js)
    }
}
