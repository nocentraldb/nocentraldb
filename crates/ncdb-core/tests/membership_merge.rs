//! The merge rotation of RFC 0008 ("Two removals that rotate the key at
//! once", steps 5 and 6): when the current generation is exposed, an admin
//! writes a key chain entry that lists every head as previous, with
//! envelopes for the current members only. Two merge rotations written at
//! the same time are concurrent rotations again (NOC-86).

#[path = "support/space.rs"]
mod space;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeSet;

use ncdb_core::change::Change;
use ncdb_core::membership::{self, Entry, MergeRotation, Resolved, Role, Verdict, resolve};

use space::{Space, Who, id};

const ALICE: u8 = 1;
const BOB: u8 = 2;
const CAROL: u8 = 3;
const XAVIER: u8 = 4;
const YVONNE: u8 = 5;

/// RFC 0008's example: Alice removes Xavier and rotates, while Bob removes
/// Yvonne and rotates; each new generation has an envelope for the member
/// the other removed. Carol is a writer throughout. Returns the space, its
/// changes and the two generations.
fn two_removals() -> (Space, Vec<Change>, u64, u64) {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let first = s.first_generation;
    let bob = s.add(ALICE, BOB, Role::Admin, &[&g]);
    let carol = s.add(ALICE, CAROL, Role::Writer, &[&bob]);
    let xavier = s.add(ALICE, XAVIER, Role::Writer, &[&carol]);
    let yvonne = s.add(ALICE, YVONNE, Role::Writer, &[&xavier]);
    let mut changes = vec![g, bob, carol, xavier, yvonne.clone()];

    let no_x = s.remove(ALICE, XAVIER, s.sequence(XAVIER), &[&yvonne]);
    let (by_alice, two) = s.rotate(ALICE, &[first], &[&no_x]);
    let mut last = by_alice.clone();
    changes.extend([no_x, by_alice]);
    for who in [ALICE, BOB, CAROL, YVONNE] {
        last = s.envelope_at(ALICE, who, two, &[&last]);
        changes.push(last.clone());
    }
    let no_y = s.remove(BOB, YVONNE, s.sequence(YVONNE), &[&yvonne]);
    let (by_bob, two_b) = s.rotate(BOB, &[first], &[&no_y]);
    let mut last = by_bob.clone();
    changes.extend([no_y, by_bob]);
    for who in [ALICE, BOB, CAROL, XAVIER] {
        last = s.envelope_at(BOB, who, two_b, &[&last]);
        changes.push(last.clone());
    }
    (s, changes, two, two_b)
}

/// The heads of `changes`: the changes no other names as a dependency.
fn heads(changes: &[Change]) -> Vec<&Change> {
    let named: BTreeSet<_> = changes
        .iter()
        .filter_map(|c| membership::ControlBody::decode(&c.header().payload).ok())
        .flat_map(|body| body.dependencies)
        .collect();
    changes
        .iter()
        .filter(|c| !named.contains(&c.id()))
        .collect()
}

/// `author` writes the merge rotation `state` asks of it after every head
/// of `changes`, and its envelopes: the changes and the new generation.
fn merge(s: &mut Space, author: Who, state: &Resolved, changes: &[Change]) -> (Vec<Change>, u64) {
    let rotation = state
        .merge_rotation(id(author))
        .expect("an exposed current generation");
    let key = space::space_key();
    let entry = rotation
        .entry(&key, |_| Some(&key))
        .expect("every simulated generation has the same key");
    let chain = s.write(author, Entry::KeyChain(entry), &heads(changes));
    let mut written = vec![chain];
    for recipient in &rotation.recipients {
        let who = (1..=YVONNE)
            .find(|w| id(*w) == *recipient)
            .expect("a member");
        let last = written.last().unwrap().clone();
        written.push(s.envelope_at(author, who, rotation.generation, &[&last]));
    }
    (written, rotation.generation)
}

#[test]
fn nothing_to_merge_while_the_current_generation_is_not_exposed() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let first = s.first_generation;
    let bob = s.add(ALICE, BOB, Role::Admin, &[&g]);
    let (rotation, two) = s.rotate(ALICE, &[first], &[&bob]);
    let to_bob = s.envelope_at(ALICE, BOB, two, &[&rotation]);
    let state = resolve(s.space, [&g, &bob, &rotation, &to_bob]);
    assert!(!state.exposed(first) && !state.exposed(two));
    assert_eq!(state.merge_rotation(id(ALICE)), None);
}

