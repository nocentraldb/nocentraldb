//! The changes of one membership call, written first and folded together
//! in one transaction (NOC-204; `View::receive_all`): a rotation, its
//! envelopes and the add that follows reach the log whole or not at all,
//! so neither an error nor a crash in the middle leaves an admin with a
//! rotation it holds no key of, or a rotation without its add.
//!
//! A rotation also re-wraps the person keys the member holds (NOC-206 part
//! 3): the batch collects their signed records, and [`Batch::commit`]
//! returns them once their entries are folded, for the caller to hand to
//! syncing.

use std::collections::BTreeSet;

use ncdb_core::change::Change;
use ncdb_core::ids::{ChangeId, SpaceId};
use ncdb_core::keys::{MemberKey, SpaceKey};
use ncdb_core::log::State;
use ncdb_core::membership::{self, ControlBody, Entry};
use ncdb_core::view::View;

use crate::db::SpaceState;
use crate::error::{Error, storage};
use crate::keys::Secrets;
use crate::next;
use crate::sync::Record;

use super::control_heads;

/// The changes a membership call writes into one space, in order, each
/// with the author's next sequence.
pub(crate) struct Batch {
    space: SpaceId,
    sequence: u64,
    /// The generation the next change names in its header.
    pub(crate) generation: u64,
    /// The key of a generation this batch rotated to, which the space's
    /// checks learn only once the batch is folded.
    pub(crate) fresh: Option<SpaceKey>,
    /// What the next change depends on, unless it names its dependencies.
    pub(crate) heads: BTreeSet<ChangeId>,
    /// The person-key records the batch's person-key entries announce.
    pub(crate) records: Vec<Record>,
    changes: Vec<Change>,
}

impl Batch {
    /// A batch that starts after what `view` holds of `space`, written with
    /// the space's current generation.
    pub(crate) fn new(
        view: &mut View,
        state: &SpaceState,
        member: &MemberKey,
        space: SpaceId,
    ) -> Batch {
        let next = next::next(view, space, &state.checks, member.id());
        Batch {
            space,
            sequence: next.sequence,
            generation: next.generation.expect("a writable space has a generation"),
            fresh: None,
            heads: control_heads(view, space),
            records: Vec::new(),
            changes: Vec::new(),
        }
    }

    /// The key of the generation the batch writes with: the one it rotated
    /// to, or the one `state` holds.
    pub(crate) fn key<'a>(&'a self, state: &'a SpaceState) -> &'a SpaceKey {
        match &self.fresh {
            Some(key) => key,
            None => state
                .checks
                .space_key(self.generation)
                .expect("a writable space's key is held"),
        }
    }

    /// Writes `entry` after `dependencies`, or after the batch's heads.
    pub(crate) fn write(
        &mut self,
        member: &MemberKey,
        entry: Entry,
        dependencies: Option<BTreeSet<ChangeId>>,
    ) -> ChangeId {
        let dependencies = dependencies.unwrap_or_else(|| self.heads.clone());
        let change = membership::write(
            member,
            self.space,
            self.sequence,
            self.generation,
            &ControlBody::new(dependencies, entry),
        );
        self.sequence += 1;
        let id = change.id();
        self.changes.push(change);
        id
    }

    /// Folds the batch in one transaction. Returns the person-key records
    /// its entries announce, which the caller hands to syncing to put.
    pub(crate) fn commit(
        self,
        view: &mut View,
        state: &mut SpaceState,
        secrets: &Secrets,
    ) -> Result<Vec<Record>, Error> {
        let frames: Vec<&[u8]> = self.changes.iter().map(Change::bytes).collect();
        commit(view, state, secrets, self.space, &frames)?;
        Ok(self.records)
    }
}

/// Receives the member's own `frames` into `space` in one transaction, and
/// checks that each was folded. When the batch is not received, the log
/// holds none of it, and the space's checks, which learned its control
/// changes as they were received, are read again from the log.
pub(crate) fn commit(
    view: &mut View,
    state: &mut SpaceState,
    secrets: &Secrets,
    space: SpaceId,
    frames: &[&[u8]],
) -> Result<(), Error> {
    if let Err(error) = view.receive_all(frames, &mut state.checks) {
        let held = std::mem::take(&mut state.person_keys);
        let bad = std::mem::take(&mut state.bad_person_key_records);
        *state = SpaceState::from_log(view, space, secrets.key());
        state.person_keys = held;
        state.bad_person_key_records = bad;
        return Err(storage(error));
    }
    for frame in frames {
        let id = Change::decode(frame).expect("received, so decoded").id();
        match view.state(space, id) {
            Some(State::Folded { parked }) if parked.is_empty() => {}
            other => panic!("the member's own change was not folded: {other:?}"),
        }
    }
    Ok(())
}
