//! What syncing changes in the database, under its lock: the spaces it
//! syncs, the relay's epoch, the entries it pulled and the results of a
//! push (RFC 0007, "Island mode"; RFC 0002, "A change the relay
//! refuses after the member folded it").

use std::collections::BTreeMap;

use ncdb_core::change::Change;
use ncdb_core::ids::{ChangeId, SpaceId};
use ncdb_core::log::Appended;
use ncdb_core::view::Error as ViewError;

use super::wire::{self, Entry, Pushed};
use super::{SyncState, again};
use crate::db::{Inner, SpaceState};
use crate::subscribe::touched;

/// The codes after which the same push may succeed later: the relay
/// appended nothing, and the change stays unconfirmed (RFC 0007, "The
/// errors"). Every other code is a refusal of the change.
const TRANSIENT: [&str; 2] = ["unavailable", "rate_limited"];

/// The relay's answer to a finalize that came before its 7 days passed
/// (RFC 0007, "The errors").
pub(super) const TOO_EARLY: &str = "too_early";

fn sync(inner: &mut Inner) -> &mut SyncState {
    inner.sync.as_mut().expect("a database that syncs")
}

/// The spaces the database syncs: those it created and those the relay
/// said it belongs to, except those it may no longer read.
pub(super) fn spaces(inner: &Inner) -> Vec<SpaceId> {
    let stopped = inner.sync.as_ref().map(|s| &s.stopped);
    inner
        .spaces
        .keys()
        .copied()
        .filter(|space| stopped.is_none_or(|s| !s.contains(space)))
        .collect()
}

/// Reads back what the database kept of its relay: the relay epoch its
/// relay positions belong to, and each space's cursor, derived from those
/// positions (NOC-145). A database that kept no epoch starts with none,
/// and its first `welcome` forgets every position.
pub(super) fn load(inner: &mut Inner) -> Result<(), ViewError> {
    let epoch = inner.view.relay_epoch()?;
    let spaces: Vec<SpaceId> = inner.view.spaces().collect();
    let mut cursors = BTreeMap::new();
    if epoch.is_some() {
        for space in spaces {
            cursors.insert(space, inner.view.relay_cursor(space)?);
        }
    }
    let sync = sync(inner);
    sync.epoch = epoch;
    sync.cursors = cursors;
    Ok(())
}

/// What `welcome` says: the relay's epoch, and the spaces this member
/// belongs to. An epoch other than the one the relay positions belong to
/// makes every relay position unconfirmed and every cursor 0 (RFC 0007,
/// "Restores and the relay epoch"), and is kept with the database, so a
/// reopened database starts from its cursors. A database that knew no
/// epoch yet, such as one kept before NOC-145, forgets its positions too,
/// without reporting a new epoch: pulling from 0 confirms again every
/// change the relay still holds, and the rest is pushed again. A space the
/// database does not hold yet is joined: its changes fold as they arrive,
/// and its keys come with them.
pub(super) fn welcome(
    inner: &mut Inner,
    epoch: [u8; 16],
    spaces: &[SpaceId],
) -> Result<(), ViewError> {
    let known = sync(inner).epoch;
    if known != Some(epoch) {
        inner.view.forget_relay_positions()?;
        inner.view.set_relay_epoch(epoch)?;
        let sync = sync(inner);
        sync.cursors.clear();
        sync.heads.clear();
        sync.pulled.clear();
        sync.epoch_changed = known.is_some();
    }
    sync(inner).epoch = Some(epoch);
    for space in spaces {
        if !inner.spaces.contains_key(space) {
            let state = SpaceState::new(*space, inner.member.key());
            inner.spaces.insert(*space, state);
        }
    }
    Ok(())
}

/// What `added` says: this member was added to `space` while connected
/// (RFC 0007, "Subscribe"). A space the database does not hold yet is
/// joined, as from `welcome`; a space the relay had stopped is synced
/// again, and once no space stays stopped the sync status no longer
/// reports the removal. Returns whether the session must pull, push and
/// subscribe to the space; a space it already follows needs nothing.
pub(super) fn added(inner: &mut Inner, space: SpaceId) -> bool {
    let joined = !inner.spaces.contains_key(&space);
    if joined {
        let state = SpaceState::new(space, inner.member.key());
        inner.spaces.insert(space, state);
    }
    let sync = sync(inner);
    let restarted = sync.stopped.remove(&space);
    if sync.stopped.is_empty() {
        sync.removed = None;
    }
    joined || restarted
}

/// Records the head of `space` the relay told: the highest relay position
/// the member heard of. Once the cursor reaches it, the space's initial
/// sync is complete (RFC 0009, "Sync and status").
pub(super) fn heard(inner: &mut Inner, space: SpaceId, head: u64) {
    let cursor = cursor(inner, space);
    let sync = sync(inner);
    let known = sync.heads.entry(space).or_insert(0);
    *known = (*known).max(head);
    if cursor >= *known {
        sync.pulled.insert(space);
    }
}

/// The cursor of `space`.
pub(super) fn cursor(inner: &Inner, space: SpaceId) -> u64 {
    let sync = inner.sync.as_ref().expect("a database that syncs");
    sync.cursors.get(&space).copied().unwrap_or(0)
}

