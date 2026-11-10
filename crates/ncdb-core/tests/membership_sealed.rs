//! RFC 0008's membership verdict for sealed changes ("Sealed changes") with
//! the generation graph (NOC-71): a change counts under a grant when its key
//! generation is the grant's from generation or descends from it. So a
//! writer keeps writing after a rotation, and a re-granted member's changes
//! count only under the fresh generation and its descendants (RFC 0008, "A
//! member demoted and raised again").

#[path = "support/space.rs"]
mod space;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::change::Change;
use ncdb_core::membership::{Resolved, Role, resolve};

use space::Space;

const ALICE: u8 = 1;
const BOB: u8 = 2;
const CAROL: u8 = 3;
const DAN: u8 = 4;

fn admits(state: &Resolved, change: &Change) -> bool {
    state.admits(change.header())
}

#[test]
fn a_writer_writes_under_a_generation_that_descends_from_its_grant() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let first = s.first_generation;
    let bob = s.add(ALICE, BOB, Role::Writer, &[&g]);
    let (rotation, two) = s.rotate(ALICE, &[first], &[&bob]);
    let (merge, three) = s.rotate(ALICE, &[two], &[&rotation]);
    let state = resolve(s.space, [&g, &bob, &rotation, &merge]);
    for generation in [first, two, three] {
        let change = s.sealed_under(BOB, generation, vec![]);
        assert!(admits(&state, &change), "generation {generation:#x}");
    }
    let elsewhere = s.sealed_under(BOB, first ^ 1, vec![]);
    assert!(!admits(&state, &elsewhere), "a generation not in the graph");
}

#[test]
fn a_grant_from_a_later_generation_does_not_reach_back() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let first = s.first_generation;
    let (rotation, two) = s.rotate(ALICE, &[first], &[&g]);
    let bob = s.add_at(ALICE, BOB, Role::Writer, two, &[&rotation]);
    let state = resolve(s.space, [&g, &rotation, &bob]);
    assert!(admits(&state, &s.sealed_under(BOB, two, vec![])));
    assert!(!admits(&state, &s.sealed_under(BOB, first, vec![])));
}

/// RFC 0008's example: Alice lowers Dan from writer to reader, Dan keeps
/// writing offline under the old generation, and Alice raises him again
/// under a fresh generation. Carol rotates concurrently to another one.
#[test]
fn a_member_demoted_and_raised_again_counts_only_under_the_fresh_generation() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let first = s.first_generation;
    let carol = s.add(ALICE, CAROL, Role::Admin, &[&g]);
    let dan = s.add(ALICE, DAN, Role::Writer, &[&carol]);
    let inside = s.sealed_under(DAN, first, vec![]);
    let cutoff = s.sequence(DAN);
    let demote = s.set_role(ALICE, DAN, Role::Reader, cutoff, &[&dan]);
    let offline = s.sealed_under(DAN, first, vec![]);
    let (rotation, fresh) = s.rotate(ALICE, &[first], &[&demote]);
    let (by_carol, concurrent) = s.rotate(CAROL, &[first], &[&dan]);
    let raise = s.set_role_at(ALICE, DAN, Role::Writer, fresh, cutoff, &[&rotation]);
    let state = resolve(
        s.space,
        [&g, &carol, &dan, &demote, &rotation, &by_carol, &raise],
    );
    let after_fresh = s.sealed_under(DAN, fresh, vec![]);
    let after_old = s.sealed_under(DAN, first, vec![]);
    let after_concurrent = s.sealed_under(DAN, concurrent, vec![]);
    assert!(admits(&state, &inside), "inside the cutoff");
    assert!(
        !admits(&state, &offline),
        "above the cutoff, before the raise"
    );
    assert!(admits(&state, &after_fresh), "under the fresh generation");
    assert!(
        !admits(&state, &after_old),
        "the old generation never counts"
    );
    assert!(
        !admits(&state, &after_concurrent),
        "8' does not descend from 8"
    );

    let (merge, merged) = s.rotate(ALICE, &[fresh, concurrent], &[&raise, &by_carol]);
    let state = resolve(
        s.space,
        [
            &g, &carol, &dan, &demote, &rotation, &by_carol, &raise, &merge,
        ],
    );
    let after_merge = s.sealed_under(DAN, merged, vec![]);
    assert!(admits(&state, &after_merge), "a descendant of both");
}
