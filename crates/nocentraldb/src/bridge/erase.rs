//! The bridge's calls that erase a person (NOC-406; RFC 0009, "The Rust
//! SDK", "Erasing a person"; Carsten, 2026-11-15, NOC-326), as the module
//! docs of [`super`] list them: the Rust SDK's `Db::erasure_request`,
//! `Account::sign_off` and `Db::erase`. The request and the sign-offs
//! cross the bridge as their texts, `ncdb-erase-1:` and
//! `ncdb-sign-off-1:`, which hold nothing personal.

use std::str::FromStr;

use ncdb_core::cbor::Value;

use super::members::notice_of;
use super::{Failed, entries, object_id, space_id, text};
use crate::db::Db;
use crate::erase::{ErasureRequest, SignOffs, Skipped};
use crate::error::{Error, ErrorCode};

/// `erasureRequest` `{"space": id, "collection": text, "id": id}`, one of
/// the person's objects as the SDK read it: the request's text. An object
/// this copy does not hold is `notFound`, one of no person `noPerson`.
pub(super) async fn request(db: &Db, args: &Value) -> Result<Value, Failed> {
    let space = db.spaces().get(space_id(args)?).await?;
    let collection = text(args, "collection")?;
    let id = object_id(args, "id")?;
    let Some(object) = space.get(collection, id).await? else {
        return Err(Error::new(
            ErrorCode::NotFound,
            crate::texts::error::no_such_object(id, collection),
        )
        .into());
    };
    let request = db.erasure_request(&object).await?;
    Ok(Value::Text(request.to_string()))
}

/// `account.signOff` `{"request": text}`: `{"signOffs": text, "notSigned":
/// [id]}`. A database that holds no account key is `notAllowed`.
pub(super) async fn sign_off(db: &Db, args: &Value) -> Result<Value, Failed> {
    let request = ErasureRequest::from_str(text(args, "request")?)?;
    let signed = db.account().sign_off(&request).await?;
    Ok(entries([
        ("signOffs", Value::Text(signed.sign_offs.to_string())),
        (
            "notSigned",
            Value::Array(
                signed
                    .not_signed
                    .iter()
                    .map(|space| Value::Text(space.to_string()))
                    .collect(),
            ),
        ),
    ]))
}

/// `erase` `{"request": text, "signOffs": text}`: `{"shreds": [{"space":
/// id, "shred": id, "provisional": bool}], "skipped": [{"space": id,
/// "reason": "noSignOff" | "notWriter" | "noKey"}], "notice": notice}`. A
/// sign-off that would not hold, or a request of another member, is
/// `invalidValue`, and nothing is written.
pub(super) async fn erase(db: &Db, args: &Value) -> Result<Value, Failed> {
    let request = ErasureRequest::from_str(text(args, "request")?)?;
    let sign_offs = SignOffs::from_str(text(args, "signOffs")?)?;
    let erased = db.erase(&request, &sign_offs).await?;
    let shreds = erased
        .shreds
        .iter()
        .map(|shred| {
            entries([
                ("space", Value::Text(shred.space.to_string())),
                ("shred", Value::Text(shred.shred.to_string())),
                ("provisional", Value::Bool(shred.provisional)),
            ])
        })
        .collect();
    let skipped = erased
        .skipped
        .iter()
        .map(|(space, reason)| {
            entries([
                ("space", Value::Text(space.to_string())),
                ("reason", Value::Text(reason_of(*reason).into())),
            ])
        })
        .collect();
    Ok(entries([
        ("shreds", Value::Array(shreds)),
        ("skipped", Value::Array(skipped)),
        ("notice", notice_of(erased.notice)),
    ]))
}

/// A skipped space's reason, in camel case as the TypeScript SDK names it.
fn reason_of(reason: Skipped) -> &'static str {
    match reason {
        Skipped::NoSignOff => "noSignOff",
        Skipped::NotWriter => "notWriter",
        Skipped::NoKey => "noKey",
    }
}
