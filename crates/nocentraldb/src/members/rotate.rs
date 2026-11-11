//! A rotation of a space's key (RFC 0003, "Rotating a space key"; RFC
//! 0008, "Key chain entry"): a key chain entry for a fresh generation,
//! which encrypts the previous generations' keys under the new one, then a
//! key envelope of the new key for every member of the space, the author
//! included, except the one the call leaves out: a removed member, which
//! must never hold the new key, or the subject of a re-grant, whose
//! envelope must follow its grant (RFC 0008, "Membership entry", "A
//! re-grant"). They are written into the call's [`Batch`], which folds them
//! with what follows in one transaction; once they are folded, the new
//! generation is current and the author holds its key.
//!
//! The key chain entry lists every head of the generation graph as a
//! previous generation, as a merge rotation does (RFC 0008, "Two removals
//! that rotate the key at once", step 5), so the new generation descends
//! from all of them and is the one head after it.
//!
//! The rotation then re-wraps every live person key of the space under the
//! fresh generation (RFC 0003, "Removing a member and rotating the space
//! key", step 3; NOC-206 part 3): a person-key entry after the envelopes,
//! which repeats the key's commitment and announces the new record by
//! generation and hash, and the record itself, signed by the author, which
//! the batch collects for syncing to put. The relay's record of the person
//! key is then replaced, so the old wrap is gone. A space with a live person
//! key the member does not hold is refused before anything is written.

use std::collections::BTreeSet;

use ncdb_core::envelope;
use ncdb_core::ids::{ChangeId, MemberId, SpaceId};
use ncdb_core::keys::{MemberKey, SpaceKey};
use ncdb_core::membership::{
    self, Entry, KeyChainEntry, PersonKeyEntry, PreviousKey, record_hash, seal_previous_key,
    sign_record, wrap_person_key,
};

use crate::db::SpaceState;
use crate::error::{Error, ErrorCode};
use crate::sync::Record;

use super::batch::Batch;

/// Whether `state`'s space can be rotated by this SDK, checked before
/// anything is written: the member holds the key of every head of the
/// generation graph, which the rotation chains, and every live person key
/// of the space, which the rotation re-wraps (RFC 0003, step 3).
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
    match live(state).any(|(id, _)| !state.person_keys.contains_key(&id)) {
        false => Ok(()),
        true => Err(Error::new(
            ErrorCode::InvalidValue,
            "the member does not hold a person key of the space, which the \
             rotation must re-wrap; it rotates once it fetched the key's record",
        )),
    }
}

/// The live person keys of `state`'s space, by id, with their commitment:
/// those an accepted entry created and no shred destroyed.
fn live(state: &SpaceState) -> impl Iterator<Item = ([u8; 16], [u8; 32])> + '_ {
    state
        .checks
        .resolved()
        .person_keys()
        .filter(|(_, key)| key.shred.is_none())
        .filter_map(|(id, key)| key.commitment.map(|commitment| (id, commitment)))
}

/// Adds to `batch` a rotation of its space's key: the key chain entry of
/// a fresh generation that chains every head, answering `answers`, and
/// one envelope of the new key per member of the space, the author
/// included and `leaving_out` left out, then a person-key entry per live
/// person key, whose record re-wraps the key under the new generation. What
/// the batch writes after it is written with the new generation, after the
/// envelopes and the person-key entries. Returns the new generation.
pub(crate) fn rotate(
    batch: &mut Batch,
    state: &SpaceState,
    member: &MemberKey,
    space: SpaceId,
    answers: BTreeSet<ChangeId>,
    leaving_out: Option<MemberId>,
) -> u64 {
    let resolved = state.checks.resolved();
    let heads: Vec<u64> = resolved.heads().into_iter().collect();
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
    let rotation = batch.write(member, Entry::KeyChain(chain), None);
    let recipients: Vec<_> = resolved
        .members()
        .filter(|(id, _)| Some(*id) != leaving_out)
        .filter_map(|(id, _)| resolved.member(id).map(|m| m.key))
        .collect();
    let mut envelopes = BTreeSet::new();
    for recipient in recipients {
        let sealed = envelope::seal(&key, space, fresh, &recipient)
            .expect("a member's public key takes an envelope");
        let sent = batch.write(
            member,
            Entry::KeyEnvelope(sealed),
            Some(BTreeSet::from([rotation])),
        );
        envelopes.insert(sent);
    }
    batch.generation = fresh;
    batch.heads = envelopes.clone();
    let mut heads = envelopes;
    for (id, commitment) in live(state).collect::<Vec<_>>() {
        let held = state
            .person_keys
            .get(&id)
            .expect("every live person key is held, as `rotatable` checks");
        let wrapped = wrap_person_key(&key, space, fresh, id, held);
        let signature = sign_record(member, space, id, fresh, &wrapped);
        let entry = Entry::PersonKey(PersonKeyEntry {
            person_key: id,
            commitment,
            generation: fresh,
            record_hash: record_hash(&wrapped),
        });
        heads.insert(batch.write(member, entry, None));
        batch.records.push(Record {
            person_key: id,
            generation: fresh,
            writer: member.id().0,
            wrapped: wrapped.to_vec(),
            signature,
        });
    }
    batch.fresh = Some(key);
    batch.heads = heads;
    fresh
}
