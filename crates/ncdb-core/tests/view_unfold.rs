//! Un-folding in the local view on SQLite (NOC-49; RFC 0006,
//! "Un-folding"): a revocation that arrives after the view folded the
//! revoked member's changes takes their operations out of the view's
//! tables, in the transaction that appends it. The tables then equal those
//! of a view that never folded the changes, and the log holds them as
//! rejected.
//!
//! Native only: the browser's view (OPFS) is a later ticket.
#![cfg(not(target_arch = "wasm32"))]

#[path = "support/space.rs"]
mod space;
#[path = "support/view.rs"]
mod view_support;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeSet;

use ncdb_core::basic::{BasicValue, Kind, Write};
use ncdb_core::cbor::Value;
use ncdb_core::change::{Change, Operation};
use ncdb_core::checks::SealChecks;
use ncdb_core::clock::Hlc;
use ncdb_core::field_op;
use ncdb_core::ids::{ChangeId, MemberId, ObjectId, OperationId};
use ncdb_core::log::{RejectReason, State};
use ncdb_core::membership::Role;
use ncdb_core::set::{Action, Edit, Set};
use ncdb_core::view::View;

use space::{Space, key};
use view_support::*;

const ALICE: u8 = 1;
const BOB: u8 = 2;
const CAROL: u8 = 3;

const T: ObjectId = ObjectId([0x11; 16]);
const U: ObjectId = ObjectId([0x22; 16]);

/// An id the encodings ignore: a change's operations take theirs from it.
const ANY: OperationId = OperationId {
    change: ChangeId([0; 32]),
    position: 0,
};

fn write(value: Value) -> Write {
    Write {
        id: ANY,
        author: MemberId([0; 32]),
        stamp: Hlc(0),
        value,
        replaces: BTreeSet::new(),
    }
}

fn title(object: ObjectId, value: &str) -> Operation {
    let text = BasicValue { kind: Kind::Text };
    let value = Value::Text(value.to_owned());
    field_op::encode(&text, &write(value), object, TASK, TITLE)
}

fn tag(object: ObjectId, element: &str) -> Operation {
    let edit = Edit {
        id: ANY,
        author: MemberId([0; 32]),
        stamp: Hlc(0),
        element: Value::Text(element.to_owned()),
        action: Action::Add {
            replaces: BTreeSet::new(),
        },
    };
    field_op::encode(&Set { kind: Kind::Text }, &edit, object, TASK, TAGS)
}

fn delete(object: ObjectId) -> Operation {
    field_op::delete(object, TASK, &write(Value::Bool(true)))
}

/// Alice founds the space with Bob as a writer and Carol as a reader, and
/// wraps the space key to Carol. Alice creates task T with a title. Bob,
/// later in clock order, overwrites T's title, deletes T, and creates task
/// U with a title and a tag. Alice then removes Bob with cutoff 0.
struct Written {
    space: Space,
    before: Vec<Change>,
    bob: Vec<Change>,
    removal: Change,
}

fn written() -> Written {
    let mut space = Space::new(ALICE);
    let genesis = space.genesis.clone();
    let bob = space.add(ALICE, BOB, Role::Writer, &[&genesis]);
    let carol = space.add(ALICE, CAROL, Role::Reader, &[&bob]);
    let to_carol = space.envelope(ALICE, CAROL, &[&carol]);
    let alice = space.sealed(ALICE, vec![field_op::create(T, TASK), title(T, "Alice's")]);
    space.skip(BOB, 100);
    let overwrite = space.sealed(BOB, vec![title(T, "Bob's")]);
    let deletion = space.sealed(BOB, vec![delete(T)]);
    let created = space.sealed(
        BOB,
        vec![field_op::create(U, TASK), title(U, "new"), tag(U, "x")],
    );
    let removal = space.remove(ALICE, BOB, 0, &[&to_carol]);
    Written {
        space,
        before: vec![genesis, bob, carol, to_carol, alice],
        bob: vec![overwrite, deletion, created],
        removal,
    }
}

/// Carol's view and her checks, after these changes in this order.
fn view(w: &Written, db: &MemDb, changes: &[&Change]) -> (View, SealChecks) {
    let mut view = View::open(&db.0, &lock()).unwrap();
    let mut checks = SealChecks::new(w.space.space, key(CAROL));
    for change in changes {
        view.receive(change.bytes(), &mut checks).unwrap();
    }
    (view, checks)
}

#[test]
fn a_late_revocation_takes_the_changes_it_rejects_out_of_the_tables() {
    let w = written();
    let space = w.space.space;
    let rejected = Some(State::Rejected(RejectReason::NotMember));
    let late: Vec<&Change> = w.before.iter().chain(&w.bob).collect();
    let late_db = MemDb::new();
    let (mut late_view, mut checks) = view(&w, &late_db, &late);
    for change in &w.bob {
        let state = late_view.state(space, change.id());
        assert_eq!(state, Some(State::Folded { parked: vec![] }), "before");
    }
    late_view.receive(w.removal.bytes(), &mut checks).unwrap();

    let never_db = MemDb::new();
    let never: Vec<&Change> = w.before.iter().chain([&w.removal]).collect();
    let _never_view = view(&w, &never_db, &never);

    assert!(
        view_bytes(&late_db.connect()) == view_bytes(&never_db.connect()),
        "the tables equal those of a view that never folded Bob's changes"
    );
    for change in &w.bob {
        assert_eq!(late_view.state(space, change.id()), rejected);
    }
    // Reopening reads the log back from its table: the un-folded changes
    // are stored as rejected.
    let reopened = View::open(&late_db.0, &lock()).unwrap();
    for change in &w.bob {
        assert_eq!(reopened.state(space, change.id()), rejected, "stored");
    }
}
