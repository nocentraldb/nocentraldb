//! A move whose device stopped between its two changes (RFC 0016, "The new
//! space first"): the new space's change is written, the old space's move
//! is not. The object then lives in both spaces, and the SDK writes the
//! move when the database opens again, from the pending record it kept on
//! the device. A pending record whose new space never got the object is
//! dropped, and nothing moves. A database at a path needs a file system
//! and a key file, which the browser's database does not have.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::db::{Db, OpenOptions, lock};
use crate::id::ObjectId;
use crate::members::tests::now;
use crate::value::Value;

const LOCK: &str = include_str!("../tests/model/model.lock.json");

/// A directory of its own, removed when dropped.
struct Dir(PathBuf);

impl Dir {
    fn new() -> Dir {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let name = format!("ncdb-move-{}-{n}", std::process::id());
        let dir = Dir(std::env::temp_dir().join(name));
        let _ = std::fs::remove_dir_all(&dir.0);
        dir
    }

    fn open(&self) -> Db {
        let options = OpenOptions::new(LOCK)
            .path(self.0.join("db"))
            .key_file(self.0.join("db.keys"));
        now(Db::open(options)).expect("the database opens")
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn space(db: &Db, id: crate::SpaceId) -> crate::Space {
    now(db.spaces().get(id)).unwrap()
}

/// A database with two spaces and a task in the first.
fn task(dir: &Dir) -> (Db, crate::SpaceId, crate::SpaceId, ObjectId) {
    let db = dir.open();
    let personal = now(db.spaces().create()).unwrap();
    let shared = now(db.spaces().create()).unwrap();
    let t = now(personal.write(|tx| {
        tx.create("tasks", [("title", Value::from("Share me"))])?;
        Ok(())
    }))
    .unwrap()
    .ids[0];
    (db, personal.id(), shared.id(), t)
}

#[test]
fn a_move_cut_off_after_the_new_space_is_finished_when_the_database_opens_again() {
    let dir = Dir::new();
    let (db, personal, shared, t) = task(&dir);
    {
        let personal = space(&db, personal);
        let task = now(personal.get("tasks", t)).unwrap().unwrap();
        crate::moving::stop_after_new_space(true);
        let stopped = now(personal.move_objects(&[task], &space(&db, shared)));
        crate::moving::stop_after_new_space(false);
        assert!(
            stopped.is_err(),
            "the test stops the move between its changes"
        );
        // Both spaces hold the object now, live: a duplicate, not a gap.
        let old = now(personal.get("tasks", t)).unwrap().unwrap();
        assert_eq!(old.moved_to(), None);
        assert!(now(space(&db, shared).get("tasks", t)).unwrap().is_some());
    }
    drop(db);

    let db = dir.open();
    let trace = now(space(&db, personal).get("tasks", t)).unwrap().unwrap();
    assert_eq!(trace.moved_to(), Some(shared));
    let moved = now(space(&db, shared).get("tasks", t)).unwrap().unwrap();
    assert_eq!(moved.get::<&str>("title").unwrap(), Some("Share me"));
    assert!(lock(&db.inner).view.pending_moves().unwrap().is_empty());
}

#[test]
fn a_pending_move_whose_new_space_lacks_the_object_is_dropped() {
    let dir = Dir::new();
    let (db, personal, shared, t) = task(&dir);
    lock(&db.inner)
        .view
        .record_pending_moves(personal.0, shared.0, &[("tasks".to_owned(), t.0)])
        .unwrap();
    drop(db);

    let db = dir.open();
    let task = now(space(&db, personal).get("tasks", t)).unwrap().unwrap();
    assert_eq!(task.moved_to(), None, "nothing moved");
    assert!(lock(&db.inner).view.pending_moves().unwrap().is_empty());
}
