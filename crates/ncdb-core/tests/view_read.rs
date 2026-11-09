//! Reading the local view on SQLite (NOC-69; RFC 0006, "Declared indexes
//! and ordered pages"): an object by its id, and one lookup through a
//! declared index, page by page with a cursor, in an order that is the
//! same on every member. Full queries come with the SDK.
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
use ncdb_core::change::{Operation, Payload};
use ncdb_core::clock::Hlc;
use ncdb_core::field_op;
use ncdb_core::ids::{ChangeId, ObjectId, OperationId};
use ncdb_core::view::{Error, QueryError, View};

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

fn set(field: &str, kind: Kind, object: ObjectId, value: Value) -> Operation {
    field_op::encode(&BasicValue { kind }, &write(value), object, TASK, field)
}

/// A project `n` with a name.
fn project(n: u8, name: &str) -> Vec<Operation> {
    let object = ObjectId([n; 16]);
    let text = BasicValue { kind: Kind::Text };
    let name = Value::Text(name.to_owned());
    vec![
        field_op::create(object, PROJECT),
        field_op::encode(&text, &write(name), object, PROJECT, NAME),
    ]
}

/// A task `n` with an optional title and rank.
fn task(n: u8, title: Option<&str>, rank: Option<u64>) -> Vec<Operation> {
    let object = ObjectId([n; 16]);
    let mut operations = vec![field_op::create(object, TASK)];
    if let Some(title) = title {
        let title = Value::Text(title.to_owned());
        operations.push(set(TITLE, Kind::Text, object, title));
    }
    if let Some(rank) = rank {
        operations.push(set(RANK, Kind::Integer, object, Value::Unsigned(rank)));
    }
    operations
}

/// A view that received one change with these operations.
fn view_with(operations: Vec<Operation>) -> View {
    let mut view = View::open_in_memory(&lock()).unwrap();
    let mut control = Control::default();
    view.receive(&envelope(FOUNDER, 1, 0), &mut control)
        .unwrap();
    let payload = Payload::new(Hlc(1), BTreeSet::new(), operations).encode();
    let change = frame(sealed_header(FOUNDER, 2, 0, payload), GOOD);
    view.receive(&change, &mut control).unwrap();
    view
}

/// Every row of an index for `equal`, page by page, `limit` at a time.
fn all_pages(view: &View, index: &str, equal: &[Value], limit: usize) -> Vec<u8> {
    pages_of(view, TASK, index, equal, limit)
}

/// Every row of the index `index` of `collection`, page by page.
fn pages_of(view: &View, collection: &str, index: &str, equal: &[Value], limit: usize) -> Vec<u8> {
    let mut ids = Vec::new();
    let mut cursor = None;
    loop {
        let page = view
            .page(collection, index, equal, cursor.as_ref(), limit)
            .unwrap();
        assert!(page.rows.len() <= limit);
        ids.extend(page.rows.iter().map(|(space, object)| {
            assert_eq!(*space, SPACE);
            object.0[0]
        }));
        match page.next {
            Some(next) => cursor = Some(next),
            None => return ids,
        }
    }
}

#[test]
fn an_object_is_read_by_its_id_and_one_the_view_does_not_hold_is_none() {
    let view = view_with(task(1, Some("a"), Some(2)));
    let row = view.get(SPACE, TASK, ObjectId([1; 16])).unwrap().unwrap();
    assert!(row.object.exists());
    assert_eq!(
        row.fields.keys().collect::<Vec<_>>(),
        [&RANK.to_owned(), &TITLE.to_owned()]
    );
    assert_eq!(view.get(SPACE, TASK, ObjectId([9; 16])).unwrap(), None);
    assert_eq!(view.get(SPACE, "nope", ObjectId([1; 16])).unwrap(), None);
}

#[test]
fn a_page_lists_rows_in_index_order_with_an_unset_value_first() {
    let operations = [
        task(1, None, Some(3)),
        task(2, None, None),
        task(3, None, Some(1)),
        task(4, None, Some(2)),
        task(5, None, None),
        task(6, None, Some(3)),
        task(7, None, Some(5)),
    ]
    .concat();
    let view = view_with(operations);
    // Ties in rank go by space id, then object id.
    let expected = [2, 5, 3, 4, 1, 6, 7];
    for limit in 1..=8 {
        assert_eq!(all_pages(&view, BY_RANK, &[], limit), expected, "{limit}");
    }
}

