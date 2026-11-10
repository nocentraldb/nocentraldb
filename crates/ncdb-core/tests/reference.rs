//! Resolving a reference (RFC 0004 Amendment 4, "Resolution"): the state
//! is worked out when the app reads, from the member's own copies of the
//! spaces it holds, to ok, not accessible or missing (decisions 17 to 20).
//! It is never part of the field's state. Encoding is in
//! `field_op_reference.rs`, the merge in `reference_contract.rs`.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeMap;

use ncdb_core::basic::{State, Write};
use ncdb_core::cbor::Value;
use ncdb_core::clock::Hlc;
use ncdb_core::field::FieldType;
use ncdb_core::ids::{ChangeId, MemberId, ObjectId, OperationId, SpaceId};
use ncdb_core::object::{Create, Object};
use ncdb_core::reference::{self, Reference, Status, Target};

/// The space of the orders, which hold the references.
const ORDERS: SpaceId = SpaceId([1; 16]);
/// The space of the customers, the targets.
const CUSTOMERS: SpaceId = SpaceId([2; 16]);

fn op(n: u8) -> OperationId {
    OperationId {
        change: ChangeId([n; 32]),
        position: 0,
    }
}

fn object(n: u8) -> ObjectId {
    ObjectId([n; 16])
}

fn customer() -> Reference {
    Reference {
        collections: ["customers".to_owned()].into(),
    }
}

/// The member's copies: each space it holds, with its objects.
type Copies = BTreeMap<SpaceId, BTreeMap<ObjectId, Object>>;

fn created(collection: &str) -> Object {
    let mut object = Object::default();
    object.create(&Create {
        id: op(1),
        author: MemberId([1; 32]),
        stamp: Hlc(5),
        collection: collection.to_owned(),
    });
    object
}

fn deletion(n: u8, stamp: u64, value: bool, replaces: &[OperationId]) -> Write {
    Write {
        id: op(n),
        author: MemberId([9; 32]),
        stamp: Hlc(stamp),
        value: Value::Bool(value),
        replaces: replaces.iter().copied().collect(),
    }
}

/// A reference field written once, by member 1, with `value`.
fn written(value: Value) -> State {
    let mut state = State::default();
    let write = Write {
        id: op(50),
        author: MemberId([1; 32]),
        stamp: Hlc(10),
        value,
        replaces: Default::default(),
    };
    customer().apply(&mut state, &write);
    state
}

fn to(space: SpaceId, n: u8) -> State {
    written(reference::value(Target {
        space,
        object: object(n),
    }))
}

fn status(state: &State, copies: &Copies) -> Status {
    let resolved = customer().resolve(state, copies);
    resolved.value.expect("a live write").status
}

#[test]
fn a_target_the_member_holds_in_a_listed_collection_is_ok() {
    let copies: Copies = [(CUSTOMERS, [(object(7), created("customers"))].into())].into();
    let state = to(CUSTOMERS, 7);
    let resolved = customer().resolve(&state, &copies);
    let winner = resolved.value.expect("a live write");
    assert_eq!(winner.status, Status::Ok);
    assert_eq!(
        winner.entry.value,
        reference::value(Target {
            space: CUSTOMERS,
            object: object(7)
        })
    );
    assert!(resolved.conflicts.is_empty());
}

#[test]
fn a_target_in_a_space_the_member_holds_no_copy_of_is_not_accessible() {
    // Decision 20: the member never joined the customers' space, or left
    // it and its rows were deleted. It holds only the orders' space.
    let copies: Copies = [(ORDERS, BTreeMap::new())].into();
    assert_eq!(status(&to(CUSTOMERS, 7), &copies), Status::NotAccessible);
}

#[test]
fn a_target_the_copy_does_not_hold_is_missing_until_it_arrives() {
    let mut copies: Copies = [(CUSTOMERS, BTreeMap::new())].into();
    let state = to(CUSTOMERS, 7);
    let bytes = customer().state_bytes(&state);
    assert_eq!(status(&state, &copies), Status::Missing);

    // The target arrives; the reference itself does not change.
    copies
        .get_mut(&CUSTOMERS)
        .unwrap()
        .insert(object(7), created("customers"));
    assert_eq!(status(&state, &copies), Status::Ok);
    assert_eq!(customer().state_bytes(&state), bytes);
}

