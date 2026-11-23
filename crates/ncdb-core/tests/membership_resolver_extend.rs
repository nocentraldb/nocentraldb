//! A [`Resolver`] folds a person-key entry whose causal past is the set it
//! folded last as that fold plus one change (NOC-442). It must give what
//! RFC 0008's `resolve` gives, where step 1 folds every node's past from
//! scratch ([`Resolver::from_scratch`]), in every order of arrival: also
//! when the entry is shredded, repeats a person-key id under another
//! commitment, or comes from an author whose role a revocation lowered,
//! and when control changes fork and merge.

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

/// Every order of `items`, by Heap's algorithm.
fn orders<T: Clone>(items: &[T]) -> Vec<Vec<T>> {
    fn heap<T: Clone>(k: usize, items: &mut Vec<T>, out: &mut Vec<Vec<T>>) {
        if k <= 1 {
            out.push(items.clone());
            return;
        }
        for i in 0..k {
            heap(k - 1, items, out);
            let j = if k.is_multiple_of(2) { i } else { 0 };
            items.swap(j, k - 1);
        }
    }
    let mut out = Vec::new();
    heap(items.len(), &mut items.to_vec(), &mut out);
    out
}

/// Feeds one resolver the changes in every order of arrival, one more
/// each call, and checks each state against the fold from scratch.
/// Returns the state of all of them.
fn in_every_order(space: &Space, changes: &[Change]) -> Resolved {
    for order in orders(changes) {
        let mut resolver = Resolver::new(space.space);
        for held in 1..=order.len() {
            let held = &order[..held];
            let oracle = Resolver::from_scratch(space.space).resolve(held);
            assert_eq!(resolver.resolve(held), oracle, "after {} changes", held.len());
        }
    }
    let all = Resolver::new(space.space).resolve(changes);
    assert_eq!(all, Resolver::from_scratch(space.space).resolve(changes));
    all
}

#[test]
fn a_chain_of_persons_folds_as_resolve_folds_it_in_any_order() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let p1 = s.person_key(ALICE, 1, 1, &[&g]);
    let p2 = s.person_key(ALICE, 2, 2, &[&p1]);
    let p3 = s.person_key(ALICE, 3, 3, &[&p2]);
    let p4 = s.person_key(ALICE, 4, 4, &[&p3]);
    let p5 = s.person_key(ALICE, 5, 5, &[&p4]);
    let resolved = in_every_order(&s, &[g, p1, p2, p3, p4, p5]);
    assert_eq!(resolved.person_keys().count(), 5);
}

#[test]
fn a_person_key_after_a_shred_of_its_id_is_rejected_as_resolve_rejects_it() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    s.certify_by(CAROL, other_account(1));
    let carol = s.add(ALICE, CAROL, Role::Admin, &[&g]);
    let p1 = s.person_key(ALICE, 1, 1, &[&carol]);
    let shred = s.shred(ALICE, 1, &other_account(1), &[&p1]);
    let again = s.person_key(ALICE, 1, 1, &[&shred]);
    let p2 = s.person_key(ALICE, 2, 2, &[&again]);
    let changes = [g, carol, p1, shred, again.clone(), p2.clone()];
    let resolved = in_every_order(&s, &changes);
    assert_ne!(resolved.verdict(again.id()), Some(&Verdict::Accepted));
    assert_eq!(resolved.verdict(p2.id()), Some(&Verdict::Accepted));
}

#[test]
fn a_person_key_under_another_commitment_is_rejected_as_resolve_rejects_it() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let p1 = s.person_key(ALICE, 1, 1, &[&g]);
    // One entry after the first, which is not valid when written, and one
    // concurrent with it, which step 5 rejects for its commitment.
    let after = s.person_key(ALICE, 1, 2, &[&p1]);
    let concurrent = s.person_key(ALICE, 1, 3, &[&g]);
    let p2 = s.person_key(ALICE, 2, 2, &[&after, &concurrent]);
    let changes = [g, p1.clone(), after.clone(), concurrent.clone(), p2.clone()];
    let resolved = in_every_order(&s, &changes);
    let verdict = |change: &Change| resolved.verdict(change.id()).copied();
    assert_eq!(
        verdict(&after),
        Some(Verdict::Rejected(Rejection::NotValidWhenWritten))
    );
    // Of the two concurrent entries, the one with the lower change id
    // comes first in the order and is accepted.
    let (first, second) = match p1.id() < concurrent.id() {
        true => (&p1, &concurrent),
        false => (&concurrent, &p1),
    };
    assert_eq!(verdict(first), Some(Verdict::Accepted));
    assert_eq!(
        verdict(second),
        Some(Verdict::Rejected(Rejection::Commitment))
    );
    assert_eq!(verdict(&p2), Some(Verdict::Accepted));
}

#[test]
fn person_keys_of_an_author_a_revocation_lowered_fold_as_resolve_folds_them() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let bob = s.add(ALICE, BOB, Role::Writer, &[&g]);
    let b1 = s.person_key(BOB, 1, 1, &[&bob]);
    let lowered = s.set_role(ALICE, BOB, Role::Reader, s.sequence(BOB), &[&b1]);
    // Bob writes once without having seen the revocation, once after it.
    let unseen = s.person_key(BOB, 2, 2, &[&b1]);
    let seen = s.person_key(BOB, 3, 3, &[&lowered, &unseen]);
    let changes = [g, bob, b1, lowered, unseen.clone(), seen.clone()];
    let resolved = in_every_order(&s, &changes);
    assert_ne!(resolved.verdict(unseen.id()), Some(&Verdict::Accepted));
    assert_ne!(resolved.verdict(seen.id()), Some(&Verdict::Accepted));
}

#[test]
fn person_keys_that_fork_and_merge_fold_as_resolve_folds_them() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let bob = s.add(ALICE, BOB, Role::Writer, &[&g]);
    let p1 = s.person_key(ALICE, 1, 1, &[&bob]);
    let a = s.person_key(ALICE, 2, 2, &[&p1]);
    let b = s.person_key(BOB, 3, 3, &[&p1]);
    let merged = s.person_key(ALICE, 4, 4, &[&a, &b]);
    let resolved = in_every_order(&s, &[g, bob, p1, a, b, merged]);
    assert_eq!(resolved.person_keys().count(), 4);
}

#[test]
fn a_chain_that_arrives_in_order_extends_the_fold_and_skips_the_folds_from_scratch() {
    let mut s = Space::new(ALICE);
    let mut changes = vec![s.genesis.clone()];
    for person in 1..=50u8 {
        let last = changes.last().expect("the genesis").clone();
        changes.push(s.person_key(ALICE, person, person, &[&last]));
    }
    let mut resolver = Resolver::new(s.space);
    let mut oracle = Resolver::from_scratch(s.space);
    assert_eq!(resolver.resolve(&changes), oracle.resolve(&changes));
    assert!(resolver.folded() < 10, "{} nodes folded", resolver.folded());
    assert!(oracle.folded() > 50 * 49 / 2, "{} nodes folded", oracle.folded());
}
