//! [`Resolver::add`] (NOC-497): the state of the set of the last call with
//! more changes, without reading that set again. The relay keeps a space's
//! resolver between requests, and a request on a space of many persons
//! adds one person-key entry to it. `add` must give what `resolve` gives
//! for the whole set, where step 1 folds every node's past from scratch
//! ([`Resolver::from_scratch`]); what it cannot add that way, it refuses,
//! and the caller resolves the whole set.

#[path = "support/persons.rs"]
mod persons;
#[path = "support/space.rs"]
mod space;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::change::Change;
use ncdb_core::membership::{Rejection, Resolved, Resolver, Role, Verdict};

use space::{Space, other_account};

const ALICE: u8 = 1;
const BOB: u8 = 2;
const CAROL: u8 = 3;

/// The state `resolve` gives for `changes`, every past folded from
/// scratch.
fn oracle(space: &Space, changes: &[Change]) -> Resolved {
    Resolver::from_scratch(space.space).resolve(changes)
}

/// Resolves `before`, adds each of `after` one by one, and checks each
/// state against the oracle. Returns the last.
fn added_one_by_one(space: &Space, before: &[Change], after: &[Change]) -> Resolved {
    let mut resolver = Resolver::new(space.space);
    let mut held = before.to_vec();
    let mut resolved = resolver.resolve(&held);
    for change in after {
        held.push(change.clone());
        resolved = resolver.add([change]).expect("the resolver adds it");
        assert_eq!(
            resolved,
            oracle(space, &held),
            "after {} changes",
            held.len()
        );
    }
    resolved
}

#[test]
fn a_chain_of_person_keys_added_one_by_one_folds_as_resolve_folds_it() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let bob = s.add(ALICE, BOB, Role::Writer, &[&g]);
    let mut after: Vec<Change> = Vec::new();
    let mut head = bob.clone();
    for person in 1..=12u8 {
        let author = if person % 3 == 0 { BOB } else { ALICE };
        let change = s.person_key(author, person, person, &[&head]);
        after.push(change.clone());
        head = change;
    }
    let resolved = added_one_by_one(&s, &[g, bob], &after);
    assert_eq!(resolved.person_keys().count(), 12);
}

#[test]
fn several_person_keys_in_one_call_fold_as_resolve_folds_them() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let p1 = s.person_key(ALICE, 1, 1, &[&g]);
    let p2 = s.person_key(ALICE, 2, 2, &[&p1]);
    let p3 = s.person_key(ALICE, 3, 3, &[&p2]);
    let mut resolver = Resolver::new(s.space);
    resolver.resolve([&g]);
    let resolved = resolver
        .add([&p1, &p2, &p3])
        .expect("the resolver adds them");
    assert_eq!(resolved, oracle(&s, &[g, p1, p2, p3]));
}

#[test]
fn person_keys_that_resolve_rejects_are_added_as_it_rejects_them() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    s.certify_by(CAROL, other_account(1));
    let carol = s.add(ALICE, CAROL, Role::Admin, &[&g]);
    let p1 = s.person_key(ALICE, 1, 1, &[&carol]);
    let shred = s.shred(ALICE, 1, &other_account(1), &[&p1]);
    // The id of a shredded person again, a new person, and that person's
    // id again under another commitment.
    let again = s.person_key(ALICE, 1, 1, &[&shred]);
    let p2 = s.person_key(ALICE, 2, 2, &[&again]);
    let other = s.person_key(ALICE, 2, 3, &[&p2]);
    let before = [g, carol, p1, shred];
    let after = [again.clone(), p2.clone(), other.clone()];
    let resolved = added_one_by_one(&s, &before, &after);
    assert_ne!(resolved.verdict(again.id()), Some(&Verdict::Accepted));
    assert_eq!(resolved.verdict(p2.id()), Some(&Verdict::Accepted));
    assert_eq!(
        resolved.verdict(other.id()),
        Some(&Verdict::Rejected(Rejection::NotValidWhenWritten))
    );
}

#[test]
fn a_person_key_of_a_reader_is_added_as_resolve_rejects_it() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let bob = s.add(ALICE, BOB, Role::Reader, &[&g]);
    let by_reader = s.person_key(BOB, 1, 1, &[&bob]);
    let resolved = added_one_by_one(&s, &[g, bob], &[by_reader.clone()]);
    assert_ne!(resolved.verdict(by_reader.id()), Some(&Verdict::Accepted));
}

