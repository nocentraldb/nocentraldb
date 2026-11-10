//! The generation graph of RFC 0008 ("Generation ids") and the own rules
//! of key chain entries and key envelopes in step 1 of "The function"
//! (NOC-71): a key chain entry's previous generations exist and its id
//! matches; an envelope's recipient is a member and its generation exists,
//! all in the state of the change's causal past.

#[path = "support/space.rs"]
mod space;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::{BTreeMap, BTreeSet};

use ncdb_core::change::Change;
use ncdb_core::membership::{Entry, KeyChainEntry, PreviousKey, Rejection, Role, Verdict, resolve};

use space::{Space, id};

const ALICE: u8 = 1;
const BOB: u8 = 2;
const CAROL: u8 = 3;

fn not_valid() -> Option<&'static Verdict> {
    Some(&Verdict::Rejected(Rejection::NotValidWhenWritten))
}

/// A key chain entry by `author` with this generation id and these
/// previous generations, as written, whether the id matches or not.
fn chain(s: &mut Space, author: u8, generation: u64, previous: &[u64], deps: &[&Change]) -> Change {
    let entry = KeyChainEntry {
        generation,
        previous: previous
            .iter()
            .map(|generation| PreviousKey {
                generation: *generation,
                key: vec![0xc4; 72],
            })
            .collect(),
        answers: BTreeSet::new(),
    };
    s.write(author, Entry::KeyChain(entry), deps)
}

#[test]
fn the_first_generation_starts_the_graph() {
    let s = Space::new(ALICE);
    let state = resolve(s.space, [&s.genesis]);
    let first = state.generation(s.first_generation).expect("generation 1");
    assert!(first.previous.is_empty());
    assert!(first.entries.is_empty());
    assert_eq!(state.generations().count(), 1);
}

#[test]
fn a_rotation_adds_a_generation_that_descends_from_its_previous_ones() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let first = s.first_generation;
    let bob = s.add(ALICE, BOB, Role::Admin, &[&g]);
    let (by_alice, two) = s.rotate(ALICE, &[first], &[&bob]);
    let (by_bob, two_b) = s.rotate(BOB, &[first], &[&bob]);
    let (merge, three) = s.rotate(ALICE, &[two, two_b], &[&by_alice, &by_bob]);
    let state = resolve(s.space, [&g, &bob, &by_alice, &by_bob, &merge]);
    for chain in [&by_alice, &by_bob, &merge] {
        assert_eq!(state.verdict(chain.id()), Some(&Verdict::Accepted));
    }
    assert_ne!(two, two_b, "concurrent rotations by two authors");
    assert_eq!(two >> 48, 2);
    assert_eq!(three >> 48, 3);
    let merged = state.generation(three).expect("the merge rotation");
    assert_eq!(merged.previous, BTreeSet::from([two, two_b]));
    assert_eq!(merged.entries, BTreeMap::from([(merge.id(), id(ALICE))]));
    assert!(state.descends(three, first));
    assert!(state.descends(three, two_b));
    assert!(state.descends(two, two));
    assert!(!state.descends(two, two_b));
    assert!(!state.descends(first, two));
    assert!(!state.descends(u64::MAX, first), "an unknown generation");
}

#[test]
fn a_key_chain_entry_whose_id_does_not_derive_is_not_valid() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let first = s.first_generation;
    let made_up = (2 << 48) | 1;
    let wrong = chain(&mut s, ALICE, made_up, &[first], &[&g]);
    let state = resolve(s.space, [&g, &wrong]);
    assert_eq!(state.verdict(wrong.id()), not_valid());
    assert!(state.generation(made_up).is_none());
}

#[test]
fn a_key_chain_entry_after_a_generation_its_past_does_not_hold_is_not_valid() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let first = s.first_generation;
    let (rotation, two) = s.rotate(ALICE, &[first], &[&g]);
    // It names generation 2 as previous, but does not depend on its entry.
    let (blind, three) = s.rotate(ALICE, &[two], &[&g]);
    let state = resolve(s.space, [&g, &rotation, &blind]);
    assert_eq!(state.verdict(rotation.id()), Some(&Verdict::Accepted));
    assert_eq!(state.verdict(blind.id()), not_valid());
    assert!(state.generation(three).is_none());
}

