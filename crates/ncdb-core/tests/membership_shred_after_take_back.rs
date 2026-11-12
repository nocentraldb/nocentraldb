//! Of two accepted shreds of one person-key id, the first in the order that
//! is not taken back takes effect (RFC 0008, "The function"; the owner's
//! NOC-242 B, Carsten, 2026-11-12). After a provisional erasure is taken
//! back, a later shred of the same person key erases the person again,
//! provisionally, with its own decision; the shred taken back stays in the
//! state as an earlier shred of the key. Without a take-back, a later shred
//! changes nothing, as before. Every order of the same set gives the same
//! state.

#[path = "support/space.rs"]
mod space;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeMap;

use ncdb_core::cbor::Value;
use ncdb_core::change::Change;
use ncdb_core::membership::{
    ACKNOWLEDGEMENT, Entry, ErasureDecision, PersonKey, Rejection, Resolved, Role, Verdict,
    resolve,
};

use space::{Space, Who, account, person};

/// The space's creator, an admin of the default account.
const ALICE: Who = 1;
/// A writer of the default account, who performs the erasures.
const BOB: Who = 2;

/// A one-account space where Bob holds person 1's key and erases it
/// provisionally (shred X). Returns the space, every change so far and X.
fn erased() -> (Space, Vec<Change>, Change) {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let bob = s.add(ALICE, BOB, Role::Writer, &[&g]);
    let key = s.person_key(BOB, 1, 0xc1, &[&bob]);
    let x = s.shred(BOB, 1, &account(), &[&key]);
    (s, vec![g, bob, key, x.clone()], x)
}

fn decide(s: &mut Space, shred: &Change, finalize: bool, deps: &[&Change]) -> Change {
    let entry = Entry::ErasureDecision(ErasureDecision {
        shred: shred.id(),
        finalize,
    });
    s.write(ALICE, entry, deps)
}

fn key(state: &Resolved) -> &PersonKey {
    state.person_key(person(1)).expect("person 1's key")
}

fn with(changes: &[Change], more: &[&Change]) -> Vec<Change> {
    let mut all = changes.to_vec();
    all.extend(more.iter().map(|c| (*c).clone()));
    all
}

/// `last` resolved in every order after `base`; every order gives the same
/// state, which is returned.
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
fn after_a_take_back_a_later_shred_erases_again_provisionally() {
    let (mut s, changes, x) = erased();
    let take_back = decide(&mut s, &x, false, &[&x]);
    let y = s.shred(BOB, 1, &account(), &[&take_back]);

    let state = resolve(s.space, &with(&changes, &[&take_back, &y]));

    assert_eq!(state.verdict(y.id()), Some(&Verdict::Accepted));
    let key = key(&state);
    assert_eq!(key.shred, Some(y.id()));
    assert!(key.provisional);
    assert_eq!((key.finalized, key.taken_back), (None, None));
    assert!(key.erased());
    assert!(!key.destroyed());
    assert_eq!(key.earlier.len(), 1);
    assert_eq!(key.earlier[0].id, x.id());
    assert_eq!(key.earlier[0].taken_back, Some(take_back.id()));
}

#[test]
fn the_later_shred_is_finalized_by_its_own_decision() {
    let (mut s, changes, x) = erased();
    let take_back = decide(&mut s, &x, false, &[&x]);
    let y = s.shred(BOB, 1, &account(), &[&take_back]);
    let finalize = decide(&mut s, &y, true, &[&y]);

    let state = resolve(s.space, &with(&changes, &[&take_back, &y, &finalize]));

    assert_eq!(state.verdict(finalize.id()), Some(&Verdict::Accepted));
    let key = key(&state);
    assert_eq!(key.shred, Some(y.id()));
    assert_eq!(key.finalized, Some(finalize.id()));
    assert!(key.destroyed());
}

#[test]
fn without_a_take_back_a_later_shred_changes_nothing() {
    let (mut s, changes, x) = erased();
    let y = s.shred(BOB, 1, &account(), &[&x]);

    let state = resolve(s.space, &with(&changes, &[&y]));

    assert_eq!(state.verdict(y.id()), Some(&Verdict::Accepted));
    let key = key(&state);
    assert_eq!(key.shred, Some(x.id()));
    assert!(key.earlier.is_empty());
}

#[test]
fn a_later_shred_concurrent_with_the_take_back_takes_effect_in_every_order() {
    let (mut s, changes, x) = erased();
    let take_back = decide(&mut s, &x, false, &[&x]);
    let y = s.shred(BOB, 1, &account(), &[&x]);

    let state = in_every_order(&s, &changes, &[take_back.clone(), y.clone()]);

    let key = key(&state);
    assert_eq!(key.shred, Some(y.id()));
    assert!(key.erased());
    assert_eq!(key.earlier[0].id, x.id());
    assert_eq!(key.earlier[0].taken_back, Some(take_back.id()));
}

/// A finalize of X wins over the concurrent take-back, so X is never taken
/// back: it takes effect and is final, and the later shred changes nothing.
#[test]
fn a_finalize_that_wins_over_the_take_back_keeps_the_first_shred() {
    let (mut s, changes, x) = erased();
    let take_back = decide(&mut s, &x, false, &[&x]);
    let finalize = decide(&mut s, &x, true, &[&x]);
    let y = s.shred(BOB, 1, &account(), &[&take_back]);

    let state = in_every_order(&s, &changes, &[take_back.clone(), finalize.clone(), y]);

    assert_eq!(
        state.verdict(take_back.id()),
        Some(&Verdict::Rejected(Rejection::Finalized(finalize.id())))
    );
    let key = key(&state);
    assert_eq!(key.shred, Some(x.id()));
    assert_eq!(key.finalized, Some(finalize.id()));
    assert!(key.destroyed());
    assert!(key.earlier.is_empty());
}

#[test]
fn when_both_shreds_are_taken_back_the_person_reads_again() {
    let (mut s, changes, x) = erased();
    let first = decide(&mut s, &x, false, &[&x]);
    let y = s.shred(BOB, 1, &account(), &[&first]);
    let second = decide(&mut s, &y, false, &[&y]);

    let state = resolve(s.space, &with(&changes, &[&first, &y, &second]));

    let key = key(&state);
    assert_eq!(key.shred, Some(y.id()));
    assert_eq!(key.taken_back, Some(second.id()));
    assert!(!key.erased());
    assert_eq!(key.earlier[0].id, x.id());
}

/// An acknowledgement that names the shred taken back stays with it.
#[test]
fn an_acknowledgement_of_the_earlier_shred_stays_with_it() {
    let (mut s, changes, x) = erased();
    let seen = BTreeMap::from([
        (8, Value::Bytes(x.id().0.to_vec())),
        (9, Value::Bool(false)),
    ]);
    let ack = s.write(
        BOB,
        Entry::Other {
            entry_type: ACKNOWLEDGEMENT,
            fields: seen,
        },
        &[&x],
    );
    let take_back = decide(&mut s, &x, false, &[&ack]);
    let y = s.shred(BOB, 1, &account(), &[&take_back]);

    let state = resolve(s.space, &with(&changes, &[&ack, &take_back, &y]));

    let key = key(&state);
    assert!(key.acknowledged.is_empty(), "Y has no acknowledgement yet");
    assert_eq!(
        key.earlier[0].acknowledged.get(&space::id(BOB)),
        Some(&false)
    );
}
