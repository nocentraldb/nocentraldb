//! The account space (RFC 0010, "The account space"; NOC-266): an
//! ordinary space whose members are the account's devices, each an admin,
//! under the model the SDK ships, `model.lock.json` beside this file. It
//! records which devices and agents the account certified, which spaces
//! each of them belongs to, which accounts are linked into it, and the
//! account key statements. Its content never holds the account key or the
//! recovery code readably.
//!
//! The view folds it by the shipped model and holds its collections under
//! their `$` names (`ncdb_core::view::RESERVED`); the app's model, which
//! [`app_model`] gives, never names them. The account space is no app
//! space: `spaces().list()` leaves it out and `spaces().get` refuses it,
//! as a space of another model.
//!
//! The account's first device creates it with the first app space it
//! creates ([`create`]): the relay lets only an account it admits create
//! spaces (NOC-257), and a space the device created shows that the relay
//! admits its account. A device that joins an account, as a pairing one
//! (NOC-267), creates none: another device adds it to the account space.
//! A device that only joined spaces of others keeps no records until it
//! holds an account space. Each device keeps its own
//! records current ([`reconcile`]): its member record, and one membership
//! record per app space it holds, with its role, deleted once it no longer
//! belongs to the space. A record's id derives from what it records, so a
//! device finds its own records again in any copy.
//!
//! An agent keeps no account space: it holds an agent certificate of an
//! account, and the account's devices record it.

mod handle;
mod remove;
mod renew;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod reopen_tests;

pub use handle::Removed;
pub(crate) use handle::take_new_key;
#[cfg(test)]
pub(crate) use renew::{new_key, renew, statements, wrapped_to};

use std::collections::BTreeMap;
use std::sync::OnceLock;

use ncdb_core::cbor;
use ncdb_core::change::WriterModel;
use ncdb_core::ids::{ObjectId as CoreObjectId, SpaceId as CoreSpaceId};
use ncdb_core::membership::{MemberKind, Role as CoreRole};
use ncdb_core::model_lock::ModelLock;
use ncdb_core::pairing::LinkStatement;
use ncdb_core::view::{Model, RESERVED, reserved_name};
use sha2::{Digest, Sha256};

use crate::db::Inner;
use crate::error::{Error, storage};
use crate::id::{ChangeId, ObjectId};
use crate::members::{Fingerprint, Role};
use crate::tx::Transaction;
use crate::value::{Repr, Value};

/// The shipped model's lock, as the SDK ships it.
const LOCK: &str = include_str!("model.lock.json");

/// The collections of the shipped model (RFC 0010, "Its model").
const MEMBERS: &str = "members";
const MEMBERSHIPS: &str = "memberships";
const LINKS: &str = "links";
const STATEMENTS: &str = "statements";
const WRAPPED_KEYS: &str = "wrappedKeys";

/// The lock of the model the SDK ships for the account space.
pub(crate) fn lock() -> ModelLock {
    ModelLock::from_json(LOCK).expect("the shipped lock reads")
}

/// The shipped model, with its collections under their own names, as its
/// changes name them.
fn model() -> &'static Model {
    static MODEL: OnceLock<Model> = OnceLock::new();
    MODEL.get_or_init(|| Model::from_lock(&lock()).expect("the shipped lock is valid"))
}

/// The `model.id` every account space's genesis names.
pub(crate) fn id() -> [u8; 16] {
    model().id
}

/// The app's model of the view's model: without the collections of the
/// shipped model, which only the account space folds by.
pub(crate) fn app_model(view: &Model) -> Model {
    let mut app = view.clone();
    let shipped = |collection: &String| collection.starts_with(RESERVED);
    app.collections.retain(|c, _| !shipped(c));
    app.unregistered.retain(|c, _| !shipped(c));
    app.personal.retain(|c, _| !shipped(c));
    app.indexes.retain(|index| !shipped(&index.collection));
    app.reserved = None;
    app
}

/// What the account certified a member as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Device,
    Agent,
}

/// A member record: a device or an agent the account certified.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MemberRecord {
    pub(crate) member: [u8; 32],
    pub(crate) kind: Kind,
    pub(crate) removed: bool,
}

/// A membership record: that a member of the account belongs to a space,
/// with its role.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MembershipRecord {
    pub(crate) member: [u8; 32],
    pub(crate) space: [u8; 16],
    pub(crate) role: Role,
}

