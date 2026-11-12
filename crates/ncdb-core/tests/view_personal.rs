//! The fold of a personal field (RFC 0006, "What the view stores in
//! plaintext"; RFC 0009, "How a personal field reads"; Carsten,
//! 2026-11-12, NOC-244): a set carries the value sealed under its person's
//! key in argument 1. The merge state keeps it sealed. The field's column,
//! which reads and indexes use, holds the opened value only while the view
//! is given the person key; without it, the field reads as unset, and it
//! reads again once the key arrives. Taking the key away, as an erasure
//! does, empties the column at once.

#[path = "support/member.rs"]
mod member;
#[path = "support/view.rs"]
mod view_support;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::{BTreeMap, BTreeSet};

use ncdb_core::basic::{BasicValue, Kind, Write};
use ncdb_core::cbor::Value;
use ncdb_core::change::{Operation, Payload};
use ncdb_core::clock::Hlc;
use ncdb_core::field::FieldType;
use ncdb_core::field_op;
use ncdb_core::ids::{ChangeId, ObjectId, OperationId};
use ncdb_core::keys::PersonKey;
use ncdb_core::model_lock::{ModelLock, Person};
use ncdb_core::personal::{self, Place, Sealed};
use ncdb_core::view::{FieldState, View};
use rusqlite::types::Value as Sql;

use member::*;
use view_support::*;

const P: ObjectId = ObjectId([0x33; 16]);
const ANNA: [u8; 16] = [0x61; 16];
const BEN: [u8; 16] = [0x62; 16];

const ANY: OperationId = OperationId {
    change: ChangeId([0; 32]),
    position: 0,
};

/// The tests' lock with the projects' `name` personal to the project
/// itself; `byName` indexes it.
fn personal_lock() -> ModelLock {
    let mut lock = lock();
    let name = lock
        .collections
        .get_mut(PROJECT)
        .unwrap()
        .fields
        .get_mut(NAME)
        .unwrap();
    name.personal = Some(Person::Itself);
    lock
}

fn key(person: [u8; 16]) -> PersonKey {
    PersonKey::from_bytes([person[0]; 32])
}

fn keys(people: &[[u8; 16]]) -> BTreeMap<[u8; 16], PersonKey> {
    people.iter().map(|p| (*p, key(*p))).collect()
}

fn place(person: [u8; 16]) -> Place<'static> {
    Place {
        space: SPACE,
        object: P,
        person_key: person,
        collection: PROJECT,
        field: NAME,
    }
}

/// A set of `name` whose argument 1 is `value` sealed under `person`'s key.
fn sealed_name(person: [u8; 16], value: Value) -> (Operation, Value) {
    let sealed = Sealed {
        person_key: person,
        bytes: personal::seal(&key(person), &place(person), &value),
    }
    .to_value();
    (set_name(sealed.clone()), sealed)
}

fn set_name(value: Value) -> Operation {
    let write = Write {
        id: ANY,
        author: member(0),
        stamp: Hlc(0),
        value,
        replaces: BTreeSet::new(),
    };
    field_op::encode(&BasicValue { kind: Kind::Text }, &write, P, PROJECT, NAME)
}

fn change(sequence: u64, clock: u64, operations: Vec<Operation>) -> Vec<u8> {
    let payload = Payload::new(Hlc(clock), BTreeSet::new(), operations).encode();
    frame(sealed_header(1, sequence, 0, payload), GOOD)
}

struct Copy {
    db: MemDb,
    view: View,
    control: Control,
}

impl Copy {
    fn new() -> Copy {
        let mut control = Control::default();
        control.keys.insert(0);
        control.members = (1..4).map(|n| (member(n), 0)).collect();
        let db = MemDb::new();
        Copy {
            view: View::open(&db.0, &personal_lock()).unwrap(),
            db,
            control,
        }
    }

    fn receive(&mut self, operations: Vec<Operation>, sequence: u64) {
        let frame = change(sequence, sequence, operations);
        self.view.receive(&frame, &mut self.control).unwrap();
    }

    fn name(&self) -> Sql {
        self.db
            .connect()
            .query_row(
                "SELECT f_name FROM c_project WHERE space_id = ?1 AND object_id = ?2",
                (SPACE.0, P.0),
                |r| r.get(0),
            )
            .unwrap()
    }

