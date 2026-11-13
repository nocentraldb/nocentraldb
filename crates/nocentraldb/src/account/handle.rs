//! `Account::remove_member` (RFC 0009, "Accounts and devices"; RFC 0010,
//! "Losing a device"; NOC-268), and how another device of the account
//! takes the new account key it hands out.
//!
//! `remove_member` runs on a device that holds the account key. It checks
//! the recovery code against the account's recovery key first, so a code
//! that is not the account's removes nothing. Then, under one lock, it
//! removes the lost member ([`super::remove`]) and draws the new account
//! key with the next number, one more than the statement the device holds,
//! which is the account's current one ([`super::renew`]). A recovering
//! device holds no statement, so recovery numbers its key differently
//! (Carsten, 2026-11-12, NOC-285). The device keeps the new key in its key
//! record, so its certificates from now on are under it, and carries the
//! new statement into its spaces.

use std::sync::{Mutex, Weak};

use ncdb_core::membership::AccountKeyStatement;
use ncdb_core::recovery::RecoveryCode;

use super::remove::{check, remove_lost};
use super::renew::{new_key, renew, statements};
use crate::db::{Inner, lock};
use crate::error::{Error, ErrorCode};
use crate::id::SpaceId;
use crate::keys::{self, Home, Keys};
use crate::members::{MemberId, Notice, carry};
use crate::pairing::Account;

/// What [`Account::remove_member`] did.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Removed {
    /// The spaces the member was removed from, each with `MemberRemoved`'s
    /// notice, in the order of their ids.
    pub removed: Vec<(SpaceId, Notice)>,
    /// The spaces where the member is a member and no device of the
    /// account is an admin, whose admins the app asks to remove it, in the
    /// order of their ids.
    pub ask_admins: Vec<SpaceId>,
    /// `AccountKeyRenewed`: members that have not learnt the new account
    /// key yet still accept the old one.
    pub notice: Notice,
}

impl Account<'_> {
    /// Removes a lost device or an agent of the account, `member`, from
    /// every space where this device is an admin, which rotates each
    /// space key, and from the account space; and, with the recovery code
    /// `words`, renews the account key with the next number and hands it
    /// to each remaining device of the account through the account space
    /// (RFC 0010, "Losing a device"). Another device of the account that
    /// is an admin of a space this device is not in removes the member
    /// there when it reads the account space. Returns the spaces it
    /// removed the member from, the spaces whose admins the app asks, and
    /// the notice `AccountKeyRenewed`.
    ///
    /// `words` are read as [`OpenOptions::recover`](crate::OpenOptions)
    /// reads them: words that are not a recovery code are `InvalidValue`,
    /// and a code that is not this account's is `NotAllowed`. A database
    /// that holds no account key, an agent's or one waiting to be paired,
    /// is `NotAllowed`; a member the account space does not record is
    /// `NotFound`, and this database's own member `InvalidValue`. Nothing
    /// is written when the call is refused.
    pub async fn remove_member(&self, member: MemberId, words: &str) -> Result<Removed, Error> {
        let code = RecoveryCode::parse(words)
            .map_err(|_| Error::new(ErrorCode::InvalidValue, "the words are no recovery code"))?;
        let recovery = code.recovery_key();
        let (keys, home, statement, removal) = {
            let mut inner = lock(&self.db.inner);
            let held = match &inner.account {
                keys::Account::Holds { statement, .. } => statement.clone(),
                _ => {
                    return Err(Error::new(
                        ErrorCode::NotAllowed,
                        "only a device that holds the account key removes a member of the account",
                    ));
                }
            };
            if recovery.public() != held.recovery_key {
                return Err(Error::new(
                    ErrorCode::NotAllowed,
                    "the recovery code is not this account's",
                ));
            }
            let number = held.number + 1;
            check(&inner, member.0)?;
            if statements(&inner)?.iter().any(|s| s.number >= number) {
                return Err(Error::new(
                    ErrorCode::InvalidValue,
                    "the account space already records a newer account key, \
                     which this device has not taken yet",
                ));
            }
            let removal = remove_lost(&mut inner, member.0)?;
            let (statement, key) = renew(&mut inner, &recovery, number)?;
            let keys = Keys {
                member: inner.member.clone(),
                account: keys::Account::Holds {
                    key,
                    statement: statement.clone(),
                },
                code: inner.code.clone(),
                upgraded: false,
            };
            (keys, inner.home.clone(), statement, removal)
        };
        install(&self.db.inner, keys, home, &statement).await?;
        Ok(Removed {
            removed: removal.removed,
            ask_admins: removal.ask_admins,
            notice: Notice::ACCOUNT_KEY_RENEWED,
        })
    }
}

/// Keeps `keys` as the database's key record, holds its account key from
/// now on, unless the database took a newer one meanwhile, and carries
/// its statement into the database's spaces.
async fn install(
    inner: &Mutex<Inner>,
    keys: Keys,
    home: Home,
    statement: &AccountKeyStatement,
) -> Result<(), Error> {
    home.save(&keys).await?;
    let mut inner = lock(inner);
    if !newer(&inner.account, statement) {
        return Ok(());
    }
    inner.account = keys.account;
    carry(&mut inner, statement)?;
    Ok(())
}

/// Whether `statement` is a newer statement of the account whose key
/// `account` holds.
fn newer(account: &keys::Account, statement: &AccountKeyStatement) -> bool {
    match account {
        keys::Account::Holds {
            statement: held, ..
        } => held.recovery_key == statement.recovery_key && held.number < statement.number,
        _ => false,
    }
}

/// Takes a newer account key the account space hands this device, if
/// there is one (RFC 0010, "Losing a device", step 3): keeps it in the key
/// record, so that the device's certificates are under it from now on,
/// and carries its statement into the device's spaces. The sync calls it
/// after each round of pulls. A key record that cannot be kept leaves the
/// device on its old key, and the next round tries again.
pub(crate) async fn take_new_key(db: &Weak<Mutex<Inner>>) {
    let Some(db) = db.upgrade() else {
        return;
    };
    let found = {
        let inner = lock(&db);
        match new_key(&inner) {
            Ok(Some((statement, key))) if newer(&inner.account, &statement) => {
                let keys = Keys {
                    member: inner.member.clone(),
                    account: keys::Account::Holds {
                        key,
                        statement: statement.clone(),
                    },
                    code: inner.code.clone(),
                    upgraded: false,
                };
                Some((keys, inner.home.clone(), statement))
            }
            _ => None,
        }
    };
    if let Some((keys, home, statement)) = found {
        let _ = install(&db, keys, home, &statement).await;
    }
}
