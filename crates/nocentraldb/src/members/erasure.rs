//! The finalize an admin's SDK writes on its own (RFC 0008, "A provisional
//! erasure in a space with one account"; Carsten, 2026-11-11, NOC-230): once
//! this device has held a provisional shred for 7 days by its own clock,
//! an admin of the space that belongs to the performer's account writes an
//! erasure decision that finalizes it. When to write it, and the push again
//! after a `too_early`, are syncing's (`sync::finalize`). It writes a
//! take-back only when an admin asks for it, through
//! `space.erasures().take_back` ([`take_back`]), and a shred only for
//! `Db::erase` ([`shred`], NOC-325).

use ncdb_core::ids::{ChangeId, SpaceId};
use ncdb_core::membership::{Entry, ErasureDecision, Role, ShredEntry, SignOff};

use crate::db::{Inner, SpaceState};
use crate::error::Error;
use crate::space::{not_stopped, writable};

use super::change::write;

/// The provisional shreds of `state`'s space that this member may
/// finalize, with when this device first held each: it is an admin of the
/// space and belongs to the performer's account, and no accepted decision
/// ended the shred.
pub(crate) fn finalizable(
    inner: &Inner,
    space: SpaceId,
    state: &SpaceState,
) -> Vec<(ChangeId, Option<u64>)> {
    let resolved = state.checks.resolved();
    let me = inner.member.key().id();
    if resolved.role(me) != Some(Role::Admin) {
        return Vec::new();
    }
    crate::status::pending(resolved)
        .filter(|(_, performer)| resolved.same_account(me, *performer))
        .map(|(shred, _)| (shred, inner.view.shred_held_at(space, shred).ok().flatten()))
        .collect()
}

/// Writes the erasure decision that finalizes `shred` in `space`, if this
/// member may still finalize it. Whether it wrote one.
pub(crate) fn finalize(inner: &mut Inner, space: SpaceId, shred: ChangeId) -> Result<bool, Error> {
    let id = crate::id::SpaceId(space);
    writable(inner, id)?;
    not_stopped(inner, id)?;
    let state = &inner.spaces[&space];
    if !finalizable(inner, space, state)
        .iter()
        .any(|(pending, _)| *pending == shred)
    {
        return Ok(false);
    }
    let me = inner.member.key().id();
    write(inner, space, me, |batch, _, key, _| {
        let entry = Entry::ErasureDecision(ErasureDecision {
            shred,
            finalize: true,
        });
        batch.write(key, entry, None);
    })?;
    Ok(true)
}

/// Writes the shreds of `signed`, each a person key of `space` with its
/// sign-off, in one batch (NOC-325): the caller checked that the member
/// may write there and that each sign-off holds. Their change ids, in
/// the order of `signed`.
pub(crate) fn shred(
    inner: &mut Inner,
    space: SpaceId,
    signed: Vec<([u8; 16], SignOff)>,
) -> Result<Vec<ChangeId>, Error> {
    let me = inner.member.key().id();
    let mut ids = Vec::new();
    write(inner, space, me, |batch, _, key, _| {
        for (person_key, sign_off) in signed {
            let entry = Entry::Shred(ShredEntry {
                person_key,
                sign_off,
            });
            ids.push(batch.write(key, entry, None));
        }
    })?;
    Ok(ids)
}

/// The notice of a take-back (Carsten, 2026-11-15, on NOC-326, Q7 a; the
/// text: pm, NOC-326, on the coding agent's proposal, in texts/texts.json).
pub(crate) const TAKEN_BACK: crate::members::Notice = crate::members::Notice {
    code: crate::members::NoticeCode::TakenBack,
    text: crate::texts::notice::TAKEN_BACK,
};

/// Takes back the provisional erasure `shred` of `space` (RFC 0008, "A
/// provisional erasure in a space with one account"; Carsten, 2026-11-15,
/// on NOC-326, Q7 a): writes the erasure decision whose key 9 is false.
/// A shred that is not a provisional one still pending is refused with
/// `InvalidValue`; a member that is not an admin of the performer's
/// account with `NotAllowed`. Nothing is written when a call is refused.
pub(crate) fn take_back(
    inner: &mut Inner,
    space: SpaceId,
    shred: ChangeId,
) -> Result<crate::members::Notice, Error> {
    let id = crate::id::SpaceId(space);
    writable(inner, id)?;
    not_stopped(inner, id)?;
    let resolved = inner.spaces[&space].checks.resolved();
    let me = inner.member.key().id();
    let pending = resolved.person_keys().map(|(_, key)| key).find(|key| {
        key.shred == Some(shred)
            && key.provisional
            && key.taken_back.is_none()
            && key.finalized.is_none()
    });
    let Some(performer) = pending.map(|key| key.performer) else {
        return Err(Error::new(
            crate::error::ErrorCode::InvalidValue,
            crate::texts::error::TAKE_BACK_NOT_PENDING,
        ));
    };
    let may = resolved.role(me) == Some(Role::Admin)
        && performer.is_some_and(|performer| resolved.same_account(me, performer));
    if !may {
        return Err(Error::new(
            crate::error::ErrorCode::NotAllowed,
            crate::texts::error::TAKE_BACK_NOT_ALLOWED,
        ));
    }
    write(inner, space, me, |batch, _, key, _| {
        let entry = Entry::ErasureDecision(ErasureDecision {
            shred,
            finalize: false,
        });
        batch.write(key, entry, None);
    })?;
    Ok(TAKEN_BACK)
}
