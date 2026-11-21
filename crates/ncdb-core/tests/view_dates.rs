//! Days and points in time in the local view (RFC 0005, "Days and points
//! in time"; RFC 0004, "Days and points in time"): each is one `INTEGER`
//! column, the days since 1970-01-01 or the milliseconds since
//! 1970-01-01T00:00:00Z, so an index sorts them in time order, before 1970
//! included. A value that does not fit, an integer outside the years 1 to
//! 9999, a float or a text, is kept in the merge state and indexes as
//! `NULL`, as an unset field does.

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
use ncdb_core::change::{Header, Kind as ChangeKind, Operation, Payload};
use ncdb_core::clock::Hlc;
use ncdb_core::field_op;
use ncdb_core::ids::{ChangeId, ObjectId, OperationId};
use ncdb_core::model_lock::ModelLock;
use ncdb_core::view::{Field, Model, View};

use member::*;
use view_support::MemDb;

/// Events with a day and a point in time, each with an index that fixes
/// nothing, so a page reads every event in the index's order.
const LOCK: &str = r#"{
  "collections": {
    "events": {
      "fields": {
        "doneOn": { "rule": 1, "since": 1, "type": "ncdb/date" },
        "occurredAt": { "rule": 1, "since": 1, "type": "ncdb/timestamp" }
      },
      "indexes": {
        "byDoneOn": { "equal": 0, "fields": [{ "direction": "asc", "field": "doneOn" }], "since": 1 },
        "byOccurredAt": { "equal": 0, "fields": [{ "direction": "desc", "field": "occurredAt" }], "since": 1 }
      },
      "since": 1
    }
  },
  "format": 1,
  "model": { "hash": "sha256:00", "id": "000102030405060708090a0b0c0d0e0f", "version": 1 },
  "plugins": {}
}"#;

const EVENTS: &str = "events";
const DAY: &str = "doneOn";
const POINT: &str = "occurredAt";

/// 0001-01-01 and 9999-12-31, in days since 1970-01-01.
const FIRST_DAY: i64 = -719_162;
const LAST_DAY: i64 = 2_932_896;
/// 0001-01-01T00:00:00.000Z and 9999-12-31T23:59:59.999Z, in milliseconds.
const FIRST_POINT: i64 = -62_135_596_800_000;
const LAST_POINT: i64 = 253_402_300_799_999;

fn lock() -> ModelLock {
    ModelLock::from_json(LOCK).unwrap()
}

/// An integer as the CBOR value RFC 0002's profile writes for it.
fn int(n: i64) -> Value {
    if n >= 0 {
        Value::Unsigned(n as u64)
    } else {
        Value::Negative((-1 - n) as u64)
    }
}

/// Event `n`, created with `field` (of `kind`) set to `value`.
fn event(n: u8, kind: Kind, field: &str, value: Value) -> Vec<Operation> {
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
        field_op::create(object, EVENTS),
        field_op::encode(&BasicValue { kind }, &write, object, EVENTS, field),
    ]
}

fn day(n: u8, value: Value) -> Vec<Operation> {
    event(n, Kind::Date, DAY, value)
}

fn point(n: u8, value: Value) -> Vec<Operation> {
    event(n, Kind::Timestamp, POINT, value)
}

