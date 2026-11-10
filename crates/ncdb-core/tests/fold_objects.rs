//! The fold of creates and deletes (NOC-39; RFC 0004, "Two creates of one
//! object id" and "Creating and deleting an object"): a
//! received change's create puts the object into the copy with its
//! collection, a delete sets its deleted flag, and a link to it resolves
//! from both. Operations that do not decode park with their reason, field
//! operations included, since the fold reads them with `field_op`'s
//! decoders.

mod examples;
mod support;

#[path = "support/member.rs"]
mod member;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::{BTreeMap, BTreeSet};

use ncdb_core::basic::{self, BasicValue, Write};
use ncdb_core::cbor::{Key, Value};
use ncdb_core::change::{Operation, Payload};
use ncdb_core::clock::Hlc;
use ncdb_core::field::FieldType;
use ncdb_core::field_op::{self, Encoding, Origin};
use ncdb_core::ids::{ObjectId, OperationId};
use ncdb_core::link::{Link, Status, Target};
use ncdb_core::log::{OperationParkReason, ParkedOperation, State};
use ncdb_core::object::{Object, ObjectOperation};
use ncdb_core::receive::Fold;

use member::*;

/// The object the tests create.
const T: ObjectId = ObjectId([0x11; 16]);

/// A member that holds the key of generation 0.
fn with_key() -> Member {
    let mut m = Member::default();
    m.receive(&envelope(FOUNDER, 1, 0));
    m
}

/// A well-signed sealed change of the founder at `clock`.
fn at(sequence: u64, clock: u64, operations: Vec<Operation>) -> Vec<u8> {
    let payload = Payload::new(Hlc(clock), BTreeSet::new(), operations).encode();
    frame(sealed_header(FOUNDER, sequence, 0, payload), GOOD)
}

/// A delete of `T` that writes `deleted` and replaces `replaces`.
fn delete(deleted: bool, replaces: &[OperationId]) -> Operation {
    let write = Write {
        // The encoding takes only the value and `replaces`.
        id: OperationId {
            change: ncdb_core::ids::ChangeId([0; 32]),
            position: 0,
        },
        author: member(FOUNDER),
        stamp: Hlc(0),
        value: Value::Bool(deleted),
        replaces: replaces.iter().copied().collect(),
    };
    field_op::delete(T, COLLECTION, &write)
}

/// How a link field of `COLLECTION` that points at `T` resolves in `m`.
fn link_to_t(m: &Member) -> Status {
    let link = Link {
        collections: [COLLECTION.to_owned()].into(),
    };
    let mut state = basic::State::default();
    let id = OperationId {
        change: ncdb_core::ids::ChangeId([9; 32]),
        position: 0,
    };
    let target = Target {
        space: SPACE,
        object: T,
    };
    let write = link
        .set(&state, id, member(FOUNDER), Hlc(1), SPACE, Some(target))
        .unwrap();
    link.apply(&mut state, &write);
    link.resolve(&state, &m.fold.objects).value.unwrap().status
}

fn folded() -> State {
    State::Folded { parked: Vec::new() }
}

#[test]
fn a_create_puts_the_object_into_the_copy_with_its_collection() {
    let mut m = with_key();
    let change = m.receive(&at(2, 5, vec![field_op::create(T, COLLECTION)]));
    assert_eq!(m.state(&change), folded());

    let object = &m.fold.objects[&T];
    assert!(object.exists());
    assert!(!object.is_deleted());
    let collection = object.collection().value.unwrap();
    assert_eq!(collection.value, Value::Text(COLLECTION.to_owned()));
    // Its id, author and stamp come from the change.
    assert_eq!(collection.id, change.operation_id(0));
    assert_eq!(collection.author, member(FOUNDER));
    assert_eq!(collection.stamp, Hlc(5));
    // A create is no field operation.
    assert!(m.fold.folded.is_empty());
    assert_eq!(link_to_t(&m), Status::Ok);
}

#[test]
fn two_creates_of_one_id_are_one_object_whose_collection_is_a_basic_value() {
    let mut m = with_key();
    m.receive(&at(2, 5, vec![field_op::create(T, COLLECTION)]));
    let later = m.receive(&at(3, 6, vec![field_op::create(T, OTHER)]));
    assert_eq!(m.fold.objects.len(), 1);
    let read = m.fold.objects[&T].collection();
    let winner = read.value.unwrap();
    assert_eq!(winner.id, later.operation_id(0));
    assert_eq!(winner.value, Value::Text(OTHER.to_owned()));
    assert_eq!(read.conflicts.len(), 1);
    assert_eq!(link_to_t(&m), Status::WrongCollection);
}

#[test]
fn a_delete_sets_the_deleted_flag_and_a_link_to_the_object_reads_deleted() {
    let mut m = with_key();
    m.receive(&at(2, 5, vec![field_op::create(T, COLLECTION)]));
    let deletion = m.receive(&at(3, 6, vec![delete(true, &[])]));
    assert_eq!(m.state(&deletion), folded());
    assert!(m.fold.objects[&T].is_deleted());
    assert_eq!(link_to_t(&m), Status::Deleted);

    // A restore that replaces the delete makes the link read ok again.
    m.receive(&at(4, 7, vec![delete(false, &[deletion.operation_id(0)])]));
    assert!(!m.fold.objects[&T].is_deleted());
    assert_eq!(link_to_t(&m), Status::Ok);
}

