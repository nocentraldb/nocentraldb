//! The hidden field `$person` (RFC 0005, "Where the person id is kept";
//! Carsten, 2026-11-12, NOC-244): every object a personal field names as
//! its person carries it, `{1: person id, 2: person-key id}`, merged as a
//! basic value. The view's model gives it to each collection whose objects
//! can be a person: one with a field personal to `"self"`, and each target
//! of a person link. A write of `$person` folds like any field.

#[path = "support/member.rs"]
mod member;
#[path = "support/view.rs"]
mod view_support;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeSet;

use ncdb_core::basic::State;
use ncdb_core::cbor::Value;
use ncdb_core::change::Payload;
use ncdb_core::clock::Hlc;
use ncdb_core::field::FieldType;
use ncdb_core::field_op;
use ncdb_core::ids::{ChangeId, ObjectId, OperationId};
use ncdb_core::model_lock::{ModelLock, Person as Whom};
use ncdb_core::personal::{PERSON_FIELD, Person, PersonField};
use ncdb_core::view::{Field, FieldState, Model, View};

use member::*;
use view_support::*;

const EXAMPLE: &str = include_str!("model_lock/rfc-0005-v2.lock.json");
const P: ObjectId = ObjectId([0x33; 16]);
const ANY: OperationId = OperationId {
    change: ChangeId([0; 32]),
    position: 0,
};

fn anna() -> Person {
    Person {
        person: [0x70; 16],
        person_key: [0x61; 16],
    }
}

#[test]
fn a_collection_whose_objects_can_be_a_person_has_the_hidden_field() {
    let model =
        Model::from_lock(&ModelLock::from_json(EXAMPLE).unwrap()).expect("the example is valid");
    // `people` has fields personal to "self", and `tasks.assignee`, the
    // person link of `notesAboutAssignee`, links to `people`.
    assert!(matches!(
        model.field("people", PERSON_FIELD),
        Some(Field::Person(_))
    ));
    assert_eq!(model.field("tasks", PERSON_FIELD), None);
}

#[test]
fn a_model_without_personal_fields_has_no_hidden_field() {
    assert_eq!(model().field(PROJECT, PERSON_FIELD), None);
    assert_eq!(model().field(TASK, PERSON_FIELD), None);
}

#[test]
fn the_field_takes_a_person_or_a_clear_and_nothing_else() {
    let field = PersonField;
    let state = State::default();
    let write = field.set(&state, ANY, member(0), Hlc(1), Some(anna()));
    assert_eq!(write.value, anna().to_value());
    let mut state = State::default();
    field.apply(&mut state, &write);
    let read = field.read(&state).value.unwrap();
    assert!(read.known);
    assert_eq!(Person::from_value(&read.value), Some(anna()));
    let clear = field.set(&state, ANY, member(0), Hlc(2), None);
    assert_eq!(clear.value, Value::Null);
}

#[test]
fn a_write_of_the_hidden_field_folds_into_the_view() {
    let mut lock = lock();
    lock.collections
        .get_mut(PROJECT)
        .unwrap()
        .fields
        .get_mut(NAME)
        .unwrap()
        .personal = Some(Whom::Itself);
    let db = MemDb::new();
    let mut view = View::open(&db.0, &lock).unwrap();
    let mut control = Control::default();
    control.keys.insert(0);
    control.members = (1..4).map(|n| (member(n), 0)).collect();
    let write = PersonField.set(&State::default(), ANY, member(0), Hlc(0), Some(anna()));
    let operations = vec![
        field_op::create(P, PROJECT),
        field_op::encode(&PersonField, &write, P, PROJECT, PERSON_FIELD),
    ];
    let payload = Payload::new(Hlc(1), BTreeSet::new(), operations).encode();
    view.receive(&frame(sealed_header(1, 1, 0, payload), GOOD), &mut control)
        .unwrap();
    let row = view.get(SPACE, PROJECT, P).unwrap().unwrap();
    let Some(FieldState::Basic(state)) = row.fields.get(PERSON_FIELD) else {
        panic!("the view holds $person");
    };
    let value = PersonField.read(state).value.unwrap().value;
    assert_eq!(Person::from_value(&value), Some(anna()));
}
