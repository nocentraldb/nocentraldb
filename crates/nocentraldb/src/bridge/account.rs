//! The bridge's account calls (NOC-316; RFC 0009, "Accounts and devices";
//! the shapes the owner decided on NOC-321, Carsten, 2026-11-15), as the
//! module docs of [`super`] list them. The recovery code's words cross the
//! bridge only in `account.recoveryCode`'s answer; no error text carries
//! them.

use ncdb_core::cbor::Value;

use super::{Failed, entries, members::notice_of};
use crate::db::Db;

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
