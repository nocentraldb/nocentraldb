//! A personal field after the database is closed and opened again, with
//! no relay (NOC-547): the device keeps the person keys it holds, so a
//! value written before the close reads the same after the reopen; a
//! device that holds no key for a person the log names says so in the
//! space status instead of reading null without a word.

use crate::personal_write_tests::lock;
use crate::{Db, OpenOptions, Value};

fn now<F: std::future::Future>(future: F) -> F::Output {
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    match std::pin::pin!(future).poll(&mut cx) {
        std::task::Poll::Ready(output) => output,
        std::task::Poll::Pending => panic!("the SDK never waits on the device"),
    }
}

/// A directory of its own under the system's temporary directory, removed
/// with everything in it when dropped.
struct Dir(std::path::PathBuf);

impl Dir {
    fn new(name: &str) -> Dir {
        let id: String = ncdb_core::ids::ObjectId::random()
            .0
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        Dir(std::env::temp_dir().join(format!("ncdb-personal-reopen-{name}-{id}")))
    }

    fn options(&self) -> OpenOptions {
        OpenOptions::new(&lock())
            .path(self.0.join("data"))
            .key_file(self.0.join("member.key"))
    }

    /// Drops the database's store of person-key records, so that the
    /// database is as one written before the store existed
    /// (0.2.0-alpha.22): the next open creates it again, empty.
    fn forget_records(&self) {
        let file = self.0.join("data").join("nocentraldb.sqlite");
        let conn = rusqlite::Connection::open(file).unwrap();
        conn.execute("DROP TABLE IF EXISTS person_key_records", [])
            .unwrap();
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Writes a project whose name is personal to itself and a task whose
/// title is personal to the project, closes the database, and gives the
/// ids.
fn written(dir: &Dir) -> (crate::SpaceId, crate::ObjectId, crate::ObjectId) {
    let db = now(Db::open(dir.options())).unwrap();
    let space = now(db.spaces().create()).unwrap();
    let project = now(space.create("projects", [("name", Value::from("Anna"))]))
        .unwrap()
        .ids[0];
    let task = now(space.create(
        "tasks",
        [
            ("project", Value::from(project)),
            ("title", Value::from("Call Anna")),
        ],
    ))
    .unwrap()
    .ids[0];
    assert_eq!(space.status().current().missing_person_keys, 0);
    (space.id(), project, task)
}

#[test]
fn a_personal_value_reads_the_same_after_a_reopen() {
    let dir = Dir::new("value");
    let (space, project, task) = written(&dir);

    let db = now(Db::open(dir.options())).unwrap();
    let space = now(db.spaces().get(space)).unwrap();
    let project = now(space.get("projects", project)).unwrap().unwrap();
    let task = now(space.get("tasks", task)).unwrap().unwrap();

    assert_eq!(project.get::<&str>("name").unwrap(), Some("Anna"));
    assert_eq!(task.get::<&str>("title").unwrap(), Some("Call Anna"));
    assert_eq!(space.status().current().missing_person_keys, 0);
}

#[test]
fn a_person_key_drawn_after_a_reopen_is_kept_too() {
    let dir = Dir::new("later");
    let (space, ..) = written(&dir);
    let bert = {
        let db = now(Db::open(dir.options())).unwrap();
        let space = now(db.spaces().get(space)).unwrap();
        now(space.create("projects", [("name", Value::from("Bert"))]))
            .unwrap()
            .ids[0]
    };

    let db = now(Db::open(dir.options())).unwrap();
    let space = now(db.spaces().get(space)).unwrap();
    let bert = now(space.get("projects", bert)).unwrap().unwrap();

    assert_eq!(bert.get::<&str>("name").unwrap(), Some("Bert"));
}

#[test]
fn a_reopen_without_the_person_key_says_so_in_the_status() {
    let dir = Dir::new("missing");
    let (space, project, _) = written(&dir);
    dir.forget_records();

    let db = now(Db::open(dir.options())).unwrap();
    let space = now(db.spaces().get(space)).unwrap();
    let project = now(space.get("projects", project)).unwrap().unwrap();

    assert_eq!(project.get::<&str>("name").unwrap(), None);
    assert_eq!(space.status().current().missing_person_keys, 1);
}
