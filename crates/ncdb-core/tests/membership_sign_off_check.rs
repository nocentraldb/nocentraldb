//! Whether a sign-off would hold for a shred not written yet (NOC-325;
//! RFC 0008, "Shred entry"): the SDK checks a sign-off before it writes
//! anything, and a device signs off only where its account may. The check
//! is the rule a written shred is judged by, in the state the shred would
//! be written in, and says whether the shred would be final or
//! provisional.

#[path = "support/space.rs"]
mod space;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::change::Change;
use ncdb_core::membership::{Role, SignOff, SignedOff, Verdict, resolve};

use space::{Space, account, id, other_account, person};

const ALICE: u8 = 1;
const BOB: u8 = 2;
const CAROL: u8 = 3;

/// Alice, of the first account, admin; Bob, of the first account, writer;
/// Carol, of a second account, admin; Bob's person key 1.
fn two_accounts() -> (Space, Vec<Change>) {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let bob = s.add(ALICE, BOB, Role::Writer, &[&g]);
    s.certify_by(CAROL, other_account(1));
    let carol = s.add(ALICE, CAROL, Role::Admin, &[&bob]);
    let key = s.person_key(BOB, 1, 0xc1, &[&carol]);
    (s, vec![g, bob, carol, key])
}

/// Alice and Bob, both of the first account; Bob's person key 1.
fn one_account() -> (Space, Vec<Change>) {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let bob = s.add(ALICE, BOB, Role::Writer, &[&g]);
    let key = s.person_key(BOB, 1, 0xc1, &[&bob]);
    (s, vec![g, bob, key])
}

#[test]
fn a_second_admin_account_s_sign_off_holds_and_makes_the_shred_final() {
    let (mut s, held) = two_accounts();
    let state = resolve(s.space, held.iter());
    let sign_off = SignOff::sign(&other_account(1), s.space, person(1), id(BOB));
    assert_eq!(
        state.sign_off(s.space, id(BOB), person(1), &sign_off),
        Some(SignedOff::Final)
    );
    // The shred written with it is accepted, as the check said.
    let shred = s.shred_with(BOB, 1, sign_off, &[&held[3]]);
    let written = resolve(s.space, held.iter().chain([&shred]));
    assert_eq!(written.verdict(shred.id()), Some(&Verdict::Accepted));
}

#[test]
fn the_performer_s_own_account_does_not_hold_beside_another_account() {
    let (s, held) = two_accounts();
    let state = resolve(s.space, held.iter());
    let sign_off = SignOff::sign(&account(), s.space, person(1), id(BOB));
    assert_eq!(state.sign_off(s.space, id(BOB), person(1), &sign_off), None);
}

#[test]
fn in_a_space_of_one_account_its_own_sign_off_holds_and_makes_the_shred_provisional() {
    let (mut s, held) = one_account();
    let state = resolve(s.space, held.iter());
    let sign_off = SignOff::sign(&account(), s.space, person(1), id(BOB));
    assert_eq!(
        state.sign_off(s.space, id(BOB), person(1), &sign_off),
        Some(SignedOff::Provisional)
    );
    let shred = s.shred_with(BOB, 1, sign_off, &[&held[2]]);
    let written = resolve(s.space, held.iter().chain([&shred]));
    assert_eq!(written.verdict(shred.id()), Some(&Verdict::Accepted));
    assert!(written.person_key(person(1)).unwrap().provisional);
}

#[test]
fn a_sign_off_for_another_performer_person_key_or_space_does_not_hold() {
    let (s, held) = two_accounts();
    let state = resolve(s.space, held.iter());
    let for_alice = SignOff::sign(&other_account(1), s.space, person(1), id(ALICE));
    assert_eq!(state.sign_off(s.space, id(BOB), person(1), &for_alice), None);
    let for_two = SignOff::sign(&other_account(1), s.space, person(2), id(BOB));
    assert_eq!(state.sign_off(s.space, id(BOB), person(1), &for_two), None);
    let mut elsewhere = s.space;
    elsewhere.0[0] ^= 1;
    let other_space = SignOff::sign(&other_account(1), elsewhere, person(1), id(BOB));
    assert_eq!(state.sign_off(s.space, id(BOB), person(1), &other_space), None);
}

#[test]
fn a_person_key_the_space_does_not_hold_cannot_be_signed_off() {
    let (s, held) = two_accounts();
    let state = resolve(s.space, held.iter());
    let sign_off = SignOff::sign(&other_account(1), s.space, person(2), id(BOB));
    assert_eq!(state.sign_off(s.space, id(BOB), person(2), &sign_off), None);
}

#[test]
fn a_performer_that_is_no_member_cannot_be_signed_off_for() {
    let (s, held) = two_accounts();
    let state = resolve(s.space, held.iter());
    let sign_off = SignOff::sign(&other_account(1), s.space, person(1), id(9));
    assert_eq!(state.sign_off(s.space, id(9), person(1), &sign_off), None);
}
