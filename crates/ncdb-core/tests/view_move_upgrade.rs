//! A view an older core folded (RFC 0016, "Formats": "An old core parks
//! it as an unknown operation, so it shows the object as live until it
//! upgrades"): the older core parked the move (operation 6) as an unknown
//! operation and `$movedFrom` as an unknown field. The first open by this
//! core folds them, so the object becomes a trace, and the view equals one
//! this core folded from the start. Later opens open no change for it.
//!
//! The older view is made by hand: the changes before the move as this
//! core folds them, then the move's change appended as the older core
//! appends it, folded with its operation parked, and no marker of the
//! operations the view knows.

mod support;

#[path = "support/member.rs"]
mod member;
#[path = "support/view.rs"]
mod view_support;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeSet;

use ncdb_core::basic::{BasicValue, Kind, Write};
use ncdb_core::cbor::{Key, Value};
use ncdb_core::change::{Change, Operation, Payload};
use ncdb_core::clock::Hlc;
use ncdb_core::field_op;
use ncdb_core::ids::{ChangeId, ObjectId, OperationId, SpaceId};
use ncdb_core::log::State;
use ncdb_core::view::View;

use member::*;
use view_support::*;

const T: ObjectId = ObjectId([0x11; 16]);
const NEW: SpaceId = SpaceId([0x5a; 16]);
const OLD: SpaceId = SpaceId([0x0d; 16]);

fn write(value: Value) -> Write {
    Write {
        id: OperationId {
            change: ChangeId([0; 32]),
            position: 0,
        },
        author: member(0),
        stamp: Hlc(0),
        value,
        replaces: BTreeSet::new(),
    }
}

fn change(sequence: u64, operations: Vec<Operation>) -> Vec<u8> {
    let payload = Payload::new(Hlc(sequence), BTreeSet::new(), operations).encode();
    frame(sealed_header(FOUNDER, sequence, 0, payload), GOOD)
}

/// The control change, a task created with a title, and the change with
/// the move and the set of `$movedFrom`.
fn frames() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let text = BasicValue { kind: Kind::Text };
    let title = Value::Text("a".into());
    let created = change(
        2,
        vec![
            field_op::create(T, TASK),
            field_op::encode(&text, &write(title), T, TASK, TITLE),
        ],
    );
    let from = Value::Map([(Key::Unsigned(1), Value::Bytes(OLD.0.to_vec()))].into());
    let moved = change(
        3,
        vec![
            field_op::move_object(T, TASK, &write(Value::Bytes(NEW.0.to_vec()))),
            field_op::moved_from(T, TASK, &write(from)),
        ],
    );
    (envelope(FOUNDER, 1, 0), created, moved)
}

/// The checks of a view opened again: they learn the control changes from
/// the log.
fn checks(_: SpaceId) -> Control {
    Control::default()
}

/// A view in `db` as an older core left it.
fn older(db: &MemDb) {
    let (control, created, moved) = frames();
    let mut view = View::open(&db.0, &lock()).unwrap();
    let mut checks = Control::default();
    view.receive(&control, &mut checks).unwrap();
    view.receive(&created, &mut checks).unwrap();
    drop(view);
    let conn = db.connect();
    let id = Change::decode(&moved).unwrap().id();
    conn.execute(
        "INSERT INTO log (space_id, change_id, state, frame) VALUES (?1, ?2, 'folded', ?3)",
        (SPACE.0, id.0, &moved),
    )
    .unwrap();
    for (position, reason) in [(0, "unknown_operation"), (1, "unknown_field")] {
        conn.execute(
            "INSERT INTO parked (space_id, change_id, position, reason) VALUES (?1, ?2, ?3, ?4)",
            (SPACE.0, id.0, position, reason),
        )
        .unwrap();
    }
    conn.execute("DELETE FROM meta WHERE key = 'operations'", ())
        .unwrap();
}

#[test]
fn the_first_open_by_this_core_folds_a_move_an_older_core_parked() {
    let db = MemDb::new();
    let keep = db.connect();
    older(&db);
    let view = View::open_with(&db.0, &lock(), checks).unwrap();

    let fresh = MemDb::new();
    let _keep_fresh = fresh.connect();
    let mut expected = View::open(&fresh.0, &lock()).unwrap();
    let (control, created, moved) = frames();
    let mut control_checks = Control::default();
    for frame in [&control, &created, &moved] {
        expected.receive(frame, &mut control_checks).unwrap();
    }

    let row = view.get(SPACE, TASK, T).unwrap().unwrap();
    assert_eq!(row.object.moved_to(), Some(NEW));
    assert_eq!(row.object.moved_from(), Some(OLD));
    assert!(row.fields.is_empty(), "the object is a trace");
    let id = Change::decode(&moved).unwrap().id();
    assert_eq!(
        view.state(SPACE, id),
        Some(State::Folded { parked: vec![] })
    );
    drop(view);
    assert_eq!(view_bytes(&db.connect()), view_bytes(&fresh.connect()));
    drop(keep);
}