#[test]
fn an_exposed_current_generation_asks_an_admin_for_a_merge_rotation() {
    let (s, changes, two, two_b) = two_removals();
    let state = resolve(s.space, &changes);
    assert!(state.exposed(two));
    let rotation = state.merge_rotation(id(ALICE)).expect("Alice is an admin");
    let previous = vec![two.min(two_b), two.max(two_b)];
    assert_eq!(
        rotation,
        MergeRotation {
            space: s.space,
            generation: membership::generation_id(s.space, id(ALICE), &previous),
            previous: previous.clone(),
            recipients: BTreeSet::from([id(ALICE), id(BOB), id(CAROL)]),
        }
    );
    assert_eq!(rotation.generation >> 48, 3);
    let key = space::space_key();
    let entry = rotation
        .entry(&key, |_| Some(&key))
        .expect("every head's key");
    assert_eq!(entry.generation, rotation.generation);
    assert_eq!(entry.previous_generations().collect::<Vec<_>>(), previous);
    for p in &entry.previous {
        let opened =
            membership::open_previous_key(&key, s.space, rotation.generation, p.generation, &p.key);
        assert!(
            opened.is_some(),
            "NOC-97's layout, under the new generation"
        );
    }
    assert!(entry.answers.is_empty());
    // Bob's merge rotation derives another id; a writer writes none.
    let by_bob = state.merge_rotation(id(BOB)).expect("Bob is an admin");
    assert_ne!(by_bob.generation, rotation.generation);
    assert_eq!(state.merge_rotation(id(CAROL)), None);
    assert_eq!(state.merge_rotation(id(XAVIER)), None);
}

#[test]
fn a_merge_rotation_ends_the_exposure() {
    let (mut s, mut changes, two, two_b) = two_removals();
    let state = resolve(s.space, &changes);
    let (merged, three) = merge(&mut s, ALICE, &state, &changes);
    changes.extend(merged.iter().cloned());
    let state = resolve(s.space, &changes);
    for change in &merged {
        assert_eq!(state.verdict(change.id()), Some(&Verdict::Accepted));
    }
    assert_eq!(state.heads(), BTreeSet::from([three]));
    assert_eq!(state.current_generation(), Some(three));
    assert!(state.descends(three, two) && state.descends(three, two_b));
    // M-C9: no one outside the state may hold the current generation.
    let holders = &state.generation(three).unwrap().holders;
    assert_eq!(holders, &BTreeSet::from([id(ALICE), id(BOB), id(CAROL)]));
    assert!(!state.exposed(three));
    assert_eq!(state.merge_rotation(id(ALICE)), None);
    let sealed = s.sealed_under(CAROL, three, vec![]);
    assert!(state.admits(sealed.header()));
}

#[test]
fn two_merge_rotations_at_once_are_concurrent_rotations_again() {
    let (mut s, mut changes, ..) = two_removals();
    let state = resolve(s.space, &changes);
    let (by_alice, three) = merge(&mut s, ALICE, &state, &changes);
    let (by_bob, three_b) = merge(&mut s, BOB, &state, &changes);
    changes.extend(by_alice.into_iter().chain(by_bob));
    let state = resolve(s.space, &changes);
    assert_eq!(state.heads(), BTreeSet::from([three, three_b]));
    assert_eq!(state.current_generation(), Some(three), "Alice is senior");
    // Both went to the current members only, so there is nothing to merge.
    assert!(!state.exposed(three) && !state.exposed(three_b));
    assert_eq!(state.merge_rotation(id(ALICE)), None);
}

#[test]
fn a_member_that_left_is_answered_by_a_merge_rotation_with_one_head() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let first = s.first_generation;
    let bob = s.add(ALICE, BOB, Role::Writer, &[&g]);
    let to_bob = s.envelope(ALICE, BOB, &[&bob]);
    let leave = s.remove(BOB, BOB, s.sequence(BOB), &[&to_bob]);
    let mut changes = vec![g, bob, to_bob, leave];
    let state = resolve(s.space, &changes);
    let rotation = state.merge_rotation(id(ALICE)).expect("Bob left");
    assert_eq!(rotation.previous, vec![first]);
    assert_eq!(rotation.recipients, BTreeSet::from([id(ALICE)]));
    let (merged, two) = merge(&mut s, ALICE, &state, &changes);
    changes.extend(merged);
    let state = resolve(s.space, &changes);
    assert_eq!(state.current_generation(), Some(two));
    assert!(!state.exposed(two));
}
