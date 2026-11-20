//! RFC 0009, "Erasures" (NOC-401; Carsten, 2026-11-20, NOC-392, option
//! A): a shred that waits for a gap in its causal past is listed with the
//! state `Waiting`, its performer and its sign-off, and no members; once
//! the gap fills, it moves on to the state RFC 0008 gives it.
//!
//! The space is NOC-391's: Dan, a writer, shreds person 1, signed off by
//! Carol's account, the space's second account. The shred depends on
//! Alice's add of Erin, so a copy that holds the shred but not that add
//! has a gap in the shred's causal past.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so these tests run on both targets.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[cfg(target_arch = "wasm32")]
use crate::shred_gap_tests::space;
#[cfg(not(target_arch = "wasm32"))]
use crate::sync::tests::relay_support::space;

use ncdb_core::change::Change;
use ncdb_core::membership::Role;
use ncdb_core::receive::Checks;

use crate::db::SpaceState;
use crate::erasures::{ErasureState, list};
use space::Space;

const ALICE: u8 = 1;
const BOB: u8 = 2;
const CAROL: u8 = 3;
const DAN: u8 = 4;
const ERIN: u8 = 5;

/// Alice's copy of the space after it learned `changes` in this order.
fn learned(s: &Space, changes: &[&Change]) -> SpaceState {
    let mut state = SpaceState::new(s.space, space::key(ALICE));
    for change in changes {
        state.checks.add_control(change);
    }
    state
}

#[test]
fn a_shred_over_a_gap_is_listed_as_waiting_and_turns_final_when_the_gap_fills() {
    let mut s = Space::with_nonce(ALICE, [9; 16]);
    let genesis = s.genesis.clone();
    let bob = s.add(ALICE, BOB, Role::Admin, &[&genesis]);
    s.certify_by(CAROL, space::other_account(1));
    let carol = s.add(ALICE, CAROL, Role::Admin, &[&bob]);
    let dan = s.add(ALICE, DAN, Role::Writer, &[&carol]);
    let person = s.person_key(ALICE, 1, 1, &[&dan]);
    let erin = s.add(ALICE, ERIN, Role::Reader, &[&person]);
    let shred = s.shred(DAN, 1, &space::other_account(1), &[&erin]);

    let with_gap = [&genesis, &bob, &carol, &dan, &person, &shred];
    let waiting = list(learned(&s, &with_gap).checks.resolved());
    let mut whole = with_gap.to_vec();
    whole.push(&erin);
    let accepted = list(learned(&s, &whole).checks.resolved());

    assert_eq!(accepted.len(), 1, "{accepted:?}");
    assert_eq!(accepted[0].state, ErasureState::Final);
    assert_eq!(waiting.len(), 1, "{waiting:?}");
    let erasure = &waiting[0];
    assert_eq!(erasure.shred, accepted[0].shred);
    assert_eq!(erasure.state, ErasureState::Waiting);
    assert_eq!(erasure.performer, accepted[0].performer);
    assert_eq!(erasure.performer_account, accepted[0].performer_account);
    assert_eq!(erasure.sign_off, accepted[0].sign_off);
    assert!(erasure.members.is_empty(), "{:?}", erasure.members);
}

/// A shred whose whole causal past makes its sign-off invalid waits while
/// the gap lasts, and leaves the list once the gap fills and it is
/// rejected, as every rejected shred is absent from it.
#[test]
fn a_waiting_shred_that_is_rejected_when_the_gap_fills_leaves_the_list() {
    let mut s = Space::with_nonce(ALICE, [9; 16]);
    let genesis = s.genesis.clone();
    let bob = s.add(ALICE, BOB, Role::Admin, &[&genesis]);
    s.certify_by(CAROL, space::other_account(1));
    let carol = s.add(ALICE, CAROL, Role::Admin, &[&bob]);
    let dan = s.add(ALICE, DAN, Role::Writer, &[&carol]);
    let person = s.person_key(ALICE, 1, 1, &[&dan]);
    let demote_carol = s.set_role(BOB, CAROL, Role::Reader, s.sequence(CAROL), &[&person]);
    let erin = s.add(ALICE, ERIN, Role::Reader, &[&demote_carol, &person]);
    let shred = s.shred(DAN, 1, &space::other_account(1), &[&erin]);

    let with_gap = [&genesis, &bob, &carol, &dan, &person, &erin, &shred];
    let waiting = list(learned(&s, &with_gap).checks.resolved());
    assert_eq!(waiting.len(), 1, "{waiting:?}");
    assert_eq!(waiting[0].state, ErasureState::Waiting);

    let mut whole = with_gap.to_vec();
    whole.push(&demote_carol);
    assert_eq!(list(learned(&s, &whole).checks.resolved()), Vec::new());
}
