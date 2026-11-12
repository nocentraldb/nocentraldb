//! What the control state keeps of a shred for the audit of erasures (RFC
//! 0009, "Erasures"; Carsten, 2026-11-12, NOC-246): the member that
//! performed it, the account key that signed it off, and the members of the
//! state of its causal past, which are the members that could have held the
//! person key. A member added concurrently, or removed before the shred,
//! is not among them; one removed concurrently still is.

#[path = "support/space.rs"]
mod space;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeSet;

use ncdb_core::change::Change;
use ncdb_core::membership::{PersonKey, Role, resolve};

use space::{Space, id, other_account, person};

const ALICE: u8 = 1;
const BOB: u8 = 2;
const CAROL: u8 = 3;
const DAN: u8 = 4;
const ERIN: u8 = 5;

/// Alice creates the space, adds Carol, an admin of a second account, Bob
/// a writer and Dan a reader. Bob writes person 1's key.
fn space() -> (Space, Vec<Change>) {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    s.certify_by(CAROL, other_account(1));
    let carol = s.add(ALICE, CAROL, Role::Admin, &[&g]);
    let bob = s.add(ALICE, BOB, Role::Writer, &[&carol]);
    let dan = s.add(ALICE, DAN, Role::Reader, &[&bob]);
    let entry = s.person_key(BOB, 1, 0xc1, &[&dan]);
    (s, vec![g, carol, bob, dan, entry])
}

fn key(s: &Space, changes: &[Change]) -> PersonKey {
    resolve(s.space, changes)
        .person_key(person(1))
        .expect("the person key")
        .clone()
}

#[test]
fn the_state_keeps_the_performer_and_the_sign_off_account_of_a_shred() {
    let (mut s, mut changes) = space();
    let entry = changes.last().unwrap().clone();
    let shred = s.shred(BOB, 1, &other_account(1), &[&entry]);
    changes.push(shred);

    let key = key(&s, &changes);

    assert_eq!(key.performer, Some(id(BOB)));
    assert_eq!(
        key.sign_off,
        Some(other_account(1).verifying_key().to_bytes())
    );
}

#[test]
fn the_members_of_the_shreds_causal_past_are_kept() {
    let (mut s, mut changes) = space();
    let entry = changes.last().unwrap().clone();
    let shred = s.shred(BOB, 1, &other_account(1), &[&entry]);
    changes.push(shred);

    let key = key(&s, &changes);

    let expected = BTreeSet::from([id(ALICE), id(BOB), id(CAROL), id(DAN)]);
    assert_eq!(key.past_members, expected);
}

#[test]
fn a_member_added_concurrently_with_the_shred_is_not_among_them() {
    let (mut s, mut changes) = space();
    let entry = changes.last().unwrap().clone();
    let shred = s.shred(BOB, 1, &other_account(1), &[&entry]);
    let erin = s.add(ALICE, ERIN, Role::Reader, &[&entry]);
    changes.extend([shred, erin]);

    let key = key(&s, &changes);

    assert!(!key.past_members.contains(&id(ERIN)));
}

#[test]
fn a_member_removed_before_the_shred_is_not_among_them() {
    let (mut s, mut changes) = space();
    let entry = changes.last().unwrap().clone();
    let removal = s.remove(ALICE, DAN, s.sequence(DAN), &[&entry]);
    let shred = s.shred(BOB, 1, &other_account(1), &[&removal]);
    changes.extend([removal, shred]);

    let key = key(&s, &changes);

    assert!(!key.past_members.contains(&id(DAN)));
}

#[test]
fn a_member_removed_concurrently_with_the_shred_is_still_among_them() {
    let (mut s, mut changes) = space();
    let entry = changes.last().unwrap().clone();
    let shred = s.shred(BOB, 1, &other_account(1), &[&entry]);
    let removal = s.remove(ALICE, DAN, s.sequence(DAN), &[&entry]);
    changes.extend([shred, removal]);

    let key = key(&s, &changes);

    assert!(key.past_members.contains(&id(DAN)));
}

#[test]
fn a_person_key_without_a_shred_keeps_none_of_it() {
    let (s, changes) = space();

    let key = key(&s, &changes);

    assert_eq!(key.performer, None);
    assert_eq!(key.sign_off, None);
    assert!(key.past_members.is_empty());
}
