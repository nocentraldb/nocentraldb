//! The bridge between the TypeScript SDK and this SDK in a dedicated worker
//! (NOC-176). Public only with the feature `bridge` and hidden from the
//! docs: the crate `nocentraldb-wasm` exports it to JavaScript (Carsten,
//! 2026-11-12, NOC-274, point 1), and no other crate turns the feature on.
//! Its API is one call, [`Bridge::handle`], bytes in and bytes out; the
//! message protocol below is what the TypeScript side mirrors.
//!
//! The protocol is not versioned: `@nocentraldb/sdk` ships the worker, the
//! wasm module and the TypeScript that speaks to it in one build, so both
//! sides are always of the same version.
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
//!   "collection": text, "id": id, "values": map}`, `{"op": "delete",
//!   "collection": text, "id": id}` or `{"op": "restore", "collection":
//!   text, "id": id}`: answers `{"change": id, "ids": [id]}`.
//! - `get` `{"space": id, "collection": text, "id": id}`: answers `null`
//!   or `{"$id": id, "$space": id, "$deleted": bool, "fields": map}`.
//!
//! A value is text, an integer, a float, a boolean, an array,
//! `{"$add": [..]}` / `{"$remove": [..]}` for a set or a link list, or
//! `null`, which clears a basic field or a link (`Value::clear`).

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::str::FromStr;

use ncdb_core::basic::{BasicValue, Kind};
pub use ncdb_core::cbor;
use ncdb_core::cbor::{Key, Value};
use ncdb_core::set::Set;
use ncdb_core::view::{Field as ModelField, Model};

use crate::db::{Db, OpenOptions};
use crate::error::{Error, ErrorCode};
use crate::id::{ObjectId, SpaceId};
use crate::members::Role;
use crate::object::Object;
use crate::query::Cursor;
use crate::subscribe::FoldFeed;
use crate::value::{Repr, Value as Field};

/// The worker's side of the bridge: at most one open database.
pub struct Bridge {
    db: Option<Db>,
    /// The fold events of the open database.
    events: Option<Events>,
    /// The cursors of the pages `find` answered, by the handle the SDK
    /// holds: a cursor holds plaintext field values and never leaves the
    /// worker (RFC 0009, "Queries and indexes").
    cursors: BTreeMap<u64, Cursor>,
    next_cursor: u64,
}

/// The fold events of a database (NOC-176): after each fold, the space and
/// the collections it touched, so that the TypeScript SDK re-runs what its
/// subscriptions follow (RFC 0009, "Subscribe"). Names and ids, never
/// values.
#[derive(Clone)]
pub struct Events(FoldFeed);

impl Events {
    /// The folds since the last take as one message, `{"event": "folded",
    /// "folds": [{"space": id, "collections": [text]}]}`; `None` when
    /// nothing folded.
    pub fn take(&self) -> Option<Vec<u8>> {
        let folds = self.0.take();
        if folds.is_empty() {
            return None;
        }
        let folds = folds
            .into_iter()
            .map(|(space, collections)| {
                entries([
                    ("space", Value::Text(space.to_string())),
                    (
                        "collections",
                        Value::Array(collections.into_iter().map(Value::Text).collect()),
                    ),
                ])
            })
            .collect();
        Some(cbor::encode(&entries([
            ("event", Value::Text("folded".into())),
            ("folds", Value::Array(folds)),
        ])))
    }

    /// Resolves when a fold waits to be taken, or the database closed.
    pub fn wait(&self) -> impl std::future::Future<Output = ()> + use<> {
        self.0.wait()
    }

    /// Whether the database these events follow is closed.
    pub fn is_closed(&self) -> bool {
        self.0.is_closed()
    }
}

/// Why a call failed: RFC 0009's code, a message and, for
/// `unknownAccount`, the account's fingerprint as twelve blocks (RFC 0009,
/// "Errors"; Carsten, 2026-11-12, NOC-274).
pub(super) struct Failed(String, String, Option<String>);

