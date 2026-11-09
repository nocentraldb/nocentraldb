//! Two creates of one object id, in any order of arrival, end in the same
//! object on every copy (NOC-39; RFC 0004, "Two creates of one object id"),
//! with deletes and restores of it and operations that park mixed in. The
//! convergence harness (NOC-9) delivers the same changes to every copy in
//! different orders and with duplicates.

mod support;

#[path = "support/member.rs"]
mod member;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeSet;

use ncdb_core::basic::{self, Write};
use ncdb_core::cbor::Value;
use ncdb_core::change::{Operation, Payload};
use ncdb_core::clock::Hlc;
use ncdb_core::field_op;
use ncdb_core::ids::{ChangeId, ObjectId, OperationId};
use proptest::prelude::*;
use support::convergence::{self, Config, Mergeable, Op, OpId, Outcome, independent};

use member::*;

/// The object every change creates or deletes.
const T: ObjectId = ObjectId([0x11; 16]);

/// One copy: a member whose control state lets every author write.
#[derive(Clone, Debug)]
struct Copy(Member);

impl Default for Copy {
    fn default() -> Copy {
        let mut m = Member::default();
        m.control.keys.insert(0);
        m.control.members = (1..4).map(|n| (member(n), 0)).collect();
        Copy(m)
    }
}

impl Mergeable for Copy {
    type Change = Vec<u8>;
    /// Whether `T` exists, its collection with conflicts, and whether it
    /// is deleted.
    type Read = (bool, basic::Read, bool);

    fn apply(&mut self, op: &Op<Vec<u8>>) {
        let m = &mut self.0;
        m.log
            .receive(&op.body, &mut m.control, &mut m.fold)
            .expect("every generated frame is appended");
    }

    /// The objects in a canonical order: equal copies, equal bytes.
    fn state_bytes(&self) -> Vec<u8> {
        format!("{:?}", self.0.fold.objects).into_bytes()
    }

    fn read(&self) -> Self::Read {
        let object = self.0.fold.objects.get(&T).cloned().unwrap_or_default();
        (object.exists(), object.collection(), object.is_deleted())
    }

    /// The copy keeps every create and delete: a losing create stays a
    /// conflict of the collection, a losing delete one of the deletion.
    fn outcome(&self, _: OpId) -> Outcome {
        Outcome::Visible
    }
}

/// A delete of `T` that writes `deleted`, replacing nothing.
fn delete(deleted: Value) -> Operation {
    let write = Write {
        id: OperationId {
            change: ChangeId([0; 32]),
            position: 0,
        },
        author: member(0),
        stamp: Hlc(0),
        value: deleted,
        replaces: BTreeSet::new(),
    };
    field_op::delete(T, COLLECTION, &write)
}

/// A change `author` makes: one or two creates of `T` in either
/// collection, a delete or restore of it, or a create that parks. Clocks
/// repeat, so the tie-break by author and position is exercised.
fn change(author: u8) -> BoxedStrategy<Vec<u8>> {
    let collection = prop_oneof![Just(COLLECTION), Just(OTHER)];
    let operation = prop_oneof![
        collection.prop_map(|c| field_op::create(T, c)),
        any::<bool>().prop_map(|d| delete(Value::Bool(d))),
        Just(delete(Value::Text("yes".into()))),
        Just(Operation::new(
            T,
            COLLECTION.into(),
            None,
            0,
            Some(Value::Null)
        )),
    ];
    (1..6u64, 1..4u64, prop::collection::vec(operation, 1..3))
        .prop_map(move |(sequence, clock, operations)| {
            let payload = Payload::new(Hlc(clock), BTreeSet::new(), operations).encode();
            frame(sealed_header(author, sequence, 0, payload), GOOD)
        })
        .boxed()
}

#[test]
fn two_creates_of_one_id_in_any_order_end_in_the_same_object_on_every_copy() {
    let config = Config {
        members: 4,
        max_changes_per_member: 4,
        ..Config::default()
    };
    let result = convergence::check::<Copy, _>(&config, &independent(change));
    assert!(result.is_ok(), "{}", result.unwrap_err());
}
