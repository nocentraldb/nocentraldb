//! The bridge's queries (NOC-176, slice 2; RFC 0009, "Queries and
//! indexes"): pages of a declared index, and the cursors the worker keeps.

use ncdb_core::cbor::Value;

use super::*;

fn find(bridge: &mut Bridge, query: &[(&str, Value)], space: Option<&Value>) -> Value {
    let mut args = vec![("query", map(query))];
    if let Some(space) = space {
        args.push(("space", space.clone()));
    }
    ok(bridge, "find", map(&args))
}

fn priorities(page: &Value) -> Vec<u64> {
    let Some(Value::Array(items)) = field(page, "items") else {
        panic!("{page:?}")
    };
    items
        .iter()
        .map(
            |item| match field(field(item, "fields").unwrap(), "priority") {
                Some(Value::Unsigned(n)) => *n,
                other => panic!("{other:?}"),
            },
        )
        .collect()
}

// db.find (RFC 0009, "Queries and indexes"): a page in index order, the
// cursor after it and whether more follow. The cursor stays in the
// worker; the SDK holds its handle.
#[test]
fn find_pages_through_an_index_with_a_cursor() {
    let mut bridge = opened();
    tasks(&mut bridge, &[(3, "open"), (1, "open"), (2, "done")]);
    let query = [
        ("collection", text("tasks")),
        ("index", text("byPriority")),
        ("take", Value::Unsigned(2)),
    ];
    let first = find(&mut bridge, &query, None);
    assert_eq!(priorities(&first), [1, 2]);
    assert_eq!(field(&first, "hasMore"), Some(&Value::Bool(true)));
    let cursor = field(&first, "cursor").expect("a cursor").clone();
    assert!(matches!(cursor, Value::Unsigned(_)), "a handle: {cursor:?}");

    let mut after = query.to_vec();
    after.push(("after", cursor));
    let second = find(&mut bridge, &after, None);
    assert_eq!(priorities(&second), [3]);
    assert_eq!(field(&second, "hasMore"), Some(&Value::Bool(false)));
    assert_eq!(field(&second, "cursor"), Some(&Value::Null));
    let item = &match field(&second, "items") {
        Some(Value::Array(items)) => items.clone(),
        _ => unreachable!(),
    }[0];
    assert!(field(item, "$space").is_some() && field(item, "$id").is_some());
}

#[test]
fn find_fixes_the_equality_prefix() {
    let mut bridge = opened();
    tasks(&mut bridge, &[(3, "open"), (1, "done"), (2, "open")]);
    let page = find(
        &mut bridge,
        &[
            ("collection", text("tasks")),
            ("index", text("byStatusPriority")),
            ("equal", Value::Array(vec![text("open")])),
        ],
        None,
    );
    assert_eq!(priorities(&page), [2, 3]);
}

#[test]
fn find_in_a_space_runs_an_index_of_one_space() {
    let mut bridge = opened();
    let mine = tasks(&mut bridge, &[(1, "open")]);
    tasks(&mut bridge, &[(2, "open")]);
    let query = [
        ("collection", text("tasks")),
        ("index", text("open")),
        ("equal", Value::Array(vec![text("open")])),
    ];
    assert_eq!(priorities(&find(&mut bridge, &query, Some(&mine))), [1]);
}

#[test]
fn a_cursor_the_worker_does_not_hold_is_of_another_model() {
    let mut bridge = opened();
    tasks(&mut bridge, &[(1, "open")]);
    let refused = code(
        &mut bridge,
        "find",
        map(&[(
            "query",
            map(&[
                ("collection", text("tasks")),
                ("index", text("byPriority")),
                ("after", Value::Unsigned(999)),
            ]),
        )]),
    );
    assert_eq!(refused, "cursorOfAnotherModel");
}

#[test]
fn a_query_of_an_index_the_model_does_not_declare_is_not_found() {
    let mut bridge = opened();
    let refused = code(
        &mut bridge,
        "find",
        map(&[(
            "query",
            map(&[("collection", text("tasks")), ("index", text("nope"))]),
        )]),
    );
    assert_eq!(refused, "notFound");
}

// The fold events (NOC-176): after a write, the worker tells the SDK which
// collections of which space a fold touched, so it re-runs what its
// subscriptions follow (RFC 0009, "Subscribe").

#[test]
fn a_cursor_of_a_closed_database_is_of_another_model() {
    let mut bridge = opened();
    tasks(&mut bridge, &[(1, "open"), (2, "open")]);
    let query = [
        ("collection", text("tasks")),
        ("index", text("byPriority")),
        ("take", Value::Unsigned(1)),
    ];
    let first = find(&mut bridge, &query, None);
    let cursor = field(&first, "cursor").unwrap().clone();
    ok(&mut bridge, "close", map(&[]));
    ok(
        &mut bridge,
        "open",
        map(&[("lock", text(LOCK)), ("inMemory", Value::Bool(true))]),
    );
    tasks(&mut bridge, &[(5, "open"), (6, "open")]);
    // A cursor of the new database takes a handle the old one never had.
    let fresh = field(&find(&mut bridge, &query, None), "cursor")
        .unwrap()
        .clone();
    assert_ne!(fresh, cursor);
    let mut after = query.to_vec();
    after.push(("after", cursor));
    let refused = code(&mut bridge, "find", map(&[("query", map(&after))]));
    assert_eq!(refused, "cursorOfAnotherModel");
}

// db.changes (RFC 0009, "Subscribe"): the folded event names, per
// collection, the ids a fold touched and whether it was this member's own
// write, "local", or one sync brought, "remote".