#[test]
fn a_page_of_an_index_fixes_its_equality_prefix_and_orders_a_descending_key() {
    let operations = [
        task(1, Some("a"), Some(1)),
        task(2, Some("a"), None),
        task(3, Some("b"), Some(9)),
        task(4, Some("a"), Some(4)),
        task(5, Some("a"), Some(4)),
        task(6, None, Some(2)),
    ]
    .concat();
    let view = view_with(operations);
    let a = [Value::Text("a".into())];
    // Descending: the largest rank first, an unset rank last; ties by
    // space id, then object id, ascending.
    for limit in 1..=5 {
        assert_eq!(all_pages(&view, BY_TITLE, &a, limit), [4, 5, 1, 2]);
    }
    assert_eq!(all_pages(&view, BY_TITLE, &[Value::Null], 2), [6]);
}

#[test]
fn text_sorts_by_its_utf8_bytes_never_by_a_locale() {
    let operations = [
        task(1, Some("b"), None),
        task(2, Some("\u{c4}"), None),
        task(3, Some("a"), None),
        task(4, Some("B"), None),
    ]
    .concat();
    let view = view_with(operations);
    // B, a, b, then "A" with a diaeresis: byte order puts it after every
    // ASCII letter, where a locale would put it next to "A".
    assert_eq!(all_pages(&view, BY_NAME, &[], 3), [4, 3, 1, 2]);
}

#[test]
fn a_lookup_that_does_not_fit_its_index_is_refused() {
    let view = view_with(task(1, Some("a"), Some(1)));
    let error = |result: Result<_, Error>| match result {
        Err(Error::Query(error)) => error,
        other => panic!("{other:?}"),
    };
    assert_eq!(
        error(view.page(TASK, "nope", &[], None, 10)),
        QueryError::UnknownIndex
    );
    // An index of another collection is not one of this collection's.
    assert_eq!(
        error(view.page(PROJECT, BY_RANK, &[], None, 10)),
        QueryError::UnknownIndex
    );
    assert_eq!(
        error(view.page(TASK, BY_TITLE, &[], None, 10)),
        QueryError::Equality
    );
    assert_eq!(
        error(view.page(TASK, BY_RANK, &[], None, 0)),
        QueryError::PageSize
    );
    assert_eq!(
        error(view.page(TASK, BY_RANK, &[], None, View::MAX_PAGE + 1)),
        QueryError::PageSize
    );
    let two = view_with([task(1, None, Some(1)), task(2, None, Some(2))].concat());
    let cursor = two.page(TASK, BY_RANK, &[], None, 1).unwrap().next.unwrap();
    let a = [Value::Text("a".into())];
    assert_eq!(
        error(view.page(TASK, BY_TITLE, &a, Some(&cursor), 10)),
        QueryError::Cursor
    );
    // A cursor from a view of another model is refused too.
    let other = View::open_in_memory(&lock_without(TASK, BY_NAME)).unwrap();
    assert_eq!(
        error(other.page(TASK, BY_RANK, &[], Some(&cursor), 10)),
        QueryError::Cursor
    );
}

#[test]
fn a_cursor_does_not_show_the_values_it_holds() {
    let view = view_with(
        [
            task(1, Some("secret"), Some(1)),
            task(2, Some("secret"), None),
        ]
        .concat(),
    );
    let page = view
        .page(TASK, BY_TITLE, &[Value::Text("secret".into())], None, 1)
        .unwrap();
    let shown = format!("{:?}", page.next.unwrap());
    assert!(!shown.contains("secret"), "{shown}");
}

#[test]
fn the_lookup_of_every_declared_index_runs_through_it_without_a_temporary_sort() {
    let view = View::open_in_memory(&lock()).unwrap();
    let indexes = [
        (TASK, BY_RANK),
        (TASK, BY_TITLE),
        (TASK, BY_NAME),
        (PROJECT, BY_NAME),
    ];
    for (collection, index) in indexes {
        let plans = view.plans(collection, index).unwrap();
        assert_eq!(plans.len(), 2, "the first page and a page after a cursor");
        for plan in plans {
            assert!(
                plan.contains(&format!("INDEX ix_{collection}_{index}")),
                "{index}: {plan}"
            );
            assert!(!plan.contains("TEMP B-TREE"), "{index}: {plan}");
        }
    }
}

#[test]
fn two_collections_each_read_through_their_own_index_of_the_same_name() {
    let operations = [
        task(1, Some("b"), None),
        task(2, Some("a"), None),
        project(3, "z"),
        project(4, "y"),
        project(5, "x"),
    ]
    .concat();
    let view = view_with(operations);
    assert_eq!(pages_of(&view, TASK, BY_NAME, &[], 1), [2, 1]);
    assert_eq!(pages_of(&view, PROJECT, BY_NAME, &[], 2), [5, 4, 3]);
    // A cursor of one collection's index does not go on in the other's.
    let cursor = view.page(TASK, BY_NAME, &[], None, 1).unwrap().next;
    assert!(matches!(
        view.page(PROJECT, BY_NAME, &[], cursor.as_ref(), 1),
        Err(Error::Query(QueryError::Cursor))
    ));
}
