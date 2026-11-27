//! A rich-text field in the view (RFC 0013, "Folding"): with the compiler
//! cfg `ncdb_rich_text`, the view's model holds `ncdb/richText` rule 1 as a field
//! type of its own, without a column, and folds its operations into
//! `fs_<collection>` in any order of arrival. Of two operations that give
//! one id different content, the earlier in stamp order keeps it and the
//! other parks as "malformed arguments" on every copy, also when the later
//! one was folded first ("The same id twice").
#![cfg(ncdb_rich_text)]

mod rich_text_support;
mod support;

#[path = "support/space.rs"]
mod space;
#[path = "support/view.rs"]
mod view_support;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::cbor::{Key, Value};
use ncdb_core::change::{Change, Operation};
use ncdb_core::checks::SealChecks;
use ncdb_core::field::FieldType;
use ncdb_core::field_op;
use ncdb_core::ids::{ObjectId, SpaceId};
use ncdb_core::log::{OperationParkReason, ParkedOperation, State};
use ncdb_core::membership::Role;
use ncdb_core::model_lock::{self, ModelLock};
use ncdb_core::rich_text::{RichText, client_id};
use ncdb_core::view::{Field, FieldState, View};

use rich_text_support::{add_block, delete_text, doc, type_text};
use space::Space;
use view_support::*;

const A: u8 = 1;
const B: u8 = 2;
const T: ObjectId = ObjectId([0x11; 16]);
const BODY: &str = "body";
const NONCE: [u8; 8] = [3; 8];

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

/// Operation 5 on the task's body, listing `who`'s client id when the
/// update adds items.
fn body_op(who: u8, update: &[u8]) -> Operation {
    let adds = !ncdb_core::rich_text::update::read(update)
        .unwrap()
        .runs
        .is_empty();
    let clients = match adds {
        true => [(Key::Unsigned(client(who)), Value::Bytes(NONCE.to_vec()))].into(),
        false => Default::default(),
    };
    Operation::new(
        T,
        TASK.to_owned(),
        Some(BODY.to_owned()),
        5,
        Some(Value::Map(
            [
                (Key::Unsigned(1), Value::Bytes(update.to_vec())),
                (Key::Unsigned(2), Value::Map(clients)),
            ]
            .into(),
        )),
    )
}

/// A space where A made B a writer and created the task: its control
/// changes and the create, which every view receives first.
fn space() -> (Space, Vec<Change>) {
    let mut space = Space::new(A);
    let genesis = space.genesis.clone();
    let add = space.add(A, B, Role::Writer, &[&genesis]);
    let envelope = space.envelope(A, B, &[&add]);
    let create = space.sealed(A, vec![field_op::create(T, TASK)]);
    (space, vec![genesis, add, envelope, create])
}

fn view_of(space: SpaceId, changes: &[&Change]) -> View {
    let mut view = View::open_in_memory(&lock_with_body()).unwrap();
    let mut checks = SealChecks::new(space, space::key(B));
    for change in changes {
        view.receive(change.bytes(), &mut checks).unwrap();
    }
    view
}

/// The body's stored state, its bytes and what it reads as Markdown.
fn body(view: &View, space: SpaceId) -> (Vec<u8>, String) {
    let row = view.get(space, TASK, T).unwrap().expect("the task");
    let Some(FieldState::RichText(state)) = row.fields.get(BODY) else {
        panic!("the body holds no rich-text state");
    };
    (RichText.state_bytes(state), RichText.read(state).markdown)
}

fn parked(position: u64) -> State {
    State::Folded {
        parked: vec![ParkedOperation {
            position,
            reason: OperationParkReason::MalformedArguments,
        }],
    }
}

#[test]
fn the_view_holds_rich_text_as_a_field_type_without_a_column() {
    let view = View::open_in_memory(&lock_with_body()).unwrap();
    let field = view.model().field(TASK, BODY).expect("the body is known");
    assert!(matches!(field, Field::RichText(_)));
    assert!(!field.has_column());
    assert!(!view.model().unregistered(TASK, BODY));
}

#[test]
fn edits_of_two_members_fold_to_the_same_state_in_any_order() {
    let (mut space, setup) = space();
    let a = doc(client(A));
    let first = add_block(&a, "paragraph", "Hello");
    let b = doc(client(B));
    rich_text_support::apply(&b, &first);
    let by_a = space.sealed(A, vec![body_op(A, &first)]);
    let by_b = space.sealed(B, vec![body_op(B, &type_text(&b, 0, 5, " world"))]);
    let by_a2 = space.sealed(A, vec![body_op(A, &delete_text(&a, 0, 0, 1))]);
    let mut ends = Vec::new();
    for order in [[&by_a, &by_b, &by_a2], [&by_a2, &by_b, &by_a]] {
        let changes: Vec<&Change> = setup.iter().chain(order).collect();
        let view = view_of(space.space, &changes);
        for change in order {
            assert_eq!(
                view.state(space.space, change.id()),
                Some(State::Folded { parked: vec![] })
            );
        }
        ends.push(body(&view, space.space));
    }
    assert_eq!(ends[0], ends[1]);
    assert!(ends[0].1.contains("ello world"), "{}", ends[0].1);
}

#[test]
fn an_update_under_another_members_client_id_parks() {
    let (mut space, setup) = space();
    let a = doc(client(A));
    // B signs a change that carries items under A's client id.
    let forged = space.sealed(B, vec![body_op(A, &add_block(&a, "paragraph", "Hi"))]);
    let changes: Vec<&Change> = setup.iter().chain([&forged]).collect();
    let view = view_of(space.space, &changes);
    assert_eq!(view.state(space.space, forged.id()), Some(parked(0)));
}

#[test]
fn of_two_changes_that_give_one_id_different_content_the_earlier_keeps_it_on_every_copy() {
    let (mut space, setup) = space();
    // A writes, is restored from a backup taken before, and writes again
    // under the same client id and clocks.
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
    let mut ends = Vec::new();
    for order in [[&earlier, &later], [&later, &earlier]] {
        let changes: Vec<&Change> = setup.iter().chain(order).collect();
        let view = view_of(space.space, &changes);
        assert_eq!(
            view.state(space.space, earlier.id()),
            Some(State::Folded { parked: vec![] })
        );
        assert_eq!(view.state(space.space, later.id()), Some(parked(1)));
        ends.push(body(&view, space.space));
    }
    assert_eq!(ends[0], ends[1]);
    assert!(ends[0].1.contains('P') && !ends[0].1.contains('Q'));
}
