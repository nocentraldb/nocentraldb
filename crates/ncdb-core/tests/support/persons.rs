//! A space with many persons (NOC-442): a genesis and a chain of
//! person-key entries, each depending on the one before, as the SDK writes
//! them when an app creates one person after another. Person ids here are
//! numbers, so that a space can hold more than the 255 persons of
//! [`super::space::person`].
#![allow(dead_code)]

use ncdb_core::change::Change;
use ncdb_core::membership::{self, Entry, PersonKeyEntry};

use super::space::Space;

/// The person-key id of person `n`.
pub fn person(n: u32) -> [u8; 16] {
    let mut id = [0u8; 16];
    id[..4].copy_from_slice(&n.to_be_bytes());
    id
}

/// The person-key entry of person `n` that the creator writes after
/// `last`.
pub fn next(space: &mut Space, n: u32, last: &Change) -> Change {
    let entry = PersonKeyEntry {
        person_key: person(n),
        commitment: [1; 32],
        generation: space.first_generation,
        record_hash: membership::record_hash(&person(n)),
    };
    space.write(1, Entry::PersonKey(entry), &[last])
}

/// The genesis of a space that member 1 creates and the entries of `n`
/// persons after it.
pub fn chain(n: u32) -> (Space, Vec<Change>) {
    let mut space = Space::new(1);
    let mut changes = vec![space.genesis.clone()];
    for person in 0..n {
        let last = changes.last().expect("the genesis").clone();
        changes.push(next(&mut space, person, &last));
    }
    (space, changes)
}
