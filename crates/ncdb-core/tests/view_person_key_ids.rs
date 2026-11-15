//! A person's person-key ids in a space, by person id (NOC-325; RFC 0003,
//! "The person key": an erasure "finds the person's keys by person id and
//! shreds them in every space the performing account can reach"; pm,
//! NOC-325, on the coding agent's proposal). The person id and the key's
//! id are kept in the hidden field `$person` of each object of the person
//! (RFC 0005, "Where the person id is kept"); the view gives the ids of
//! the keys every live `$person` of a space names for that person id.

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
use ncdb_core::change::Payload;
use ncdb_core::clock::Hlc;
use ncdb_core::field::FieldType;
use ncdb_core::field_op;
use ncdb_core::ids::{ChangeId, ObjectId, OperationId, SpaceId};
use ncdb_core::model_lock::{ModelLock, Person as Whom};
use ncdb_core::personal::{PERSON_FIELD, Person, PersonField};
use ncdb_core::view::View;

use member::*;
use view_support::*;

const ANNA: [u8; 16] = [0x70; 16];
const BOB: [u8; 16] = [0x71; 16];
const ANNAS_KEY: [u8; 16] = [0x61; 16];
const ANNAS_OTHER_KEY: [u8; 16] = [0x62; 16];
const BOBS_KEY: [u8; 16] = [0x63; 16];

const ANY: OperationId = OperationId {
    change: ChangeId([0; 32]),
    position: 0,
};

/// The test lock with the project's name personal to the project itself,
/// so a project can be a person and carries `$person`.
fn personal_lock() -> ModelLock {
    let mut lock = lock();
    lock.collections
        .get_mut(PROJECT)
        .unwrap()
        .fields
        .get_mut(NAME)
        .unwrap()
        .personal = Some(Whom::Itself);
    lock
}

/// A database of the personal lock in which members 1 to 3 may write.
fn personal_view(db: &MemDb) -> (View, Control) {
    let view = View::open(&db.0, &personal_lock()).unwrap();
    let mut control = Control::default();
    control.keys.insert(0);
    control.members = (1..4).map(|n| (member(n), 0)).collect();
    (view, control)
}

/// Member 1's change number `sequence`: it creates `object` and sets its
/// `$person` to `person`, or clears it with `None`.
fn person_change(
    view: &mut View,
    control: &mut Control,
    sequence: u64,
    object: ObjectId,
    person: Option<Person>,
) {
    let write = PersonField.set(&State::default(), ANY, member(1), Hlc(sequence), person);
    let operations = vec![
        field_op::create(object, PROJECT),
        field_op::encode(&PersonField, &write, object, PROJECT, PERSON_FIELD),
    ];
    let payload = Payload::new(Hlc(sequence), BTreeSet::new(), operations).encode();
    view.receive(
        &frame(sealed_header(1, sequence, 0, payload), GOOD),
        control,
    )
    .unwrap();
}

fn person(person: [u8; 16], person_key: [u8; 16]) -> Option<Person> {
    Some(Person { person, person_key })
}

fn ids(ids: &[[u8; 16]]) -> BTreeSet<[u8; 16]> {
    ids.iter().copied().collect()
}

#[test]
fn a_person_s_key_is_found_by_the_person_id_once_however_many_objects_name_it() {
    let db = MemDb::new();
    let (mut view, mut control) = personal_view(&db);
    person_change(&mut view, &mut control, 1, ObjectId([0x31; 16]), person(ANNA, ANNAS_KEY));
    person_change(&mut view, &mut control, 2, ObjectId([0x32; 16]), person(ANNA, ANNAS_KEY));
    person_change(&mut view, &mut control, 3, ObjectId([0x33; 16]), person(BOB, BOBS_KEY));

    assert_eq!(view.person_key_ids(SPACE, ANNA).unwrap(), ids(&[ANNAS_KEY]));
    assert_eq!(view.person_key_ids(SPACE, BOB).unwrap(), ids(&[BOBS_KEY]));
}

/// Two devices that drew a key for one person at once leave two keys of
/// the person in the space (RFC 0009, "Two first keys"); an erasure must
/// reach both.
#[test]
fn every_key_a_person_has_in_the_space_is_found() {
    let db = MemDb::new();
    let (mut view, mut control) = personal_view(&db);
    person_change(&mut view, &mut control, 1, ObjectId([0x31; 16]), person(ANNA, ANNAS_KEY));
    person_change(
        &mut view,
        &mut control,
        2,
        ObjectId([0x32; 16]),
        person(ANNA, ANNAS_OTHER_KEY),
    );

    assert_eq!(
        view.person_key_ids(SPACE, ANNA).unwrap(),
        ids(&[ANNAS_KEY, ANNAS_OTHER_KEY])
    );
}

#[test]
fn a_person_without_objects_in_the_space_has_no_key_there() {
    let db = MemDb::new();
    let (mut view, mut control) = personal_view(&db);
    person_change(&mut view, &mut control, 1, ObjectId([0x31; 16]), person(ANNA, ANNAS_KEY));

    assert_eq!(view.person_key_ids(SPACE, BOB).unwrap(), BTreeSet::new());
    assert_eq!(
        view.person_key_ids(SpaceId([0x5e; 16]), ANNA).unwrap(),
        BTreeSet::new()
    );
}

#[test]
fn a_cleared_person_names_no_key() {
    let db = MemDb::new();
    let (mut view, mut control) = personal_view(&db);
    person_change(&mut view, &mut control, 1, ObjectId([0x31; 16]), None);

    assert_eq!(view.person_key_ids(SPACE, ANNA).unwrap(), BTreeSet::new());
}

#[test]
fn a_model_without_personal_fields_finds_no_key() {
    let db = MemDb::new();
    let view = View::open(&db.0, &lock()).unwrap();
    assert_eq!(view.person_key_ids(SPACE, ANNA).unwrap(), BTreeSet::new());
}
