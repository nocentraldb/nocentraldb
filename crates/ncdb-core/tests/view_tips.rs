//! The tips of a space's log in the local view (NOC-138), after the view
//! is reopened and after it folds the log again for a changed lock
//! (NOC-148): the view reads the states of the log back without opening
//! its changes, so the tips open each folded sealed change once, the first
//! time they are read, and equal a full recount from then on.

mod support;

#[path = "support/space.rs"]
mod space;
#[path = "support/tips.rs"]
mod tips;
#[path = "support/view.rs"]
mod view_support;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::checks::SealChecks;
use ncdb_core::ids::SpaceId;
use ncdb_core::log::State;
use ncdb_core::model_lock::ModelLock;
use ncdb_core::view::View;

use tips::{Counting, of, pool, recount};
use view_support::{RANK, TASK, TempDb, lock};

/// [`lock`] one version before, without the tasks' rank: a database
/// written under it folds again when [`lock`] opens it.
fn older() -> ModelLock {
    let mut lock = lock();
    let task = lock.collections.get_mut(TASK).unwrap();
    task.fields.remove(RANK).unwrap();
    task.indexes
        .retain(|_, index| index.fields.iter().all(|f| f.field != RANK));
    lock.model.version -= 1;
    lock
}

fn carols(space: SpaceId) -> SealChecks {
    SealChecks::new(space, space::key(tips::CAROL))
}

/// Carol's view in `db`, written under `lock`, after every change of the
/// pool but the last, closed again.
fn written(db: &TempDb, lock: &ModelLock) {
    let mut view = View::open_with(&db.0, lock, carols).unwrap();
    let mut checks = Counting::new();
    let changes = &pool().changes;
    for change in &changes[..changes.len() - 1] {
        view.receive(change.bytes(), &mut checks).unwrap();
    }
}

/// The number of folded sealed changes the view's log of the pool holds.
fn folded_sealed(view: &View) -> usize {
    pool()
        .sealed
        .iter()
        .filter(|change| {
            matches!(
                view.state(pool().space, change.id()),
                Some(State::Folded { .. })
            )
        })
        .count()
}

/// Reads the tips of the reopened `view` twice, then receives the pool's
/// last change: the first read opens each folded sealed change once, the
/// second none, and each equals a full recount.
fn assert_rebuilt_once(mut view: View) {
    let space = pool().space;
    let mut checks = Counting::new();
    for change in view.log(space).unwrap().after(Default::default()) {
        if matches!(view.state(space, change.1.id()), Some(State::Folded { .. }))
            && change.1.header().kind == ncdb_core::change::Kind::CONTROL
        {
            ncdb_core::receive::Checks::add_control(&mut checks, change.1);
        }
    }
    let folded = folded_sealed(&view);
    assert!(folded > 0);

    let first = of(view.tips(space, &checks).unwrap());
    assert_eq!(checks.take_opened(), folded);
    let second = of(view.tips(space, &checks).unwrap());
    assert_eq!(checks.take_opened(), 0);

    let expected = recount(view.log(space).unwrap(), &checks);
    assert_eq!(first, expected);
    assert_eq!(second, expected);

    let last = pool().changes.last().unwrap();
    view.receive(last.bytes(), &mut checks).unwrap();
    checks.take_opened();
    let after = of(view.tips(space, &checks).unwrap());
    assert_eq!(checks.take_opened(), 0);
    assert_eq!(after, recount(view.log(space).unwrap(), &checks));
}

#[test]
fn a_reopened_view_opens_each_folded_change_once_for_its_tips() {
    let db = TempDb::new("tips-reopen");
    written(&db, &lock());

    assert_rebuilt_once(View::open(&db.0, &lock()).unwrap());
}

#[test]
fn a_view_folded_again_for_a_changed_lock_opens_each_folded_change_once_for_its_tips() {
    let db = TempDb::new("tips-refold");
    written(&db, &older());

    assert_rebuilt_once(View::open_with(&db.0, &lock(), carols).unwrap());
}

#[test]
fn a_view_has_no_tips_for_a_space_it_holds_no_log_of() {
    let db = TempDb::new("tips-none");
    let mut view = View::open(&db.0, &lock()).unwrap();

    assert!(view.tips(pool().space, &Counting::new()).is_none());
}
