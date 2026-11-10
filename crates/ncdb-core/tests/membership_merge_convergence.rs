//! RFC 0008's M-C1 and M-C9 for the current generation, the exposure rule
//! and merge rotations (NOC-86): two removals that rotate at once, and the
//! merge rotations two admins write when they find the current generation
//! exposed, end in the same current generation, the same exposure and the
//! same state on every copy, whatever order the changes arrive in.

#[path = "support/space.rs"]
mod space;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeSet;

use ncdb_core::change::Change;
use ncdb_core::ids::SpaceId;
use ncdb_core::membership::{ControlBody, Entry, Resolved, Role, resolve};

use space::{Space, Who, id};

const ALICE: u8 = 1;
const BOB: u8 = 2;
const CAROL: u8 = 3;
const XAVIER: u8 = 4;
const YVONNE: u8 = 5;

/// Orders of arrival tried per set.
const ORDERS: u64 = 64;

/// The members of the space, in the order `two_removals` adds them.
const MEMBERS: [Who; 5] = [ALICE, BOB, CAROL, XAVIER, YVONNE];

/// `items` in a pseudo-random order that `seed` fixes, so that native and
/// wasm32 runs try the same orders.
fn shuffled<T: Clone>(items: &[T], seed: u64) -> Vec<T> {
    let mut items = items.to_vec();
    let mut state = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
    for i in (1..items.len()).rev() {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let j = usize::try_from(state % (i as u64 + 1)).expect("a small index");
        items.swap(i, j);
    }
    items
}

/// The changes no other change in `changes` names as a dependency.
fn heads(changes: &[Change]) -> Vec<&Change> {
    let named: BTreeSet<_> = changes
        .iter()
        .filter_map(|c| ControlBody::decode(&c.header().payload).ok())
        .flat_map(|body| body.dependencies)
        .collect();
    changes
        .iter()
        .filter(|c| !named.contains(&c.id()))
        .collect()
}

/// `author` removes `subject`, rotates and wraps the new generation to
/// `recipients`, after `after`.
fn remove_and_rotate(
    s: &mut Space,
    author: Who,
    subject: Who,
    recipients: &[Who],
    after: &Change,
) -> Vec<Change> {
    let first = s.first_generation;
    let remove = s.remove(author, subject, s.sequence(subject), &[after]);
    let (rotation, generation) = s.rotate(author, &[first], &[&remove]);
    let mut written = vec![remove, rotation];
    for who in recipients {
        let last = written.last().unwrap().clone();
        written.push(s.envelope_at(author, *who, generation, &[&last]));
    }
    written
}

/// `author` writes the merge rotation `state` asks of it, after every head
/// of `seen`, and its envelopes.
fn merge(s: &mut Space, author: Who, state: &Resolved, seen: &[Change]) -> Vec<Change> {
    let rotation = state
        .merge_rotation(id(author))
        .expect("an exposed current generation");
    let key = space::space_key();
    let chain = s.write(
        author,
        Entry::KeyChain(
            rotation
                .entry(&key, |_| Some(&key))
                .expect("every simulated generation has the same key"),
        ),
        &heads(seen),
    );
    let mut written = vec![chain];
    for recipient in &rotation.recipients {
        let who = *MEMBERS
            .iter()
            .find(|w| id(**w) == *recipient)
            .expect("a member");
        let last = written.last().unwrap().clone();
        written.push(s.envelope_at(author, who, rotation.generation, &[&last]));
    }
    written
}

/// The state of `base` and `changes`, with `changes` arriving in many
/// orders: one state and one current generation, or the test fails.
fn in_any_order(space: SpaceId, base: &[Change], changes: &[Change]) -> Resolved {
    let first = resolve(space, base.iter().chain(changes));
    for seed in 0..ORDERS {
        let order = shuffled(changes, seed);
        let state = resolve(space, base.iter().chain(&order));
        assert_eq!(state.current_generation(), first.current_generation());
        assert_eq!(state.heads(), first.heads());
        assert_eq!(state.encode(), first.encode());
    }
    first
}

/// The two islands of RFC 0008's example, and the merge rotations Alice
/// and Bob write at once once they meet: the space, the base every copy
/// holds, each island's changes and the two merges.
struct Example {
    s: Space,
    base: Vec<Change>,
    islands: Vec<Change>,
    merges: Vec<Change>,
}

fn example() -> Example {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let mut base = vec![g];
    for who in [BOB, CAROL, XAVIER, YVONNE] {
        let role = if who == BOB {
            Role::Admin
        } else {
            Role::Writer
        };
        let last = base.last().unwrap().clone();
        base.push(s.add(ALICE, who, role, &[&last]));
    }
    let last = base.last().unwrap().clone();
    let mut islands = remove_and_rotate(&mut s, ALICE, XAVIER, &[ALICE, BOB, CAROL, YVONNE], &last);
    islands.extend(remove_and_rotate(
        &mut s,
        BOB,
        YVONNE,
        &[ALICE, BOB, CAROL, XAVIER],
        &last,
    ));
    let met: Vec<Change> = base.iter().chain(&islands).cloned().collect();
    let state = resolve(s.space, &met);
    let mut merges = merge(&mut s, ALICE, &state, &met);
    merges.extend(merge(&mut s, BOB, &state, &met));
    Example {
        s,
        base,
        islands,
        merges,
    }
}

#[test]
fn concurrent_rotations_end_in_one_current_generation_in_any_order() {
    let e = example();
    let state = in_any_order(e.s.space, &e.base, &e.islands);
    let current = state.current_generation().expect("a genesis");
    assert_eq!(state.heads().len(), 2);
    assert!(state.exposed(current));
}

#[test]
fn merge_rotations_end_in_one_unexposed_current_generation_in_any_order() {
    let e = example();
    let changes: Vec<Change> = e.islands.iter().chain(&e.merges).cloned().collect();
    let state = in_any_order(e.s.space, &e.base, &changes);
    let current = state.current_generation().expect("a genesis");
    assert_eq!(current >> 48, 3, "a merge rotation");
    // M-C9: once a merge rotation is accepted, no one outside the state
    // may hold the current generation.
    assert!(!state.exposed(current));
    let holders = &state.generation(current).unwrap().holders;
    assert_eq!(holders, &BTreeSet::from([id(ALICE), id(BOB), id(CAROL)]));
    assert_eq!(state.merge_rotation(id(ALICE)), None);
}

/// The check fires: under a broken rule that takes the generation of the
/// key chain entry that arrived last, the orders tried here end in
/// different current generations.
#[test]
fn the_last_rotation_to_arrive_would_diverge() {
    let e = example();
    let changes: Vec<Change> = e.islands.iter().chain(&e.merges).cloned().collect();
    let last_arrived = |order: &[Change]| {
        order
            .iter()
            .rev()
            .find_map(|c| match ControlBody::decode(&c.header().payload) {
                Ok(ControlBody {
                    entry: Entry::KeyChain(chain),
                    ..
                }) => Some(chain.generation),
                _ => None,
            })
    };
    let outcomes: BTreeSet<Option<u64>> = (0..ORDERS)
        .map(|seed| last_arrived(&shuffled(&changes, seed)))
        .collect();
    assert!(outcomes.len() > 1, "the broken rule diverges");
}
