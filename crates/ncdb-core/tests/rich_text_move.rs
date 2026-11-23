//! A rich-text field of a moved object whose move is taken back (RFC 0016,
//! "Moving back, and moving again"; RFC 0013, "The same id twice"). While
//! the object is a trace, the view keeps none of its fields; the take-back
//! folds them again from the log, and an operation folded then can
//! displace one the field holds, which the refold in stamp order then
//! parks. Revived fields fold first, then the displaced ones are folded
//! again, so every order of arrival of the two rich-text changes, the move
//! and its take-back ends in the same body and the same change states.
#![cfg(ncdb_rich_text)]

mod rich_text_support;
mod support;

#[path = "support/space.rs"]
mod space;
#[path = "support/view.rs"]
mod view_support;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeSet;

use ncdb_core::basic::Write;
use ncdb_core::cbor::{Key, Value};
use ncdb_core::change::{Change, Operation};
use ncdb_core::checks::SealChecks;
use ncdb_core::clock::Hlc;
use ncdb_core::field::FieldType;
use ncdb_core::field_op;
use ncdb_core::ids::{ChangeId, ObjectId, OperationId, SpaceId};
use ncdb_core::log::{OperationParkReason, ParkedOperation, State};
use ncdb_core::membership::Role;
use ncdb_core::model_lock::{self, ModelLock};
use ncdb_core::rich_text::{RichText, client_id};
use ncdb_core::view::{FieldState, View};

use rich_text_support::{add_block, doc};
use space::Space;
use view_support::*;

const A: u8 = 1;
const B: u8 = 2;
const T: ObjectId = ObjectId([0x11; 16]);
const BODY: &str = "body";
const NONCE: [u8; 8] = [3; 8];
const NEW: SpaceId = SpaceId([0x5a; 16]);

/// The view tests' lock, with a rich-text body on tasks.
fn lock_with_body() -> ModelLock {
    let mut lock = lock();
    let mut field = lock.field(TASK, RANK).unwrap().clone();
    field.kind = model_lock::Kind::RichText;
    field.rule = 1;
    let task = lock.collections.get_mut(TASK).unwrap();
    task.fields.insert(BODY.into(), field);
    lock
}

fn client(who: u8) -> u64 {
    client_id(&space::id(who), &NONCE)
}

/// Operation 5 on the task's body, adding items under `who`'s client id.
fn body_op(who: u8, update: &[u8]) -> Operation {
    Operation::new(
        T,
        TASK.to_owned(),
        Some(BODY.to_owned()),
        5,
        Some(Value::Map(
            [
                (Key::Unsigned(1), Value::Bytes(update.to_vec())),
                (
                    Key::Unsigned(2),
                    Value::Map([(Key::Unsigned(client(who)), Value::Bytes(NONCE.to_vec()))].into()),
                ),
            ]
            .into(),
        )),
    )
}

/// A write of the task's move: `value` is the new space, or null.
fn move_write(value: Value, replaces: BTreeSet<OperationId>) -> Operation {
    let write = Write {
        id: OperationId {
            change: ChangeId([0; 32]),
            position: 0,
        },
        author: space::id(A),
        stamp: Hlc(0),
        value,
        replaces,
    };
    field_op::move_object(T, TASK, &write)
}

fn view_of(space: SpaceId, changes: &[&Change]) -> View {
    let mut view = View::open_in_memory(&lock_with_body()).unwrap();
    let mut checks = SealChecks::new(space, space::key(B));
    for change in changes {
        view.receive(change.bytes(), &mut checks).unwrap();
    }
    view
}

/// The body's stored state and its XML, or `None` while the view keeps
/// none.
fn body(view: &View, space: SpaceId) -> Option<(Vec<u8>, String)> {
    let row = view.get(space, TASK, T).unwrap().expect("the task");
    match row.fields.get(BODY) {
        Some(FieldState::RichText(state)) => {
            Some((RichText.state_bytes(state), RichText.read(state).xml))
        }
        _ => None,
    }
}

/// Every order of `items`.
fn orders<T: Clone>(items: &[T]) -> Vec<Vec<T>> {
    if items.len() <= 1 {
        return vec![items.to_vec()];
    }
    let mut all = Vec::new();
    for i in 0..items.len() {
        let mut rest = items.to_vec();
        let first = rest.remove(i);
        for mut tail in orders(&rest) {
            tail.insert(0, first.clone());
            all.push(tail);
        }
    }
    all
}

