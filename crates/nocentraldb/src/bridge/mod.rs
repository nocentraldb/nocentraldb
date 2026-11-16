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
//!   on OPFS. Answers `{"sync": status}`, the first sync status (below).
//!   A second `open` fails with `invalidValue`.
//! - `close` `{}`: drops the database; any later call but `open` fails
//!   with `closed`. Answers `null`.
//! - `spaces.create` `{}`: answers the new space's id, 32 hex digits.
//! - `spaces.get` `{"space": id}`: answers the id, or fails with `notFound`.
//! - `spaces.list` `{}`: answers `[{"id": id, "role": text|null,
//!   "model": "ours" | "another"}]`.
//! - `write` `{"space": id, "ops": [op]}`, each op one of `{"op":
//!   "create", "collection": text, "values": map}`, `{"op": "update",
//!   "collection": text, "id": id, "values": map}`, `{"op": "delete",
//!   "collection": text, "id": id}` or `{"op": "restore", "collection":
//!   text, "id": id}`: answers `{"change": id, "ids": [id]}`.
//! - `get` `{"space": id, "collection": text, "id": id}`: answers `null`
//!   or `{"$id": id, "$space": id, "$deleted": bool, "fields": map}`.
//!
//! The calls of slice 4b, members (RFC 0009, "Invitations and members"),
//! in `members.rs`: `invitation`, `members.add`, `members.list`,
//! `members.remove` and `members.setRole`.
//!
//! The calls of slice 4, sync (RFC 0009, "Sync and status"):
//!
//! - `sync.pause` `{}` and `sync.resume` `{}`: stop and restart syncing.
//!   Answer `null`.
//! - `sync.isConfirmed` `{"change": id}`: whether the relay holds the
//!   change. Answers a bool. Waiting for a confirmation is the SDK's: a
//!   call that waits would hold back every call after it.
//! - `space.status` `{"space": id}`: the space's status (see
//!   [`space_status`]), and from then on a `spaceStatus` event whenever
//!   it changes, until the database closes.
//!
//! The calls of NOC-315, slice 1 (RFC 0009, "Spaces"):
//!
//! - `spaces.follow` `{}`: answers the list as `spaces.list` does, and
//!   from then on a `spaces` event whenever it changes, until the
//!   database closes.
//! - `space.leave` `{"space": id}`: writes this member's removal of
//!   itself. Answers `{"notice": notice}`; the last admin is refused with
//!   `lastAdmin`.
//!
//! The calls of NOC-318 S4, ad-hoc spaces (RFC 0009, "Sub-agents and
//! ad-hoc spaces"), in `ad_hoc.rs`: `spaces.createAdHoc`,
//! `space.sourceOf` and `space.copyOf`.
//!
//! The call of NOC-315, slice 2, in `erasures.rs`: `erasures.list`
//! `{"space": id}`, the audit of the space's erasures (RFC 0009,
//! "Erasures").
//!
//! The calls of NOC-315, slice 3 (RFC 0006, "What the SDK reports"; RFC
//! 0009, "Sync and status"), in `reports.rs`: `open` also answers
//! `"durability"` and `"view"`; `durability` `{}` reads again whether the
//! browser keeps the copy persistent and answers the durability; `view`
//! `{}` answers the view's report. A `durability` or `view` event follows
//! each change.
//!
//! The calls of NOC-316, slice 1 (RFC 0009, "Accounts and devices"), in
//! `account.rs`: `open` also takes `"pairing": true` and `"recover":
//! words`; `account.recoveryCode` `{}` answers `{"words": [text]}` or
//! `null`; `account.confirmRecoveryCode` `{}` forgets the code;
//! `account.recovered` `{}` answers a notice or `null`.
//!
//! The calls of NOC-316, slice 2, in `account.rs`: `account.pair` `{"kind":
//! "device" | "agent", "invitation": text}` answers `{"answer": text,
//! "added": [{"space": id, "notice": notice}], "askAdmins": [id]}`;
//! `account.completePairing` `{"answer": text}` answers `{"notices":
//! [notice]}`.
//!
//! The call of NOC-316, slice 3, in `account.rs`: `account.removeMember`
//! `{"member": id, "recover": words}` answers `{"removed": [{"space": id,
//! "notice": notice}], "askAdmins": [id], "notice": notice}`.
//!
//! A sync status is `{"connection": "connected" | "connecting" |
//! "offline" | "paused" | "none", "unconfirmed": u64, "refused": u64,
//! "epochChanged": bool, "error": text | null}`.
//!
//! Events, which the worker sends unasked: `{"event": "folded", ..}`
//! after folds (see [`Events::take`]), `{"event": "sync", "status":
//! status}` whenever the sync status changed, and `{"event":
//! "spaceStatus", "space": id, "status": status}` whenever the status of
//! a space `space.status` asked for changed, and `{"event": "spaces",
//! "spaces": list}` whenever the list `spaces.follow` asked for changed.
//!
//! A value is text, an integer, a float, a boolean, an array,
//! `{"$add": [..]}` / `{"$remove": [..]}` for a set or a link list, or
//! `null`, which clears a basic field or a link (`Value::clear`).

