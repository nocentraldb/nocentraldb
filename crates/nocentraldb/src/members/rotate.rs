//! A rotation of a space's key (RFC 0003, "Rotating a space key"; RFC
//! 0008, "Key chain entry"): a key chain entry for a fresh generation,
//! which encrypts the previous generations' keys under the new one, then a
//! key envelope of the new key for every member of the space, the author
//! included. Once they are folded, the new generation is current and the
//! author holds its key, so what it writes next is sealed under it.
//!
//! The key chain entry lists every head of the generation graph as a
//! previous generation, as a merge rotation does (RFC 0008, "Two removals
//! that rotate the key at once", step 5), so the new generation descends
//! from all of them and is the one head after it.
//!
//! **What this does not do yet.** It does not re-wrap person-key records (RFC 0003,
//! step 3): a space that holds a person key is refused before anything
//! is written, since the SDK holds no record to re-wrap.

use std::collections::BTreeSet;

use ncdb_core::envelope;
use ncdb_core::ids::{ChangeId, SpaceId};
use ncdb_core::keys::{MemberKey, SpaceKey};
use ncdb_core::membership::{
    self, ControlBody, Entry, KeyChainEntry, PreviousKey, seal_previous_key,
};
use ncdb_core::view::View;

use crate::db::SpaceState;
use crate::error::{Error, ErrorCode};
use crate::next;
use crate::space::fold;

use super::control_heads;

/// Whether `state`'s space can be rotated by this SDK, checked before
/// anything is written: the member holds the key of every head of the
/// generation graph, which the rotation chains, and the space holds no
/// person key, whose records a rotation re-wraps (RFC 0003, step 3), which
/// is not built.
pub(crate) fn rotatable(state: &SpaceState) -> Result<(), Error> {
    let resolved = state.checks.resolved();
    if resolved
        .heads()
        .iter()
        .any(|head| state.checks.space_key(*head).is_none())
    {
        return Err(Error::new(
            ErrorCode::InvalidValue,
            "the member holds no key of a generation the rotation must chain; \
             it rotates once an envelope for it arrives",
        ));
    }
    match resolved.person_keys().next() {
        None => Ok(()),
        Some(_) => Err(Error::new(
            ErrorCode::InvalidValue,
            "the space holds a person key, and re-wrapping person-key records \
             in a rotation is not built",
        )),
    }
}

/// Rotates the key of `space`, which `member`, an admin, holds with the
/// key of its current generation: writes the key chain entry, answering
/// `answers`, and one envelope per member of the space, and folds them.
/// Returns the new generation.
pub(crate) fn rotate(
    view: &mut View,
    state: &mut SpaceState,
    member: &MemberKey,
    space: SpaceId,
    answers: BTreeSet<ChangeId>,
) -> Result<u64, Error> {
    let next = next::next(view, space, &state.checks, member.id());
    let current = next.generation.expect("a writable space has a generation");
    let heads: Vec<u64> = state.checks.resolved().heads().into_iter().collect();
    let fresh = membership::generation_id(space, member.id(), &heads);
    let key = SpaceKey::generate();
    let previous = heads
        .iter()
        .map(|head| {
            let held = state
                .checks
                .space_key(*head)
                .expect("the key of every head is held, as `rotatable` checks");
            PreviousKey {
                generation: *head,
                key: seal_previous_key(&key, space, fresh, *head, held),
            }
        })
        .collect();
    let chain = KeyChainEntry {
        generation: fresh,
        previous,
        answers,
    };
    let rotation = membership::write(
        member,
        space,
        next.sequence,
        current,
        &ControlBody::new(control_heads(view, space), Entry::KeyChain(chain)),
    );
    fold(view, &mut state.checks, &rotation)?;
    let recipients: Vec<_> = state
        .checks
        .resolved()
        .members()
        .filter_map(|(id, _)| state.checks.resolved().member(id).map(|m| m.key))
        .collect();
    for (offset, recipient) in (1..).zip(recipients) {
        let sealed = envelope::seal(&key, space, fresh, &recipient)
            .expect("a member's public key takes an envelope");
        let sent = membership::write(
            member,
            space,
            next.sequence + offset,
            current,
            &ControlBody::new(BTreeSet::from([rotation.id()]), Entry::KeyEnvelope(sealed)),
        );
        fold(view, &mut state.checks, &sent)?;
    }
    Ok(fresh)
}