/// Folds entries of `space` in position order, from its cursor on, and
/// records each one's relay position. Returns whether the entries left a
/// gap after the cursor, so that the member pulls the missing range.
pub(super) fn entries(
    inner: &mut Inner,
    space: SpaceId,
    entries: Vec<Entry>,
) -> Result<bool, ViewError> {
    let mut cursor = cursor(inner, space);
    let mut folded = false;
    let mut gap = false;
    for entry in entries {
        if entry.position <= cursor {
            continue;
        }
        if entry.position != cursor + 1 {
            gap = true;
            break;
        }
        let Inner {
            view,
            spaces,
            member,
            ..
        } = &mut *inner;
        let state = spaces
            .entry(space)
            .or_insert_with(|| SpaceState::new(space, member.key()));
        match view.receive(&entry.frame, &mut state.checks) {
            Ok(Appended::New(_)) => {
                folded = true;
                state.drop_shredded();
                crate::personal::refresh(view, space, state)?;
                crate::status::note_shreds(view, space, state.checks.resolved())?;
                sync(inner).folds += 1;
                if again::check(inner, space)? {
                    sync(inner).wake.send_replace(());
                }
            }
            Ok(Appended::Duplicate(_)) | Err(ViewError::Receive(_)) => {}
            Err(error) => return Err(error),
        }
        if let Ok(change) = Change::decode(&entry.frame) {
            inner.view.confirm(space, change.id(), entry.position)?;
        }
        cursor = entry.position;
    }
    sync(inner).cursors.insert(space, cursor);
    if folded {
        // The acknowledgements are written again by the next fold if this
        // fails, since the log then holds none of them.
        let _ = super::acknowledge::acknowledge(inner, space);
        let collections = inner.model.collections.keys().cloned().collect();
        touched(&mut inner.watchers, space, &collections);
    }
    Ok(gap)
}

/// The changes of `space` the relay has neither confirmed nor refused, in
/// local order: this member's own and those it received from others. A
/// refused change the fold newly accepted since, after a control change
/// the member wrote itself, is among them (RFC 0008, "How the relay applies the rules").
pub(super) fn unconfirmed(
    inner: &mut Inner,
    space: SpaceId,
) -> Result<Vec<(ChangeId, Vec<u8>)>, ViewError> {
    again::check(inner, space)?;
    Ok(inner
        .view
        .unconfirmed(space)?
        .into_iter()
        .map(|change| (change.id(), change.bytes().to_vec()))
        .collect())
}

/// Records the results of a push of the changes `ids` of `space`. A
/// refusal is recorded by the code [`wire::mapped_code`] maps it to, as the
/// sync status shows it, so the relay's own text never reaches the log
/// (NOC-188). Returns the transient code that stopped it, after which the
/// rest stays unconfirmed. A `too_early`, the relay's answer to a finalize
/// that came before its 7 days passed, is no refusal: the finalize, and
/// every change after it, stay unconfirmed, since those after it may rest
/// on it and the relay would refuse them; the space is pushed again later
/// (RFC 0007, "The errors"; pm, NOC-234). It is returned like a transient
/// code, but it ends only this space's push.
pub(super) fn pushed(
    inner: &mut Inner,
    space: SpaceId,
    ids: &[ChangeId],
    results: Vec<Pushed>,
) -> Result<Option<String>, ViewError> {
    for (id, result) in ids.iter().zip(results) {
        match result {
            Pushed::Accepted(position) | Pushed::Duplicate(position) => {
                inner.view.confirm(space, *id, position)?;
            }
            Pushed::Rejected(code) => {
                let code = wire::mapped_code(&code);
                if TRANSIENT.contains(&code.as_str()) {
                    return Ok(Some(code));
                }
                if code == TOO_EARLY {
                    let sync = sync(inner);
                    let at = super::time::Instant::now() + sync.too_early_retry;
                    sync.retries.entry(space).or_insert(at);
                    return Ok(Some(code));
                }
                inner.view.refuse(space, *id, &code)?;
                again::refused(inner, space, *id);
            }
        }
    }
    Ok(None)
}

/// Stops syncing `space`: the relay said, with `code`, that this member
/// may not read it. The member keeps its copy (RFC 0007, `removed_member`),
/// and the sync status reports the code. A space that was not stopped
/// before has its unconfirmed changes pushed once more ([`flushing`]).
pub(super) fn stop(inner: &mut Inner, space: SpaceId, code: String) {
    let sync = sync(inner);
    if sync.stopped.insert(space) {
        sync.flush.insert(space);
    }
    sync.live.remove(&space);
    sync.removed = Some(code);
}

/// The stopped spaces whose unconfirmed changes wait to be pushed once
/// more: a change the relay appended though its answer was lost, as a
/// leave can be, is confirmed then as a duplicate, and the others are
/// refused, so none waits for a confirmation that cannot come (NOC-276).
pub(super) fn flushing(inner: &Inner) -> Vec<SpaceId> {
    inner
        .sync
        .as_ref()
        .map(|sync| sync.flush.iter().copied().collect())
        .unwrap_or_default()
}

/// Records that the push of a stopped space's unconfirmed changes is done.
pub(super) fn flushed(inner: &mut Inner, space: SpaceId) {
    sync(inner).flush.remove(&space);
}

/// Records that the subscription to `space` caught up with the relay's
/// head.
pub(super) fn caught_up(inner: &mut Inner, space: SpaceId) {
    sync(inner).live.insert(space);
}

/// This member's changes the relay has not confirmed, and those it
/// refused, across every space the database holds. The view keeps these
/// counts as changes are written, received, confirmed and refused, and
/// reads them back when it opens (NOC-147), so this decodes nothing.
pub(super) fn own_counts(inner: &Inner) -> (u64, u64) {
    let me = inner.member.key().id();
    inner
        .spaces
        .keys()
        .fold((0, 0), |(unconfirmed, refused), space| {
            let counts = inner.view.relay_counts(*space, me);
            (unconfirmed + counts.unconfirmed, refused + counts.refused)
        })
}