mod account;
mod ad_hoc;
mod erasures;
mod events;
mod members;
mod reports;
#[cfg(test)]
mod tests;
mod values;

pub use events::Events;
use events::{space_list, space_status, sync_status};
use values::{elements, fields, read};

use std::collections::BTreeMap;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

pub use ncdb_core::cbor;
use ncdb_core::cbor::{Key, Value};

use crate::db::{Db, OpenOptions};
use crate::error::{Error, ErrorCode};
use crate::id::{ChangeId, ObjectId, SpaceId};
use crate::members::Role;
use crate::query::Cursor;

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
    lower_camel(&format!("{code:?}"))
}

/// A Rust variant's name in lower camel case, as `NotFound` is `notFound`.
fn lower_camel(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => first.to_lowercase().chain(chars).collect(),
        None => String::new(),
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
            _ => Err(invalid(crate::texts::bridge::NOT_AREQUEST)),
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

    /// Closes the open database, as `close` does, and gives its OPFS
    /// handles back, before the worker ends (NOC-352): the next tab's
    /// worker can then open the database at once, even while the page of
    /// this one is frozen. Whether the handles are free: with the database
    /// still in use elsewhere, they are not, and they go with the worker.
    #[cfg(target_arch = "wasm32")]
    pub fn release(&mut self) -> bool {
        self.close();
        crate::browser::release()
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
                self.close();
                Ok(Value::Null)
            }
            "erasures.list" => erasures::list(&db, args).await,
            "spaces.createAdHoc" => ad_hoc::create(&db, args).await,
            "space.sourceOf" => ad_hoc::source_of(&db, args).await,
            "space.copyOf" => ad_hoc::copy_of(&db, args).await,
            "account.recoveryCode" => account::recovery_code(&db).await,
            "account.confirmRecoveryCode" => account::confirm_recovery_code(&db).await,
            "account.recovered" => account::recovered(&db).await,
            "account.pair" => account::pair(&db, args).await,
            "account.completePairing" => account::complete_pairing(&db, args).await,
            "account.removeMember" => account::remove_member(&db, args).await,
            "durability" => {
                db.refresh_storage().await;
                Ok(reports::durability(&db.durability()))
            }
            "view" => Ok(reports::view(&db.view().current())),
            "find" => self.find(&db, args).await,
            "invitation" => members::invitation(&db).await,
            "members.add" => members::add(&db, args).await,
            "members.list" => members::list(&db, args).await,
            "members.remove" => members::remove(&db, args).await,
            "members.setRole" => members::set_role(&db, args).await,
            "sync.pause" => {
                db.sync().pause().await;
                Ok(Value::Null)
            }
            "sync.resume" => {
                db.sync().resume().await;
                Ok(Value::Null)
            }
            "space.status" => {
                let space = db.spaces().get(space_id(args)?).await?;
                let (status, watch) = space.status().watch();
                if let Some(events) = &self.events {
                    events.spaces().insert(space.id(), watch);
                }
                Ok(space_status(&status))
            }
            "sync.isConfirmed" => {
                let change = ChangeId::from_str(text(args, "change")?)
                    .map_err(|_| invalid(crate::texts::bridge::NOT_ACHANGE_ID))?;
                Ok(Value::Bool(db.sync().is_confirmed(change)))
            }
            "spaces.create" => Ok(Value::Text(db.spaces().create().await?.id().to_string())),
            "spaces.get" => {
                let space = db.spaces().get(space_id(args)?).await?;
                Ok(Value::Text(space.id().to_string()))
            }
            "spaces.list" => Ok(space_list(&db.spaces().list().await?)),
            "spaces.follow" => {
                let (list, watch) = db.spaces().watch();
                if let Some(events) = &self.events {
                    *events.list.lock().unwrap_or_else(|p| p.into_inner()) = Some(watch);
                }
                Ok(space_list(&list))
            }
            "space.leave" => {
                let space = db.spaces().get(space_id(args)?).await?;
                Ok(entries([(
                    "notice",
                    members::notice_of(space.leave().await?),
                )]))
            }
            "write" => write(&db, args).await,
            "get" => {
                let space = db.spaces().get(space_id(args)?).await?;
                let collection = text(args, "collection")?;
                let object = space.get(collection, object_id(args, "id")?).await?;
                Ok(object.map_or(Value::Null, |object| read(&object)))
            }
            other => Err(invalid(crate::texts::bridge::no_such_call(other))),
        }
    }

    /// Drops the open database, its events and its cursors.
    fn close(&mut self) {
        self.db = None;
        if let Some(events) = self.events.take() {
            events.feed.close();
        }
        self.cursors.clear();
    }

    async fn open(&mut self, args: &Value) -> Result<Value, Failed> {
        if self.db.is_some() {
            return Err(invalid(crate::texts::bridge::OPEN_ALREADY));
        }
        let mut options = OpenOptions::new(text(args, "lock")?);
        match (get(args, "inMemory"), get(args, "name")) {
            (Some(Value::Bool(true)), None) => options = options.in_memory(),
            #[cfg(target_arch = "wasm32")]
            (None | Some(Value::Bool(false)), Some(Value::Text(name))) => {
                options = options.browser(name)
            }
            _ => {
                return Err(invalid(crate::texts::bridge::OPEN_TAKES_IN_MEMORY_OR_NAME));
            }
        }
        if let Some(Value::Text(relay)) = get(args, "relay") {
            options = options.relay(relay);
        }
        if let Some(Value::Bool(true)) = get(args, "pairing") {
            options = options.pairing();
        }
        if let Some(Value::Text(words)) = get(args, "recover") {
            options = options.recover(words);
        }
        let db = Db::open(options).await?;
        let (status, sync) = db.sync().watch();
        let (durability, durability_watch) = db.durability_watch();
        let (view, view_watch) = db.view().watch();
        self.events = Some(Events {
            feed: db.folds(),
            sync: Arc::new(Mutex::new(sync)),
            spaces: Arc::new(Mutex::new(BTreeMap::new())),
            list: Arc::new(Mutex::new(None)),
            durability: Arc::new(Mutex::new(durability_watch)),
            view: Arc::new(Mutex::new(view_watch)),
        });
        self.db = Some(db);
        Ok(entries([
            ("sync", sync_status(&status)),
            ("durability", reports::durability(&durability)),
            ("view", reports::view(&view)),
        ]))
    }
}

