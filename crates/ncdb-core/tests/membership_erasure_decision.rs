//! The provisional erasure in a space with one account, and the erasure
//! decision that ends it (RFC 0008, "Shred entry", "Erasure decision" and
//! "A provisional erasure in a space with one account"; Carsten,
//! 2026-11-11, NOC-228, NOC-230): the decision's bytes against the vectors
//! of tests/vectors/membership/, which generate.py writes with cbor2
//! independently of this crate; when the performer's own account may sign
//! off, and that the shred is then provisional; when a finalize and a
//! take-back are valid when written; that a finalize wins over every
//! concurrent take-back; and that only an accepted finalize, never the
//! provisional shred alone, destroys the person key.

#[path = "support/space.rs"]
mod space;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::change::Change;
use ncdb_core::ids::ChangeId;
use ncdb_core::membership::{
    ControlBody, ERASURE_DECISION, Entry, ErasureDecision, PersonKey, Rejection, Resolved, Role,
    Verdict, resolve,
};

use space::{Space, Who, account, other_account, person};

const FINALIZE: &str = include_str!("vectors/membership/erasure-decision-finalize.body.hex");
const TAKE_BACK: &str = include_str!("vectors/membership/erasure-decision-take-back.body.hex");

/// The space's creator, an admin of the default account.
const ALICE: Who = 1;
/// A writer of the default account, who performs the erasure.
const BOB: Who = 2;
/// A second admin of the default account.
const ERIN: Who = 5;
/// A member of another account.
const CAROL: Who = 3;

const ACCEPTED: Option<&Verdict> = Some(&Verdict::Accepted);
const NOT_VALID: Option<&Verdict> = Some(&Verdict::Rejected(Rejection::NotValidWhenWritten));

fn hex(text: &str) -> Vec<u8> {
    let text: String = text.split_whitespace().collect();
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

#[test]
fn the_decision_reads_and_writes_the_vectors_bytes() {
    let shred = ChangeId([0x5d; 32]);
    for (text, finalize) in [(FINALIZE, true), (TAKE_BACK, false)] {
        let bytes = hex(text);
        let body = ControlBody::decode(&bytes).expect("the vector decodes");
        assert_eq!(body.entry_type(), ERASURE_DECISION);
        assert_eq!(
            body.entry,
            Entry::ErasureDecision(ErasureDecision { shred, finalize })
        );
        assert_eq!(
            body.dependencies.iter().copied().collect::<Vec<_>>(),
            [shred]
        );
        assert_eq!(body.encode(), bytes, "the body encodes to the vector");
    }
}

/// A space whose members all belong to the default account: Alice, an
/// admin, Bob, a writer, and Erin, an admin; Bob holds person 1's key and
/// erases it, signed off by his own account. Returns the space, every
/// change so far, and the shred.
fn one_account() -> (Space, Vec<Change>, Change) {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let bob = s.add(ALICE, BOB, Role::Writer, &[&g]);
    let erin = s.add(ALICE, ERIN, Role::Admin, &[&bob]);
    let key = s.person_key(BOB, 1, 0xc1, &[&erin]);
    let shred = s.shred(BOB, 1, &account(), &[&key]);
    (s, vec![g, bob, erin, key, shred.clone()], shred)
}

fn decide(s: &mut Space, author: Who, shred: &Change, finalize: bool, deps: &[&Change]) -> Change {
    let entry = Entry::ErasureDecision(ErasureDecision {
        shred: shred.id(),
        finalize,
    });
    s.write(author, entry, deps)
}

fn key(state: &Resolved) -> &PersonKey {
    state.person_key(person(1)).expect("person 1's key")
}

/// `changes` resolved in every order of `last`, after `base`; every order
/// gives the same state, which is returned.
fn in_every_order(s: &Space, base: &[Change], last: &[Change]) -> Resolved {
    let mut orders: Vec<Vec<Change>> = vec![Vec::new()];
    for change in last {
        orders = orders
            .into_iter()
            .flat_map(|order| {
                (0..=order.len()).map(move |i| {
                    let mut next = order.clone();
                    next.insert(i, change.clone());
                    next
                })
            })
            .collect();
    }
    let states: Vec<Resolved> = orders
        .iter()
        .map(|order| resolve(s.space, base.iter().chain(order)))
        .collect();
    for state in &states[1..] {
        assert_eq!(state.encode(), states[0].encode(), "the copies differ");
    }
    states.into_iter().next().expect("one order")
}

#[test]
fn in_a_one_account_space_the_performers_own_account_signs_off_and_the_shred_is_provisional() {
    let (s, changes, shred) = one_account();
    let state = resolve(s.space, &changes);
    assert_eq!(state.verdict(shred.id()), ACCEPTED);
    let key = key(&state);
    assert_eq!(key.shred, Some(shred.id()));
    assert!(key.provisional);
    assert!(key.erased(), "the person shows as erased at once");
    assert!(!key.destroyed(), "nobody destroys the key yet");
}

#[test]
fn a_member_of_another_account_keeps_the_own_sign_off_invalid() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let bob = s.add(ALICE, BOB, Role::Writer, &[&g]);
    s.certify_by(CAROL, other_account(1));
    let carol = s.add(ALICE, CAROL, Role::Reader, &[&bob]);
    let key = s.person_key(BOB, 1, 0xc1, &[&carol]);
    let shred = s.shred(BOB, 1, &account(), &[&key]);
    let state = resolve(s.space, [&g, &bob, &carol, &key, &shred]);
    assert_eq!(state.verdict(shred.id()), NOT_VALID);
}

