//! The erasure acknowledgements (RFC 0008, "Acknowledgement"; NOC-238):
//! a member that accepts a shred writes two acknowledgements for it,
//! "seen" and then "deleted". A shred is final when accepted, so the
//! member drops its copy of the person key at once (NOC-229) and the
//! "deleted" acknowledgement follows straight after "seen". A provisional
//! shred keeps the key (NOC-234): it gets "seen" at once, and "deleted"
//! only once a decision finalizes it.
//!
//! After the receive path folds new changes of a space, [`acknowledge`]
//! writes what the member owes and has not written in its own log yet, in
//! one batch, each at the member's next author sequence (RFC 0002) and
//! after the control heads, so the shred is in their causal past. Then it
//! wakes syncing, which pushes them; offline they wait in the log like any
//! own change. Every member writes them, a reader too, since every role
//! acknowledges a shred (RFC 0008, "Roles and rights"); a member that is
//! no longer a member writes none.

use std::collections::{BTreeMap, BTreeSet};

use ncdb_core::cbor::Value;
use ncdb_core::change::Kind;
use ncdb_core::ids::{ChangeId, SpaceId};
use ncdb_core::keys::MemberKey;
use ncdb_core::log::Position;
use ncdb_core::membership::{ACKNOWLEDGEMENT, ControlBody, Entry};
use ncdb_core::view::View;

use crate::db::Inner;
use crate::error::Error;
use crate::members::batch::Batch;

/// Writes what the member owes for the shreds of `space`: "seen" for
/// every accepted shred it has not acknowledged, a provisional one too, and
/// "deleted" for every shred whose key is destroyed (a shred, or a
/// provisional one that was finalized) and that it has not acknowledged as
/// deleted. A provisional shred keeps its key, so it gets "seen" alone until
/// the decision that finalizes it; one taken back gets nothing more (RFC
/// 0008, "A shred is final when accepted"). Wakes syncing when it wrote
/// any. A member that is not a member of the space, or holds no key to
/// write with yet, writes none.
pub(crate) fn acknowledge(inner: &mut Inner, space: SpaceId) -> Result<(), Error> {
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
    let (seen, deleted) = acknowledged(view, space, me);
    let owed: Vec<(ChangeId, bool, bool)> = resolved
        .person_keys()
        .filter_map(|(_, key)| key.shred.map(|shred| (shred, key.destroyed())))
        .filter_map(|(shred, destroyed)| {
            let see = !seen.contains(&shred);
            let delete = destroyed && !deleted.contains(&shred);
            (see || delete).then_some((shred, see, delete))
        })
        .collect();
    if owed.is_empty() {
        return Ok(());
    }
    let next = crate::next::next(view, space, &state.checks, me);
    if next.generation.is_none() {
        return Ok(());
    }
    state.drop_shredded();
    let key = secrets.key();
    let mut batch = Batch::new(view, state, &key, space);
    for (shred, see, delete) in owed {
        write(&mut batch, &key, shred, see, delete);
    }
    batch.commit(view, state, secrets)?;
    crate::sync::wake(inner);
    Ok(())
}

/// Writes into `batch` the "seen" acknowledgement of `shred` when `seen`,
/// and the "deleted" one after it when `deleted`, and moves the batch's
/// heads past them. Returns the last acknowledgement written, or `None`.
pub(crate) fn write(
    batch: &mut Batch,
    key: &MemberKey,
    shred: ChangeId,
    seen: bool,
    deleted: bool,
) -> Option<ChangeId> {
    let mut last = None;
    if seen {
        last = Some(batch.write(key, entry(shred, false), None));
    }
    if deleted {
        let after = last.map(|id| BTreeSet::from([id]));
        last = Some(batch.write(key, entry(shred, true), after));
    }
    if let Some(id) = last {
        batch.heads = BTreeSet::from([id]);
    }
    last
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

/// The shreds `member` acknowledged as seen, and as deleted, in its own
/// changes of `space` in `view`'s log.
fn acknowledged(
    view: &View,
    space: SpaceId,
    member: ncdb_core::ids::MemberId,
) -> (BTreeSet<ChangeId>, BTreeSet<ChangeId>) {
    let (mut seen, mut deleted) = (BTreeSet::new(), BTreeSet::new());
    let Some(log) = view.log(space) else {
        return (seen, deleted);
    };
    let own = log
        .after(Position::START)
        .map(|(_, change)| change)
        .filter(|change| change.header().author == member && change.header().kind == Kind::CONTROL);
    for change in own {
        let Ok(body) = ControlBody::decode(&change.header().payload) else {
            continue;
        };
        let Entry::Other { entry_type, fields } = body.entry else {
            continue;
        };
        if entry_type != ACKNOWLEDGEMENT {
            continue;
        }
        let (Some(Value::Bytes(id)), Some(Value::Bool(kind))) = (fields.get(&8), fields.get(&9))
        else {
            continue;
        };
        let Ok(id) = <[u8; 32]>::try_from(id.as_slice()) else {
            continue;
        };
        match kind {
            true => deleted.insert(ChangeId(id)),
            false => seen.insert(ChangeId(id)),
        };
    }
    (seen, deleted)
}
