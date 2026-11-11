//! What a member needs to report a waiting shred (RFC 0008, "Effects that
//! wait until a change is stable"; RFC 0009, "Sync and status":
//! `shredsWaiting`): which members' runs the log holds without a gap, the
//! stability its checks compute from them, and when this device first held
//! each shred, in the table `shreds_held` of the member's SQLite file.
//! Control changes carry no clock (RFC 0008, "The control bodies"), so a
//! shred's age is counted from when this device first held it (NOC-218).
//! Like `confirmed_accounts`, the table is not computed from the log, so
//! nothing rebuilds it.
#![cfg(not(target_arch = "wasm32"))]

mod support;

#[path = "support/space.rs"]
mod space;
#[path = "support/view.rs"]
mod view_support;

use std::collections::{BTreeMap, BTreeSet};

use ncdb_core::change::Change;
use ncdb_core::checks::SealChecks;
use ncdb_core::field_op;
use ncdb_core::ids::{ChangeId, ObjectId};
use ncdb_core::membership::Role;
use ncdb_core::view::View;
use rusqlite::Connection;

use space::{Space, id, other_account};
use view_support::*;

/// The space's creator, an admin.
const A: u8 = 1;
/// The member whose view this is: a writer, who shreds.
const B: u8 = 2;
/// An admin of a second account, who signs the shred off.
const C: u8 = 3;

/// 2026-11-10T12:00:00Z, in milliseconds since the Unix epoch.
const NOON: u64 = 1_794_312_000_000;

/// B's view and checks after A made B a writer and C an admin of a second
/// account, and B announced a person key and shredded it with C's sign-off.
/// Returns the shred.
fn shredded(view: &mut View, space: &mut Space) -> (SealChecks, Change) {
    let mut checks = SealChecks::new(space.space, space::key(B));
    let genesis = space.genesis.clone();
    let add = space.add(A, B, Role::Writer, &[&genesis]);
    let envelope = space.envelope(A, B, &[&add]);
    space.certify_by(C, other_account(1));
    let carol = space.add(A, C, Role::Admin, &[&envelope]);
    let entry = space.person_key(B, 1, 0xc1, &[&carol]);
    let shred = space.shred(B, 1, &other_account(1), &[&entry]);
    for change in [&genesis, &add, &envelope, &carol, &entry, &shred] {
        view.receive(change.bytes(), &mut checks).unwrap();
    }
    (checks, shred)
}

#[test]
fn the_runs_are_each_authors_changes_from_sequence_1_without_a_gap() {
    let mut view = View::open_in_memory(&lock()).unwrap();
    let mut space = Space::new(A);
    let (mut checks, _) = shredded(&mut view, &mut space);
    // B wrote two control changes; a task of B's at sequence 4 follows a
    // gap at 3, which the run does not cross.
    space.skip(B, 1);
    let task = space.sealed(B, vec![field_op::create(ObjectId([1; 16]), TASK)]);
    view.receive(task.bytes(), &mut checks).unwrap();

    let runs = view.log(space.space).unwrap().runs();
    let expected: BTreeMap<_, _> = [(id(A), space.sequence(A)), (id(B), 2)].into();
    assert_eq!(runs, expected);
}

#[test]
fn the_checks_compute_which_admins_block_a_shred() {
    let mut view = View::open_in_memory(&lock()).unwrap();
    let mut space = Space::new(A);
    let (checks, shred) = shredded(&mut view, &mut space);
    let runs = view.log(space.space).unwrap().runs();

    let stability = checks.stability(|member| runs.get(&member).copied().unwrap_or(0));

    let blocking: BTreeSet<_> = [id(A), id(C)].into();
    let waiting: Vec<_> = stability
        .shreds_waiting()
        .map(|(change, members)| (change, members.clone()))
        .collect();
    assert_eq!(waiting, [(shred.id(), blocking)]);
}

#[test]
fn a_shred_this_device_never_held_has_no_time() {
    let view = View::open_in_memory(&lock()).unwrap();
    let space = Space::new(A);
    assert_eq!(
        view.shred_held_at(space.space, ChangeId([7; 32])).unwrap(),
        None
    );
}

#[test]
fn a_shred_keeps_the_time_this_device_first_held_it() {
    let mut view = View::open_in_memory(&lock()).unwrap();
    let space = Space::new(A);
    let other = Space::with_nonce(A, [9; 16]);
    let shred = ChangeId([7; 32]);
    view.note_shred_held(space.space, shred, NOON).unwrap();
    view.note_shred_held(space.space, shred, NOON + 1).unwrap();
    assert_eq!(view.shred_held_at(space.space, shred).unwrap(), Some(NOON));
    assert_eq!(view.shred_held_at(other.space, shred).unwrap(), None);
}

#[test]
fn the_time_a_shred_was_first_held_survives_a_reopen() {
    let db = TempDb::new("shreds-held-reopen");
    let space = Space::new(A);
    let shred = ChangeId([7; 32]);
    let mut view = View::open(&db.0, &lock()).unwrap();
    view.note_shred_held(space.space, shred, NOON).unwrap();
    drop(view);
    let view = View::open(&db.0, &lock()).unwrap();
    assert_eq!(view.shred_held_at(space.space, shred).unwrap(), Some(NOON));
}

/// The table's layout, an addition to the on-disk format for the owner's
/// tick (NOC-218): the space, the shred and when this device first held it,
/// nothing else.
#[test]
fn the_table_holds_the_space_the_shred_and_when_it_was_first_held() {
    let db = TempDb::new("shreds-held-layout");
    drop(View::open(&db.0, &lock()).unwrap());
    let conn = Connection::open(&db.0).unwrap();
    let columns: Vec<(String, String, bool, i64)> = conn
        .prepare("SELECT name, type, \"notnull\", pk FROM pragma_table_info('shreds_held')")
        .unwrap()
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(
        columns,
        [
            ("space_id".to_owned(), "BLOB".to_owned(), true, 1),
            ("change_id".to_owned(), "BLOB".to_owned(), true, 2),
            ("held_at".to_owned(), "INTEGER".to_owned(), true, 0),
        ]
    );
    let strict: i64 = conn
        .query_row(
            "SELECT strict FROM pragma_table_list WHERE name = 'shreds_held'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(strict, 1);
}