#[test]
fn only_an_admin_rotates_and_a_writers_rotation_is_not_in_the_graph() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let first = s.first_generation;
    let bob = s.add(ALICE, BOB, Role::Writer, &[&g]);
    let (by_bob, two) = s.rotate(BOB, &[first], &[&bob]);
    let state = resolve(s.space, [&g, &bob, &by_bob]);
    assert_eq!(state.verdict(by_bob.id()), not_valid());
    assert!(state.generation(two).is_none());
}

#[test]
fn a_rotation_a_later_revocation_rejects_still_links_its_generation() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let first = s.first_generation;
    let bob = s.add(ALICE, BOB, Role::Admin, &[&g]);
    let cutoff = s.sequence(BOB);
    // Alice removes Bob without having seen his rotation.
    let (rotation, two) = s.rotate(BOB, &[first], &[&bob]);
    let remove = s.remove(ALICE, BOB, cutoff, &[&bob]);
    let state = resolve(s.space, [&g, &bob, &rotation, &remove]);
    assert_eq!(
        state.verdict(rotation.id()),
        Some(&Verdict::Rejected(Rejection::Excluded(remove.id())))
    );
    assert!(state.generation(two).is_some(), "decision M17");
    assert!(state.descends(two, first));
}

#[test]
fn two_accepted_key_chain_entries_with_one_id_are_both_rejected() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let first = s.first_generation;
    // Alice rotates twice from the same view: both entries derive one id.
    let (one, two) = s.rotate(ALICE, &[first], &[&g]);
    let (again, same) = s.rotate(ALICE, &[first], &[&g]);
    assert_eq!(two, same);
    let state = resolve(s.space, [&g, &one, &again]);
    let duplicate = Some(&Verdict::Rejected(Rejection::DuplicateGeneration));
    assert_eq!(state.verdict(one.id()), duplicate, "decision M23");
    assert_eq!(state.verdict(again.id()), duplicate);
    let generation = state.generation(two).expect("both are valid when written");
    assert_eq!(generation.entries.len(), 2);
}

#[test]
fn an_envelope_goes_to_a_member_for_a_generation_that_exists() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let first = s.first_generation;
    let bob = s.add(ALICE, BOB, Role::Writer, &[&g]);
    let (rotation, two) = s.rotate(ALICE, &[first], &[&bob]);
    let to_bob = s.envelope_at(ALICE, BOB, two, &[&rotation]);
    let to_carol = s.envelope_at(ALICE, CAROL, two, &[&rotation]);
    // It names generation 2, but does not depend on the rotation.
    let too_early = s.envelope_at(ALICE, BOB, two, &[&bob]);
    let nowhere = s.envelope_at(ALICE, BOB, first ^ 1, &[&bob]);
    let state = resolve(
        s.space,
        [
            &g, &bob, &rotation, &to_bob, &to_carol, &too_early, &nowhere,
        ],
    );
    assert_eq!(state.verdict(to_bob.id()), Some(&Verdict::Accepted));
    assert_eq!(state.verdict(to_carol.id()), not_valid(), "not a member");
    assert_eq!(state.verdict(too_early.id()), not_valid());
    assert_eq!(state.verdict(nowhere.id()), not_valid());
}

#[test]
fn the_graph_records_every_envelopes_recipient_whatever_its_verdict() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let first = s.first_generation;
    let bob = s.add(ALICE, BOB, Role::Writer, &[&g]);
    let (rotation, two) = s.rotate(ALICE, &[first], &[&bob]);
    let to_bob = s.envelope_at(ALICE, BOB, two, &[&rotation]);
    let to_carol = s.envelope_at(ALICE, CAROL, two, &[&rotation]);
    let state = resolve(s.space, [&g, &bob, &rotation, &to_bob, &to_carol]);
    assert_eq!(state.verdict(to_carol.id()), not_valid());
    let recipients = &state.generation(two).unwrap().recipients;
    assert_eq!(recipients, &BTreeSet::from([id(BOB), id(CAROL)]));
}
