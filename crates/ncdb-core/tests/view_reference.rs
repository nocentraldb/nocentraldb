//! A reference in the local view (RFC 0006, "Declared indexes"): one column of 32
//! bytes, an SQLite `BLOB`, the target's space id followed by its object
//! id, as the reference is stored and whatever its state. An unset
//! reference and a value that does not fit index as `NULL`. "All orders of
//! customer X" is one equality lookup, also for a member that holds no
//! copy of the customers' space.

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

use ncdb_core::basic::Write;
use ncdb_core::cbor::Value;
use ncdb_core::change::{Header, Kind as ChangeKind, Operation, Payload};
use ncdb_core::clock::Hlc;
use ncdb_core::field_op;
use ncdb_core::ids::{ChangeId, ObjectId, OperationId, SpaceId};
use ncdb_core::model_lock::ModelLock;
use ncdb_core::reference::{self, Reference, Target};
use ncdb_core::view::{Field, Model, View};

use member::*;
use view_support::MemDb;

/// Orders that reference customers; the member holds only the orders'
/// space, [`SPACE`].
const LOCK: &str = r#"{
  "collections": {
    "customers": {
      "fields": { "name": { "rule": 1, "since": 1, "type": "ncdb/string" } },
      "since": 1
    },
    "orders": {
      "fields": {
        "customer": { "rule": 1, "since": 1, "targets": ["customers"], "type": "ncdb/ref" }
      },
      "indexes": {
        "byCustomer": {
          "equal": 1,
          "fields": [{ "direction": "asc", "field": "customer" }],
          "since": 1
        }
      },
      "since": 1
    }
  },
  "format": 1,
  "model": { "hash": "sha256:00", "id": "000102030405060708090a0b0c0d0e0f", "version": 1 },
  "plugins": {}
}"#;

const ORDERS: &str = "orders";
const CUSTOMER: &str = "customer";
/// The customers' space, which the member holds no copy of.
const CUSTOMERS: SpaceId = SpaceId([0x33; 16]);

fn lock() -> ModelLock {
    ModelLock::from_json(LOCK).unwrap()
}

/// [`LOCK`] whose index fixes nothing, so that a page reads every order
/// in the index's order.
fn sorted() -> ModelLock {
    let text = LOCK
        .replace("\"byCustomer\"", "\"customerOrder\"")
        .replace("\"equal\": 1", "\"equal\": 0");
    ModelLock::from_json(&text).unwrap()
}

fn customer(n: u8) -> Value {
    reference::value(Target {
        space: CUSTOMERS,
        object: ObjectId([n; 16]),
    })
}

fn field() -> Reference {
    Reference {
        collections: ["customers".to_owned()].into(),
    }
}

/// Order `n`, created with its customer set to `value`.
fn order(n: u8, value: Value) -> Vec<Operation> {
    let object = ObjectId([n; 16]);
    let write = Write {
        id: OperationId {
            change: ChangeId([0; 32]),
            position: 0,
        },
        author: member(0),
        stamp: Hlc(0),
        value,
        replaces: BTreeSet::new(),
    };
    vec![
        field_op::create(object, ORDERS),
        field_op::encode(&field(), &write, object, ORDERS, CUSTOMER),
    ]
}

/// A view of [`LOCK`] in `db` that received `operations` in one change.
fn view_with(db: &MemDb, operations: Vec<Operation>) -> View {
    view_of(&lock(), db, operations)
}

/// A view of `lock` in `db` that received `operations` in one change.
fn view_of(lock: &ModelLock, db: &MemDb, operations: Vec<Operation>) -> View {
    let mut view = View::open(&db.0, lock).unwrap();
    let mut control = Control::default();
    view.receive(&envelope(FOUNDER, 1, 0), &mut control)
        .unwrap();
    let payload = Payload::new(Hlc(1), BTreeSet::new(), operations).encode();
    let header = Header::new(
        SPACE,
        member(FOUNDER),
        2,
        0,
        ChangeKind::SEALED,
        payload,
        SUITE,
    );
    view.receive(&frame(header, GOOD), &mut control).unwrap();
    view
}

