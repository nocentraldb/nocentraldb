//! A write whose change would exceed the relay's change limit (NOC-421;
//! NOC-431, B′): it is split at object boundaries into several changes,
//! each within the limit, which the writer folds together. One operation,
//! or one object, that alone exceeds the limit is refused before anything
//! is written.

use std::collections::BTreeMap;

use ncdb_core::change::{Change, Kind};
use ncdb_core::ids::ObjectId as CoreObjectId;
use ncdb_core::receive::Checks;

use crate::{Db, ErrorCode, OpenOptions, Space, Value};

const LOCK: &str = include_str!("../tests/model/model.lock.json");

/// The largest frame the relay accepts (RFC 0007, "Large changes").
const RELAY_LIMIT: usize = 16 * 1024 * 1024;

fn now<F: std::future::Future>(future: F) -> F::Output {
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    match std::pin::pin!(future).poll(&mut cx) {
        std::task::Poll::Ready(output) => output,
        std::task::Poll::Pending => panic!("the SDK never waits on the device"),
    }
}

fn opened() -> (Db, Space) {
    let db = now(Db::open(OpenOptions::new(LOCK).in_memory())).unwrap();
    let space = now(db.spaces().create()).unwrap();
    (db, space)
}

/// A title of 256 KiB, distinct per `n`.
fn title(n: usize) -> String {
    format!("{n:08}{}", "x".repeat(256 * 1024 - 8))
}

/// 72 tasks of 256 KiB titles: about 18 MiB in one write.
const TASKS: usize = 72;

/// The sealed changes of `space` in its log, in log order.
fn sealed(db: &Db, space: &Space) -> Vec<Change> {
    let inner = db.lock();
    let log = inner.view.log(space.id().0).unwrap();
    log.after(ncdb_core::log::Position::default())
        .map(|(_, change)| change.clone())
        .filter(|change| change.header().kind == Kind::SEALED)
        .collect()
}

#[test]
fn a_write_above_the_change_limit_becomes_several_changes_each_within_it() {
    let (db, space) = opened();

    let written = now(space.write(|tx| {
        for n in 0..TASKS {
            tx.create("tasks", [("title", Value::from(title(n).as_str()))])?;
        }
        Ok(())
    }))
    .expect("a write above the limit is split, not refused");

    let changes = sealed(&db, &space);
    assert!(changes.len() > 1, "{} change(s)", changes.len());
    for change in &changes {
        assert!(
            change.bytes().len() <= RELAY_LIMIT,
            "a part of {} bytes",
            change.bytes().len()
        );
    }
    assert_eq!(written.ids.len(), TASKS);
    for (n, id) in written.ids.iter().enumerate() {
        let task = now(space.get("tasks", *id)).unwrap().expect("each task");
        assert_eq!(task.get::<&str>("title").unwrap(), Some(title(n).as_str()));
    }
}

#[test]
fn a_split_write_keeps_each_object_in_one_change_and_chains_its_parts() {
    let (db, space) = opened();
    now(space.write(|tx| {
        for n in 0..TASKS {
            tx.create(
                "tasks",
                [
                    ("title", Value::from(title(n).as_str())),
                    ("priority", Value::from(n as i64)),
                ],
            )?;
        }
        Ok(())
    }))
    .unwrap();

    let changes = sealed(&db, &space);
    assert!(changes.len() > 1, "{} change(s)", changes.len());
    let inner = db.lock();
    let checks = &inner.spaces[&space.id().0].checks;
    let mut part_of: BTreeMap<CoreObjectId, usize> = BTreeMap::new();
    for (part, change) in changes.iter().enumerate() {
        let payload = checks
            .open(change)
            .expect("the member opens its own change");
        for operation in &payload.operations {
            let first = *part_of.entry(operation.object).or_insert(part);
            assert_eq!(first, part, "an object's operations span two changes");
        }
        if part > 0 {
            assert!(
                payload.dependencies.contains(&changes[part - 1].id()),
                "part {part} does not depend on the part before it"
            );
        }
    }
    assert_eq!(part_of.len(), TASKS);
}

#[test]
fn a_single_operation_above_the_limit_is_refused_and_nothing_is_written() {
    let (db, space) = opened();
    let before = sealed(&db, &space).len();
    let mut small = None;

    let refused = now(space.write(|tx| {
        small = Some(tx.create("tasks", [("title", Value::from("small"))])?);
        let huge = "x".repeat(RELAY_LIMIT + 1);
        tx.create("tasks", [("title", Value::from(huge.as_str()))])?;
        Ok(())
    }));

    let error = refused.expect_err("an operation above the limit is refused");
    assert_eq!(error.code(), ErrorCode::TooLarge);
    assert!(
        error.to_string().contains(&RELAY_LIMIT.to_string()),
        "{error}"
    );
    assert_eq!(sealed(&db, &space).len(), before, "a change was written");
    assert!(now(space.get("tasks", small.unwrap())).unwrap().is_none());
}

#[test]
fn an_object_whose_operations_together_exceed_the_limit_is_refused() {
    let (db, space) = opened();
    let before = sealed(&db, &space).len();
    let half = "x".repeat(RELAY_LIMIT / 2 + 1);

    let refused = now(space.write(|tx| {
        tx.create(
            "tasks",
            [
                ("title", Value::from(half.as_str())),
                ("tags", Value::add([half.as_str()])),
            ],
        )?;
        Ok(())
    }));

    let error = refused.expect_err("an object above the limit is refused");
    assert_eq!(error.code(), ErrorCode::TooLarge);
    assert!(
        error.to_string().contains(&RELAY_LIMIT.to_string()),
        "{error}"
    );
    assert_eq!(sealed(&db, &space).len(), before, "a change was written");
}

#[test]
fn a_write_within_the_limit_stays_one_change() {
    let (db, space) = opened();
    let before = sealed(&db, &space).len();

    now(space.write(|tx| {
        for n in 0..8 {
            tx.create("tasks", [("title", Value::from(title(n).as_str()))])?;
        }
        Ok(())
    }))
    .unwrap();

    assert_eq!(sealed(&db, &space).len(), before + 1);
}
