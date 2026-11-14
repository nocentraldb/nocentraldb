//! Whether the database joined an account another device created (RFC
//! 0010, "The account space"; the owner's NOC-312 C3, Carsten,
//! 2026-11-14): a paired device keeps a row that says so beside its log,
//! so that after a reopen it still creates no account space of its own
//! before the account's reaches it. Unlike the view's tables, no fold
//! computes it, so neither a reopen nor folding the log again drops it.

mod support;

#[path = "support/space.rs"]
mod space;
#[path = "support/view.rs"]
mod view_support;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::checks::SealChecks;
use ncdb_core::ids::SpaceId;
use ncdb_core::model_lock::ModelLock;
use ncdb_core::view::View;

use view_support::*;

#[test]
fn a_database_has_not_joined_until_it_says_so_and_forgets_it_when_cleared() {
    let mut view = View::open_in_memory(&lock()).unwrap();
    assert!(!view.joined().unwrap());
    view.set_joined().unwrap();
    view.set_joined().unwrap();
    assert!(view.joined().unwrap());
    view.clear_joined().unwrap();
    assert!(!view.joined().unwrap());
}

#[test]
fn the_joined_row_survives_a_reopen() {
    let db = TempDb::new("joined");
    let lock = lock();
    {
        let mut view = View::open(&db.0, &lock).unwrap();
        view.set_joined().unwrap();
    }
    let view = View::open(&db.0, &lock).unwrap();
    assert!(view.joined().unwrap());
}

fn older() -> ModelLock {
    let mut lock = lock();
    let task = lock.collections.get_mut(TASK).unwrap();
    task.fields.remove(RANK).unwrap();
    task.indexes
        .retain(|_, index| index.fields.iter().all(|f| f.field != RANK));
    lock.model.version -= 1;
    lock
}

fn checks_for(space: SpaceId) -> SealChecks {
    SealChecks::new(space, space::key(2))
}

/// Folding the log again for a changed model drops and rebuilds the view's
/// tables; the joined row is no part of the view and stays.
#[test]
fn folding_the_log_again_keeps_the_joined_row() {
    let db = TempDb::new("joined-refold");
    let mut view = View::open_with(&db.0, &older(), checks_for).unwrap();
    view.set_joined().unwrap();
    drop(view);

    let view = View::open_with(&db.0, &lock(), checks_for).unwrap();
    assert!(view.model().collections[TASK].contains_key(RANK));
    assert!(view.joined().unwrap());
}