#[test]
fn a_shred_signed_off_by_another_account_is_final_at_once() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let bob = s.add(ALICE, BOB, Role::Writer, &[&g]);
    s.certify_by(CAROL, other_account(1));
    let carol = s.add(ALICE, CAROL, Role::Writer, &[&bob]);
    let key = s.person_key(BOB, 1, 0xc1, &[&carol]);
    let shred = s.shred(BOB, 1, &other_account(1), &[&key]);
    let state = resolve(s.space, [&g, &bob, &carol, &key, &shred]);
    assert_eq!(state.verdict(shred.id()), ACCEPTED);
    let key = self::key(&state);
    assert!(!key.provisional);
    assert!(key.erased() && key.destroyed());
}

#[test]
fn an_accepted_finalize_destroys_the_key() {
    let (mut s, mut changes, shred) = one_account();
    let finalize = decide(&mut s, ALICE, &shred, true, &[&shred]);
    changes.push(finalize.clone());
    let state = resolve(s.space, &changes);
    assert_eq!(state.verdict(finalize.id()), ACCEPTED);
    let key = key(&state);
    assert_eq!(key.finalized, Some(finalize.id()));
    assert!(key.erased() && key.destroyed());
}

#[test]
fn an_accepted_take_back_restores_the_person() {
    let (mut s, mut changes, shred) = one_account();
    let take_back = decide(&mut s, ERIN, &shred, false, &[&shred]);
    changes.push(take_back.clone());
    let state = resolve(s.space, &changes);
    assert_eq!(state.verdict(take_back.id()), ACCEPTED);
    let key = key(&state);
    assert_eq!(key.taken_back, Some(take_back.id()));
    assert!(!key.erased() && !key.destroyed());
}

#[test]
fn only_an_admin_of_the_performers_account_decides() {
    // Bob performed the erasure, but he is a writer.
    let (mut s, mut changes, shred) = one_account();
    let writer = decide(&mut s, BOB, &shred, true, &[&shred]);
    changes.push(writer.clone());
    assert_eq!(resolve(s.space, &changes).verdict(writer.id()), NOT_VALID);

    // An admin of an account that joined after the shred.
    let (mut s, mut changes, shred) = one_account();
    s.certify_by(CAROL, other_account(1));
    let carol = s.add(ALICE, CAROL, Role::Admin, &[&shred]);
    let stranger = decide(&mut s, CAROL, &shred, true, &[&carol]);
    changes.extend([carol, stranger.clone()]);
    let state = resolve(s.space, &changes);
    assert_eq!(state.verdict(stranger.id()), NOT_VALID);
    assert!(!key(&state).destroyed());
}

