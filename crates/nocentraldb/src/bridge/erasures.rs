//! The bridge's `erasures.list` (NOC-315, slice 2; RFC 0009, "Erasures";
//! the shapes the owner decided on NOC-246, Carsten, 2026-11-12), as the
//! module docs of [`super`] list it.

use ncdb_core::cbor::Value;

use super::{Failed, entries, space_id};
use crate::db::Db;
use crate::erasures::{Acknowledged, Erasure, ErasureState};

/// `erasures.list` `{"space": id}`: the audit of the space's erasures, each
/// as [`erasure`] encodes it, in shred-id order.
pub(super) async fn list(db: &Db, args: &Value) -> Result<Value, Failed> {
    let space = db.spaces().get(space_id(args)?).await?;
    let erasures = space.erasures().list().await?;
    Ok(Value::Array(erasures.iter().map(erasure).collect()))
}

/// An erasure as the bridge sends it: `{"shred": id, "state": "final" |
/// "provisional" | "takenBack", "performer": {"id": member, "account":
/// fingerprint}, "signOff": fingerprint, "members": [{"id": member,
/// "account": fingerprint, "current": bool, "acknowledged": "deleted" |
/// "seen" | "missing"}]}`. A member is its 64 lowercase hex digits
/// (NOC-298), a fingerprint its text form.
pub(super) fn erasure(erasure: &Erasure) -> Value {
    let state = match erasure.state {
        ErasureState::Final => "final",
        ErasureState::Provisional => "provisional",
        ErasureState::TakenBack => "takenBack",
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