#[test]
fn a_revived_rich_text_field_that_displaces_an_operation_converges_in_every_order() {
    let mut space = Space::new(A);
    let genesis = space.genesis.clone();
    let add = space.add(A, B, Role::Writer, &[&genesis]);
    let envelope = space.envelope(A, B, &[&add]);
    let create = space.sealed(A, vec![field_op::create(T, TASK)]);
    let setup = [genesis, add, envelope, create];
    // A writes, is restored from a backup taken before, and writes again
    // under the same client id and clocks: the earlier keeps the id.
    let earlier = space.sealed(
        A,
        vec![body_op(A, &add_block(&doc(client(A)), "paragraph", "P"))],
    );
    let later = space.sealed(
        A,
        vec![
            field_op::create(ObjectId([0x22; 16]), TASK),
            body_op(A, &add_block(&doc(client(A)), "paragraph", "Q")),
        ],
    );
    let moved = space.sealed(
        A,
        vec![move_write(Value::Bytes(NEW.0.to_vec()), BTreeSet::new())],
    );
    let back = space.sealed(
        A,
        vec![move_write(
            Value::Null,
            BTreeSet::from([OperationId {
                change: moved.id(),
                position: 0,
            }]),
        )],
    );

    let mut ends = Vec::new();
    for order in orders(&[&earlier, &later, &moved, &back]) {
        let changes: Vec<&Change> = setup.iter().chain(order.iter().copied()).collect();
        let view = view_of(space.space, &changes);
        let states = [&earlier, &later, &moved, &back].map(|c| view.state(space.space, c.id()));
        let row = view.get(space.space, TASK, T).unwrap().unwrap();
        ends.push((states, body(&view, space.space), row.object.moved_to()));
    }
    for end in &ends {
        assert_eq!(end, &ends[0], "every order ends the same");
    }
    let (states, body, moved_to) = &ends[0];
    assert_eq!(moved_to, &None, "the move is taken back");
    assert_eq!(states[0], Some(State::Folded { parked: vec![] }));
    assert_eq!(
        states[1],
        Some(State::Folded {
            parked: vec![ParkedOperation {
                position: 1,
                reason: OperationParkReason::MalformedArguments,
            }],
        })
    );
    let (_, xml) = body.as_ref().expect("the revived body");
    assert!(xml.contains('P') && !xml.contains('Q'), "{xml}");
}

#[test]
fn rich_text_operations_on_a_trace_park_as_they_would_on_the_live_object_in_every_order() {
    // Without a take-back the object stays a trace and the view keeps no
    // body, but whether the later change's operation parks must not
    // depend on whether it arrived before or after the move.
    let mut space = Space::new(A);
    let genesis = space.genesis.clone();
    let add = space.add(A, B, Role::Writer, &[&genesis]);
    let envelope = space.envelope(A, B, &[&add]);
    let create = space.sealed(A, vec![field_op::create(T, TASK)]);
    let setup = [genesis, add, envelope, create];
    let earlier = space.sealed(
        A,
        vec![body_op(A, &add_block(&doc(client(A)), "paragraph", "P"))],
    );
    let later = space.sealed(
        A,
        vec![
            field_op::create(ObjectId([0x22; 16]), TASK),
            body_op(A, &add_block(&doc(client(A)), "paragraph", "Q")),
        ],
    );
    let moved = space.sealed(
        A,
        vec![move_write(Value::Bytes(NEW.0.to_vec()), BTreeSet::new())],
    );

    let mut ends = Vec::new();
    for order in orders(&[&earlier, &later, &moved]) {
        let changes: Vec<&Change> = setup.iter().chain(order.iter().copied()).collect();
        let view = view_of(space.space, &changes);
        let states = [&earlier, &later, &moved].map(|c| view.state(space.space, c.id()));
        ends.push((states, body(&view, space.space)));
    }
    for end in &ends {
        assert_eq!(end, &ends[0], "every order ends the same");
    }
    let (states, body) = &ends[0];
    assert_eq!(body, &None, "a trace keeps no body");
    assert_eq!(
        states[1],
        Some(State::Folded {
            parked: vec![ParkedOperation {
                position: 1,
                reason: OperationParkReason::MalformedArguments,
            }],
        })
    );
}
