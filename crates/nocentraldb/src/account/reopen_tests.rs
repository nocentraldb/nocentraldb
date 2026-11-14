//! A database reopened before the person's account space reaches it
//! (NOC-311): a linked database keeps its link statement beside its log
//! until the relay holds it in the person's account space, so while it
//! keeps one it creates no account space of its own. A database at a path
//! needs a file system and a key file, which the browser's database does
//! not have.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::db::{Db, OpenOptions, lock};
use crate::members::tests::now;
use crate::pairing::PairKind;

const LOCK: &str = include_str!("../../tests/model/model.lock.json");

/// A directory of its own, removed when dropped.
struct Dir(PathBuf);

impl Dir {
    fn new() -> Dir {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let name = format!("ncdb-reopen-{}-{n}", std::process::id());
        let dir = Dir(std::env::temp_dir().join(name));
        let _ = std::fs::remove_dir_all(&dir.0);
        dir
    }

    /// The database `name` in this directory, with its key file there.
    fn open(&self, name: &str) -> Db {
        let options = OpenOptions::new(LOCK)
            .path(self.0.join(name))
            .key_file(self.0.join(format!("{name}.keys")));
        now(Db::open(options)).expect("the database opens")
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The spaces of the shipped account model `db` holds.
fn account_spaces(db: &Db) -> usize {
    let inner = lock(&db.inner);
    let shipped = super::id();
    inner
        .spaces
        .values()
        .filter(|state| state.checks.model_id() == Some(shipped))
        .count()
}

#[test]
fn a_linked_database_reopened_before_its_link_is_written_creates_no_account_space() {
    let dir = Dir::new();
    let laptop = dir.open("laptop");
    let tablet = dir.open("tablet");
    // The tablet's first space comes with an account space of its old
    // account (NOC-266).
    now(tablet.spaces().create()).unwrap();
    assert_eq!(account_spaces(&tablet), 1);

    let invitation = now(tablet.invitation());
    let paired = now(laptop.account().pair(PairKind::Device, &invitation)).unwrap();
    now(tablet.account().complete_pairing(&paired.answer)).unwrap();
    drop(tablet);

    // Reopened before any relay brought it the person's account space, the
    // tablet still keeps its link statement and makes no account space of
    // the person's account for itself.
    let tablet = dir.open("tablet");
    assert!(!lock(&tablet.inner).view.pending_links().unwrap().is_empty());
    now(tablet.spaces().create()).unwrap();
    assert_eq!(super::space(&lock(&tablet.inner)), None);
    assert_eq!(account_spaces(&tablet), 1, "only the old account's");
}

#[test]
fn a_paired_phone_reopened_before_its_first_sync_creates_no_account_space() {
    // NOC-311, with the owner's NOC-312 C3: a device paired into an account
    // another device created remembers it beside its log, so after a
    // reopen without `pairing()` it still creates no account space before
    // the account's reaches it.
    let dir = Dir::new();
    let laptop = dir.open("laptop");
    now(laptop.spaces().create()).unwrap();
    let phone = now(Db::open(
        OpenOptions::new(LOCK)
            .path(dir.0.join("phone"))
            .key_file(dir.0.join("phone.keys"))
            .pairing(),
    ))
    .unwrap();
    let invitation = now(phone.invitation());
    let paired = now(laptop.account().pair(PairKind::Device, &invitation)).unwrap();
    now(phone.account().complete_pairing(&paired.answer)).unwrap();
    drop(phone);

    let phone = dir.open("phone");
    now(phone.spaces().create()).unwrap();
    assert_eq!(account_spaces(&phone), 0, "no account space of its own");
}
