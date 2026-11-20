//! RFC 0008, "A shred is final when accepted" (NOC-391): a member destroys
//! its copy of a person key only once it holds the shred and the shred's
//! whole causal past. Over a gap, the shred waits and the key stays.
//!
//! Dan, a writer, shreds person 1, signed off by Carol's account, the
//! space's second account. Bob had demoted Carol to a reader before, and
//! Dan's shred depends on that demotion through Alice's add of a reader,
//! which saw it: in the shred's whole causal past, Carol's account holds no
//! admin, so the sign-off is not valid. A member that holds the add and the
//! shred but not yet the demotion must keep the key.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so these tests run on both targets.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

// Natively the sync tests already load the core's simulated space; a
// second copy of the module is refused, so this uses theirs.
#[cfg(target_arch = "wasm32")]
#[path = "../../ncdb-core/tests/support/space.rs"]
mod space;
#[cfg(not(target_arch = "wasm32"))]
use crate::sync::tests::relay_support::space;

use ncdb_core::change::Change;
use ncdb_core::keys::PersonKey;
use ncdb_core::membership::Role;
use ncdb_core::receive::Checks;

use crate::db::SpaceState;
use space::Space;

const ALICE: u8 = 1;
const BOB: u8 = 2;
const CAROL: u8 = 3;
const DAN: u8 = 4;
const ERIN: u8 = 5;

/// Alice's copy of the space, holding person 1's key, after it learned
/// `changes` in this order.
fn learned(s: &Space, changes: &[&Change]) -> SpaceState {
    let mut state = SpaceState::new(s.space, space::key(ALICE));
    state
        .person_keys
        .insert(space::person(1), PersonKey::from_bytes([7; 32]));
    for change in changes {
        state.checks.add_control(change);
        state.drop_shredded();
    }
    state
}

#[test]
fn a_member_keeps_the_person_key_while_the_shreds_causal_past_has_a_gap() {
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
    let base = [&genesis, &bob, &carol, &dan, &person];

    let mut with_gap = base.to_vec();
    with_gap.extend([&erin, &shred]);
    let state = learned(&s, &with_gap);
    assert!(
        state.person_keys.contains_key(&space::person(1)),
        "the member destroyed the key over a gap in the shred's causal past"
    );

    let mut whole = with_gap.clone();
    whole.push(&demote_carol);
    let state = learned(&s, &whole);
    assert!(state.person_keys.contains_key(&space::person(1)));
}

/// The same shred without the demotion, so with a valid sign-off: once
/// the member holds its whole causal past, the key goes.
#[test]
fn a_member_destroys_the_person_key_once_it_holds_the_shreds_whole_causal_past() {
    let mut s = Space::with_nonce(ALICE, [9; 16]);
    let genesis = s.genesis.clone();
    let bob = s.add(ALICE, BOB, Role::Admin, &[&genesis]);
    s.certify_by(CAROL, space::other_account(1));
    let carol = s.add(ALICE, CAROL, Role::Admin, &[&bob]);
    let dan = s.add(ALICE, DAN, Role::Writer, &[&carol]);
    let person = s.person_key(ALICE, 1, 1, &[&dan]);
    let erin = s.add(ALICE, ERIN, Role::Reader, &[&person]);
    let shred = s.shred(DAN, 1, &space::other_account(1), &[&erin]);

    let shred_first = [&genesis, &bob, &carol, &dan, &person, &shred];
    let state = learned(&s, &shred_first);
    assert!(state.person_keys.contains_key(&space::person(1)));

    let mut whole = shred_first.to_vec();
    whole.push(&erin);
    let state = learned(&s, &whole);
    assert!(!state.person_keys.contains_key(&space::person(1)));
}
