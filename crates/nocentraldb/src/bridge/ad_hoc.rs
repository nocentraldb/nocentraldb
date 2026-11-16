//! The bridge's ad-hoc spaces (NOC-318 S4; RFC 0009, "Sub-agents and
//! ad-hoc spaces"; the TypeScript shapes the owner decided on NOC-351,
//! Carsten, 2026-11-16), as the module docs of [`super`] list them:
//!
//! - `spaces.createAdHoc` `{"copy": [{"space": id, "collection": text,
//!   "id": id}], "personal": bool?, "local": bool?}`: creates the ad-hoc
//!   space with [`Spaces::create_ad_hoc`](crate::Spaces::create_ad_hoc).
//!   Answers `{"space": id, "notice": notice, "copies": [{"source": id,
//!   "copy": id}], "clearedLinks": u64, "pluginFieldsNotCopied": u64}`,
//!   the copies in the order of `copy`.
//! - `space.sourceOf` `{"space": id, "copy": id}`: answers `{"space": id,
//!   "id": id}`, the source of the copy, or `null`.
//! - `space.copyOf` `{"space": id, "sourceSpace": id, "source": id}`:
//!   answers the copy's id, or `null`.

use ncdb_core::cbor::Value;

use super::{Failed, entries, get, invalid, members, object_id, space_id, text};
use crate::ad_hoc::AdHoc;
use crate::db::Db;
use crate::error::{Error, ErrorCode};
use crate::id::SpaceId;
use crate::object::Object;

/// `spaces.createAdHoc`: each source is read first, so that a source the
/// database does not hold is `notFound`, as the Rust SDK has it.
pub(super) async fn create(db: &Db, args: &Value) -> Result<Value, Failed> {
    let Some(Value::Array(sources)) = get(args, "copy") else {
        return Err(invalid(crate::texts::bridge::COPY_IS_ALIST));
    };
    let mut copy = Vec::with_capacity(sources.len());
    for source in sources {
        copy.push(read(db, source).await?);
    }
    let created = db
        .spaces()
        .create_ad_hoc(AdHoc {
            copy,
            personal: flag(args, "personal")?,
            local: flag(args, "local")?,
        })
        .await?;
    let copies = created
        .copies
        .iter()
        .map(|(source, copy)| {
            entries([
                ("source", Value::Text(source.to_string())),
                ("copy", Value::Text(copy.to_string())),
            ])
        })
        .collect();
    Ok(entries([
        ("space", Value::Text(created.space.id().to_string())),
        ("notice", members::notice_of(created.notice)),
        ("copies", Value::Array(copies)),
        ("clearedLinks", count(created.cleared_links)),
        (
            "pluginFieldsNotCopied",
            count(created.plugin_fields_not_copied),
        ),
    ]))
}

/// The object a source names, as `get` reads it.
async fn read(db: &Db, source: &Value) -> Result<Object, Failed> {
    if !matches!(source, Value::Map(_)) {
        return Err(invalid(crate::texts::bridge::COPY_IS_ALIST));
    }
    let space = space_id(source)?;
    let collection = text(source, "collection")?;
    let id = object_id(source, "id")?;
    let object = db.spaces().get(space).await?.get(collection, id).await?;
    object.ok_or_else(|| {
        Error::new(
            ErrorCode::NotFound,
            crate::texts::error::ad_hoc_source_not_held(space, id),
        )
        .into()
    })
}

/// An option that is `false` when left out.
fn flag(args: &Value, key: &str) -> Result<bool, Failed> {
    match get(args, key) {
        None => Ok(false),
        Some(Value::Bool(value)) => Ok(*value),
        Some(_) => Err(invalid(crate::texts::bridge::argument_is_abool(key))),
    }
}

fn count(n: usize) -> Value {
    Value::Unsigned(u64::try_from(n).unwrap_or(u64::MAX))
}

/// `space.sourceOf`.
pub(super) async fn source_of(db: &Db, args: &Value) -> Result<Value, Failed> {
    let space = db.spaces().get(space_id(args)?).await?;
    let source = space.source_of(object_id(args, "copy")?).await?;
    Ok(source.map_or(Value::Null, |(space, id)| {
        entries([
            ("space", Value::Text(space.to_string())),
            ("id", Value::Text(id.to_string())),
        ])
    }))
}

/// `space.copyOf`.
pub(super) async fn copy_of(db: &Db, args: &Value) -> Result<Value, Failed> {
    let space = db.spaces().get(space_id(args)?).await?;
    let source_space: SpaceId = text(args, "sourceSpace")?
        .parse()
        .map_err(|_| invalid(crate::texts::bridge::NOT_ASPACE_ID))?;
    let copy = space
        .copy_of(source_space, object_id(args, "source")?)
        .await?;
    Ok(copy.map_or(Value::Null, |copy| Value::Text(copy.to_string())))
}
