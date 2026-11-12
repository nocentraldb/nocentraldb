//! Plugin field types in the Rust SDK (RFC 0011; RFC 0009, "Field
//! operations per field type" and "Errors"; Carsten, 2026-11-12, NOC-232),
//! with the test plugin `test/counter`: `OpenOptions::plugin` hands a
//! plugin to the core, a plugin field is written with `Value::from(Data)`
//! through the plugin's `write` and read with `get::<Data>`, a write to a
//! type the database has no code for is refused with `UnknownType`, and
//! what was parked without the plugin is folded once it is registered
//! (NOC-84).

mod support;

use ncdb_plugin::Plugin;
use nocentraldb::{Data, Db, ErrorCode, OpenOptions, ParkReason, Value};
use support::{LOCK, TempDir, now};

/// The tests' lock with a field `votes` of `test/counter`, rule 1, on
/// tasks, and the plugin's entry. The lock's hash is not checked on read.
fn plugin_lock() -> String {
    let lock = LOCK.replacen(
        r#""estimate": { "rule": 1, "since": 1, "type": "ncdb/float" },"#,
        r#""estimate": { "rule": 1, "since": 1, "type": "ncdb/float" },
        "votes": { "rule": 1, "since": 1, "type": "test/counter" },"#,
        1,
    );
    let lock = lock.replacen(
        r#""plugins": {}"#,
        r#""plugins": { "test": { "crate": "ncdb-test-counter" } }"#,
        1,
    );
    assert!(lock.contains("test/counter") && lock.contains("ncdb-test-counter"));
    lock
}

fn with_plugin(lock: &str) -> OpenOptions {
    OpenOptions::new(lock)
        .in_memory()
        .plugin(ncdb_test_counter::plugin())
}

fn at(dir: &TempDir, lock: &str) -> OpenOptions {
    OpenOptions::new(lock)
        .path(dir.join("data"))
        .key_file(dir.join("member.key"))
}

/// A task with `votes` added up from `adds`, in one write each; the
/// space and the task.
fn voted(db: &Db, adds: &[i128]) -> (nocentraldb::Space, nocentraldb::ObjectId) {
    let space = now(db.spaces().create()).unwrap();
    let task = now(space.write(|tx| {
        tx.create("tasks", [("title", Value::from("Vote"))])?;
        Ok(())
    }))
    .unwrap()
    .ids[0];
    for n in adds {
        now(space.write(|tx| tx.update("tasks", task, [("votes", Value::from(Data::Integer(*n)))])))
            .unwrap();
    }
    (space, task)
}

fn votes(space: &nocentraldb::Space, task: nocentraldb::ObjectId) -> Option<Data> {
    let read = now(space.get("tasks", task)).unwrap().unwrap();
    read.get::<Data>("votes").unwrap()
}

#[test]
fn a_plugin_field_is_written_through_the_plugin_and_read_as_data() {
    let db = now(Db::open(with_plugin(&plugin_lock()))).unwrap();
    let (space, task) = voted(&db, &[2, 3, -1]);
    assert_eq!(votes(&space, task), Some(Data::Integer(4)));
}

#[test]
fn an_unwritten_plugin_field_reads_as_the_plugins_initial_read_or_unset() {
    let db = now(Db::open(with_plugin(&plugin_lock()))).unwrap();
    let (space, task) = voted(&db, &[]);
    let read = votes(&space, task);
    assert!(read.is_none() || read == Some(Data::Integer(0)), "{read:?}");
}

#[test]
fn an_intent_the_plugin_refuses_is_invalid_and_writes_nothing() {
    let db = now(Db::open(with_plugin(&plugin_lock()))).unwrap();
    let (space, task) = voted(&db, &[1]);
    let refused = now(space.write(|tx| {
        tx.update(
            "tasks",
            task,
            [("votes", Value::from(Data::Text("one".into())))],
        )
    }))
    .unwrap_err();
    assert_eq!(refused.code(), ErrorCode::InvalidValue);
    assert!(
        refused.to_string().contains("a counter adds an integer"),
        "{refused}"
    );
    // A value that is no Data is refused as well.
    let refused = now(space.write(|tx| tx.update("tasks", task, [("votes", Value::from(1i64))])))
        .unwrap_err();
    assert_eq!(refused.code(), ErrorCode::InvalidValue);
    assert_eq!(votes(&space, task), Some(Data::Integer(1)));
}

#[test]
fn a_write_to_a_type_without_code_is_refused_with_unknown_type() {
    let db = now(Db::open(OpenOptions::new(&plugin_lock()).in_memory())).unwrap();
    let space = now(db.spaces().create()).unwrap();
    let task = now(space.write(|tx| {
        tx.create("tasks", [("title", Value::from("Vote"))])?;
        Ok(())
    }))
    .unwrap()
    .ids[0];
    let refused =
        now(space.write(|tx| tx.update("tasks", task, [("votes", Value::from(Data::Integer(1)))])))
            .unwrap_err();
    assert_eq!(refused.code(), ErrorCode::UnknownType);
    let message = refused.to_string();
    assert!(
        message.contains("test/counter") && message.contains('1'),
        "{message}"
    );
    let read = now(space.get("tasks", task)).unwrap().unwrap();
    assert_eq!(read.get::<Data>("votes").unwrap(), None);
}

#[test]
fn two_plugins_for_one_namespace_or_one_for_ncdb_are_refused_at_open() {
    let twice = with_plugin(&plugin_lock()).plugin(ncdb_test_counter::plugin());
    let refused = now(Db::open(twice)).err().unwrap();
    assert_eq!(refused.code(), ErrorCode::InvalidValue);
    let reserved = OpenOptions::new(LOCK)
        .in_memory()
        .plugin(Plugin::new("ncdb").field_type("counter", 1, ncdb_test_counter::Counter));
    let refused = now(Db::open(reserved)).err().unwrap();
    assert_eq!(refused.code(), ErrorCode::InvalidValue);
}

#[test]
fn a_plugin_the_lock_does_not_name_is_ignored() {
    let db = now(Db::open(
        OpenOptions::new(LOCK)
            .in_memory()
            .plugin(ncdb_test_counter::plugin()),
    ));
    assert!(db.is_ok());
}

#[test]
fn what_was_parked_without_the_plugin_is_folded_once_it_is_registered() {
    let dir = TempDir::new("plugin-parked");
    let lock = plugin_lock();
    let (space_id, task) = {
        let db = now(Db::open(
            at(&dir, &lock).plugin(ncdb_test_counter::plugin()),
        ))
        .unwrap();
        let (space, task) = voted(&db, &[2, 5]);
        (space.id(), task)
    };

    // Without the plugin, the field's operations park as unknown type.
    {
        let db = now(Db::open(at(&dir, &lock))).unwrap();
        let space = now(db.spaces().get(space_id)).unwrap();
        assert_eq!(votes(&space, task), None);
        let parked = space.status().current().parked;
        assert_eq!(parked.get(&ParkReason::UnknownType), Some(&2), "{parked:?}");
    }

    // With it again, the next open folds them (NOC-84).
    let db = now(Db::open(
        at(&dir, &lock).plugin(ncdb_test_counter::plugin()),
    ))
    .unwrap();
    let space = now(db.spaces().get(space_id)).unwrap();
    assert_eq!(votes(&space, task), Some(Data::Integer(7)));
    assert_eq!(
        space
            .status()
            .current()
            .parked
            .get(&ParkReason::UnknownType),
        None
    );
}
