//! Links (RFC 0004, "Links"): a link merges as a basic value of an object
//! id, links as a set of them, and each target resolves, when the app
//! reads, against the objects the copy holds: ok, deleted, missing or
//! wrong collection. The contract C1 to C7 is checked in `set_contract.rs`.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::{BTreeMap, BTreeSet};

use ncdb_core::basic::{self, Write};
use ncdb_core::cbor::Value;
use ncdb_core::clock::Hlc;
use ncdb_core::field::{FieldType, Outcome};
use ncdb_core::ids::{ChangeId, MemberId, ObjectId, OperationId, SpaceId};
use ncdb_core::link::{self, Link, Links, OtherSpace, Status, Target};
use ncdb_core::object::{Create, Object};
use ncdb_core::set;

const HERE: SpaceId = SpaceId([1; 16]);
const THERE: SpaceId = SpaceId([2; 16]);

fn op(n: u8) -> OperationId {
    OperationId {
        change: ChangeId([n; 32]),
        position: 0,
    }
}

fn member(n: u8) -> MemberId {
    MemberId([n; 32])
}

fn object(n: u8) -> ObjectId {
    ObjectId([n; 16])
}

fn here(n: u8) -> Target {
    Target {
        space: HERE,
        object: object(n),
    }
}

fn tasks() -> BTreeSet<String> {
    ["tasks".to_owned()].into()
}

fn depends_on() -> Links {
    Links {
        collections: tasks(),
    }
}

fn parent() -> Link {
    Link {
        collections: tasks(),
    }
}

fn create(id: OperationId, author: u8, stamp: u64, collection: &str) -> Create {
    Create {
        id,
        author: member(author),
        stamp: Hlc(stamp),
        collection: collection.to_owned(),
    }
}

fn deleted(id: OperationId, stamp: u64, value: bool, replaces: &[OperationId]) -> Write {
    Write {
        id,
        author: member(9),
        stamp: Hlc(stamp),
        value: Value::Bool(value),
        replaces: replaces.iter().copied().collect(),
    }
}

/// A copy that holds one object, made by `creates`.
fn copy_with(id: ObjectId, creates: &[Create]) -> BTreeMap<ObjectId, Object> {
    let mut object = Object::default();
    for c in creates {
        object.create(c);
    }
    [(id, object)].into()
}

/// A links field with one addition of each target, by member 1.
fn linked(targets: &[ObjectId]) -> set::State {
    let mut state = set::State::default();
    for (i, target) in targets.iter().enumerate() {
        let edit = depends_on()
            .add(
                &state,
                op(100 + i as u8),
                member(1),
                Hlc(10),
                HERE,
                Target {
                    space: HERE,
                    object: *target,
                },
            )
            .expect("a target in this space");
        depends_on().apply(&mut state, &edit);
    }
    state
}

fn statuses(links: &Links, state: &set::State, copy: &BTreeMap<ObjectId, Object>) -> Vec<Status> {
    links
        .resolve(state, copy)
        .iter()
        .map(|l| l.status)
        .collect()
}

#[test]
fn a_link_to_an_object_of_an_allowed_collection_is_ok() {
    let state = linked(&[object(7)]);
    let copy = copy_with(object(7), &[create(op(1), 1, 5, "tasks")]);
    let resolved = depends_on().resolve(&state, &copy);
    assert_eq!(resolved.len(), 1);
    assert_eq!(resolved[0].element.value, link::value(object(7)));
    assert!(resolved[0].element.known);
    assert_eq!(resolved[0].status, Status::Ok);
}

#[test]
fn a_link_to_an_object_in_another_collection_is_wrong_collection() {
    let state = linked(&[object(7)]);
    let copy = copy_with(object(7), &[create(op(1), 1, 5, "notes")]);
    assert_eq!(
        statuses(&depends_on(), &state, &copy),
        vec![Status::WrongCollection]
    );
}

#[test]
fn a_target_not_received_yet_reads_missing_until_its_create_arrives() {
    let state = linked(&[object(7)]);
    let bytes = depends_on().state_bytes(&state);
    let mut copy = BTreeMap::new();
    assert_eq!(
        statuses(&depends_on(), &state, &copy),
        vec![Status::Missing]
    );

    // The target's create arrives; the link itself does not change.
    copy.entry(object(7))
        .or_insert_with(Object::default)
        .create(&create(op(1), 1, 5, "tasks"));
    assert_eq!(statuses(&depends_on(), &state, &copy), vec![Status::Ok]);
    assert_eq!(depends_on().state_bytes(&state), bytes);
}

#[test]
fn an_object_whose_deletion_arrived_before_any_create_is_missing() {
    let state = linked(&[object(7)]);
    let mut target = Object::default();
    target.set_deleted(&deleted(op(2), 20, true, &[]));
    assert!(!target.exists());
    let copy = [(object(7), target)].into();
    assert_eq!(
        statuses(&depends_on(), &state, &copy),
        vec![Status::Missing]
    );
}

