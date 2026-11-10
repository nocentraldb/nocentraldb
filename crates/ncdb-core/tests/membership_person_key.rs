//! `resolve` of RFC 0008 over person-key and shred entries ("The function",
//! "Person-key entry", "Shred entry"): a person-key entry repeats the
//! commitment of its id, a shred names a person key that exists and carries
//! a second admin account's sign-off, and a shred wins over every
//! person-key entry of its id that it did not see.

#[path = "support/space.rs"]
mod space;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeSet;

use ncdb_core::change::Change;
use ncdb_core::membership::{Rejection, Role, SignOff, Verdict, record_hash, resolve};

use space::{Space, account, id, other_account, person, wrapped};

const ALICE: u8 = 1;
const BOB: u8 = 2;
const CAROL: u8 = 3;
const DAN: u8 = 4;

const ACCEPTED: Option<&Verdict> = Some(&Verdict::Accepted);
const NOT_VALID: Option<&Verdict> = Some(&Verdict::Rejected(Rejection::NotValidWhenWritten));

/// Alice, of the first account, admin; Bob, of the first account, writer;
/// Carol, of a second account, admin.
fn space() -> (Space, Vec<Change>) {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let bob = s.add(ALICE, BOB, Role::Writer, &[&g]);
    s.certify_by(CAROL, other_account(1));
    let carol = s.add(ALICE, CAROL, Role::Admin, &[&bob]);
    (s, vec![g, bob, carol])
}

fn record(s: &Space, who: u8) -> (u64, [u8; 32]) {
    (
        s.first_generation,
        record_hash(&wrapped(who, s.first_generation)),
    )
}

#[test]
fn the_first_person_key_entry_creates_the_person_key() {
    let (mut s, held) = space();
    let entry = s.person_key(BOB, 1, 0xc1, &[&held[2]]);
    let state = resolve(s.space, held.iter().chain([&entry]));
    assert_eq!(state.verdict(entry.id()), ACCEPTED);
    let key = state.person_key(person(1)).expect("the person key exists");
    assert_eq!(key.commitment, [0xc1; 32]);
    assert_eq!(key.records, BTreeSet::from([record(&s, 1)]));
    assert_eq!(key.shred, None);
    assert_eq!(state.person_key(person(2)), None);
}

#[test]
fn a_later_entry_repeats_the_commitment_or_is_not_valid_when_written() {
    let (mut s, held) = space();
    let first = s.person_key(BOB, 1, 0xc1, &[&held[2]]);
    let (chain, next) = s.rotate(ALICE, &[s.first_generation], &[&first]);
    let rewrap = s.person_key_at(ALICE, 1, 0xc1, next, &[&chain]);
    let swapped = s.person_key_at(ALICE, 1, 0xc2, next, &[&rewrap]);
    let state = resolve(
        s.space,
        held.iter().chain([&first, &chain, &rewrap, &swapped]),
    );
    assert_eq!(state.verdict(rewrap.id()), ACCEPTED);
    assert_eq!(state.verdict(swapped.id()), NOT_VALID);
    let key = state.person_key(person(1)).unwrap();
    let rewrapped = (next, record_hash(&wrapped(1, next)));
    assert_eq!(key.records, BTreeSet::from([record(&s, 1), rewrapped]));
}

#[test]
fn of_two_concurrent_first_entries_with_other_commitments_the_first_in_the_order_wins() {
    let (mut s, held) = space();
    let one = s.person_key(BOB, 1, 0xc1, &[&held[2]]);
    let two = s.person_key(ALICE, 1, 0xc2, &[&held[2]]);
    let state = resolve(s.space, held.iter().chain([&one, &two]));
    // Alice, the creator, is senior, so hers comes first.
    assert_eq!(state.verdict(two.id()), ACCEPTED);
    assert_eq!(
        state.verdict(one.id()),
        Some(&Verdict::Rejected(Rejection::Commitment))
    );
    assert_eq!(state.person_key(person(1)).unwrap().commitment, [0xc2; 32]);
}

#[test]
fn a_shred_signed_off_by_a_second_admin_account_destroys_the_person_key() {
    let (mut s, held) = space();
    let entry = s.person_key(BOB, 1, 0xc1, &[&held[2]]);
    let shred = s.shred(BOB, 1, &other_account(1), &[&entry]);
    let state = resolve(s.space, held.iter().chain([&entry, &shred]));
    assert_eq!(state.verdict(shred.id()), ACCEPTED);
    let key = state.person_key(person(1)).unwrap();
    assert_eq!(key.shred, Some(shred.id()));
    assert_eq!(key.records, BTreeSet::from([record(&s, 1)]));
}

#[test]
fn the_performer_cannot_sign_off_with_its_own_account() {
    let (mut s, held) = space();
    let entry = s.person_key(BOB, 1, 0xc1, &[&held[2]]);
    // Alice is an admin of Bob's account, but it is still Bob's account.
    let shred = s.shred(BOB, 1, &account(), &[&entry]);
    let state = resolve(s.space, held.iter().chain([&entry, &shred]));
    assert_eq!(state.verdict(shred.id()), NOT_VALID);
    assert_eq!(state.person_key(person(1)).unwrap().shred, None);
}