/// The column of each order, by the order's first id byte.
fn column(db: &MemDb) -> Vec<(u8, Option<Vec<u8>>)> {
    let conn = db.connect();
    let mut statement = conn
        .prepare("SELECT object_id, f_customer FROM c_orders ORDER BY object_id")
        .unwrap();
    statement
        .query_map([], |r| Ok((r.get::<_, Vec<u8>>(0)?[0], r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

#[test]
fn the_view_reads_a_reference_from_the_lock() {
    let model = Model::from_lock(&lock()).unwrap();
    assert_eq!(model.field(ORDERS, CUSTOMER), Some(&Field::Ref(field())));
    assert!(Field::Ref(field()).has_column());
}

#[test]
fn a_reference_is_one_blob_column_of_the_space_id_then_the_object_id() {
    let db = MemDb::new();
    let _view = view_with(&db, order(1, customer(0x44)));
    let conn = db.connect();
    let kind: String = conn
        .query_row(
            "SELECT type FROM pragma_table_info('c_orders') WHERE name = 'f_customer'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(kind, "BLOB");
    let expected = [[0x33u8; 16], [0x44; 16]].concat();
    assert_eq!(column(&db), vec![(1, Some(expected))]);
}

#[test]
fn an_unset_reference_and_a_value_that_does_not_fit_index_as_null() {
    let db = MemDb::new();
    let bare = Value::Bytes(vec![0x44; 16]);
    let _view = view_with(
        &db,
        [
            order(1, Value::Null),
            order(2, bare),
            order(3, customer(0x44)),
            vec![field_op::create(ObjectId([4; 16]), ORDERS)],
        ]
        .concat(),
    );
    let column = column(&db);
    assert_eq!(column.len(), 4);
    for (n, value) in &column {
        assert_eq!(value.is_none(), *n != 3, "order {n}");
    }
}

#[test]
fn all_orders_of_one_customer_are_one_equality_lookup() {
    let db = MemDb::new();
    let view = view_with(
        &db,
        [
            order(1, customer(0x44)),
            order(2, customer(0x55)),
            order(3, customer(0x44)),
        ]
        .concat(),
    );
    // The member holds no copy of the customers' space: the index holds
    // the reference as stored, whatever its state.
    let page = view
        .page(ORDERS, "byCustomer", &[customer(0x44)], None, 10)
        .unwrap();
    let ids: Vec<u8> = page.rows.iter().map(|(_, o)| o.0[0]).collect();
    assert_eq!(ids, [1, 3]);
    let page = view
        .page(ORDERS, "byCustomer", &[customer(0x55)], None, 10)
        .unwrap();
    assert_eq!(page.rows.len(), 1);
}

#[test]
fn references_sort_by_their_bytes_with_unset_first() {
    let db = MemDb::new();
    let other = |n: u8| {
        reference::value(Target {
            space: SpaceId([0x22; 16]),
            object: ObjectId([n; 16]),
        })
    };
    let view = view_of(
        &sorted(),
        &db,
        [
            order(1, customer(0x11)),
            order(2, other(0x99)),
            order(3, Value::Null),
            order(4, customer(0x01)),
        ]
        .concat(),
    );
    let page = view.page(ORDERS, "customerOrder", &[], None, 10).unwrap();
    let ids: Vec<u8> = page.rows.iter().map(|(_, o)| o.0[0]).collect();
    // Unset, then space 22…, then space 33… by object id.
    assert_eq!(ids, [3, 2, 4, 1]);
}

#[test]
fn the_model_hash_tells_a_reference_from_a_link() {
    let reference = Model::from_lock(&lock()).unwrap();
    let text = LOCK.replace("\"ncdb/ref\"", "\"ncdb/link\"");
    let link = Model::from_lock(&ModelLock::from_json(&text).unwrap()).unwrap();
    assert_ne!(reference.hash(), link.hash());
}