#[test]
fn a_deleted_target_keeps_its_link_and_reads_ok_again_when_restored() {
    let state = linked(&[object(7)]);
    let mut copy = copy_with(object(7), &[create(op(1), 1, 5, "tasks")]);
    let target = copy.get_mut(&object(7)).expect("the target");
    target.set_deleted(&deleted(op(2), 20, true, &[]));
    assert!(target.is_deleted());
    assert_eq!(
        statuses(&depends_on(), &state, &copy),
        vec![Status::Deleted]
    );
    assert_eq!(depends_on().read(&state).elements.len(), 1);

    let target = copy.get_mut(&object(7)).expect("the target");
    target.set_deleted(&deleted(op(3), 30, false, &[op(2)]));
    assert!(!target.is_deleted());
    assert_eq!(statuses(&depends_on(), &state, &copy), vec![Status::Ok]);
}

#[test]
fn a_deleted_target_in_another_collection_reads_deleted() {
    let state = linked(&[object(7)]);
    let mut copy = copy_with(object(7), &[create(op(1), 1, 5, "notes")]);
    copy.get_mut(&object(7))
        .expect("the target")
        .set_deleted(&deleted(op(2), 20, true, &[]));
    assert_eq!(
        statuses(&depends_on(), &state, &copy),
        vec![Status::Deleted]
    );
}

#[test]
fn two_creates_of_one_id_are_one_object_whose_collection_is_the_last_in_stamp_order() {
    let creates = [create(op(1), 1, 10, "tasks"), create(op(2), 2, 20, "notes")];
    let state = linked(&[object(7)]);
    for order in [[0, 1], [1, 0]] {
        let ordered: Vec<Create> = order.iter().map(|&i| creates[i].clone()).collect();
        let copy = copy_with(object(7), &ordered);
        let target = &copy[&object(7)];
        let collection = target.collection();
        assert_eq!(
            collection.value.expect("a collection").value,
            Value::Text("notes".to_owned())
        );
        // The create that named another collection is a conflict the app
        // can read; creates replace nothing.
        let conflicts: Vec<Value> = collection
            .conflicts
            .iter()
            .map(|c| c.value.clone())
            .collect();
        assert_eq!(conflicts, vec![Value::Text("tasks".to_owned())]);
        assert_eq!(
            statuses(&depends_on(), &state, &copy),
            vec![Status::WrongCollection]
        );
    }
}

#[test]
fn two_creates_of_one_id_in_one_collection_resolve_ok_without_a_conflict() {
    let copy = copy_with(
        object(7),
        &[create(op(1), 1, 10, "tasks"), create(op(2), 2, 20, "tasks")],
    );
    assert!(copy[&object(7)].collection().conflicts.is_empty());
    assert_eq!(
        statuses(&depends_on(), &linked(&[object(7)]), &copy),
        vec![Status::Ok]
    );
}

#[test]
fn a_create_neither_deletes_nor_restores() {
    let mut target = Object::default();
    target.create(&create(op(1), 1, 10, "tasks"));
    target.set_deleted(&deleted(op(2), 20, true, &[]));
    target.create(&create(op(3), 2, 30, "tasks"));
    assert!(target.is_deleted());
    let mut fresh = Object::default();
    fresh.set_deleted(&deleted(op(2), 20, true, &[]));
    fresh.create(&create(op(3), 2, 30, "tasks"));
    fresh.create(&create(op(1), 1, 10, "tasks"));
    assert_eq!(fresh, target, "the same changes in another order");
}

#[test]
fn the_writing_side_refuses_a_link_into_another_space() {
    let elsewhere = Target {
        space: THERE,
        object: object(7),
    };
    let state = set::State::default();
    assert_eq!(
        depends_on().add(&state, op(1), member(1), Hlc(1), HERE, elsewhere),
        Err(OtherSpace)
    );
    assert_eq!(
        parent().set(
            &basic::State::default(),
            op(1),
            member(1),
            Hlc(1),
            HERE,
            Some(elsewhere)
        ),
        Err(OtherSpace)
    );
}

#[test]
fn a_link_into_another_space_reads_missing_forever() {
    // A buggy or hostile member wrote a link to an object of another space.
    // The other space's copy holds it; this space's copy never will.
    let state = linked(&[object(7)]);
    let other_space = copy_with(object(7), &[create(op(1), 1, 5, "tasks")]);
    let this_space = copy_with(object(8), &[create(op(2), 1, 5, "tasks")]);
    assert_eq!(
        statuses(&depends_on(), &state, &other_space),
        vec![Status::Ok]
    );
    assert_eq!(
        statuses(&depends_on(), &state, &this_space),
        vec![Status::Missing]
    );
}

