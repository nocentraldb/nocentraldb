//! The exposure rule of RFC 0008 ("Two removals that rotate the key at
//! once", step 4): who may hold a generation is the author of its key chain
//! entry, every recipient of any envelope for it, accepted or rejected, and
//! everyone who may hold a generation that descends from it. A generation is
//! exposed when one of them is not a member of the state (NOC-86).

#[path = "support/space.rs"]
mod space;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeSet;

use ncdb_core::membership::{Rejection, Role, Verdict, resolve};

use space::{Space, id};

const ALICE: u8 = 1;
const BOB: u8 = 2;
const CAROL: u8 = 3;
const XAVIER: u8 = 4;
const YVONNE: u8 = 5;

#[test]
fn the_creator_holds_the_first_generation_and_nothing_is_exposed() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let first = s.first_generation;
    let own = s.envelope(ALICE, ALICE, &[&g]);
    let bob = s.add(ALICE, BOB, Role::Writer, &[&own]);
    let to_bob = s.envelope(ALICE, BOB, &[&bob]);
    let state = resolve(s.space, [&g, &own, &bob, &to_bob]);
    let holders = &state.generation(first).expect("the first").holders;
    assert_eq!(holders, &BTreeSet::from([id(ALICE), id(BOB)]));
    assert!(!state.exposed(first));
}

#[test]
fn whoever_holds_a_descendant_may_hold_its_previous_generations() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let first = s.first_generation;
    let bob = s.add(ALICE, BOB, Role::Writer, &[&g]);
    let (rotation, two) = s.rotate(ALICE, &[first], &[&bob]);
    let to_bob = s.envelope_at(ALICE, BOB, two, &[&rotation]);
    let cutoff = s.sequence(BOB);
    let remove = s.remove(ALICE, BOB, cutoff, &[&to_bob]);
    let (answer, three) = s.rotate(ALICE, &[two], &[&remove]);
    let own = s.envelope_at(ALICE, ALICE, three, &[&answer]);
    let state = resolve(
        s.space,
        [&g, &bob, &rotation, &to_bob, &remove, &answer, &own],
    );
    let holders = |generation| state.generation(generation).unwrap().holders.clone();
    // Bob holds generation 2 through his envelope, and generation 1
    // through the key chain of generation 2.
    assert_eq!(holders(first), BTreeSet::from([id(ALICE), id(BOB)]));
    assert_eq!(holders(two), BTreeSet::from([id(ALICE), id(BOB)]));
    assert_eq!(holders(three), BTreeSet::from([id(ALICE)]));
    assert!(state.exposed(first));
    assert!(state.exposed(two));
    assert_eq!(state.current_generation(), Some(three));
    assert!(!state.exposed(three), "the rotation answered the removal");
}

/// RFC 0008's example: Alice removes Xavier and rotates while Bob removes
/// Yvonne and rotates. Each new generation has an envelope for the member
/// the other removed.
#[test]
fn two_removals_that_rotate_at_once_leave_the_current_generation_exposed() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let first = s.first_generation;
    let bob = s.add(ALICE, BOB, Role::Admin, &[&g]);
    let xavier = s.add(ALICE, XAVIER, Role::Writer, &[&bob]);
    let yvonne = s.add(ALICE, YVONNE, Role::Writer, &[&xavier]);
    let base = [&g, &bob, &xavier, &yvonne];

    let no_x = s.remove(ALICE, XAVIER, s.sequence(XAVIER), &[&yvonne]);
    let (by_alice, two) = s.rotate(ALICE, &[first], &[&no_x]);
    let mut alice = vec![no_x, by_alice];
    for who in [ALICE, BOB, YVONNE] {
        let envelope = s.envelope_at(ALICE, who, two, &[alice.last().unwrap()]);
        alice.push(envelope);
    }
    let no_y = s.remove(BOB, YVONNE, s.sequence(YVONNE), &[&yvonne]);
    let (by_bob, two_b) = s.rotate(BOB, &[first], &[&no_y]);
    let mut bob_side = vec![no_y, by_bob];
    for who in [ALICE, BOB, XAVIER] {
        let envelope = s.envelope_at(BOB, who, two_b, &[bob_side.last().unwrap()]);
        bob_side.push(envelope);
    }

    let state = resolve(s.space, base.into_iter().chain(&alice).chain(&bob_side));
    assert_eq!(state.role(id(XAVIER)), None);
    assert_eq!(state.role(id(YVONNE)), None);
    assert_eq!(state.current_generation(), Some(two), "Alice is senior");
    let holders = &state.generation(two).unwrap().holders;
    assert!(holders.contains(&id(YVONNE)));
    assert!(state.exposed(two));
    assert!(state.exposed(two_b));
}

#[test]
fn a_rejected_envelope_counts() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let first = s.first_generation;
    // Carol was never added, so the envelope is not valid when written,
    // but it may still have been delivered.
    let to_carol = s.envelope(ALICE, CAROL, &[&g]);
    let state = resolve(s.space, [&g, &to_carol]);
    assert_eq!(
        state.verdict(to_carol.id()),
        Some(&Verdict::Rejected(Rejection::NotValidWhenWritten))
    );
    assert!(
        state
            .generation(first)
            .unwrap()
            .holders
            .contains(&id(CAROL))
    );
    assert!(state.exposed(first));
}

#[test]
fn a_rejected_rotations_author_holds_its_generation() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let first = s.first_generation;
    let bob = s.add(ALICE, BOB, Role::Admin, &[&g]);
    let cutoff = s.sequence(BOB);
    let (rotation, two) = s.rotate(BOB, &[first], &[&bob]);
    let remove = s.remove(ALICE, BOB, cutoff, &[&bob]);
    let state = resolve(s.space, [&g, &bob, &rotation, &remove]);
    assert_eq!(state.current_generation(), Some(two));
    assert_eq!(
        state.generation(two).unwrap().holders,
        BTreeSet::from([id(BOB)])
    );
    assert!(state.exposed(two));
}

#[test]
fn a_member_that_left_exposes_what_it_held() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let first = s.first_generation;
    let bob = s.add(ALICE, BOB, Role::Writer, &[&g]);
    let to_bob = s.envelope(ALICE, BOB, &[&bob]);
    let held = resolve(s.space, [&g, &bob, &to_bob]);
    assert!(!held.exposed(first));
    let leave = s.remove(BOB, BOB, s.sequence(BOB), &[&to_bob]);
    let state = resolve(s.space, [&g, &bob, &to_bob, &leave]);
    assert_eq!(state.verdict(leave.id()), Some(&Verdict::Accepted));
    assert!(state.exposed(first));
}

#[test]
fn a_generation_outside_the_graph_is_not_exposed() {
    let s = Space::new(ALICE);
    let state = resolve(s.space, [&s.genesis]);
    assert!(!state.exposed(s.first_generation ^ 1));
}
