//! What a fold touched (RFC 0006, "Folding", step 6): after the commit,
//! the view hands out the `(collection, object id)` of each object a fold
//! wrote, per space, once, so that subscriptions and the SDK's change
//! events re-read only those (NOC-176). It names objects, never values. A
//! fold that rolls back leaves nothing behind.

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
use ncdb_core::cbor::Value;
use ncdb_core::change::{Operation, Payload};
use ncdb_core::clock::Hlc;
use ncdb_core::field_op;
use ncdb_core::ids::{ChangeId, ObjectId, OperationId};
use ncdb_core::view::View;

use member::*;
use view_support::*;

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

fn object(n: u8) -> ObjectId {
    ObjectId([n; 16])
}

/// Creates task `n` with a title.
fn task(n: u8, title: &str) -> Vec<Operation> {
    let kind = BasicValue { kind: Kind::Text };
    vec![
        field_op::create(object(n), TASK),
        field_op::encode(
            &kind,
            &write(Value::Text(title.into())),
            object(n),
            TASK,
            TITLE,
        ),
    ]
}

/// Changes of the founder, one sequence number after another.
struct Writer {
    control: Control,
    sequence: u64,
}

impl Writer {
    /// A view on `view` that holds the space's genesis.
    fn new(view: &mut View) -> Writer {
        let mut control = Control::default();
        view.receive(&envelope(FOUNDER, 1, 0), &mut control)
            .unwrap();
        Writer {
            control,
            sequence: 1,
        }
    }

    /// The frame of the next change, with these operations.
    fn next(&mut self, operations: Vec<Operation>) -> Vec<u8> {
        self.sequence += 1;
        let payload = Payload::new(Hlc(self.sequence), BTreeSet::new(), operations).encode();
        frame(sealed_header(FOUNDER, self.sequence, 0, payload), GOOD)
    }
}

#[test]
fn a_fold_hands_out_the_objects_it_touched_once() {
    let mut view = View::open_in_memory(&lock()).unwrap();
    let mut writer = Writer::new(&mut view);
    view.take_touched(SPACE);
    let bytes = writer.next([task(1, "a"), task(2, "b")].concat());
    view.receive(&bytes, &mut writer.control).unwrap();

    assert_eq!(
        view.take_touched(SPACE),
        vec![(TASK.to_owned(), object(1)), (TASK.to_owned(), object(2))]
    );
    assert!(view.take_touched(SPACE).is_empty(), "handed out once");
}

#[test]
fn a_delete_names_the_object_in_its_collection() {
    let mut view = View::open_in_memory(&lock()).unwrap();
    let mut writer = Writer::new(&mut view);
    let bytes = writer.next(task(1, "a"));
    view.receive(&bytes, &mut writer.control).unwrap();
    view.take_touched(SPACE);

    let delete = field_op::delete(object(1), TASK, &write(Value::Bool(true)));
    let bytes = writer.next(vec![delete]);
    view.receive(&bytes, &mut writer.control).unwrap();
    assert_eq!(view.take_touched(SPACE), vec![(TASK.to_owned(), object(1))]);
}

#[test]
fn a_duplicate_touches_nothing() {
    let mut view = View::open_in_memory(&lock()).unwrap();
    let mut writer = Writer::new(&mut view);
    let bytes = writer.next(task(1, "a"));
    view.receive(&bytes, &mut writer.control).unwrap();
    view.take_touched(SPACE);
    view.receive(&bytes, &mut writer.control).unwrap();
    assert!(view.take_touched(SPACE).is_empty());
}

// A fold that fails after it wrote some rows rolls back, and what it
// touched is not handed out: only committed folds count.
#[test]
fn a_fold_that_rolls_back_leaves_nothing_touched() {
    let db = TempDb::new("touched-rollback");
    let mut view = View::open(&db.0, &lock()).unwrap();
    let mut writer = Writer::new(&mut view);
    view.take_touched(SPACE);

    // The task's field states are gone, so the fold of its title fails
    // after its create was written.
    let conn = rusqlite::Connection::open(&db.0).unwrap();
    conn.execute_batch(&format!("DROP TABLE \"fs_{TASK}\""))
        .unwrap();
    drop(conn);
    let bytes = writer.next(task(1, "a"));
    assert!(view.receive(&bytes, &mut writer.control).is_err());
    assert!(view.take_touched(SPACE).is_empty());
}