    /// The value of the winning write in the merge state.
    fn state(&self) -> Value {
        let row = self.view.get(SPACE, PROJECT, P).unwrap().unwrap();
        let Some(FieldState::Basic(state)) = row.fields.get(NAME) else {
            panic!("name is a basic value");
        };
        let text = BasicValue { kind: Kind::Text };
        text.read(state).value.expect("a winning write").value
    }
}

fn anna() -> Value {
    Value::Text("Anna Schmidt".into())
}

#[test]
fn with_the_key_the_column_holds_the_opened_value_and_the_state_stays_sealed() {
    let mut copy = Copy::new();
    copy.view.set_person_keys(SPACE, keys(&[ANNA])).unwrap();
    let (set, sealed) = sealed_name(ANNA, anna());
    copy.receive(vec![field_op::create(P, PROJECT), set], 1);
    assert_eq!(copy.name(), Sql::Text("Anna Schmidt".into()));
    assert_eq!(copy.state(), sealed, "the merge state keeps it sealed");
}

#[test]
fn without_the_key_it_reads_as_unset_and_reads_again_once_the_key_arrives() {
    let mut copy = Copy::new();
    let (set, _) = sealed_name(ANNA, anna());
    copy.receive(vec![field_op::create(P, PROJECT), set], 1);
    assert_eq!(copy.name(), Sql::Null);
    copy.view.set_person_keys(SPACE, keys(&[ANNA])).unwrap();
    assert_eq!(copy.name(), Sql::Text("Anna Schmidt".into()));
}

#[test]
fn taking_the_key_away_empties_the_column_at_once() {
    let mut copy = Copy::new();
    copy.view.set_person_keys(SPACE, keys(&[ANNA])).unwrap();
    let (set, _) = sealed_name(ANNA, anna());
    copy.receive(vec![field_op::create(P, PROJECT), set], 1);
    copy.view.set_person_keys(SPACE, keys(&[])).unwrap();
    assert_eq!(copy.name(), Sql::Null);
}

#[test]
fn a_value_under_another_persons_key_reads_as_unset() {
    let mut copy = Copy::new();
    copy.view.set_person_keys(SPACE, keys(&[ANNA])).unwrap();
    let (set, _) = sealed_name(BEN, anna());
    copy.receive(vec![field_op::create(P, PROJECT), set], 1);
    assert_eq!(copy.name(), Sql::Null);
}

#[test]
fn a_value_that_does_not_open_or_does_not_fit_reads_as_unset() {
    let mut copy = Copy::new();
    copy.view.set_person_keys(SPACE, keys(&[ANNA])).unwrap();
    // Sealed at another field: it does not open here.
    let mut elsewhere = place(ANNA);
    elsewhere.field = "email";
    let moved = Sealed {
        person_key: ANNA,
        bytes: personal::seal(&key(ANNA), &elsewhere, &anna()),
    };
    copy.receive(
        vec![field_op::create(P, PROJECT), set_name(moved.to_value())],
        1,
    );
    assert_eq!(copy.name(), Sql::Null);
    // An integer in a text field.
    let (set, _) = sealed_name(ANNA, Value::Unsigned(7));
    copy.receive(vec![set], 2);
    assert_eq!(copy.name(), Sql::Null);
}

#[test]
fn a_clear_is_a_plain_null() {
    let mut copy = Copy::new();
    copy.view.set_person_keys(SPACE, keys(&[ANNA])).unwrap();
    let (set, _) = sealed_name(ANNA, anna());
    copy.receive(vec![field_op::create(P, PROJECT), set], 1);
    copy.receive(vec![set_name(Value::Null)], 2);
    assert_eq!(copy.name(), Sql::Null);
}

#[test]
fn keys_of_one_space_do_not_open_another() {
    let mut copy = Copy::new();
    let other = ncdb_core::ids::SpaceId([9; 16]);
    copy.view.set_person_keys(other, keys(&[ANNA])).unwrap();
    let (set, _) = sealed_name(ANNA, anna());
    copy.receive(vec![field_op::create(P, PROJECT), set], 1);
    assert_eq!(copy.name(), Sql::Null);
}