#[test]
fn the_sign_off_account_must_hold_an_admin_in_the_shreds_causal_past() {
    let (mut s, held) = space();
    s.certify_by(DAN, other_account(2));
    let dan = s.add(ALICE, DAN, Role::Writer, &[&held[2]]);
    let entry = s.person_key(BOB, 1, 0xc1, &[&dan]);
    // Dan's account holds a writer, not an admin.
    let by_writer = s.shred(BOB, 1, &other_account(2), &[&entry]);
    // A third account holds no member at all.
    let by_stranger = s.shred(BOB, 1, &other_account(3), &[&entry]);
    // Dan becomes admin, but the shred did not see it.
    let raise = s.set_role(ALICE, DAN, Role::Admin, 0, &[&entry]);
    let unseen = s.shred(BOB, 1, &other_account(2), &[&entry]);
    let all = held
        .iter()
        .chain([&dan, &entry, &by_writer, &by_stranger, &raise, &unseen]);
    let state = resolve(s.space, all);
    assert_eq!(state.verdict(raise.id()), ACCEPTED);
    for shred in [&by_writer, &by_stranger, &unseen] {
        assert_eq!(state.verdict(shred.id()), NOT_VALID);
    }
    assert_eq!(state.person_key(person(1)).unwrap().shred, None);
}

#[test]
fn a_sign_off_for_another_performer_or_person_key_does_not_verify() {
    let (mut s, held) = space();
    let entry = s.person_key(BOB, 1, 0xc1, &[&held[2]]);
    let other = s.person_key(BOB, 2, 0xc2, &[&entry]);
    // Carol's account signs off for Dan as performer, and for person 2.
    let for_dan = SignOff::sign(&other_account(1), s.space, person(1), id(DAN));
    let for_two = SignOff::sign(&other_account(1), s.space, person(2), id(BOB));
    let by_bob = s.shred_with(BOB, 1, for_dan, &[&other]);
    let of_one = s.shred_with(BOB, 1, for_two, &[&other]);
    let state = resolve(
        s.space,
        held.iter().chain([&entry, &other, &by_bob, &of_one]),
    );
    assert_eq!(state.verdict(by_bob.id()), NOT_VALID);
    assert_eq!(state.verdict(of_one.id()), NOT_VALID);
}

#[test]
fn a_shred_names_a_person_key_in_its_causal_past() {
    let (mut s, held) = space();
    let unknown = s.shred(BOB, 1, &other_account(1), &[&held[2]]);
    let entry = s.person_key(BOB, 1, 0xc1, &[&held[2]]);
    let state = resolve(s.space, held.iter().chain([&unknown, &entry]));
    assert_eq!(state.verdict(unknown.id()), NOT_VALID);
    assert_eq!(state.verdict(entry.id()), ACCEPTED);
}

#[test]
fn a_shred_wins_over_every_person_key_entry_it_did_not_see() {
    let (mut s, held) = space();
    let entry = s.person_key(BOB, 1, 0xc1, &[&held[2]]);
    let (chain, next) = s.rotate(ALICE, &[s.first_generation], &[&entry]);
    // A re-wrap concurrent with the shred, and one written after it.
    let concurrent = s.person_key_at(ALICE, 1, 0xc1, next, &[&chain]);
    let shred = s.shred(BOB, 1, &other_account(1), &[&chain]);
    let after = s.person_key_at(ALICE, 1, 0xc1, next, &[&shred]);
    let other = s.person_key(ALICE, 2, 0xc2, &[&shred]);
    let state = resolve(
        s.space,
        held.iter()
            .chain([&entry, &chain, &concurrent, &shred, &after, &other]),
    );
    let shredded = Some(&Verdict::Rejected(Rejection::Shredded(shred.id())));
    assert_eq!(state.verdict(entry.id()), ACCEPTED);
    assert_eq!(state.verdict(shred.id()), ACCEPTED);
    assert_eq!(state.verdict(concurrent.id()), shredded);
    assert_eq!(state.verdict(after.id()), shredded);
    assert_eq!(state.verdict(other.id()), ACCEPTED, "another person key");
    let key = state.person_key(person(1)).unwrap();
    assert_eq!(key.records, BTreeSet::from([record(&s, 1)]));
    assert_eq!(key.shred, Some(shred.id()));
}

#[test]
fn of_two_shreds_of_one_person_key_the_first_in_the_order_takes_effect() {
    let (mut s, held) = space();
    let entry = s.person_key(BOB, 1, 0xc1, &[&held[2]]);
    let by_bob = s.shred(BOB, 1, &other_account(1), &[&entry]);
    let by_alice = s.shred(ALICE, 1, &other_account(1), &[&entry]);
    let state = resolve(s.space, held.iter().chain([&entry, &by_bob, &by_alice]));
    assert_eq!(state.verdict(by_bob.id()), ACCEPTED);
    assert_eq!(state.verdict(by_alice.id()), ACCEPTED);
    // Alice, the creator, is senior.
    assert_eq!(
        state.person_key(person(1)).unwrap().shred,
        Some(by_alice.id())
    );
}

#[test]
fn the_person_keys_are_part_of_the_states_canonical_bytes() {
    let (mut s, held) = space();
    let entry = s.person_key(BOB, 1, 0xc1, &[&held[2]]);
    let shred = s.shred(BOB, 1, &other_account(1), &[&entry]);
    let before = resolve(s.space, held.iter().chain([&entry]));
    let after = resolve(s.space, held.iter().chain([&entry, &shred]));
    let reversed = resolve(
        s.space,
        [&shred, &entry].into_iter().chain(held.iter().rev()),
    );
    assert_ne!(before.encode(), after.encode());
    assert_eq!(after.encode(), reversed.encode());
}