impl From<Error> for Failed {
    fn from(error: Error) -> Failed {
        let fingerprint = error.fingerprint().map(ToString::to_string);
        Failed(code_name(error.code()), error.to_string(), fingerprint)
    }
}

fn invalid(message: impl Into<String>) -> Failed {
    Failed("invalidValue".into(), message.into(), None)
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

impl Default for Bridge {
    fn default() -> Bridge {
        Bridge::new()
    }
}

impl Bridge {
    /// A bridge with no database open.
    pub fn new() -> Bridge {
        Bridge {
            db: None,
            events: None,
            cursors: BTreeMap::new(),
            next_cursor: 0,
        }
    }

    /// Answers one request, given and answered as CBOR.
    pub async fn handle(&mut self, request: &[u8]) -> Vec<u8> {
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
            Err(failed) => failure_of(id, &failed),
        })
    }

    /// The fold events of the open database; `None` before `open`.
    pub fn events(&self) -> Option<Events> {
        self.events.clone()
    }

    async fn call(&mut self, call: &str, args: &Value) -> Result<Value, Failed> {
        if call == "open" {
            return self.open(args).await;
        }
        let Some(db) = self.db.clone() else {
            return Err(Failed(
                "closed".into(),
                "the database is not open".into(),
                None,
            ));
        };
        match call {
            "close" => {
                self.db = None;
                if let Some(events) = self.events.take() {
                    events.0.close();
                }
                self.cursors.clear();
                Ok(Value::Null)
            }
            "find" => self.find(&db, args).await,
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
            "write" => write(&db, args).await,
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
        let db = Db::open(options).await?;
        self.events = Some(Events(db.folds()));
        self.db = Some(db);
        Ok(Value::Null)
    }
}

impl Bridge {
    /// `{"query": {"collection", "index", "equal"?, "take"?, "after"?},
    /// "space"?}`: a page of a declared index, in one space when it names
    /// one (RFC 0009, "Queries and indexes"). Answers `{"items": [object],
    /// "cursor": handle | null, "hasMore": bool}`.
    async fn find(&mut self, db: &Db, args: &Value) -> Result<Value, Failed> {
        let Some(query) = get(args, "query") else {
            return Err(invalid("find takes a query"));
        };
        let mut built = db.index(text(query, "collection")?, text(query, "index")?)?;
        match get(query, "equal") {
            Some(Value::Array(values)) => built = built.equal(elements(values)?),
            None => {}
            Some(_) => return Err(invalid("equal is an array")),
        }
        match get(query, "take") {
            Some(Value::Unsigned(n)) => {
                built = built.take(usize::try_from(*n).unwrap_or(usize::MAX));
            }
            None => {}
            Some(_) => return Err(invalid("take is a number")),
        }
        match get(query, "after") {
            Some(Value::Unsigned(handle)) => match self.cursors.get(handle) {
                Some(cursor) => built = built.after(cursor.clone()),
                None => {
                    return Err(Failed(
                        "cursorOfAnotherModel".into(),
                        "the cursor is not of this database; start from the first page".into(),
                        None,
                    ));
                }
            },
            None => {}
            Some(_) => return Err(invalid("after is a cursor")),
        }
        let page = match get(args, "space") {
            Some(_) => db.spaces().get(space_id(args)?).await?.find(&built).await?,
            None => db.find(&built).await?,
        };
        let cursor = match page.cursor {
            Some(cursor) => {
                self.next_cursor += 1;
                self.cursors.insert(self.next_cursor, cursor);
                Value::Unsigned(self.next_cursor)
            }
            None => Value::Null,
        };
        Ok(entries([
            ("items", Value::Array(page.items.iter().map(read).collect())),
            ("cursor", cursor),
            ("hasMore", Value::Bool(page.has_more)),
        ]))
    }
}

