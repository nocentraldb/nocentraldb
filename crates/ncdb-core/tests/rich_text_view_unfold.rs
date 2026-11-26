//! Un-folding rich text in the view (RFC 0013, "Un-folding a rejected
//! change"; RFC 0008, "What a new change can change"): when Carol's
//! removal arrives after the view folded her change, her items stay as
//! deleted placeholders, the text she deleted is back, and a write that
//! parked because of her change is checked again. A view that receives the
//! removal first ends in the same state, as does one where the removal is
//! itself rejected later.
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

use ncdb_core::cbor::{Key, Value};
use ncdb_core::change::{Change, Operation, Payload};
use ncdb_core::checks::SealChecks;
use ncdb_core::clock::Hlc;
use ncdb_core::field::FieldType;
use ncdb_core::field_op;
use ncdb_core::ids::{ObjectId, SpaceId};
use ncdb_core::log::{RejectReason, State};
use ncdb_core::membership::Role;
use ncdb_core::model_lock::{self, ModelLock};
use ncdb_core::rich_text::{RichText, client_id};
use ncdb_core::seal;
use ncdb_core::view::{FieldState, View};

use rich_text_support::{add_block, apply, delete_text, doc, plain_text, type_text};
use space::Space;
use view_support::*;

const ALICE: u8 = 1;
const BOB: u8 = 2;
const CAROL: u8 = 3;
const ERIN: u8 = 5;
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

/// A sealed change by `author` at `sequence`, stamped `stamp`.
fn sealed_at(space: &Space, author: u8, sequence: u64, stamp: u64, op: Operation) -> Change {
    let payload = Payload::new(Hlc(stamp), BTreeSet::new(), vec![op]);
    seal::seal(
        &space::key(author),
        &space::space_key(),
        space.space,
        sequence,
        space.first_generation,
        &payload,
    )
}

/// Alice founds the space, makes Bob and Carol writers and Erin an admin
/// junior to Alice, and creates the task; Bob's copy.
fn space() -> (Space, Vec<Change>) {
    let mut space = Space::new(ALICE);
    let genesis = space.genesis.clone();
    let bob = space.add(ALICE, BOB, Role::Writer, &[&genesis]);
    let carol = space.add(ALICE, CAROL, Role::Writer, &[&bob]);
    let erin = space.add(ALICE, ERIN, Role::Admin, &[&carol]);
    let envelope = space.envelope(ALICE, BOB, &[&erin]);
    let create = space.sealed(ALICE, vec![field_op::create(T, TASK)]);
    (space, vec![genesis, bob, carol, erin, envelope, create])
}

fn view_of(space: SpaceId, changes: &[&Change]) -> View {
    let mut view = View::open_in_memory(&lock_with_body()).unwrap();
    let mut checks = SealChecks::new(space, space::key(BOB));
    for change in changes {
        view.receive(change.bytes(), &mut checks).unwrap();
    }
    view
}

/// The body's stored state bytes and its text.
fn body(view: &View, space: SpaceId) -> (Vec<u8>, String) {
    let row = view.get(space, TASK, T).unwrap().expect("the task");
    let Some(FieldState::RichText(state)) = row.fields.get(BODY) else {
        panic!("the body holds no rich-text state");
    };
    (RichText.state_bytes(state), plain_text(state.document()))
}

const FOLDED: State = State::Folded { parked: vec![] };
const REJECTED: State = State::Rejected(RejectReason::NotMember);

/// One order's end: the body's state bytes and text, and the state of each
/// change watched.
type End = ((Vec<u8>, String), Vec<Option<State>>);

/// Each order's view: the body, and the state of each of `watched`.
fn ends_of(
    space: &Space,
    setup: &[Change],
    orders: &[Vec<&Change>],
    watched: &[&Change],
) -> Vec<End> {
    orders
        .iter()
        .map(|order| {
            let changes: Vec<&Change> = setup.iter().chain(order.iter().copied()).collect();
            let view = view_of(space.space, &changes);
            let states = watched
                .iter()
                .map(|c| view.state(space.space, c.id()))
                .collect();
            (body(&view, space.space), states)
        })
        .collect()
}

fn assert_all_equal<T: PartialEq + std::fmt::Debug>(ends: &[T]) {
    for (i, end) in ends.iter().enumerate().skip(1) {
        assert_eq!(end, &ends[0], "order {i} differs from order 0");
    }
}

#[test]
fn a_late_removal_leaves_the_removed_members_text_as_placeholders_next_to_which_later_text_stays() {
    let (mut space, setup) = space();
    let alice = doc(client(ALICE));
    let base = add_block(&alice, "paragraph", "Base.");
    let carol_doc = doc(client(CAROL));
    apply(&carol_doc, &base);
    let carols = type_text(&carol_doc, 0, 5, " Carol.");
    let bob_doc = doc(client(BOB));
    apply(&bob_doc, &base);
    apply(&bob_doc, &carols);
    let bobs = type_text(&bob_doc, 0, 12, " Bob after Carol.");
    let base = space.sealed(ALICE, vec![body_op(ALICE, &base)]);
    let carols = space.sealed(CAROL, vec![body_op(CAROL, &carols)]);
    let bobs = space.sealed(BOB, vec![body_op(BOB, &bobs)]);
    // Alice removes Carol, having held none of Carol's changes.
    let removal = space.remove(ALICE, CAROL, 0, &[&setup[4]]);
    let orders = [
        vec![&base, &carols, &bobs, &removal],
        vec![&removal, &base, &carols, &bobs],
        vec![&bobs, &carols, &removal, &base],
    ];
    let ends = ends_of(&space, &setup, &orders, &[&carols, &bobs]);
    assert_all_equal(&ends);
    assert_eq!(ends[0].0.1, "Base. Bob after Carol.");
    assert_eq!(ends[0].1, vec![Some(REJECTED), Some(FOLDED)]);
}

