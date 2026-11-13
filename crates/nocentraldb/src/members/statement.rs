//! Carrying a newer account key statement into the member's spaces (RFC
//! 0008, "Account key statement entry"; Carsten, 2026-11-11, NOC-187):
//! once the SDK learns a newer statement for its account, it writes an
//! account key statement entry into every space where it is a member and
//! a certificate names the statement's recovery key, once per space and
//! number. The fold of each space then refuses adds and sign-offs under
//! the superseded account key.
//!
//! The SDK learns a newer statement when a device of the account removes a
//! lost member and renews the account key, and when another device takes
//! that key from the account space (RFC 0010, "Losing a device"; NOC-268).

use ncdb_core::ids::MemberId;
use ncdb_core::membership::{AccountKeyStatement, Entry, Resolved};

use super::batch::Batch;
use crate::db::{Db, Inner, lock};
use crate::error::{Error, ErrorCode};
use crate::id::SpaceId;
use crate::space::{not_stopped, writable_or_account};

impl Db {
    /// Writes `statement` as an account key statement entry into every
    /// space this database may write into where it is a member, a current
    /// member's certificate names the statement's recovery key, and no
    /// statement for that recovery key with the same or a higher number is
    /// in the space yet: the app's spaces and the account space. Returns
    /// the spaces written into, in the order of their ids. A statement
    /// whose signature does not verify is refused with `InvalidValue`, and
    /// nothing is written.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) async fn carry_statement(
        &self,
        statement: &AccountKeyStatement,
    ) -> Result<Vec<SpaceId>, Error> {
        carry(&mut lock(&self.inner), statement)
    }
}

/// [`Db::carry_statement`] under the database's lock.
pub(crate) fn carry(
    inner: &mut Inner,
    statement: &AccountKeyStatement,
) -> Result<Vec<SpaceId>, Error> {
    if !statement.verify() {
        return Err(Error::new(
            ErrorCode::InvalidValue,
            "the account key statement does not verify",
        ));
    }
    let me = inner.member.key().id();
    let held: Vec<_> = inner.spaces.keys().copied().collect();
    let mut written = Vec::new();
    for id in held {
        let space = SpaceId(id);
        if writable_or_account(inner, space).is_err()
            || not_stopped(inner, space).is_err()
            || !wanted(inner.spaces[&id].checks.resolved(), me, statement)
        {
            continue;
        }
        let member = inner.member.key();
        let Inner {
            view,
            spaces,
            member: secrets,
            ..
        } = &mut *inner;
        let state = spaces.get_mut(&id).expect("a held space");
        let mut batch = Batch::new(view, state, &member, id);
        batch.write(&member, Entry::AccountKeyStatement(statement.clone()), None);
        batch.commit(view, state, secrets)?;
        written.push(space);
    }
    if !written.is_empty() {
        crate::sync::wake(inner);
    }
    Ok(written)
}

/// Whether `me` writes `statement` into the space of `resolved`: it is a
/// member, a current member's certificate names the statement's recovery
/// key, and every statement for that recovery key there has a lower
/// number, so the entry is valid when written and new.
fn wanted(resolved: &Resolved, me: MemberId, statement: &AccountKeyStatement) -> bool {
    let recovery = statement.recovery_key;
    let named = resolved.members().any(|(member, _)| {
        resolved
            .member(member)
            .and_then(|state| state.certificate.statement.as_deref())
            .is_some_and(|s| s.recovery_key == recovery)
    });
    resolved.role(me).is_some()
        && named
        && resolved
            .all_statements()
            .filter(|s| s.recovery_key == recovery)
            .all(|s| s.number < statement.number)
}
