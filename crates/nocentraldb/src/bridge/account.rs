//! The bridge's account calls (NOC-316; RFC 0009, "Accounts and devices";
//! the shapes the owner decided on NOC-321, Carsten, 2026-11-15), as the
//! module docs of [`super`] list them. The recovery code's words cross the
//! bridge only in `account.recoveryCode`'s answer; no error text carries
//! them.

use std::str::FromStr;

use ncdb_core::cbor::Value;

use super::members::{member_id, notice_of};
use super::{Failed, entries, invalid, text};
use crate::db::Db;
use crate::id::SpaceId;
use crate::members::{Invitation, Notice};
use crate::pairing::{PairKind, PairingAnswer};

/// `account.recoveryCode` `{}`: `{"words": [text; 12]}` while the person
/// has not confirmed they stored the code, else `null`.
pub(super) async fn recovery_code(db: &Db) -> Result<Value, Failed> {
    Ok(match db.account().recovery_code().await? {
        Some(code) => entries([(
            "words",
            Value::Array(
                code.words()
                    .iter()
                    .map(|word| Value::Text((*word).into()))
                    .collect(),
            ),
        )]),
        None => Value::Null,
    })
}

/// `account.confirmRecoveryCode` `{}`: the database forgets the code.
/// Answers `null`, also when there is none left to confirm.
pub(super) async fn confirm_recovery_code(db: &Db) -> Result<Value, Failed> {
    if let Some(code) = db.account().recovery_code().await? {
        code.confirm().await?;
    }
    Ok(Value::Null)
}

/// `account.recovered` `{}`: the notice `accountRecovered` on the open
/// that recovered the account, else `null`.
pub(super) async fn recovered(db: &Db) -> Result<Value, Failed> {
    Ok(db
        .account()
        .recovered()
        .await?
        .map_or(Value::Null, notice_of))
}

/// `account.pair` `{"kind": "device" | "agent", "invitation": text}`, on a
/// device of the account: `{"answer": text, "added": [{"space": id,
/// "notice": notice}], "askAdmins": [id]}` (NOC-321 points 3 and 4).
pub(super) async fn pair(db: &Db, args: &Value) -> Result<Value, Failed> {
    let kind = match text(args, "kind")? {
        "device" => PairKind::Device,
        "agent" => PairKind::Agent,
        _ => return Err(invalid("a kind is \"device\" or \"agent\"")),
    };
    let invitation = Invitation::from_str(text(args, "invitation")?)?;
    let paired = db.account().pair(kind, &invitation).await?;
    Ok(entries([
        ("answer", Value::Text(paired.answer.to_string())),
        ("added", spaces_with_notices(paired.added)),
        ("askAdmins", space_ids(&paired.ask_admins)),
    ]))
}

/// `account.completePairing` `{"answer": text}`, on the new database:
/// `{"notices": [notice]}` (NOC-321 point 5). A text that is no answer is
/// `invalidValue`, naming what is wrong, never a byte.
pub(super) async fn complete_pairing(db: &Db, args: &Value) -> Result<Value, Failed> {
    let answer = PairingAnswer::from_str(text(args, "answer")?)?;
    let notices = db.account().complete_pairing(&answer).await?;
    Ok(entries([(
        "notices",
        Value::Array(notices.into_iter().map(notice_of).collect()),
    )]))
}

/// `account.removeMember` `{"member": id, "recover": words}`, on a device
/// of the account: `{"removed": [{"space": id, "notice": notice}],
/// "askAdmins": [id], "notice": notice}` (NOC-321 point 6). Words that are
/// no recovery code are `invalidValue`, and no error names them.
pub(super) async fn remove_member(db: &Db, args: &Value) -> Result<Value, Failed> {
    let member = member_id(args)?;
    let removed = db
        .account()
        .remove_member(member, text(args, "recover")?)
        .await?;
    Ok(entries([
        ("removed", spaces_with_notices(removed.removed)),
        ("askAdmins", space_ids(&removed.ask_admins)),
        ("notice", notice_of(removed.notice)),
    ]))
}

/// `[{"space": id, "notice": notice}]`, objects as the TypeScript SDK
/// returns them (NOC-321 point 4).
fn spaces_with_notices(spaces: Vec<(SpaceId, Notice)>) -> Value {
    Value::Array(
        spaces
            .into_iter()
            .map(|(space, notice)| {
                entries([
                    ("space", Value::Text(space.to_string())),
                    ("notice", notice_of(notice)),
                ])
            })
            .collect(),
    )
}

/// `[id]`.
fn space_ids(spaces: &[SpaceId]) -> Value {
    Value::Array(
        spaces
            .iter()
            .map(|space| Value::Text(space.to_string()))
            .collect(),
    )
}