#[test]
fn a_decision_needs_a_provisional_shred_in_its_causal_past() {
    // A shred another account signed off is final, not provisional.
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let bob = s.add(ALICE, BOB, Role::Writer, &[&g]);
    s.certify_by(CAROL, other_account(1));
    let carol = s.add(ALICE, CAROL, Role::Writer, &[&bob]);
    let key = s.person_key(BOB, 1, 0xc1, &[&carol]);
    let shred = s.shred(BOB, 1, &other_account(1), &[&key]);
    let take_back = decide(&mut s, ALICE, &shred, false, &[&shred]);
    let state = resolve(s.space, [&g, &bob, &carol, &key, &shred, &take_back]);
    assert_eq!(state.verdict(take_back.id()), NOT_VALID);
    assert!(self::key(&state).destroyed());

    // A decision that does not hold the shred in its causal past.
    let (mut s, changes, shred) = one_account();
    let blind = decide(&mut s, ALICE, &shred, true, &[&changes[3]]);
    let state = resolve(s.space, changes.iter().chain([&blind]));
    assert_eq!(state.verdict(blind.id()), NOT_VALID);
    assert!(!self::key(&state).destroyed());
}

#[test]
fn a_decision_does_not_reverse_one_it_saw() {
    let (mut s, changes, shred) = one_account();
    let take_back = decide(&mut s, ALICE, &shred, false, &[&shred]);
    let finalize = decide(&mut s, ERIN, &shred, true, &[&take_back]);
    let state = resolve(s.space, changes.iter().chain([&take_back, &finalize]));
    assert_eq!(state.verdict(take_back.id()), ACCEPTED);
    assert_eq!(state.verdict(finalize.id()), NOT_VALID);
    assert!(!key(&state).erased());

    let (mut s, changes, shred) = one_account();
    let finalize = decide(&mut s, ALICE, &shred, true, &[&shred]);
    let take_back = decide(&mut s, ERIN, &shred, false, &[&finalize]);
    let state = resolve(s.space, changes.iter().chain([&finalize, &take_back]));
    assert_eq!(state.verdict(finalize.id()), ACCEPTED);
    assert_eq!(state.verdict(take_back.id()), NOT_VALID);
    assert!(key(&state).destroyed());
}

#[test]
fn a_finalize_wins_over_a_concurrent_take_back_in_every_order() {
    let (mut s, base, shred) = one_account();
    let finalize = decide(&mut s, ALICE, &shred, true, &[&shred]);
    let take_back = decide(&mut s, ERIN, &shred, false, &[&shred]);
    let state = in_every_order(&s, &base, &[take_back.clone(), finalize.clone()]);
    assert_eq!(state.verdict(finalize.id()), ACCEPTED);
    assert_eq!(
        state.verdict(take_back.id()),
        Some(&Verdict::Rejected(Rejection::Finalized(finalize.id())))
    );
    assert!(key(&state).destroyed());
}

#[test]
fn a_finalize_valid_when_written_stands_against_a_concurrent_removal_of_its_author() {
    let (mut s, base, shred) = one_account();
    let finalize = decide(&mut s, ERIN, &shred, true, &[&shred]);
    let remove = s.remove(ALICE, ERIN, s.sequence(ERIN) - 1, &[&shred]);
    let state = in_every_order(&s, &base, &[finalize.clone(), remove.clone()]);
    assert_eq!(state.verdict(remove.id()), ACCEPTED);
    assert_eq!(state.verdict(finalize.id()), ACCEPTED);
    assert!(key(&state).destroyed());
}