#[test]
fn the_text_a_rejected_change_deleted_is_back() {
    let (mut space, setup) = space();
    let alice = doc(client(ALICE));
    let base = add_block(&alice, "paragraph", "Keep this.");
    let carol_doc = doc(client(CAROL));
    apply(&carol_doc, &base);
    let cut = delete_text(&carol_doc, 0, 0, 5);
    let base = space.sealed(ALICE, vec![body_op(ALICE, &base)]);
    let cut = space.sealed(CAROL, vec![body_op(CAROL, &cut)]);
    let removal = space.remove(ALICE, CAROL, 0, &[&setup[4]]);
    let orders = [vec![&base, &cut, &removal], vec![&removal, &cut, &base]];
    let ends = ends_of(&space, &setup, &orders, &[&cut]);
    assert_all_equal(&ends);
    assert_eq!(ends[0].0.1, "Keep this.");
    assert_eq!(ends[0].1, vec![Some(REJECTED)]);
}

#[test]
fn a_write_that_parked_because_of_the_rejected_change_is_checked_again() {
    let (space, setup) = space();
    // Carol writes Q at sequence 1; restored from a backup with a clock
    // behind, she writes P under the same ids at sequence 5, stamped
    // earlier. Alice removes her with cutoff 1: P is rejected, Q is not.
    let q = add_block(&doc(client(CAROL)), "paragraph", "Q");
    let p = add_block(&doc(client(CAROL)), "paragraph", "P");
    let q = sealed_at(&space, CAROL, 1, 9 << 16, body_op(CAROL, &q));
    let p = sealed_at(&space, CAROL, 5, 2 << 16, body_op(CAROL, &p));
    let mut space = space;
    let removal = space.remove(ALICE, CAROL, 1, &[&setup[4]]);
    let orders = [
        vec![&p, &q, &removal],
        vec![&q, &p, &removal],
        vec![&removal, &q, &p],
        vec![&p, &removal, &q],
    ];
    let ends = ends_of(&space, &setup, &orders, &[&q, &p]);
    assert_all_equal(&ends);
    assert_eq!(ends[0].0.1, "Q");
    assert_eq!(ends[0].1, vec![Some(FOLDED), Some(REJECTED)]);
}

#[test]
fn a_change_whose_rejection_is_itself_rejected_folds_again_whole() {
    let (mut space, setup) = space();
    let alice = doc(client(ALICE));
    let base = add_block(&alice, "paragraph", "Base.");
    let carol_doc = doc(client(CAROL));
    apply(&carol_doc, &base);
    let carols = type_text(&carol_doc, 0, 5, " Carol.");
    let base = space.sealed(ALICE, vec![body_op(ALICE, &base)]);
    let carols = space.sealed(CAROL, vec![body_op(CAROL, &carols)]);
    // Erin removes Carol; Alice, senior to Erin, concurrently removes Erin
    // with cutoff 0, which excludes Erin's removal of Carol.
    let by_erin = space.remove(ERIN, CAROL, 0, &[&setup[4]]);
    let by_alice = space.remove(ALICE, ERIN, 0, &[&setup[4]]);
    let orders = [
        vec![&base, &carols, &by_erin, &by_alice],
        vec![&by_erin, &carols, &base, &by_alice],
        vec![&by_alice, &by_erin, &carols, &base],
    ];
    let ends = ends_of(&space, &setup, &orders, &[&carols, &by_erin]);
    assert_all_equal(&ends);
    assert_eq!(ends[0].0.1, "Base. Carol.");
    assert_eq!(ends[0].1[0], Some(FOLDED));
    // Whatever arrived, the state is that of a view that never saw Erin's
    // removal.
    let without = ends_of(&space, &setup, &[vec![&base, &carols]], &[]);
    assert_eq!(ends[0].0, without[0].0);
}

#[test]
fn a_field_the_model_adds_later_places_the_changes_rejected_before() {
    let (mut space, setup) = space();
    let alice = doc(client(ALICE));
    let base = add_block(&alice, "paragraph", "Base.");
    let carol_doc = doc(client(CAROL));
    apply(&carol_doc, &base);
    let carols = type_text(&carol_doc, 0, 5, " Carol.");
    let bob_doc = doc(client(BOB));
    apply(&bob_doc, &base);
    apply(&bob_doc, &carols);
    let bobs = type_text(&bob_doc, 0, 12, " Bob after Carol.");
    let base = space.sealed(ALICE, vec![body_op(ALICE, &base)]);
    let carols = space.sealed(CAROL, vec![body_op(CAROL, &carols)]);
    let bobs = space.sealed(BOB, vec![body_op(BOB, &bobs)]);
    let removal = space.remove(ALICE, CAROL, 0, &[&setup[4]]);
    // Bob's view first folds by a model without the body, which parks the
    // body's operations; then the model adds the body.
    let db = TempDb::new("rich-text-unfold-migrate");
    let checks = |space| SealChecks::new(space, space::key(BOB));
    {
        let mut view = View::open_with(&db.0, &lock(), checks).unwrap();
        let mut c = checks(space.space);
        for change in setup.iter().chain([&removal, &base, &carols, &bobs]) {
            view.receive(change.bytes(), &mut c).unwrap();
        }
    }
    let view = View::open_with(&db.0, &lock_with_body(), checks).unwrap();
    let expected = ends_of(
        &space,
        &setup,
        &[vec![&removal, &base, &carols, &bobs]],
        &[],
    );
    assert_eq!(body(&view, space.space), expected[0].0);
    assert_eq!(expected[0].0.1, "Base. Bob after Carol.");
}
