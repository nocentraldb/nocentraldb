//! Opening a space with many persons costs time that grows with the
//! persons, not with their square (NOC-442). A copy that opens resolves
//! every control change of the space at once, and step 1 judges each node
//! from the fold of its causal past (RFC 0008, "The function"): folding
//! each past from scratch walks about half a million nodes at 1,000
//! persons. A [`Resolver`] folds a person-key entry whose past is the set
//! it folded last as that fold plus one change, so every node is walked a
//! bounded number of times.

#[path = "support/persons.rs"]
mod persons;
#[path = "support/space.rs"]
mod space;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::membership::{Resolver, Verdict};

/// The persons of a large space of nexflow.it's heavy users, who import an
/// address book (NOC-442).
const PERSONS: u32 = 10_000;

#[test]
fn opening_a_space_of_10k_persons_folds_each_control_change_a_bounded_number_of_times() {
    let (space, changes) = persons::chain(PERSONS);
    let mut resolver = Resolver::new(space.space);
    let resolved = resolver.resolve(&changes);
    assert_eq!(resolver.judged(), changes.len(), "the first call judges all");
    assert!(
        resolver.folded() <= 2 * changes.len(),
        "{} nodes folded for {} control changes",
        resolver.folded(),
        changes.len()
    );
    let accepted = resolved.verdicts().values();
    assert!(accepted.into_iter().all(|v| *v == Verdict::Accepted));
    assert_eq!(resolved.person_keys().count(), PERSONS as usize);
}

#[test]
fn a_new_person_in_a_space_of_10k_persons_folds_only_itself() {
    let (mut space, mut changes) = persons::chain(PERSONS);
    let mut resolver = Resolver::new(space.space);
    resolver.resolve(&changes);
    let last = changes.last().expect("a change").clone();
    changes.push(persons::next(&mut space, PERSONS, &last));
    let resolved = resolver.resolve(&changes);
    assert_eq!(resolver.judged(), 1);
    assert_eq!(resolver.folded(), 0, "the set extends the last fold");
    assert_eq!(resolved.person_keys().count(), PERSONS as usize + 1);
}
