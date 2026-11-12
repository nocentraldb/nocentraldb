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
//! A value is text, an integer, a float, a boolean, an array, or
//! `{"$add": [..]}` / `{"$remove": [..]}` for a set or a link list. `null`,
//! which clears a field in RFC 0009, waits for the Rust SDK to write a
//! clear; it is refused with `invalidValue` until then.

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::str::FromStr;

use ncdb_core::cbor::{self, Key, Value};

use crate::db::{Db, OpenOptions};
use crate::error::{Error, ErrorCode};
use crate::id::{ObjectId, SpaceId};
use crate::members::Role;
use crate::object::Object;
use crate::value::{Repr, Value as Field};

/// The worker's side of the bridge: at most one open database.
pub(crate) struct Bridge {
    db: Option<Db>,
}

/// Why a call failed: RFC 0009's code and a message.
struct Failed(String, String);

impl From<Error> for Failed {
    fn from(error: Error) -> Failed {
        Failed(code_name(error.code()), error.to_string())
    }
}

fn invalid(message: impl Into<String>) -> Failed {
    Failed("invalidValue".into(), message.into())
}

/// RFC 0009's name of an error code: the variant's name in lower camel
/// case, as `NotFound` is `notFound`.
fn code_name(code: ErrorCode) -> String {
    let name = format!("{code:?}");
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => first.to_lowercase().chain(chars).collect(),
        None => name,
    }
}

impl Bridge {
    pub(crate) fn new() -> Bridge {
        Bridge { db: None }
    }

    /// Answers one request, given and answered as CBOR.
    pub(crate) async fn handle(&mut self, request: &[u8]) -> Vec<u8> {
        let Ok(request) = cbor::decode(request) else {
            return cbor::encode(&failure(0, "invalidValue", "the request is not CBOR"));
        };
        let id = match get(&request, "id") {
            Some(Value::Unsigned(id)) => *id,
            _ => 0,
        };
        let answer = match (get(&request, "call"), get(&request, "args")) {
            (Some(Value::Text(call)), Some(args)) if id > 0 => self.call(call, args).await,
            _ => Err(invalid("a request is {id, call, args}")),
        };
        cbor::encode(&match answer {
            Ok(value) => entries([("id", Value::Unsigned(id)), ("ok", value)]),
            Err(Failed(code, message)) => failure(id, &code, &message),
        })
    }

    async fn call(&mut self, call: &str, args: &Value) -> Result<Value, Failed> {
        if call == "open" {
            return self.open(args).await;
        }
        let Some(db) = &self.db else {
            return Err(Failed("closed".into(), "the database is not open".into()));
        };
        match call {
            "close" => {
                self.db = None;
                Ok(Value::Null)
            }
            "spaces.create" => Ok(Value::Text(db.spaces().create().await?.id().to_string())),
            "spaces.get" => {
                let space = db.spaces().get(space_id(args)?).await?;
                Ok(Value::Text(space.id().to_string()))
            }
            "spaces.list" => {
                let list = db.spaces().list().await?;
                Ok(Value::Array(
                    list.into_iter()
                        .map(|info| {
                            let role = info
                                .role
                                .map_or(Value::Null, |role| Value::Text(role_name(role).into()));
                            entries([("id", Value::Text(info.id.to_string())), ("role", role)])
                        })
                        .collect(),
                ))
            }
            "write" => write(db, args).await,
            "get" => {
                let space = db.spaces().get(space_id(args)?).await?;
                let collection = text(args, "collection")?;
                let object = space.get(collection, object_id(args, "id")?).await?;
                Ok(object.map_or(Value::Null, |object| read(&object)))
            }
            other => Err(invalid(format!("no call {other}"))),
        }
    }

    async fn open(&mut self, args: &Value) -> Result<Value, Failed> {
        if self.db.is_some() {
            return Err(invalid("the database is open already"));
        }
        let mut options = OpenOptions::new(text(args, "lock")?);
        match (get(args, "inMemory"), get(args, "name")) {
            (Some(Value::Bool(true)), None) => options = options.in_memory(),
            #[cfg(target_arch = "wasm32")]
            (None | Some(Value::Bool(false)), Some(Value::Text(name))) => {
                options = options.browser(name)
            }
            _ => {
                return Err(invalid(
                    "open takes inMemory: true, or a name in the browser",
                ));
            }
        }
        if let Some(Value::Text(relay)) = get(args, "relay") {
            options = options.relay(relay);
        }
        self.db = Some(Db::open(options).await?);
        Ok(Value::Null)
    }
}