#[test]
fn links_merge_as_a_set_add_wins_with_each_target_resolved() {
    // Alice removes the dependency on 7 having seen Bob's addition; Carol
    // adds it again, offline. Add wins; the removal stays readable.
    let field = depends_on();
    let mut state = linked(&[object(7), object(8)]);
    let removal = depends_on().remove(&state, op(2), member(2), Hlc(20), object(7));
    let readd = depends_on()
        .add(
            &set::State::default(),
            op(3),
            member(3),
            Hlc(15),
            HERE,
            here(7),
        )
        .expect("a target in this space");
    field.apply(&mut state, &removal);
    field.apply(&mut state, &readd);
    let copy = copy_with(object(8), &[create(op(1), 1, 5, "tasks")]);
    let resolved = depends_on().resolve(&state, &copy);
    let read: Vec<(Value, usize, Status)> = resolved
        .iter()
        .map(|l| {
            (
                l.element.value.clone(),
                l.element.lost_removals.len(),
                l.status,
            )
        })
        .collect();
    assert_eq!(
        read,
        vec![
            (link::value(object(7)), 1, Status::Missing),
            (link::value(object(8)), 0, Status::Ok),
        ]
    );
    assert_eq!(depends_on().outcome(&state, op(2)), Some(Outcome::Lost));
}

#[test]
fn removing_a_link_into_another_space_is_allowed() {
    let mut state = set::State::default();
    let stray = set::Edit {
        id: op(1),
        author: member(1),
        stamp: Hlc(1),
        element: link::value(object(7)),
        action: set::Action::Add {
            replaces: BTreeSet::new(),
        },
    };
    depends_on().apply(&mut state, &stray);
    let removal = depends_on().remove(&state, op(2), member(2), Hlc(2), object(7));
    depends_on().apply(&mut state, &removal);
    assert!(depends_on().read(&state).elements.is_empty());
}

#[test]
fn a_link_merges_as_a_basic_value_with_the_winner_and_each_conflict_resolved() {
    let field = parent();
    let mut state = basic::State::default();
    let first = parent()
        .set(&state, op(1), member(1), Hlc(10), HERE, Some(here(7)))
        .expect("a target in this space");
    let second = parent()
        .set(&state, op(2), member(2), Hlc(20), HERE, Some(here(8)))
        .expect("a target in this space");
    field.apply(&mut state, &second);
    field.apply(&mut state, &first);
    let mut copy = copy_with(object(8), &[create(op(3), 1, 5, "tasks")]);
    copy.insert(
        object(7),
        copy_with(object(7), &[create(op(4), 1, 5, "notes")])[&object(7)].clone(),
    );
    let resolved = parent().resolve(&state, &copy);
    let winner = resolved.value.expect("a winner");
    assert_eq!(
        (winner.entry.value, winner.status),
        (link::value(object(8)), Status::Ok)
    );
    let conflicts: Vec<(Value, Status)> = resolved
        .conflicts
        .into_iter()
        .map(|c| (c.entry.value, c.status))
        .collect();
    assert_eq!(
        conflicts,
        vec![(link::value(object(7)), Status::WrongCollection)]
    );
    assert_eq!(parent().read(&state), field.read(&state));
}

#[test]
fn a_cleared_link_resolves_to_no_target() {
    let field = parent();
    let mut state = basic::State::default();
    let set = parent()
        .set(&state, op(1), member(1), Hlc(10), HERE, Some(here(7)))
        .expect("a target in this space");
    field.apply(&mut state, &set);
    let clear = parent()
        .set(&state, op(2), member(1), Hlc(20), HERE, None)
        .expect("a clear");
    assert_eq!(clear.value, Value::Null);
    assert_eq!(clear.replaces, [op(1)].into());
    field.apply(&mut state, &clear);
    let resolved = parent().resolve(&state, &BTreeMap::new());
    assert_eq!(resolved.value.map(|v| v.entry.value), Some(Value::Null));
}

#[test]
fn a_value_that_is_no_object_id_is_kept_unknown_and_reads_missing() {
    let not_an_id = Write {
        id: op(1),
        author: member(1),
        stamp: Hlc(10),
        value: Value::Bytes(vec![7; 15]),
        replaces: BTreeSet::new(),
    };
    let mut state = basic::State::default();
    parent().apply(&mut state, &not_an_id);
    let copy = copy_with(object(7), &[create(op(2), 1, 5, "tasks")]);
    let winner = parent().resolve(&state, &copy).value.expect("a winner");
    assert!(!winner.entry.known);
    assert_eq!(winner.status, Status::Missing);
}

#[test]
fn only_an_object_id_of_sixteen_bytes_reads_as_known() {
    let mut state = basic::State::default();
    let values = [
        link::value(object(7)),
        Value::Bytes(vec![7; 15]),
        Value::Bytes(vec![7; 17]),
        Value::Text("tasks/7".to_owned()),
    ];
    for (n, value) in values.into_iter().enumerate() {
        let w = Write {
            id: op(n as u8),
            author: member(n as u8),
            stamp: Hlc(10),
            value,
            replaces: BTreeSet::new(),
        };
        parent().apply(&mut state, &w);
    }
    let read = parent().read(&state);
    let mut known: Vec<(Value, bool)> = read
        .conflicts
        .into_iter()
        .chain(read.value)
        .map(|e| (e.value, e.known))
        .collect();
    known.sort_by_key(|(_, k)| *k);
    assert_eq!(known.iter().filter(|(_, k)| *k).count(), 1);
    assert_eq!(
        known.last().map(|(v, _)| v.clone()),
        Some(link::value(object(7)))
    );
}