#[test]
fn a_write_of_the_deletion_from_the_objects_state_replaces_the_deletions_the_copy_holds() {
    let mut m = with_key();
    m.receive(&at(2, 5, vec![field_op::create(T, COLLECTION)]));
    let deletion = m.receive(&at(3, 6, vec![delete(true, &[])]));

    // What the SDK writes for a delete or a restore (NOC-88).
    let deleted = BasicValue {
        kind: basic::Kind::Boolean,
    };
    let id = OperationId {
        change: ncdb_core::ids::ChangeId([0; 32]),
        position: 0,
    };
    let restore = deleted
        .set(
            m.fold.objects[&T].deletion(),
            id,
            member(FOUNDER),
            Hlc(7),
            Value::Bool(false),
        )
        .unwrap();
    assert_eq!(restore.replaces, BTreeSet::from([deletion.operation_id(0)]));
}

#[test]
fn a_delete_before_its_create_is_kept_and_applies_once_the_object_exists() {
    let mut m = with_key();
    m.receive(&at(2, 6, vec![delete(true, &[])]));
    assert!(!m.fold.objects[&T].exists());
    assert_eq!(link_to_t(&m), Status::Missing);

    m.receive(&at(3, 5, vec![field_op::create(T, COLLECTION)]));
    assert!(m.fold.objects[&T].is_deleted());
    assert_eq!(link_to_t(&m), Status::Deleted);
}

#[test]
fn an_object_operation_that_does_not_decode_parks_with_its_reason() {
    let mut m = with_key();
    let with_arguments = Operation::new(T, COLLECTION.into(), None, 0, Some(Value::Null));
    let unknown_number = Operation::new(T, COLLECTION.into(), None, 9, None);
    let mut unknown_argument = BTreeMap::new();
    unknown_argument.insert(Key::Unsigned(1), Value::Bool(true));
    unknown_argument.insert(Key::Unsigned(2), Value::Array(Vec::new()));
    unknown_argument.insert(Key::Unsigned(4), Value::Null);
    let unknown_argument = Operation::new(
        T,
        COLLECTION.into(),
        None,
        field_op::DELETE,
        Some(Value::Map(unknown_argument)),
    );
    let change = m.receive(&at(
        2,
        5,
        vec![
            with_arguments,
            unknown_number,
            unknown_argument,
            field_op::create(T, COLLECTION),
        ],
    ));

    let parked = |position, reason| ParkedOperation { position, reason };
    assert_eq!(
        m.state(&change),
        State::Folded {
            parked: vec![
                parked(0, OperationParkReason::MalformedArguments),
                parked(1, OperationParkReason::UnknownOperation),
                parked(2, OperationParkReason::UnknownArgument),
            ]
        }
    );
    // The rest of the change is folded.
    let object = &m.fold.objects[&T];
    assert!(object.exists());
    assert!(!object.is_deleted());
}

/// A model of one collection, `tasks`, with one field, `title`, a basic
/// value of text; it folds with `field_op`'s decoder.
#[derive(Default)]
struct Tasks {
    titles: BTreeMap<ObjectId, basic::State>,
    objects: BTreeMap<ObjectId, Object>,
}

impl Fold for Tasks {
    fn model_id(&self) -> [u8; 16] {
        [0; 16]
    }

    fn knows_collection(&self, collection: &str) -> bool {
        collection == "tasks"
    }

    fn knows_field(&self, collection: &str, field: &str) -> bool {
        collection == "tasks" && field == "title"
    }

    fn fold(&mut self, origin: Origin, operation: &Operation) -> Result<(), OperationParkReason> {
        let title = examples::title();
        let write = title.decode(operation, origin)?;
        title.apply(self.titles.entry(operation.object).or_default(), &write);
        Ok(())
    }

    fn fold_object(&mut self, object: ObjectId, operation: &ObjectOperation) {
        self.objects.entry(object).or_default().apply(operation);
    }

    fn clear_field(&mut self, object: ObjectId, _collection: &str, _field: &str) {
        self.titles.remove(&object);
    }

    fn clear_object(&mut self, object: ObjectId) {
        self.objects.remove(&object);
    }
}

#[test]
fn a_field_operation_that_does_not_decode_parks_with_its_reason() {
    let mut log = ncdb_core::log::Log::new(SPACE);
    let mut control = Control::default();
    let mut tasks = Tasks::default();
    log.receive(&envelope(FOUNDER, 1, 0), &mut control, &mut tasks)
        .unwrap();
    // E1 sets the title of T1, E10 is E1 with number 9.
    let e1 = examples::operation(examples::E1);
    let e10 = examples::operation(examples::E10);
    let bytes = at(2, 5, vec![e10, e1]);
    log.receive(&bytes, &mut control, &mut tasks).unwrap();

    let change = ncdb_core::change::Change::decode(&bytes).unwrap();
    assert_eq!(
        log.state(change.id()),
        Some(&State::Folded {
            parked: vec![ParkedOperation {
                position: 0,
                reason: OperationParkReason::UnknownOperation
            }]
        })
    );
    let read = BasicValue {
        kind: basic::Kind::Text,
    }
    .read(&tasks.titles[&examples::T1]);
    let title = read.value.unwrap();
    assert_eq!(title.value, Value::Text("Ship v1".to_owned()));
    assert_eq!(title.id, change.operation_id(1));
    assert_eq!(title.author, member(FOUNDER));
    assert_eq!(title.stamp, Hlc(5));
}
