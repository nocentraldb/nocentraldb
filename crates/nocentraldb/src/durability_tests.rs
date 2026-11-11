//! What a member's copy would lose if it were deleted now (RFC 0006, "What
//! the SDK reports"; NOC-175): this member's own changes the relay has not
//! confirmed, and whether the copy is at risk, by where it is stored. The
//! report is crate-internal until NOC-176 names it (NOC-210).

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so these tests run on both targets (NOC-175).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ed25519_dalek::SigningKey;

use crate::db::Db;
use crate::durability::{Durability, Storage, durability};
use crate::members::Role;
use crate::members::tests::{copy, joined, now, of_account, space};
use crate::space::Space;
use crate::value::Value;

/// The report of `db` with its copy stored as `storage`.
fn report(db: &Db, storage: Storage) -> Durability {
    durability(&db.lock(), storage)
}

/// The ids of the changes `db`'s log of `space` holds that its own member
/// wrote.
fn own_changes(db: &Db, space: &Space) -> Vec<ncdb_core::ids::ChangeId> {
    let inner = db.lock();
    let me = inner.member.key().id();
    let log = inner.view.log(space.id().0).expect("the space's log");
    log.after(ncdb_core::log::Position::START)
        .filter(|(_, change)| change.header().author == me)
        .map(|(_, change)| change.id())
        .collect()
}

/// `db` writes one task into `space`.
fn write(space: &Space) {
    now(space.write(|tx| {
        tx.create("tasks", [("title", Value::from("unconfirmed"))])?;
        Ok(())
    }))
    .unwrap();
}

#[test]
fn a_new_database_holds_nothing_unconfirmed_and_is_not_at_risk() {
    let db = of_account(1, &SigningKey::from_bytes(&[9; 32]));

    let report = report(&db, Storage::BestEffort);

    assert_eq!(report.storage, Storage::BestEffort);
    assert_eq!(report.unconfirmed, 0);
    assert!(!report.at_risk);
}

#[test]
fn the_members_own_changes_are_unconfirmed_until_the_relay_confirms_them() {
    let db = of_account(1, &SigningKey::from_bytes(&[9; 32]));
    let project = space(&db);
    write(&project);
    let own = own_changes(&db, &project);
    assert!(
        own.len() >= 3,
        "the genesis, the creator's envelope and the write"
    );

    assert_eq!(
        report(&db, Storage::BestEffort).unconfirmed,
        own.len() as u64
    );

    {
        let mut inner = db.lock();
        for (position, id) in own.iter().enumerate() {
            inner
                .view
                .confirm(project.id().0, *id, position as u64 + 1)
                .unwrap();
        }
    }
    let report = report(&db, Storage::BestEffort);
    assert_eq!(report.unconfirmed, 0);
    assert!(!report.at_risk);
}

#[test]
fn a_change_the_relay_refused_stays_unconfirmed() {
    let db = of_account(1, &SigningKey::from_bytes(&[9; 32]));
    let project = space(&db);
    let own = own_changes(&db, &project);
    {
        let mut inner = db.lock();
        for (position, id) in own.iter().enumerate().skip(1) {
            inner
                .view
                .confirm(project.id().0, *id, position as u64 + 1)
                .unwrap();
        }
        inner
            .view
            .refuse(project.id().0, own[0], "not_allowed")
            .unwrap();
    }

    assert_eq!(report(&db, Storage::BestEffort).unconfirmed, 1);
}

#[test]
fn another_members_changes_are_not_counted() {
    let account = SigningKey::from_bytes(&[9; 32]);
    let (admin, project, member, _) = joined(&account, Role::Writer);
    copy(&admin, &member, &project);
    write(&project);
    copy(&admin, &member, &project);

    assert_eq!(report(&member, Storage::BestEffort).unconfirmed, 0);
    assert!(report(&admin, Storage::BestEffort).unconfirmed > 0);
}

#[test]
fn unconfirmed_changes_are_at_risk_only_where_the_copy_is_not_persistent() {
    let db = of_account(1, &SigningKey::from_bytes(&[9; 32]));
    space(&db);

    assert!(!report(&db, Storage::Persistent).at_risk);
    assert!(report(&db, Storage::BestEffort).at_risk);
    assert!(report(&db, Storage::Temporary).at_risk);
}
