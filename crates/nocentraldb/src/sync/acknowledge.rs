//! The erasure acknowledgements (RFC 0008, "Acknowledgement"; NOC-238):
//! a member that accepts a shred writes two acknowledgements for it,
//! "seen" and then "deleted". A shred is final when accepted, so the
//! member drops its copy of the person key at once (NOC-229) and the
//! "deleted" acknowledgement follows straight after "seen".
//!
//! After the receive path folds new changes of a space, [`acknowledge`]
//! writes both for every accepted shred the member has not acknowledged in
//! its own log yet, in one batch, each at the member's next author
//! sequence (RFC 0002) and after the control heads, so the shred is in
//! their causal past. Then it wakes syncing, which pushes them; offline
//! they wait in the log like any own change. Every member writes them, a
//! reader too, since every role acknowledges a shred (RFC 0008, "Roles
//! and rights"); a member that is no longer a member writes none.

use std::collections::{BTreeMap, BTreeSet};

use ncdb_core::cbor::Value;
use ncdb_core::change::Kind;
use ncdb_core::ids::{ChangeId, SpaceId};
use ncdb_core::log::Position;
use ncdb_core::membership::{ACKNOWLEDGEMENT, ControlBody, Entry};
use ncdb_core::view::View;

use crate::db::Inner;
use crate::error::Error;
use crate::members::batch::Batch;

/// Writes "seen" and "deleted" for every shred of `space` the member
/// accepted and has not acknowledged yet, and wakes syncing when it wrote
/// any. A member that is not a member of the space, or holds no key to
/// write with yet, writes none.
pub(super) fn acknowledge(inner: &mut Inner, space: SpaceId) -> Result<(), Error> {
    let me = inner.member.key().id();
    let Inner {
        view,
        spaces,
        member: secrets,
        ..
    } = &mut *inner;
    let Some(state) = spaces.get_mut(&space) else {
        return Ok(());
    };
    let resolved = state.checks.resolved();
    if resolved.role(me).is_none() {
        return Ok(());
    }
    let shredded: BTreeSet<ChangeId> = resolved
        .person_keys()
        .filter_map(|(_, key)| key.shred)
        .collect();
    let done = acknowledged(view, space, me);
    let missing: Vec<ChangeId> = shredded
        .into_iter()
        .filter(|shred| !done.contains(shred))
        .collect();
    if missing.is_empty() {
        return Ok(());
    }
    let next = crate::next::next(view, space, &state.checks, me);
    if next.generation.is_none() {
        return Ok(());
    }
    state.drop_shredded();
    let key = secrets.key();
    let mut batch = Batch::new(view, state, &key, space);
    for shred in missing {
        let seen = batch.write(&key, entry(shred, false), None);
        let deleted = batch.write(&key, entry(shred, true), Some(BTreeSet::from([seen])));
        batch.heads = BTreeSet::from([deleted]);
    }
    batch.commit(view, state, secrets)?;
    crate::sync::wake(inner);
    Ok(())
}

/// An acknowledgement of `shred`: field 8 its change id, field 9 whether
/// the member deleted its copy of the key.
fn entry(shred: ChangeId, deleted: bool) -> Entry {
    Entry::Other {
        entry_type: ACKNOWLEDGEMENT,
        fields: BTreeMap::from([
            (8, Value::Bytes(shred.0.to_vec())),
            (9, Value::Bool(deleted)),
        ]),
    }
}

/// The shreds `member` acknowledged as deleted in its own changes of
/// `space` in `view`'s log.
fn acknowledged(
    view: &View,
    space: SpaceId,
    member: ncdb_core::ids::MemberId,
) -> BTreeSet<ChangeId> {
    let Some(log) = view.log(space) else {
        return BTreeSet::new();
    };
    log.after(Position::START)
        .map(|(_, change)| change)
        .filter(|change| change.header().author == member && change.header().kind == Kind::CONTROL)
        .filter_map(
            |change| match ControlBody::decode(&change.header().payload).ok()?.entry {
                Entry::Other { entry_type, fields } if entry_type == ACKNOWLEDGEMENT => {
                    match (fields.get(&8), fields.get(&9)) {
                        (Some(Value::Bytes(id)), Some(Value::Bool(true))) => {
                            Some(ChangeId(id.as_slice().try_into().ok()?))
                        }
                        _ => None,
                    }
                }
                _ => None,
            },
        )
        .collect()
}