/// The account space of this database's current account, if the copy
/// holds it: the first, by id, whose genesis names the shipped model and
/// whose creator's certificate names this account, by fingerprint
/// (NOC-310). A database that created spaces before it was linked into
/// another account keeps the account space of its old account; that one
/// is no longer its account space, and stays as it is. `None` while the
/// copy holds no account space of its current account, or while the
/// database has no account yet.
pub(crate) fn space(inner: &Inner) -> Option<CoreSpaceId> {
    let shipped = id();
    let me = inner.member.key().id();
    let ours = Fingerprint::of(&inner.account.certificate(me, inner.kind)?);
    inner
        .spaces
        .iter()
        .find(|(_, state)| {
            let resolved = state.checks.resolved();
            let creator = resolved
                .seniority()
                .first()
                .and_then(|creator| resolved.member(*creator));
            state.checks.model_id() == Some(shipped)
                && creator.is_some_and(|creator| Fingerprint::of(&creator.certificate) == ours)
        })
        .map(|(space, _)| *space)
}

/// Lowercase hex.
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The id of a record: the first 16 bytes of SHA-256 over a context
/// string and what the record is about, so that each device derives the
/// same id for the same record.
fn record_id(context: &str, parts: &[&[u8]]) -> CoreObjectId {
    let mut hash = Sha256::new();
    hash.update(context.as_bytes());
    for part in parts {
        hash.update(part);
    }
    let digest = hash.finalize();
    CoreObjectId(digest[..16].try_into().expect("16 of 32 bytes"))
}

fn member_object(member: [u8; 32]) -> CoreObjectId {
    record_id("nocentraldb/account-member", &[&member])
}

fn membership_object(member: [u8; 32], space: [u8; 16]) -> CoreObjectId {
    record_id("nocentraldb/account-membership", &[&member, &space])
}

/// The values of a record's fields, by field name.
type Values = BTreeMap<String, Option<Value>>;

/// The rows of a shipped collection in the account space, each object
/// that is not deleted with its fields' values, read through the index
/// `index`.
fn rows(
    inner: &Inner,
    collection: &str,
    index: &str,
) -> Result<Vec<(CoreObjectId, Values)>, Error> {
    let Some(space) = space(inner) else {
        return Ok(Vec::new());
    };
    let name = reserved_name(collection);
    let fields = &model().collections[collection];
    let mut rows = Vec::new();
    let mut after = None;
    loop {
        let page = inner
            .view
            .page_in(space, &name, index, &[], after.as_ref(), 256)
            .map_err(storage)?;
        for (_, object) in page.rows {
            let Some(row) = inner.view.get(space, &name, object).map_err(storage)? else {
                continue;
            };
            if row.object.is_deleted() {
                continue;
            }
            let values = fields
                .iter()
                .map(|(field, kind)| (field.clone(), crate::object::value(kind, &row, field)))
                .collect();
            rows.push((object, values));
        }
        match page.next {
            Some(next) => after = Some(next),
            None => return Ok(rows),
        }
    }
}

fn text(values: &Values, field: &str) -> Option<String> {
    match values.get(field)? {
        Some(Value(Repr::Text(text))) => Some(text.clone()),
        _ => None,
    }
}

fn id_of<const N: usize>(text: &str) -> Option<[u8; N]> {
    if text.len() != 2 * N {
        return None;
    }
    let mut bytes = [0u8; N];
    for (i, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(text.get(2 * i..2 * i + 2)?, 16).ok()?;
    }
    Some(bytes)
}

/// The member records of the account space.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn members(inner: &Inner) -> Result<Vec<MemberRecord>, Error> {
    Ok(member_rows(inner)?.into_values().collect())
}

/// The member records by the id of their object.
fn member_rows(inner: &Inner) -> Result<BTreeMap<CoreObjectId, MemberRecord>, Error> {
    let mut records = BTreeMap::new();
    for (object, values) in rows(inner, MEMBERS, "byMemberId")? {
        let Some(member) = text(&values, "memberId").and_then(|t| id_of::<32>(&t)) else {
            continue;
        };
        let kind = match text(&values, "kind").as_deref() {
            Some("agent") => Kind::Agent,
            _ => Kind::Device,
        };
        let removed = matches!(
            values.get("removed"),
            Some(Some(Value(Repr::Boolean(true))))
        );
        records.insert(
            object,
            MemberRecord {
                member,
                kind,
                removed,
            },
        );
    }
    Ok(records)
}

/// The membership records of the account space.
pub(crate) fn memberships(inner: &Inner) -> Result<Vec<MembershipRecord>, Error> {
    let members = member_rows(inner)?;
    let mut records = Vec::new();
    for (_, values) in rows(inner, MEMBERSHIPS, "bySpace")? {
        let member = match values.get("member") {
            Some(Some(Value(Repr::Id(object)))) => members.get(&object.0),
            _ => None,
        };
        let space = text(&values, "space").and_then(|t| id_of::<16>(&t));
        let role = match text(&values, "role").as_deref() {
            Some("reader") => Role::Reader,
            Some("writer") => Role::Writer,
            Some("admin") => Role::Admin,
            _ => continue,
        };
        if let (Some(member), Some(space)) = (member, space) {
            records.push(MembershipRecord {
                member: member.member,
                space,
                role,
            });
        }
    }
    Ok(records)
}

