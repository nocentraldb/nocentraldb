//! An index declared with `scope: "space"` (RFC 0006, "Declared
//! indexes"; NOC-81): the space id comes first and the object id last, a lookup
//! fixes the space as the start of its equality prefix, and the cursor and
//! the plan check ("The check") apply unchanged.

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
use ncdb_core::ids::{ChangeId, ObjectId, OperationId, SpaceId};
use ncdb_core::view::{Error, QueryError, View};

use member::*;
use view_support::*;

/// A second space of the member.
const OTHER_SPACE: SpaceId = SpaceId([2; 16]);
/// Tasks of one space by rank (declared in `tests/view/model.ts`).
const IN_SPACE: &str = "inSpace";
/// Tasks of one space with a title, by rank, descending.
const IN_SPACE_BY_TITLE: &str = "inSpaceByTitle";

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

/// Task `n`, with an optional title and a rank.
fn task(n: u8, title: Option<&str>, rank: u64) -> Vec<Operation> {
    let object = ObjectId([n; 16]);
    let field = |name: &str, kind, value| {
        field_op::encode(&BasicValue { kind }, &write(value), object, TASK, name)
    };
    let mut operations = vec![
        field_op::create(object, TASK),
        field(RANK, Kind::Integer, Value::Unsigned(rank)),
    ];
    if let Some(title) = title {
        operations.push(field(TITLE, Kind::Text, Value::Text(title.into())));
    }
    operations
}

/// A view of the scoped lock that received one change in each space.
fn view_with(spaces: [(SpaceId, Vec<Operation>); 2]) -> View {
    let mut view = View::open_in_memory(&lock()).unwrap();
    let mut control = Control::default();
    view.receive(&envelope(FOUNDER, 1, 0), &mut control)
        .unwrap();
    for (sequence, (space, operations)) in (2..).zip(spaces) {
        let payload = Payload::new(Hlc(1), BTreeSet::new(), operations).encode();
        let header = Header::new(
            space,
            member(FOUNDER),
            sequence,
            0,
            ChangeKind::SEALED,
            payload,
            SUITE,
        );
        view.receive(&frame(header, GOOD), &mut control).unwrap();
    }
    view
}

/// Every row of the space-scoped `index` in `space`, page by page.
fn pages_in(view: &View, space: SpaceId, index: &str, equal: &[Value], limit: usize) -> Vec<u8> {
    let mut ids = Vec::new();
    let mut cursor = None;
    loop {
        let page = view
            .page_in(space, TASK, index, equal, cursor.as_ref(), limit)
            .unwrap();
        assert!(page.rows.len() <= limit);
        ids.extend(page.rows.iter().map(|(s, object)| {
            assert_eq!(*s, space);
            object.0[0]
        }));
        match page.next {
            Some(next) => cursor = Some(next),
            None => return ids,
        }
    }
}

/// The columns of an SQLite index, in order.
fn columns(view_db: &MemDb, index: &str) -> Vec<String> {
    let conn = view_db.connect();
    let mut statement = conn
        .prepare(&format!(
            "SELECT name FROM pragma_index_info('{index}') ORDER BY seqno"
        ))
        .unwrap();
    statement
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

#[test]
fn a_space_scoped_index_puts_the_space_id_first_and_the_object_id_last() {
    let db = MemDb::new();
    let _view = View::open(&db.0, &lock()).unwrap();
    assert_eq!(
        columns(&db, "ix_task_inSpace"),
        ["space_id", "f_rank", "object_id"]
    );
    assert_eq!(
        columns(&db, "ix_task_inSpaceByTitle"),
        ["space_id", "f_title", "f_rank", "object_id"]
    );
    // An index without the option spans every space, as before.
    assert_eq!(
        columns(&db, "ix_task_byRank"),
        ["f_rank", "space_id", "object_id"]
    );
}

#[test]
fn a_lookup_in_one_space_reads_only_that_space_in_index_order() {
    let view = view_with([
        (
            SPACE,
            [task(1, None, 3), task(2, None, 1), task(3, None, 2)].concat(),
        ),
        (OTHER_SPACE, [task(4, None, 0), task(5, None, 9)].concat()),
    ]);
    for limit in 1..=4 {
        assert_eq!(pages_in(&view, SPACE, IN_SPACE, &[], limit), [2, 3, 1]);
        assert_eq!(pages_in(&view, OTHER_SPACE, IN_SPACE, &[], limit), [4, 5]);
    }
    // The index spanning every space still reads both.
    let page = view.page(TASK, BY_RANK, &[], None, 10).unwrap();
    assert_eq!(page.rows.len(), 5);
}

#[test]
fn the_equality_prefix_after_the_space_and_a_descending_key_work_as_before() {
    let view = view_with([
        (
            SPACE,
            [
                task(1, Some("a"), 1),
                task(2, Some("a"), 4),
                task(3, Some("b"), 9),
                task(4, Some("a"), 4),
            ]
            .concat(),
        ),
        (OTHER_SPACE, task(5, Some("a"), 7)),
    ]);
    let a = [Value::Text("a".into())];
    for limit in 1..=4 {
        assert_eq!(
            pages_in(&view, SPACE, IN_SPACE_BY_TITLE, &a, limit),
            [2, 4, 1]
        );
    }
    assert_eq!(pages_in(&view, OTHER_SPACE, IN_SPACE_BY_TITLE, &a, 1), [5]);
}

#[test]
fn a_lookup_names_a_space_exactly_when_its_index_is_scoped_to_one() {
    let view = view_with([(SPACE, task(1, None, 1)), (OTHER_SPACE, task(2, None, 2))]);
    let error = |result: Result<_, Error>| match result {
        Err(Error::Query(error)) => error,
        other => panic!("{other:?}"),
    };
    assert_eq!(
        error(view.page(TASK, IN_SPACE, &[], None, 10)),
        QueryError::Space
    );
    assert_eq!(
        error(view.page_in(SPACE, TASK, BY_RANK, &[], None, 10)),
        QueryError::Space
    );
    // A cursor of one space does not go on in another.
    let one = view_with([
        (SPACE, [task(1, None, 1), task(3, None, 3)].concat()),
        (OTHER_SPACE, task(2, None, 2)),
    ]);
    let cursor = one
        .page_in(SPACE, TASK, IN_SPACE, &[], None, 1)
        .unwrap()
        .next;
    assert!(cursor.is_some());
    assert_eq!(
        error(one.page_in(OTHER_SPACE, TASK, IN_SPACE, &[], cursor.as_ref(), 1)),
        QueryError::Cursor
    );
}

#[test]
fn the_lookup_of_a_space_scoped_index_seeks_its_space_without_a_temporary_sort() {
    let view = View::open_in_memory(&lock()).unwrap();
    for index in [IN_SPACE, IN_SPACE_BY_TITLE] {
        let plans = view.plans(TASK, index).unwrap();
        assert_eq!(plans.len(), 2, "the first page and a page after a cursor");
        for plan in plans {
            // A SEARCH that fixes the space, through the declared index.
            assert!(plan.starts_with("SEARCH c_task USING "), "{index}: {plan}");
            assert!(
                plan.contains(&format!("INDEX ix_task_{index} (space_id=?")),
                "{index}: {plan}"
            );
            assert!(!plan.contains("TEMP B-TREE"), "{index}: {plan}");
        }
    }
}
