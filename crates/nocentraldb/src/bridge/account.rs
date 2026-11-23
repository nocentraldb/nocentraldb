//! The bridge's account calls (NOC-316; RFC 0009, "Accounts and devices";
//! the shapes the owner decided on NOC-321, Carsten, 2026-11-15), as the
//! module docs of [`super`] list them. The recovery code's words cross the
//! bridge only in `account.recoveryCode`'s answer; no error text carries
//! them.

use std::str::FromStr;

use ncdb_core::cbor::Value;

use super::members::{member_id, notice_of};
use super::{Failed, entries, get, invalid, text};
use crate::db::Db;
use crate::id::SpaceId;
use crate::members::{Invitation, Notice};
use crate::pairing::{PairKind, PairingAnswer};

/// `account.recoveryCode` `{}`: `{"words": [text; 12], "challenge": [n,
/// n]}` while the person has not confirmed they stored the code, else
/// `null`. `challenge` holds the two positions, counted from 1 and in
/// order, whose words the person types back (NOC-502).
pub(super) async fn recovery_code(db: &Db) -> Result<Value, Failed> {
    Ok(match db.account().recovery_code().await? {
        Some(code) => entries([
            (
                "words",
                Value::Array(
                    code.words()
                        .iter()
                        .map(|word| Value::Text((*word).into()))
                        .collect(),
                ),
            ),
            (
                "challenge",
                Value::Array(
                    code.challenge()
                        .iter()
                        .map(|position| Value::Unsigned(*position as u64))
                        .collect(),
                ),
            ),
        ]),
        None => Value::Null,
    })
}

/// `account.confirmRecoveryCode` `{"answers": [{"position": n, "word":
/// text}; 2]}`: when both words match the code at their positions, the
/// database forgets the code (NOC-502). Answers `null`, also when there is
/// none left to confirm. Answers that are not two words at two different
/// positions from 1 to 12 are `invalidValue`, and so is a word that does not
/// match, which names its position, never a word.
pub(super) async fn confirm_recovery_code(db: &Db, args: &Value) -> Result<Value, Failed> {
    let given = match get(args, "answers") {
        Some(Value::Array(given)) if given.len() == 2 => given,
        _ => return Err(invalid(crate::texts::bridge::RECOVERY_ANSWERS)),
    };
    let mut answers = [(0usize, ""); 2];
    for (slot, answer) in answers.iter_mut().zip(given) {
        let position = match get(answer, "position") {
            Some(Value::Unsigned(n)) if (1..=12).contains(n) => *n as usize,
            _ => return Err(invalid(crate::texts::bridge::RECOVERY_ANSWERS)),
        };
        let word = match get(answer, "word") {
            Some(Value::Text(word)) => word.as_str(),
            _ => return Err(invalid(crate::texts::bridge::RECOVERY_ANSWERS)),
        };
        *slot = (position, word);
    }
    if answers[0].0 == answers[1].0 {
        return Err(invalid(crate::texts::bridge::RECOVERY_ANSWERS));
    }
    if let Some(code) = db.account().recovery_code().await? {
        code.confirm_at(answers).await?;
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
        _ => return Err(invalid(crate::texts::bridge::KIND_IS_DEVICE_OR_AGENT)),
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
    Ok(entries([
        (
            "notices",
            Value::Array(notices.into_iter().map(notice_of).collect()),
        ),
        ("identity", identity(db)),
    ]))
}

/// The account's identity, as `open` and `account.completePairing` answer
/// it: its recovery key in hex, or `null` while the database waits to be
/// paired (RFC 0015, "In the SDK").
pub(super) fn identity(db: &Db) -> Value {
    db.account().identity().map_or(Value::Null, Value::Text)
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