/// Every text the account space's records hold, for the check that none
/// of them is a key.
#[cfg(test)]
pub(crate) fn texts(inner: &Inner) -> Result<Vec<String>, Error> {
    let mut texts = Vec::new();
    let indexes = [
        (MEMBERS, "byMemberId"),
        (MEMBERSHIPS, "bySpace"),
        (LINKS, "byAccount"),
        (STATEMENTS, "byNumber"),
        (WRAPPED_KEYS, "byNumber"),
    ];
    for (collection, index) in indexes {
        for (_, values) in rows(inner, collection, index)? {
            texts.extend(values.into_values().flatten().filter_map(|v| match v.0 {
                Repr::Text(text) => Some(text),
                _ => None,
            }));
        }
    }
    Ok(texts)
}

/// The role as the membership record writes it.
fn role_text(role: CoreRole) -> &'static str {
    match role {
        CoreRole::Reader => "reader",
        CoreRole::Writer => "writer",
        CoreRole::Admin => "admin",
    }
}

/// Creates the account space, when the device holds none and creates the
/// account's first: a device that joins an account another device created
/// creates none, nor does a linked database that still keeps its link
/// statement, nor a paired one that keeps that it joined an account until
/// the account's account space reaches it (NOC-312 C3), and an agent keeps
/// no account space.
pub(crate) fn create(inner: &mut Inner) -> Result<(), Error> {
    if inner.kind == MemberKind::Device
        && !inner.joins_account
        && space(inner).is_none()
        && !keeps_a_link(inner)?
        && !inner.view.joined().map_err(storage)?
    {
        crate::space::create_space(inner, id())?;
    }
    Ok(())
}

/// Whether the database keeps a link statement it has not yet seen reach
/// the person's account space (NOC-311). It was linked into an account
/// whose account space a device of it created, and it keeps the statement
/// until the relay holds it there, by which time it holds that space; so
/// while it keeps one it creates no account space, also after a reopen,
/// when `joins_account` no longer says so.
fn keeps_a_link(inner: &Inner) -> Result<bool, Error> {
    Ok(!inner.view.pending_links().map_err(storage)?.is_empty())
}

/// Brings the device's own records in the account space up to date: its
/// member record, and a membership record for each app space it belongs
/// to, with its role. First it removes each member another device marked
/// removed from the app spaces this device administers; last it writes
/// the link statements a completed pairing kept (NOC-291). Without an
/// account space, and on an agent, it writes no record, as it does when
/// the records are current.
pub(crate) fn reconcile(inner: &mut Inner) -> Result<(), Error> {
    // Once the account's account space reached a device that joined the
    // account, it no longer waits for it (NOC-311).
    if space(inner).is_some() && inner.view.joined().map_err(storage)? {
        inner.view.clear_joined().map_err(storage)?;
    }
    records(inner)?;
    flush_links(inner)
}

#[cfg_attr(not(test), allow(dead_code))]
/// The account's link statements the account space holds, each read from
/// its record; a record that does not read as one is left out.
pub(crate) fn links(inner: &Inner) -> Result<Vec<LinkStatement>, Error> {
    let mut links = Vec::new();
    for (_, values) in rows(inner, LINKS, "byAccount")? {
        let bytes = text(&values, "statement")
            .and_then(|text| data_encoding::BASE64URL_NOPAD.decode(text.as_bytes()).ok());
        if let Some(link) = bytes.and_then(|bytes| LinkStatement::decode(&bytes).ok()) {
            links.push(link);
        }
    }
    Ok(links)
}

fn link_object(old_account_key: [u8; 32]) -> CoreObjectId {
    record_id("nocentraldb/account-link", &[&old_account_key])
}

/// Writes the link statements a completed pairing kept into the account
/// space's `links` (RFC 0010, "The link statement"), once the database
/// holds the account space, and forgets each once the relay holds the
/// change that wrote it, or the account space already holds the link.
fn flush_links(inner: &mut Inner) -> Result<(), Error> {
    let Some(account) = space(inner) else {
        return Ok(());
    };
    if crate::space::writable_by(inner, account, id()).is_err() {
        return Ok(());
    }
    for (bytes, change) in inner.view.pending_links().map_err(storage)? {
        let Ok(link) = LinkStatement::decode(&bytes) else {
            inner.view.delete_pending_link(&bytes).map_err(storage)?;
            continue;
        };
        let object = link_object(link.old_account_key);
        match change {
            Some(change) => {
                let held = inner
                    .spaces
                    .keys()
                    .any(|space| inner.view.relay_position(*space, change).is_some());
                if held {
                    inner.view.delete_pending_link(&bytes).map_err(storage)?;
                }
            }
            None if exists(inner, account, LINKS, object)? => {
                inner.view.delete_pending_link(&bytes).map_err(storage)?;
            }
            None => {
                let mut tx = Transaction::reserved(model());
                tx.create_at(
                    LINKS,
                    object,
                    [
                        ("account", Value::from(hex(&link.old_account_key).as_str())),
                        (
                            "statement",
                            Value::from(data_encoding::BASE64URL_NOPAD.encode(&bytes).as_str()),
                        ),
                    ],
                )?;
                let change = write(inner, account, tx)?;
                inner
                    .view
                    .set_pending_link_change(&bytes, change.0)
                    .map_err(storage)?;
            }
        }
    }
    Ok(())
}

