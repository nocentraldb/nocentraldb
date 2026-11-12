//! The acknowledgements of a shred in the control state (RFC 0008,
//! "Acknowledgement" and "The state"; NOC-238): per person key, every
//! member that acknowledged the key's shred, and whether it said "deleted".
//! An acknowledgement that names another change, or that the fold rejects,
//! adds nothing; a repeated one changes nothing. Every order of arrival
//! gives the same state.

#[path = "support/space.rs"]
mod space;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeMap;

use ncdb_core::cbor::Value;
use ncdb_core::change::Change;
use ncdb_core::ids::{ChangeId, MemberId, SpaceId};
use ncdb_core::membership::{ACKNOWLEDGEMENT, Entry, Resolved, Role, Verdict, resolve};

use space::{Space, Who, id, other_account, person};

const ALICE: u8 = 1;
const BOB: u8 = 2;
const CAROL: u8 = 3;

/// Alice creates the space, Bob is a writer, Carol an admin of a second
/// account; Bob writes person 1's key and shreds it under Carol's account.
fn shredded() -> (Space, Vec<Change>, Change) {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let bob = s.add(ALICE, BOB, Role::Writer, &[&g]);
    s.certify_by(CAROL, other_account(1));
    let carol = s.add(ALICE, CAROL, Role::Admin, &[&bob]);
    let entry = s.person_key(BOB, 1, 0xc1, &[&carol]);
    let shred = s.shred(BOB, 1, &other_account(1), &[&entry]);
    (s, vec![g, bob, carol, entry, shred.clone()], shred)
}

/// An acknowledgement by `who` of `shred`, after `deps`.
fn acknowledgement(
    s: &mut Space,
    who: Who,
    shred: ChangeId,
    deleted: bool,
    deps: &[&Change],
) -> Change {
    let fields = BTreeMap::from([
        (8, Value::Bytes(shred.0.to_vec())),
        (9, Value::Bool(deleted)),
    ]);
    s.write(
        who,
        Entry::Other {
            entry_type: ACKNOWLEDGEMENT,
            fields,
        },
        deps,
    )
}

fn acknowledged(state: &Resolved) -> BTreeMap<MemberId, bool> {
    state
        .person_key(person(1))
        .expect("the person key")
        .acknowledged
        .clone()
}

/// Every order of `items`.
fn permutations<T: Clone>(items: &[T]) -> Vec<Vec<T>> {
    if items.len() <= 1 {
        return vec![items.to_vec()];
    }
    let mut all = Vec::new();
    for i in 0..items.len() {
        let mut rest = items.to_vec();
        let first = rest.remove(i);
        for mut tail in permutations(&rest) {
            tail.insert(0, first.clone());
            all.push(tail);
        }
    }
    all
}

fn in_every_order(space: SpaceId, held: &[Change], late: &[Change]) -> Resolved {
    let first = resolve(space, held.iter().chain(late));
    for order in permutations(late) {
        let state = resolve(space, held.iter().chain(&order));
        assert_eq!(state.encode(), first.encode());
    }
    first
}

#[test]
fn a_shred_with_no_acknowledgement_lists_none() {
    let (s, changes, _) = shredded();
    assert!(acknowledged(&resolve(s.space, &changes)).is_empty());
}

#[test]
fn seen_and_deleted_are_listed_per_member_in_any_order() {
    let (mut s, changes, shred) = shredded();
    let seen = acknowledgement(&mut s, CAROL, shred.id(), false, &[&shred]);
    let deleted = acknowledgement(&mut s, CAROL, shred.id(), true, &[&seen]);
    let alice_seen = acknowledgement(&mut s, ALICE, shred.id(), false, &[&shred]);

    let state = in_every_order(s.space, &changes, &[seen, deleted, alice_seen]);

    assert_eq!(
        acknowledged(&state),
        BTreeMap::from([(id(ALICE), false), (id(CAROL), true)])
    );
}

#[test]
fn a_repeated_acknowledgement_changes_nothing() {
    let (mut s, changes, shred) = shredded();
    let deleted = acknowledgement(&mut s, CAROL, shred.id(), true, &[&shred]);
    let again = acknowledgement(&mut s, CAROL, shred.id(), false, &[&deleted]);

    let state = resolve(s.space, changes.iter().chain([&deleted, &again]));

    assert_eq!(acknowledged(&state), BTreeMap::from([(id(CAROL), true)]));
}

#[test]
fn an_acknowledgement_of_another_change_adds_nothing() {
    let (mut s, changes, shred) = shredded();
    let elsewhere = acknowledgement(&mut s, CAROL, changes[1].id(), true, &[&shred]);

    let state = resolve(s.space, changes.iter().chain([&elsewhere]));

    assert!(acknowledged(&state).is_empty());
}

/// A member removed with a cutoff below its acknowledgement has it
/// rejected like any of its changes: the audit shows it as never having
/// confirmed (RFC 0008, "Acknowledgement").
#[test]
fn a_rejected_acknowledgement_is_not_listed() {
    let (mut s, changes, shred) = shredded();
    let cutoff = s.sequence(CAROL);
    let removal = s.remove(ALICE, CAROL, cutoff, &[&shred]);
    let late = acknowledgement(&mut s, CAROL, shred.id(), true, &[&shred]);

    let state = resolve(s.space, changes.iter().chain([&removal, &late]));

    assert_ne!(state.verdict(late.id()), Some(&Verdict::Accepted));
    assert!(acknowledged(&state).is_empty());
}
