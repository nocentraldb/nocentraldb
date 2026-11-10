//! Deleted objects and declared indexes (RFC 0006, Amendment 6, point 2;
//! NOC-81): no declared index holds a deleted object, so no page lists
//! one, while a read by id still finds it, and a restore puts it back.
//!
//! Native only: the browser's view (OPFS) is a later ticket.
#![cfg(not(target_arch = "wasm32"))]

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
use ncdb_core::change::{Change, Operation, Payload};
use ncdb_core::clock::Hlc;
use ncdb_core::field_op;
use ncdb_core::ids::{ChangeId, ObjectId, OperationId};
use ncdb_core::view::View;

use member::*;
use view_support::*;

fn write(value: Value, replaces: BTreeSet<OperationId>) -> Write {
    Write {
        id: OperationId {
            change: ChangeId([0; 32]),
            position: 0,
        },
        author: member(0),
        stamp: Hlc(0),
        value,
        replaces,
    }
}

fn object(n: u8) -> ObjectId {
    ObjectId([n; 16])
}

/// Task `n` with a title and a rank.
fn task(n: u8, title: &str, rank: u64) -> Vec<Operation> {
    let field = |name: &str, kind, value| {
        let write = write(value, BTreeSet::new());
        field_op::encode(&BasicValue { kind }, &write, object(n), TASK, name)
    };
    vec![
        field_op::create(object(n), TASK),
        field(TITLE, Kind::Text, Value::Text(title.into())),
        field(RANK, Kind::Integer, Value::Unsigned(rank)),
    ]
}

/// Deletes task `n`.
fn delete(n: u8) -> Operation {
    field_op::delete(object(n), TASK, &write(Value::Bool(true), BTreeSet::new()))
}

/// Restores task `n`, replacing its deletion, the operation `deletion`.
fn restore(n: u8, deletion: OperationId) -> Operation {
    let write = write(Value::Bool(false), BTreeSet::from([deletion]));
    field_op::delete(object(n), TASK, &write)
}

/// A view and the member that writes its changes, one at a time.
struct Copy {
    view: View,
    control: Control,
    sequence: u64,
}

impl Copy {
    fn new() -> Copy {
        let mut view = View::open_in_memory(&lock()).unwrap();
        let mut control = Control::default();
        view.receive(&envelope(FOUNDER, 1, 0), &mut control)
            .unwrap();
        Copy {
            view,
            control,
            sequence: 1,
        }
    }

    /// Receives one change with these operations; returns its id.
    fn receive(&mut self, operations: Vec<Operation>) -> ChangeId {
        self.sequence += 1;
        let clock = Hlc(self.sequence);
        let payload = Payload::new(clock, BTreeSet::new(), operations).encode();
        let bytes = frame(sealed_header(FOUNDER, self.sequence, 0, payload), GOOD);
        self.view.receive(&bytes, &mut self.control).unwrap();
        Change::decode(&bytes).unwrap().id()
    }

    /// The first bytes of the object ids of every row of a page of 50.
    fn page(&self, index: &str, equal: &[Value]) -> Vec<u8> {
        let page = self.view.page(TASK, index, equal, None, 50).unwrap();
        page.rows.iter().map(|(_, object)| object.0[0]).collect()
    }

    fn deleted(&self, n: u8) -> bool {
        let row = self.view.get(SPACE, TASK, object(n)).unwrap();
        row.expect("a read by id finds the object")
            .object
            .is_deleted()
    }
}

#[test]
fn no_declared_index_lists_a_deleted_object_and_a_read_by_id_still_finds_it() {
    let mut copy = Copy::new();
    copy.receive([task(1, "a", 1), task(2, "a", 2), task(3, "b", 3)].concat());
    copy.receive(vec![delete(2)]);
    let a = [Value::Text("a".into())];
    assert_eq!(copy.page(BY_RANK, &[]), [1, 3]);
    assert_eq!(copy.page(BY_NAME, &[]), [1, 3]);
    assert_eq!(copy.page(BY_TITLE, &a), [1]);
    assert!(copy.deleted(2));
    let row = copy.view.get(SPACE, TASK, object(2)).unwrap().unwrap();
    assert_eq!(row.fields.len(), 2, "its fields stay readable by id");
}

#[test]
fn an_object_deleted_in_the_change_that_creates_it_is_in_no_index() {
    let mut copy = Copy::new();
    copy.receive([task(1, "a", 1), task(2, "a", 2), vec![delete(2)]].concat());
    assert_eq!(copy.page(BY_RANK, &[]), [1]);
    // A field written after the deletion leaves the object out too.
    let rank = write(Value::Unsigned(0), BTreeSet::new());
    let integer = BasicValue {
        kind: Kind::Integer,
    };
    copy.receive(vec![field_op::encode(
        &integer,
        &rank,
        object(2),
        TASK,
        RANK,
    )]);
    assert_eq!(copy.page(BY_RANK, &[]), [1]);
    assert!(copy.deleted(2));
}

#[test]
fn a_deletion_that_arrives_before_the_create_keeps_the_object_out() {
    let mut copy = Copy::new();
    copy.receive(vec![delete(2)]);
    copy.receive([task(1, "a", 1), task(2, "a", 2)].concat());
    assert_eq!(copy.page(BY_RANK, &[]), [1]);
}

#[test]
fn a_restore_puts_the_object_back_into_every_index() {
    let mut copy = Copy::new();
    copy.receive([task(1, "a", 1), task(2, "a", 2)].concat());
    let deletion = copy.receive(vec![delete(2)]);
    assert_eq!(copy.page(BY_RANK, &[]), [1]);
    let deletion = OperationId {
        change: deletion,
        position: 0,
    };
    copy.receive(vec![restore(2, deletion)]);
    assert_eq!(copy.page(BY_RANK, &[]), [1, 2]);
    assert_eq!(copy.page(BY_TITLE, &[Value::Text("a".into())]), [2, 1]);
    assert!(!copy.deleted(2));
}