/// Keeps the device's own records current: its member record, and one
/// membership record per app space it holds.
fn records(inner: &mut Inner) -> Result<(), Error> {
    if inner.kind != MemberKind::Device {
        return Ok(());
    }
    let Some(account) = space(inner) else {
        return Ok(());
    };
    // A space whose key the copy does not hold yet takes no write.
    if crate::space::writable_by(inner, account, id()).is_err() {
        return Ok(());
    }
    // A member another device removed as lost leaves this device's spaces
    // too (RFC 0010, "Losing a device").
    remove::remove_marked(inner)?;
    let me = inner.member.key().id();
    let own = member_object(me.0);
    let mut tx = Transaction::reserved(model());
    let mut changed = false;
    if !member_rows(inner)?.contains_key(&own) {
        let kind = Value::from("device");
        tx.create_at(
            MEMBERS,
            own,
            [
                ("memberId", Value::from(hex(&me.0).as_str())),
                ("kind", kind),
                ("removed", Value::from(false)),
            ],
        )?;
        changed = true;
    }
    // The app spaces the device belongs to, with its role.
    let app = inner.model_id;
    let wanted: BTreeMap<[u8; 16], CoreRole> = inner
        .spaces
        .iter()
        .filter(|(_, state)| state.checks.model_id() == Some(app))
        .filter_map(|(space, state)| Some((space.0, state.checks.resolved().role(me)?)))
        .collect();
    let recorded: BTreeMap<[u8; 16], Role> = memberships(inner)?
        .into_iter()
        .filter(|m| m.member == me.0)
        .map(|m| (m.space, m.role))
        .collect();
    for (space, role) in &wanted {
        let object = membership_object(me.0, *space);
        let values = [
            ("member", Value::from(ObjectId(own))),
            ("space", Value::from(hex(space).as_str())),
            ("role", Value::from(role_text(*role))),
        ];
        match recorded.get(space) {
            Some(held) if role_text(*role) == role_text(core_role(*held)) => {}
            Some(_) => {
                tx.update(MEMBERSHIPS, ObjectId(object), values)?;
                changed = true;
            }
            None if exists(inner, account, MEMBERSHIPS, object)? => {
                tx.restore(MEMBERSHIPS, ObjectId(object))?;
                tx.update(MEMBERSHIPS, ObjectId(object), values)?;
                changed = true;
            }
            None => {
                tx.create_at(MEMBERSHIPS, object, values)?;
                changed = true;
            }
        }
    }
    for space in recorded.keys().filter(|space| !wanted.contains_key(*space)) {
        tx.delete(MEMBERSHIPS, ObjectId(membership_object(me.0, *space)))?;
        changed = true;
    }
    if !changed {
        return Ok(());
    }
    write(inner, account, tx).map(drop)
}

/// Writes `tx` into the account space `account`, by the shipped model,
/// and returns the id of the change it wrote.
fn write(inner: &mut Inner, account: CoreSpaceId, tx: Transaction) -> Result<ChangeId, Error> {
    let writer = WriterModel {
        id: model().id,
        version: model().version,
    };
    Ok(crate::space::write_change(inner, account, tx, writer, |_| Ok(()))?.change)
}

/// Whether the copy holds the object `object` of a shipped collection in
/// the account space, deleted or not.
fn exists(
    inner: &Inner,
    account: CoreSpaceId,
    collection: &str,
    object: CoreObjectId,
) -> Result<bool, Error> {
    let row = inner
        .view
        .get(account, &reserved_name(collection), object)
        .map_err(storage)?;
    Ok(row.is_some_and(|row| {
        row.object.collection().value.map(|entry| entry.value)
            == Some(cbor::Value::Text(reserved_name(collection)))
    }))
}

fn core_role(role: Role) -> CoreRole {
    match role {
        Role::Reader => CoreRole::Reader,
        Role::Writer => CoreRole::Writer,
        Role::Admin => CoreRole::Admin,
    }
}