#[test]
fn a_deleted_target_is_missing_and_ok_again_when_restored() {
    let mut copies: Copies = [(CUSTOMERS, [(object(7), created("customers"))].into())].into();
    let state = to(CUSTOMERS, 7);
    let target = copies
        .get_mut(&CUSTOMERS)
        .unwrap()
        .get_mut(&object(7))
        .unwrap();
    target.set_deleted(&deletion(2, 20, true, &[]));
    assert_eq!(status(&state, &copies), Status::Missing);

    let target = copies
        .get_mut(&CUSTOMERS)
        .unwrap()
        .get_mut(&object(7))
        .unwrap();
    target.set_deleted(&deletion(3, 30, false, &[op(2)]));
    assert_eq!(status(&state, &copies), Status::Ok);
}

#[test]
fn an_object_whose_deletion_arrived_before_any_create_is_missing() {
    let mut target = Object::default();
    target.set_deleted(&deletion(2, 20, true, &[]));
    let copies: Copies = [(CUSTOMERS, [(object(7), target)].into())].into();
    assert_eq!(status(&to(CUSTOMERS, 7), &copies), Status::Missing);
}

#[test]
fn a_target_in_a_collection_the_schema_does_not_list_is_missing() {
    // Decision 18: no fourth state; an old client reads a target in a
    // collection added later as missing until it upgrades.
    let copies: Copies = [(CUSTOMERS, [(object(7), created("suppliers"))].into())].into();
    assert_eq!(status(&to(CUSTOMERS, 7), &copies), Status::Missing);
}

#[test]
fn a_reference_into_its_own_space_resolves_as_any_other() {
    // Decision 19: the member holds its own space, so ok or missing.
    let mut copies: Copies = [(ORDERS, BTreeMap::new())].into();
    let state = to(ORDERS, 7);
    assert_eq!(status(&state, &copies), Status::Missing);
    copies
        .get_mut(&ORDERS)
        .unwrap()
        .insert(object(7), created("customers"));
    assert_eq!(status(&state, &copies), Status::Ok);
}

#[test]
fn a_value_that_does_not_fit_is_missing_and_read_as_written() {
    // Decision 17: a bare object id names no object, even where the
    // member holds a space whose copy has an object with those bytes.
    let copies: Copies = [(CUSTOMERS, [(object(7), created("customers"))].into())].into();
    let bare = Value::Bytes(object(7).0.to_vec());
    let state = written(bare.clone());
    let winner = customer().resolve(&state, &copies).value.unwrap();
    assert_eq!(winner.status, Status::Missing);
    assert_eq!((winner.entry.value, winner.entry.known), (bare, false));
}

#[test]
fn a_cleared_reference_resolves_to_no_target() {
    let copies: Copies = BTreeMap::new();
    let state = written(Value::Null);
    let resolved = customer().resolve(&state, &copies);
    // The empty value is the winner's value, as for every basic value;
    // it names nothing, so it reads missing, as a cleared link does.
    let winner = resolved.value.unwrap();
    assert_eq!(
        (winner.entry.value, winner.status),
        (Value::Null, Status::Missing)
    );
}

#[test]
fn each_conflict_resolves_on_its_own() {
    let copies: Copies = [(CUSTOMERS, [(object(7), created("customers"))].into())].into();
    let mut state = State::default();
    for (n, space, target) in [(60, CUSTOMERS, 7), (61, SpaceId([3; 16]), 8)] {
        let write = Write {
            id: op(n),
            author: MemberId([n; 32]),
            stamp: Hlc(10),
            value: reference::value(Target {
                space,
                object: object(target),
            }),
            replaces: Default::default(),
        };
        customer().apply(&mut state, &write);
    }
    let resolved = customer().resolve(&state, &copies);
    // Member 61's write wins the tie; member 60's stays a conflict.
    assert_eq!(resolved.value.unwrap().status, Status::NotAccessible);
    let conflicts: Vec<Status> = resolved.conflicts.iter().map(|c| c.status).collect();
    assert_eq!(conflicts, vec![Status::Ok]);
}

#[test]
fn two_members_with_the_same_copies_compute_the_same_state() {
    let one: Copies = [(CUSTOMERS, [(object(7), created("customers"))].into())].into();
    let two = one.clone();
    let without: Copies = [(ORDERS, BTreeMap::new())].into();
    let state = to(CUSTOMERS, 7);
    assert_eq!(
        customer().resolve(&state, &one),
        customer().resolve(&state, &two)
    );
    // A member that is not in the customers' space reads the same
    // reference, with another state.
    assert_ne!(
        customer().resolve(&state, &one),
        customer().resolve(&state, &without)
    );
}