/// Resolves `before`, and checks that the resolver refuses to add
/// `change`, and that resolving the whole set then gives what `resolve`
/// gives.
fn refused(space: &Space, before: &[Change], change: &Change) {
    let mut resolver = Resolver::new(space.space);
    resolver.resolve(before);
    assert!(resolver.add([change]).is_none(), "the resolver refuses it");
    let mut all = before.to_vec();
    all.push(change.clone());
    assert_eq!(resolver.resolve(&all), oracle(space, &all));
}

#[test]
fn the_resolver_refuses_to_add_a_control_change_other_than_a_person_key_entry() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let p1 = s.person_key(ALICE, 1, 1, &[&g]);
    let bob = s.add(ALICE, BOB, Role::Writer, &[&p1]);
    refused(&s, &[g, p1], &bob);
}

#[test]
fn the_resolver_refuses_to_add_a_person_key_whose_past_is_not_the_whole_set() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let p1 = s.person_key(ALICE, 1, 1, &[&g]);
    let concurrent = s.person_key(ALICE, 2, 2, &[&g]);
    refused(&s, &[g, p1], &concurrent);
}

#[test]
fn the_resolver_refuses_to_add_a_person_key_after_a_change_it_does_not_hold() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let p1 = s.person_key(ALICE, 1, 1, &[&g]);
    let p2 = s.person_key(ALICE, 2, 2, &[&p1]);
    refused(&s, &[g], &p2);
}

#[test]
fn the_resolver_refuses_to_add_a_change_it_holds() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let p1 = s.person_key(ALICE, 1, 1, &[&g]);
    let mut resolver = Resolver::new(s.space);
    resolver.resolve([&g, &p1]);
    assert!(resolver.add([&p1]).is_none(), "the resolver refuses it");
}

#[test]
fn the_resolver_refuses_to_add_a_change_another_held_change_depends_on() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let p1 = s.person_key(ALICE, 1, 1, &[&g]);
    let p2 = s.person_key(ALICE, 2, 2, &[&p1]);
    // The set holds p2 without p1, a gap that p1 fills.
    refused(&s, &[g, p2], &p1);
}

#[test]
fn the_resolver_refuses_to_add_a_person_key_whose_signature_does_not_verify() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let p1 = s.person_key(ALICE, 1, 1, &[&g]);
    // The same header under another signature has the same change id.
    let mut bytes = p1.bytes().to_vec();
    let last = bytes.len() - 1;
    bytes[last] ^= 1;
    let forged = Change::decode(&bytes).expect("a change that decodes");
    assert_eq!(forged.id(), p1.id());
    let mut resolver = Resolver::new(s.space);
    resolver.resolve([&g, &p1]);
    resolver.resolve([&g]);
    assert!(resolver.add([&forged]).is_none(), "the resolver refuses it");
    assert_eq!(
        resolver.resolve([&g, &forged]),
        oracle(&s, &[g, forged.clone()])
    );
}

#[test]
fn after_a_refusal_and_a_resolve_the_resolver_adds_again() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let p1 = s.person_key(ALICE, 1, 1, &[&g]);
    let bob = s.add(ALICE, BOB, Role::Writer, &[&p1]);
    let p2 = s.person_key(BOB, 2, 2, &[&bob]);
    let mut resolver = Resolver::new(s.space);
    resolver.resolve([&g, &p1]);
    assert!(resolver.add([&bob]).is_none());
    resolver.resolve([&g, &p1, &bob]);
    let resolved = resolver.add([&p2]).expect("the resolver adds it");
    assert_eq!(resolved, oracle(&s, &[g, p1, bob, p2]));
}

#[test]
fn a_new_person_added_to_a_space_of_10k_persons_reads_only_itself() {
    const PERSONS: u32 = 10_000;
    let (mut space, changes) = persons::chain(PERSONS);
    let mut resolver = Resolver::new(space.space);
    resolver.resolve(&changes);
    assert_eq!(
        resolver.read(),
        changes.len(),
        "resolve reads the whole set"
    );
    let last = changes.last().expect("a change");
    let next = persons::next(&mut space, PERSONS, last);
    let resolved = resolver.add([&next]).expect("the resolver adds it");
    assert_eq!(resolver.read(), 1, "add reads only the new change");
    assert_eq!(resolver.judged(), 1);
    assert_eq!(resolver.folded(), 0);
    assert_eq!(resolved.person_keys().count(), PERSONS as usize + 1);
}
