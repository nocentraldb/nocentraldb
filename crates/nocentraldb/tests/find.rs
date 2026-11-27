//! Queries run only through declared indexes (RFC 0009, "Queries and
//! indexes"): a query names an index, fixes its equality prefix and asks
//! for a page; `db.find` reads an index that spans every space of the
//! member, `space.find` one that runs in one space. Pages come in index
//! order, and the cursor of one page starts the next.

mod support;

use nocentraldb::{Db, ErrorCode, ObjectId, Space, Value};
use support::{now, open};

/// Writes a task with this title, status and priority into `space`.
fn task(space: &Space, title: &str, status: &str, priority: i64) -> ObjectId {
    let written = now(space.write(|tx| {
        tx.create(
            "tasks",
            [
                ("title", Value::from(title)),
                ("status", Value::from(status)),
                ("priority", Value::from(priority)),
            ],
        )?;
        Ok(())
    }))
    .unwrap();
    written.ids[0]
}

fn titles(page: &nocentraldb::Page) -> Vec<&str> {
    page.items
        .iter()
        .map(|item| item.get::<&str>("title").unwrap().unwrap())
        .collect()
}

/// Two spaces with open and done tasks.
fn two_spaces(db: &Db) -> (Space, Space) {
    let a = now(db.spaces().create()).unwrap();
    let b = now(db.spaces().create()).unwrap();
    task(&a, "a3", "open", 3);
    task(&b, "b1", "open", 1);
    task(&a, "a2", "done", 2);
    task(&b, "b4", "open", 4);
    task(&a, "a5", "open", 5);
    (a, b)
}

#[test]
fn db_find_reads_pages_of_an_index_across_every_space_in_index_order() {
    let db = open();
    let (a, b) = two_spaces(&db);
    let query = db
        .index("tasks", "byStatusPriority")
        .unwrap()
        .equal(["open"])
        .take(2);

    let first = now(db.find(&query)).unwrap();
    assert_eq!(titles(&first), ["b1", "a3"]);
    assert_eq!(first.items[0].space(), b.id());
    assert_eq!(first.items[1].space(), a.id());
    assert!(first.has_more);

    let cursor = first.cursor.clone().unwrap();
    let second = now(db.find(&query.after(cursor))).unwrap();
    assert_eq!(titles(&second), ["b4", "a5"]);
    assert!(!second.has_more);
}

#[test]
fn where_eq_reads_the_pages_equal_reads() {
    // RFC 0018, R3: `where_eq` is the Rust spelling of TypeScript's
    // `.where(...)`, and `equal` stays as the same call.
    let db = open();
    two_spaces(&db);
    let index = db.index("tasks", "byStatusPriority").unwrap();
    let by_where = index.clone().where_eq(["open"]).take(2);
    let by_equal = index.equal(["open"]).take(2);

    let (first, same) = (
        now(db.find(&by_where)).unwrap(),
        now(db.find(&by_equal)).unwrap(),
    );
    assert_eq!(titles(&first), ["b1", "a3"]);
    assert_eq!(first, same);

    let after = |query: &nocentraldb::Query, page: &nocentraldb::Page| {
        now(db.find(&query.clone().after(page.cursor.clone().unwrap()))).unwrap()
    };
    let (second, again) = (after(&by_where, &first), after(&by_equal, &same));
    assert_eq!(titles(&second), ["b4", "a5"]);
    assert_eq!(second, again);
}

#[test]
fn a_query_fixes_a_where_field_to_unset_with_a_cleared_value() {
    // RFC 0006, "Declared indexes and ordered pages": the equality prefix
    // is written `k IS ?`, so a lookup can fix a key to unset.
    let db = open();
    let (a, _) = two_spaces(&db);
    now(a.write(|tx| {
        tx.create(
            "tasks",
            [("title", Value::from("u2")), ("priority", Value::from(2))],
        )?;
        tx.create(
            "tasks",
            [("title", Value::from("u1")), ("priority", Value::from(1))],
        )?;
        Ok(())
    }))
    .unwrap();
    let index = db.index("tasks", "byStatusPriority").unwrap();

    let by_where = now(db.find(&index.clone().where_eq([Value::clear()]))).unwrap();
    let by_equal = now(db.find(&index.equal([Value::clear()]))).unwrap();

    assert_eq!(titles(&by_where), ["u1", "u2"]);
    assert_eq!(by_where, by_equal);
}