impl Bridge {
    /// `{"query": {"collection", "index", "equal"?, "take"?, "after"?},
    /// "space"?}`: a page of a declared index, in one space when it names
    /// one (RFC 0009, "Queries and indexes"). Answers `{"items": [object],
    /// "cursor": handle | null, "hasMore": bool}`.
    async fn find(&mut self, db: &Db, args: &Value) -> Result<Value, Failed> {
        let Some(query) = get(args, "query") else {
            return Err(invalid(crate::texts::bridge::FIND_TAKES_AQUERY));
        };
        let mut built = db.index(text(query, "collection")?, text(query, "index")?)?;
        match get(query, "equal") {
            Some(Value::Array(values)) => built = built.equal(elements(values)?),
            None => {}
            Some(_) => return Err(invalid(crate::texts::bridge::EQUAL_IS_AN_ARRAY)),
        }
        match get(query, "take") {
            Some(Value::Unsigned(n)) => {
                built = built.take(usize::try_from(*n).unwrap_or(usize::MAX));
            }
            None => {}
            Some(_) => return Err(invalid(crate::texts::bridge::TAKE_IS_ANUMBER)),
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
            Some(_) => return Err(invalid(crate::texts::bridge::AFTER_IS_ACURSOR)),
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
        return Err(invalid(crate::texts::bridge::WRITE_TAKES_OPS));
    };
    let mut parsed = Vec::with_capacity(ops.len());
    for op in ops {
        let collection = text(op, "collection")?.to_owned();
        let values = match get(op, "values") {
            Some(Value::Map(map)) => fields(&model, &collection, map)?,
            None => Vec::new(),
            Some(_) => return Err(invalid(crate::texts::bridge::VALUES_IS_AMAP)),
        };
        let kind = text(op, "op")?;
        let id = match (kind, get(op, "id")) {
            ("create", None) => None,
            ("create" | "update" | "delete" | "restore", _) => Some(object_id(op, "id")?),
            (other, _) => return Err(invalid(crate::texts::bridge::no_such_op(other))),
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
        _ => Err(invalid(crate::texts::bridge::argument_is_text(key))),
    }
}

fn space_id(args: &Value) -> Result<SpaceId, Failed> {
    SpaceId::from_str(text(args, "space")?)
        .map_err(|_| invalid(crate::texts::bridge::NOT_ASPACE_ID))
}

fn object_id(args: &Value, key: &str) -> Result<ObjectId, Failed> {
    ObjectId::from_str(text(args, key)?)
        .map_err(|_| invalid(crate::texts::bridge::NOT_AN_OBJECT_ID))
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
