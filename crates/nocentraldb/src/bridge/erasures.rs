//! The bridge's `erasures.list` (NOC-315, slice 2; RFC 0009, "Erasures";
//! the shapes the owner decided on NOC-246, Carsten, 2026-11-12) and
//! `erasures.takeBack` (NOC-406; Carsten, 2026-11-15, NOC-326), as the
//! module docs of [`super`] list them.

use std::str::FromStr;

use ncdb_core::cbor::Value;

use super::members::notice_of;
use super::{Failed, entries, invalid, space_id, text};
use crate::db::Db;
use crate::erasures::{Acknowledged, Erasure, ErasureState};
use crate::id::ChangeId;

/// `erasures.list` `{"space": id}`: the audit of the space's erasures, each
/// as [`erasure`] encodes it, in shred-id order.
pub(super) async fn list(db: &Db, args: &Value) -> Result<Value, Failed> {
    let space = db.spaces().get(space_id(args)?).await?;
    let erasures = space.erasures().list().await?;
    Ok(Value::Array(erasures.iter().map(erasure).collect()))
}

/// `erasures.takeBack` `{"space": id, "shred": id}`: takes the space's
/// provisional erasure back and answers the notice `takenBack`. Anyone but
/// an admin of the performer's account is `notAllowed`; a shred that is
/// final, decided or not of the space is `invalidValue`.
pub(super) async fn take_back(db: &Db, args: &Value) -> Result<Value, Failed> {
    let space = db.spaces().get(space_id(args)?).await?;
    let shred = ChangeId::from_str(text(args, "shred")?)
        .map_err(|_| invalid(crate::texts::bridge::NOT_ACHANGE_ID))?;
    Ok(notice_of(space.erasures().take_back(shred).await?))
}

/// An erasure as the bridge sends it: `{"shred": id, "state": "final" |
/// "provisional" | "takenBack" | "waiting", "performer": {"id": member, "account":
/// fingerprint}, "signOff": fingerprint, "members": [{"id": member,
/// "account": fingerprint, "current": bool, "acknowledged": "deleted" |
/// "seen" | "missing"}]}`. A member is its 64 lowercase hex digits
/// (NOC-298), a fingerprint its text form.
pub(super) fn erasure(erasure: &Erasure) -> Value {
    let state = match erasure.state {
        ErasureState::Final => "final",
        ErasureState::Provisional => "provisional",
        ErasureState::TakenBack => "takenBack",
        ErasureState::Waiting => "waiting",
    };
    let members = erasure
        .members
        .iter()
        .map(|member| {
            let acknowledged = match member.acknowledged {
                Acknowledged::Deleted => "deleted",
                Acknowledged::Seen => "seen",
                Acknowledged::Missing => "missing",
            };
            entries([
                ("id", Value::Text(member.id.to_string())),
                ("account", Value::Text(member.account.to_string())),
                ("current", Value::Bool(member.current)),
                ("acknowledged", Value::Text(acknowledged.into())),
            ])
        })
        .collect();
    entries([
        ("shred", Value::Text(erasure.shred.to_string())),
        ("state", Value::Text(state.into())),
        (
            "performer",
            entries([
                ("id", Value::Text(erasure.performer.to_string())),
                (
                    "account",
                    Value::Text(erasure.performer_account.to_string()),
                ),
            ]),
        ),
        ("signOff", Value::Text(erasure.sign_off.to_string())),
        ("members", Value::Array(members)),
    ])
}