/// A view of [`LOCK`] in `db` that received `operations` in one change.
fn view_with(db: &MemDb, operations: Vec<Operation>) -> View {
    let mut view = View::open(&db.0, &lock()).unwrap();
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

/// The column `field` of each event, by the event's first id byte.
fn column(db: &MemDb, field: &str) -> Vec<(u8, Option<i64>)> {
    let conn = db.connect();
    let mut statement = conn
        .prepare(&format!(
            "SELECT object_id, f_{field} FROM c_events ORDER BY object_id"
        ))
        .unwrap();
    statement
        .query_map([], |r| Ok((r.get::<_, Vec<u8>>(0)?[0], r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

/// The SQLite type of the column `field`.
fn column_type(db: &MemDb, field: &str) -> String {
    db.connect()
        .query_row(
            &format!("SELECT type FROM pragma_table_info('c_events') WHERE name = 'f_{field}'"),
            [],
            |r| r.get(0),
        )
        .unwrap()
}

/// The events of a page of `index`, by their first id byte.
fn order(view: &View, index: &str) -> Vec<u8> {
    let page = view.page(EVENTS, index, &[], None, 10).unwrap();
    page.rows.iter().map(|(_, o)| o.0[0]).collect()
}

#[test]
fn the_view_reads_both_as_basic_values_from_the_lock() {
    let model = Model::from_lock(&lock()).unwrap();
    assert_eq!(
        model.field(EVENTS, DAY),
        Some(&Field::Basic(BasicValue { kind: Kind::Date }))
    );
    assert_eq!(
        model.field(EVENTS, POINT),
        Some(&Field::Basic(BasicValue {
            kind: Kind::Timestamp
        }))
    );
}

#[test]
fn a_day_and_a_point_in_time_are_integer_columns() {
    let db = MemDb::new();
    let _view = view_with(
        &db,
        [day(1, int(20_778)), point(2, int(1_795_288_124_636))].concat(),
    );
    assert_eq!(column_type(&db, DAY), "INTEGER");
    assert_eq!(column_type(&db, POINT), "INTEGER");
    assert_eq!(column(&db, DAY), vec![(1, Some(20_778)), (2, None)]);
    assert_eq!(
        column(&db, POINT),
        vec![(1, None), (2, Some(1_795_288_124_636))]
    );
}

#[test]
fn the_ends_of_the_range_fit_and_one_beyond_does_not() {
    let db = MemDb::new();
    let _view = view_with(
        &db,
        [
            day(1, int(FIRST_DAY)),
            day(2, int(LAST_DAY)),
            day(3, int(FIRST_DAY - 1)),
            day(4, int(LAST_DAY + 1)),
            point(5, int(FIRST_POINT)),
            point(6, int(LAST_POINT)),
            point(7, int(FIRST_POINT - 1)),
            point(8, int(LAST_POINT + 1)),
        ]
        .concat(),
    );
    let days: Vec<_> = column(&db, DAY).into_iter().take(4).collect();
    assert_eq!(
        days,
        vec![
            (1, Some(FIRST_DAY)),
            (2, Some(LAST_DAY)),
            (3, None),
            (4, None)
        ]
    );
    let points: Vec<_> = column(&db, POINT).into_iter().skip(4).collect();
    assert_eq!(
        points,
        vec![
            (5, Some(FIRST_POINT)),
            (6, Some(LAST_POINT)),
            (7, None),
            (8, None)
        ]
    );
}

#[test]
fn a_value_of_another_kind_indexes_as_null() {
    let db = MemDb::new();
    let _view = view_with(
        &db,
        [
            day(1, Value::Float(20_778.0)),
            day(2, Value::Text("2026-11-21".into())),
            day(3, Value::Null),
            day(4, int(20_778)),
        ]
        .concat(),
    );
    assert_eq!(
        column(&db, DAY),
        vec![(1, None), (2, None), (3, None), (4, Some(20_778))]
    );
}

#[test]
fn days_sort_in_time_order_with_unset_first() {
    let db = MemDb::new();
    let view = view_with(
        &db,
        [
            day(1, int(20_778)), // 2026-11-21
            day(2, int(-1)),     // 1969-12-31
            day(3, Value::Null), // unset
            day(4, int(0)),      // 1970-01-01
            day(5, int(FIRST_DAY)),
        ]
        .concat(),
    );
    assert_eq!(order(&view, "byDoneOn"), [3, 5, 2, 4, 1]);
}

#[test]
fn points_in_time_sort_newest_first_in_a_descending_index() {
    let db = MemDb::new();
    let view = view_with(
        &db,
        [
            point(1, int(1_795_288_124_636)),
            point(2, int(-86_400_000)),
            point(3, int(1_795_288_124_637)),
            point(4, Value::Null),
        ]
        .concat(),
    );
    // Descending: the latest first, unset last.
    assert_eq!(order(&view, "byOccurredAt"), [3, 1, 2, 4]);
}

#[test]
fn the_model_hash_tells_a_day_from_an_int_and_from_a_point_in_time() {
    let hash = |text: &str| {
        Model::from_lock(&ModelLock::from_json(text).unwrap())
            .unwrap()
            .hash()
    };
    let day = hash(LOCK);
    let int = hash(&LOCK.replace("\"ncdb/date\"", "\"ncdb/int\""));
    let point = hash(&LOCK.replace("\"ncdb/date\"", "\"ncdb/timestamp\""));
    assert_ne!(day, int);
    assert_ne!(day, point);
}
