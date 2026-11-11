//! Several changes received in one SQLite transaction (NOC-204; RFC 0006,
//! decision 7): `View::receive_all` appends and folds every frame of a
//! batch, or none of them, so a member that writes a key rotation, its
//! envelopes and the add that follows never leaves half of it in its log.

mod support;

#[path = "support/member.rs"]
mod member;
#[path = "support/view.rs"]
mod view_support;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::change::Change;
use ncdb_core::field_op;
use ncdb_core::ids::ObjectId;
use ncdb_core::log::{Appended, State};
use ncdb_core::view::View;

use member::*;
use view_support::*;

const T: ObjectId = ObjectId([0x11; 16]);

fn id(frame: &[u8]) -> ncdb_core::ids::ChangeId {
    Change::decode(frame).unwrap().id()
}

/// The envelope of generation 0, then a task sealed under it.
fn batch() -> [Vec<u8>; 2] {
    [
        envelope(FOUNDER, 1, 0),
        sealed(FOUNDER, 2, 0, vec![field_op::create(T, TASK)]),
    ]
}

#[test]
fn every_change_of_a_batch_is_appended_and_folded() {
    let db = TempDb::new("batch");
    let [key, task] = batch();
    let mut view = View::open(&db.0, &lock()).unwrap();

    let appended = view
        .receive_all(&[&key, &task], &mut Control::default())
        .unwrap();

    assert!(matches!(appended[..], [Appended::New(_), Appended::New(_)]));
    let folded = Some(State::Folded { parked: Vec::new() });
    assert_eq!(view.state(SPACE, id(&task)), folded);
    drop(view);
    let view = View::open(&db.0, &lock()).unwrap();
    assert_eq!(view.state(SPACE, id(&key)), folded);
    assert_eq!(view.state(SPACE, id(&task)), folded);
}

#[test]
fn a_batch_with_one_bad_frame_leaves_nothing_after_a_reopen_either() {
    let db = TempDb::new("batch-bad");
    let [key, task] = batch();
    let mut view = View::open(&db.0, &lock()).unwrap();

    let refused = view.receive_all(&[&key, &task, GARBLED], &mut Control::default());

    assert!(refused.is_err());
    assert_eq!(view.state(SPACE, id(&key)), None);
    assert_eq!(view.state(SPACE, id(&task)), None);
    assert!(
        view.log(SPACE)
            .is_none_or(|log| log.after(ncdb_core::log::Position::START).next().is_none())
    );
    drop(view);
    let view = View::open(&db.0, &lock()).unwrap();
    assert_eq!(view.state(SPACE, id(&key)), None);
    assert_eq!(view.state(SPACE, id(&task)), None);
}

#[test]
fn a_change_the_log_holds_already_is_a_duplicate_in_a_batch() {
    let db = TempDb::new("batch-duplicate");
    let [key, task] = batch();
    let mut view = View::open(&db.0, &lock()).unwrap();
    view.receive(&key, &mut Control::default()).unwrap();

    let appended = view
        .receive_all(&[&key, &task], &mut Control::default())
        .unwrap();

    assert!(matches!(
        appended[..],
        [Appended::Duplicate(_), Appended::New(_)]
    ));
}
