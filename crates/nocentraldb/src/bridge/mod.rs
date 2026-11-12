//! The bridge between the TypeScript SDK and this SDK in a dedicated worker
//! (NOC-176). Crate-internal and provisional: the export to JavaScript,
//! the crate it lives in and every public TypeScript name wait for the
//! owner's answers on NOC-274. What is fixed here is the message protocol
//! the worker speaks, which the TypeScript side mirrors.
//!
//! Each message is CBOR (`ncdb_core::cbor`), so the bridge adds no
//! dependency and its dispatch runs natively in tests, on a database in
//! memory, as well as in the worker.
//!
//! A request is a map `{"id": u64, "call": text, "args": map}`. Its
//! answer is `{"id": u64, "ok": value}`, or `{"id": u64, "error": {"code":
//! text, "message": text}}` with RFC 0009's error code (`notFound`,
//! `invalidValue`, ...). Calls are answered one at a time, in order.
//!
//! The calls of slice 1:
//!
//! - `open` `{"lock": text, "inMemory": bool, "name": text?, "relay":
//!   text?}`: opens the database, in memory or, in the browser, at a name
//!   on OPFS. Answers `null`. A second `open` fails with `invalidValue`.
//! - `close` `{}`: drops the database; any later call but `open` fails
//!   with `closed`. Answers `null`.
//! - `spaces.create` `{}`: answers the new space's id, 32 hex digits.
//! - `spaces.get` `{"space": id}`: answers the id, or fails with `notFound`.
//! - `spaces.list` `{}`: answers `[{"id": id, "role": text|null}]`.
//! - `write` `{"space": id, "ops": [op]}`, each op one of `{"op":
//!   "create", "collection": text, "values": map}`, `{"op": "update",
//!   "collection": text, "id": id, "values": map}` or `{"op": "delete",
//!   "collection": text, "id": id}`: answers `{"change": id, "ids": [id]}`.
//! - `get` `{"space": id, "collection": text, "id": id}`: answers `null`
//!   or `{"$id": id, "$space": id, "$deleted": bool, "fields": map}`.
//!
//! A value is text, an integer, a float, a boolean, an array, `null` to
//! clear a field, or `{"$add": [..]}` / `{"$remove": [..]}` for a set or a
//! link list.

#[cfg(test)]
mod tests;

use ncdb_core::cbor::{self, Key, Value};

use crate::db::Db;

/// The worker's side of the bridge: at most one open database.
pub(crate) struct Bridge {
    db: Option<Db>,
    closed: bool,
}

impl Bridge {
    pub(crate) fn new() -> Bridge {
        Bridge {
            db: None,
            closed: false,
        }
    }

    /// Answers one request, given and answered as CBOR.
    pub(crate) async fn handle(&mut self, request: &[u8]) -> Vec<u8> {
        let _ = (&self.db, self.closed, request);
        cbor::encode(&failure(0, "invalidValue", "the bridge is not built yet"))
    }
}

/// An error answer to the request `id`.
fn failure(id: u64, code: &str, message: &str) -> Value {
    let error = Value::Map(
        [
            (Key::Text("code".into()), Value::Text(code.into())),
            (Key::Text("message".into()), Value::Text(message.into())),
        ]
        .into(),
    );
    Value::Map(
        [
            (Key::Text("id".into()), Value::Unsigned(id)),
            (Key::Text("error".into()), error),
        ]
        .into(),
    )
}
