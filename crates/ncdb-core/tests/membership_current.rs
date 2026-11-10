//! The current generation of RFC 0008 ("Two removals that rotate the key at
//! once", step 3), which a writer seals under: a head of the generation
//! graph, a generation that no accepted key chain entry lists as previous;
//! among the heads, the highest ordinal, then the one whose key chain entry
//! comes first in the order (NOC-86).

#[path = "support/space.rs"]
mod space;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeSet;

use ncdb_core::membership::{Rejection, Role, Verdict, resolve};

use space::Space;

const ALICE: u8 = 1;
const BOB: u8 = 2;

#[test]
fn a_new_space_writes_under_its_first_generation() {
    let s = Space::new(ALICE);
    let state = resolve(s.space, [&s.genesis]);
    assert_eq!(state.heads(), BTreeSet::from([s.first_generation]));
    assert_eq!(state.current_generation(), Some(s.first_generation));
}

#[test]
fn a_space_without_a_genesis_has_no_current_generation() {
    let s = Space::new(ALICE);
    let state = resolve(s.space, []);
    assert_eq!(state.current_generation(), None);
}

#[test]
fn a_rotation_becomes_the_current_generation_and_a_writer_seals_under_it() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let first = s.first_generation;
    let bob = s.add(ALICE, BOB, Role::Writer, &[&g]);
    let (rotation, two) = s.rotate(ALICE, &[first], &[&bob]);
    let state = resolve(s.space, [&g, &bob, &rotation]);
    assert_eq!(state.heads(), BTreeSet::from([two]));
    let current = state.current_generation().expect("a genesis");
    assert_eq!(current, two);
    let sealed = s.sealed_under(BOB, current, vec![]);
    assert!(state.admits(sealed.header()));
}

#[test]
fn of_two_concurrent_rotations_the_one_first_in_the_order_is_current() {
    for (creator, other) in [(ALICE, BOB), (BOB, ALICE)] {
        let mut s = Space::new(creator);
        let g = s.genesis.clone();
        let first = s.first_generation;
        let add = s.add(creator, other, Role::Admin, &[&g]);
        let (by_creator, mine) = s.rotate(creator, &[first], &[&add]);
        let (by_other, theirs) = s.rotate(other, &[first], &[&add]);
        let state = resolve(s.space, [&g, &add, &by_creator, &by_other]);
        assert_eq!(state.heads(), BTreeSet::from([mine, theirs]));
        let position = |id| state.order().iter().position(|o| *o == id);
        assert!(position(by_creator.id()) < position(by_other.id()));
        // The creator is the most senior member, so its rotation comes
        // first in the order, whichever change id is lower.
        assert_eq!(state.current_generation(), Some(mine));
    }
}

#[test]
fn a_head_with_a_higher_ordinal_wins() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let first = s.first_generation;
    let bob = s.add(ALICE, BOB, Role::Admin, &[&g]);
    let (by_alice, two) = s.rotate(ALICE, &[first], &[&bob]);
    let (by_bob, two_b) = s.rotate(BOB, &[first], &[&bob]);
    let (again, three) = s.rotate(BOB, &[two_b], &[&by_bob]);
    let state = resolve(s.space, [&g, &bob, &by_alice, &by_bob, &again]);
    assert_eq!(state.heads(), BTreeSet::from([two, three]));
    assert_eq!(state.current_generation(), Some(three));
}

#[test]
fn a_merge_rotation_leaves_one_head() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let first = s.first_generation;
    let bob = s.add(ALICE, BOB, Role::Admin, &[&g]);
    let (by_alice, two) = s.rotate(ALICE, &[first], &[&bob]);
    let (by_bob, two_b) = s.rotate(BOB, &[first], &[&bob]);
    let (merge, three) = s.rotate(ALICE, &[two, two_b], &[&by_alice, &by_bob]);
    let state = resolve(s.space, [&g, &bob, &by_alice, &by_bob, &merge]);
    assert_eq!(state.heads(), BTreeSet::from([three]));
    assert_eq!(state.current_generation(), Some(three));
}

#[test]
fn a_rotation_by_an_admin_removed_concurrently_still_counts_as_a_head() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let first = s.first_generation;
    let bob = s.add(ALICE, BOB, Role::Admin, &[&g]);
    let cutoff = s.sequence(BOB);
    let (rotation, two) = s.rotate(BOB, &[first], &[&bob]);
    let remove = s.remove(ALICE, BOB, cutoff, &[&bob]);
    let state = resolve(s.space, [&g, &bob, &rotation, &remove]);
    assert_eq!(
        state.verdict(rotation.id()),
        Some(&Verdict::Rejected(Rejection::Excluded(remove.id())))
    );
    // A rejected rotation lists nothing, so the first generation stays a
    // head; generation 2 has the higher ordinal (decision M17), and the
    // exposure rule makes the next admin rotate away from it.
    assert_eq!(state.heads(), BTreeSet::from([first, two]));
    assert_eq!(state.current_generation(), Some(two));
}

#[test]
fn a_generation_two_key_chain_entries_name_is_never_current() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let first = s.first_generation;
    // Alice rotates twice from the same view: both entries derive one id,
    // and both are rejected (decision M23).
    let (one, two) = s.rotate(ALICE, &[first], &[&g]);
    let (again, same) = s.rotate(ALICE, &[first], &[&g]);
    assert_eq!(two, same);
    let state = resolve(s.space, [&g, &one, &again]);
    assert_eq!(state.heads(), BTreeSet::from([first]));
    assert_eq!(state.current_generation(), Some(first));
    // A rotation by an author that saw both is accepted, and its
    // generation, named by one entry, becomes current.
    let (after, three) = s.rotate(ALICE, &[two], &[&one, &again]);
    let state = resolve(s.space, [&g, &one, &again, &after]);
    assert_eq!(state.verdict(after.id()), Some(&Verdict::Accepted));
    assert_eq!(state.current_generation(), Some(three));
}