/// `{"space": id, "ops": [op]}`, written as one change.
async fn write(db: &Db, args: &Value) -> Result<Value, Failed> {
    let space = db.spaces().get(space_id(args)?).await?;
    let Some(Value::Array(ops)) = get(args, "ops") else {
        return Err(invalid("a write takes ops"));
    };
    let mut parsed = Vec::with_capacity(ops.len());
    for op in ops {
        let collection = text(op, "collection")?.to_owned();
        let values = match get(op, "values") {
            Some(Value::Map(map)) => fields(map)?,
            None => Vec::new(),
            Some(_) => return Err(invalid("values is a map")),
        };
        let kind = text(op, "op")?;
        let id = match kind {
            "create" => None,
            "update" | "delete" => Some(object_id(op, "id")?),
            other => return Err(invalid(format!("no op {other}"))),
        };
        parsed.push((kind.to_owned(), collection, id, values));
    }
    let written = space
        .write(|tx| {
            for (kind, collection, id, values) in &parsed {
                let values = values
                    .iter()
                    .map(|(name, value)| (name.as_str(), value.clone()));
                match (kind.as_str(), id) {
                    ("create", _) => {
                        tx.create(collection, values)?;
                    }
                    ("update", Some(id)) => tx.update(collection, *id, values)?,
                    (_, Some(id)) => tx.delete(collection, *id)?,
                    _ => unreachable!("parsed above"),
                }
            }
            Ok(())
        })
        .await?;
    Ok(entries([
        ("change", Value::Text(written.change.to_string())),
        (
            "ids",
            Value::Array(
                written
                    .ids
                    .iter()
                    .map(|id| Value::Text(id.to_string()))
                    .collect(),
            ),
        ),
    ]))
}

/// The values of a create or an update, by field name.
fn fields(map: &BTreeMap<Key, Value>) -> Result<Vec<(String, Field)>, Failed> {
    map.iter()
        .map(|(key, value)| match key {
            Key::Text(name) => Ok((name.clone(), field(value)?)),
            Key::Unsigned(_) => Err(invalid("a field's name is text")),
        })
        .collect()
}

/// A value to write, from its protocol form.
fn field(value: &Value) -> Result<Field, Failed> {
    Ok(match value {
        Value::Text(text) => Field::from(text.as_str()),
        Value::Unsigned(n) => Field::from(i64::try_from(*n).map_err(|_| invalid("too large"))?),
        Value::Negative(n) => {
            let n = i64::try_from(*n).map_err(|_| invalid("too small"))?;
            Field::from(-1 - n)
        }
        Value::Float(f) => Field::from(*f),
        Value::Bool(b) => Field::from(*b),
        Value::Map(map) => {
            let one = |key: &str| map.get(&Key::Text(key.into()));
            match (map.len(), one("$add"), one("$remove")) {
                (1, Some(Value::Array(items)), None) => Field(Repr::Add(elements(items)?)),
                (1, None, Some(Value::Array(items))) => Field(Repr::Remove(elements(items)?)),
                _ => return Err(invalid("a map value is {$add: [..]} or {$remove: [..]}")),
            }
        }
        Value::Null => return Err(invalid("clearing a field is not built yet")),
        _ => return Err(invalid("not a value a field takes")),
    })
}

fn elements(items: &[Value]) -> Result<Vec<Field>, Failed> {
    items.iter().map(field).collect()
}

/// The protocol form of a value read from a field.
fn value_of(field: &Field) -> Value {
    match &field.0 {
        Repr::Text(text) => Value::Text(text.clone()),
        Repr::Integer(n) if *n >= 0 => Value::Unsigned(*n as u64),
        Repr::Integer(n) => Value::Negative((-1 - *n) as u64),
        Repr::Float(f) => Value::Float(*f),
        Repr::Boolean(b) => Value::Bool(*b),
        Repr::Id(id) => Value::Text(id.to_string()),
        Repr::List(items) | Repr::Add(items) | Repr::Remove(items) => {
            Value::Array(items.iter().map(value_of).collect())
        }
    }
}

/// An object as `get` answers it.
fn read(object: &Object) -> Value {
    let fields = object
        .fields()
        .iter()
        .map(|(name, value)| {
            let value = value.as_ref().map_or(Value::Null, value_of);
            (Key::Text(name.clone()), value)
        })
        .collect();
    entries([
        ("$id", Value::Text(object.id().to_string())),
        ("$space", Value::Text(object.space().to_string())),
        ("$deleted", Value::Bool(object.deleted())),
        ("fields", Value::Map(fields)),
    ])
}

fn role_name(role: Role) -> &'static str {
    match role {
        Role::Reader => "reader",
        Role::Writer => "writer",
        Role::Admin => "admin",
    }
}

fn get<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    match value {
        Value::Map(map) => map.get(&Key::Text(key.into())),
        _ => None,
    }
}

fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str, Failed> {
    match get(value, key) {
        Some(Value::Text(text)) => Ok(text),
        _ => Err(invalid(format!("{key} is text"))),
    }
}

fn space_id(args: &Value) -> Result<SpaceId, Failed> {
    SpaceId::from_str(text(args, "space")?).map_err(|_| invalid("not a space id"))
}

fn object_id(args: &Value, key: &str) -> Result<ObjectId, Failed> {
    ObjectId::from_str(text(args, key)?).map_err(|_| invalid("not an object id"))
}

fn entries<const N: usize>(pairs: [(&str, Value); N]) -> Value {
    Value::Map(
        pairs
            .into_iter()
            .map(|(key, value)| (Key::Text(key.into()), value))
            .collect(),
    )
}

/// An error answer to the request `id`.
fn failure(id: u64, code: &str, message: &str) -> Value {
    let error = entries([
        ("code", Value::Text(code.into())),
        ("message", Value::Text(message.into())),
    ]);
    entries([("id", Value::Unsigned(id)), ("error", error)])
}