/// `{"space": id, "ops": [op]}`, written as one change.
async fn write(db: &Db, args: &Value) -> Result<Value, Failed> {
    let space = db.spaces().get(space_id(args)?).await?;
    let model = db.lock().model.clone();
    let Some(Value::Array(ops)) = get(args, "ops") else {
        return Err(invalid("a write takes ops"));
    };
    let mut parsed = Vec::with_capacity(ops.len());
    for op in ops {
        let collection = text(op, "collection")?.to_owned();
        let values = match get(op, "values") {
            Some(Value::Map(map)) => fields(&model, &collection, map)?,
            None => Vec::new(),
            Some(_) => return Err(invalid("values is a map")),
        };
        let kind = text(op, "op")?;
        let id = match (kind, get(op, "id")) {
            ("create", None) => None,
            ("create" | "update" | "delete" | "restore", _) => Some(object_id(op, "id")?),
            (other, _) => return Err(invalid(format!("no op {other}"))),
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
                    ("create", Some(id)) => {
                        tx.create_at(collection, id.0, values)?;
                    }
                    ("create", None) => {
                        tx.create(collection, values)?;
                    }
                    ("update", Some(id)) => tx.update(collection, *id, values)?,
                    ("restore", Some(id)) => tx.restore(collection, *id)?,
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

/// The values of a create or an update of `collection`, by field name. A
/// whole number for a float field, or for an element of a set of floats,
/// is that float: canonical CBOR writes 2.0 as the integer 2.
fn fields(
    model: &Model,
    collection: &str,
    map: &BTreeMap<Key, Value>,
) -> Result<Vec<(String, Field)>, Failed> {
    map.iter()
        .map(|(key, value)| match key {
            Key::Text(name) => {
                let float = matches!(
                    model.field(collection, name),
                    Some(ModelField::Basic(BasicValue { kind: Kind::Float }))
                        | Some(ModelField::Set(Set { kind: Kind::Float }))
                );
                let mut value = field(value)?;
                if float {
                    value = as_float(value);
                }
                Ok((name.clone(), value))
            }
            Key::Unsigned(_) => Err(invalid("a field's name is text")),
        })
        .collect()
}

/// `value` with each integer in it as a float.
fn as_float(value: Field) -> Field {
    let floats = |items: Vec<Field>| items.into_iter().map(as_float).collect();
    Field(match value.0 {
        Repr::Integer(n) => Repr::Float(n as f64),
        Repr::Add(items) => Repr::Add(floats(items)),
        Repr::Remove(items) => Repr::Remove(floats(items)),
        other => other,
    })
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
        // `null` clears a basic field or a link (RFC 0009, "Field operations
        // per field type"; Carsten, 2026-11-12, NOC-277); the SDK refuses it
        // for a set or many links.
        Value::Null => Field::clear(),
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
        // A plugin field's data passes through as its own CBOR value; how
        // the TypeScript SDK shows it comes with plugin fields there.
        Repr::Data(data) => data.to_value().unwrap_or(Value::Null),
        // A read never holds a clear; an unset field reads null.
        Repr::Clear => Value::Null,
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

fn entries<'a>(pairs: impl IntoIterator<Item = (&'a str, Value)>) -> Value {
    Value::Map(
        pairs
            .into_iter()
            .map(|(key, value)| (Key::Text(key.into()), value))
            .collect(),
    )
}

/// An error answer to the request `id`.
fn failure(id: u64, code: &str, message: &str) -> Value {
    failure_of(id, &Failed(code.into(), message.into(), None))
}

/// The answer to request `id` that failed with `failed`.
pub(super) fn failure_of(id: u64, failed: &Failed) -> Value {
    let Failed(code, message, fingerprint) = failed;
    let mut error = vec![
        ("code", Value::Text(code.clone())),
        ("message", Value::Text(message.clone())),
    ];
    if let Some(fingerprint) = fingerprint {
        error.push(("fingerprint", Value::Text(fingerprint.clone())));
    }
    entries([("id", Value::Unsigned(id)), ("error", entries(error))])
}