#[test]
fn an_index_without_an_equality_prefix_takes_no_values() {
    let db = open();
    two_spaces(&db);

    let query = db.index("tasks", "byPriority").unwrap().take(3);
    let page = now(db.find(&query)).unwrap();

    assert_eq!(titles(&page), ["b1", "a2", "a3"]);
}

#[test]
fn a_query_leaves_deleted_objects_out() {
    let db = open();
    let (a, _) = two_spaces(&db);
    let a3 = task(&a, "a0", "open", 0);
    now(a.write(|tx| tx.delete("tasks", a3))).unwrap();

    let query = db
        .index("tasks", "byStatusPriority")
        .unwrap()
        .equal(["open"]);
    let page = now(db.find(&query)).unwrap();

    assert_eq!(titles(&page), ["b1", "a3", "b4", "a5"]);
}

#[test]
fn space_find_reads_an_index_that_runs_in_one_space() {
    let db = open();
    let (a, b) = two_spaces(&db);
    let query = db.index("tasks", "open").unwrap().equal(["open"]);

    assert_eq!(titles(&now(a.find(&query)).unwrap()), ["a3", "a5"]);
    assert_eq!(titles(&now(b.find(&query)).unwrap()), ["b1", "b4"]);
}

#[test]
fn an_index_of_the_other_scope_is_refused() {
    let db = open();
    let (a, _) = two_spaces(&db);
    let in_space = db.index("tasks", "open").unwrap().equal(["open"]);
    let everywhere = db.index("tasks", "byPriority").unwrap();

    assert_eq!(
        now(db.find(&in_space)).unwrap_err().code(),
        ErrorCode::InvalidValue
    );
    assert_eq!(
        now(a.find(&everywhere)).unwrap_err().code(),
        ErrorCode::InvalidValue
    );
}

#[test]
fn a_query_with_the_wrong_number_of_values_or_page_size_is_refused() {
    let db = open();
    let index = db.index("tasks", "byStatusPriority").unwrap();

    let none = now(db.find(&index)).unwrap_err();
    let two = now(db.find(&index.clone().equal(["open", "done"]))).unwrap_err();
    let empty = now(db.find(&index.equal(["open"]).take(0))).unwrap_err();

    for error in [none, two, empty] {
        assert_eq!(error.code(), ErrorCode::InvalidValue);
    }
}

#[test]
fn a_cursor_of_another_index_is_refused() {
    let db = open();
    two_spaces(&db);
    let by_priority = db.index("tasks", "byPriority").unwrap().take(1);
    let cursor = now(db.find(&by_priority)).unwrap().cursor.unwrap();

    let other = db
        .index("tasks", "byStatusPriority")
        .unwrap()
        .equal(["open"])
        .after(cursor);
    let error = now(db.find(&other)).unwrap_err();

    assert_eq!(error.code(), ErrorCode::CursorOfAnotherModel);
}

#[test]
fn an_index_the_model_does_not_declare_is_not_found() {
    let db = open();

    assert_eq!(
        db.index("tasks", "byTitle").unwrap_err().code(),
        ErrorCode::NotFound
    );
    assert_eq!(
        db.index("people", "byName").unwrap_err().code(),
        ErrorCode::NotFound
    );
}

#[test]
fn a_cursor_prints_redacted() {
    let db = open();
    two_spaces(&db);
    let query = db
        .index("tasks", "byStatusPriority")
        .unwrap()
        .equal(["open"])
        .take(1);
    let page = now(db.find(&query)).unwrap();

    let shown = format!("{:?} {:?}", page.cursor.unwrap(), query);
    assert!(!shown.contains("open"), "{shown}");
    assert!(!shown.contains("b1"), "{shown}");
}
