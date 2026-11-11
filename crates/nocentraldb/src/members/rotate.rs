//! A rotation of a space's key (RFC 0003, "Rotating a space key"; RFC
//! 0008, "Key chain entry"): a key chain entry for a fresh generation,
//! which encrypts the current generation's key under the new one, then a
//! key envelope of the new key for every member of the space, the author
//! included. Once they are folded, the new generation is current and the
//! author holds its key, so what it writes next is sealed under it.
//!
//! **What this does not do yet.** It rotates away from the current
//! generation only, not from every head of the generation graph, as a
//! merge rotation does (RFC 0008, "Two removals that rotate the key at
//! once", step 5). It does not re-wrap person-key records (RFC 0003,
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

/// Whether `state`'s space can be rotated by this SDK: it holds no person
/// key, whose records a rotation re-wraps (RFC 0003, step 3), which is not
/// built. Checked before anything is written.
pub(crate) fn rotatable(state: &SpaceState) -> Result<(), Error> {
    match state.checks.resolved().person_keys().next() {
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
    let fresh = membership::generation_id(space, member.id(), &[current]);
    let key = SpaceKey::generate();
    let previous = state
        .checks
        .space_key(current)
        .expect("a writable space's key is held");
    let chain = KeyChainEntry {
        generation: fresh,
        previous: vec![PreviousKey {
            generation: current,
            key: seal_previous_key(&key, space, fresh, current, previous),
        }],
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
