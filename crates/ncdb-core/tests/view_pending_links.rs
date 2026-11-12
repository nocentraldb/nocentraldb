//! The link statements a completed pairing has yet to write into the
//! account space (RFC 0010, "The link statement"; NOC-267): kept beside the
//! log, so that a rebuild of the view never drops them, until the relay
//! confirms the change that wrote them.

mod support;

#[path = "support/view.rs"]
mod view_support;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::ids::ChangeId;
use ncdb_core::view::View;
use view_support::*;

#[test]
fn a_pending_link_is_kept_with_the_change_that_wrote_it_until_it_is_deleted() {
    let mut view = View::open_in_memory(&lock()).unwrap();
    assert!(view.pending_links().unwrap().is_empty());
    view.put_pending_link(b"first").unwrap();
    view.put_pending_link(b"second").unwrap();
    view.put_pending_link(b"first").unwrap();
    assert_eq!(
        view.pending_links().unwrap(),
        [(b"first".to_vec(), None), (b"second".to_vec(), None)]
    );
    let change = ChangeId([7; 32]);
    view.set_pending_link_change(b"first", change).unwrap();
    assert_eq!(
        view.pending_links().unwrap()[0],
        (b"first".to_vec(), Some(change))
    );
    view.delete_pending_link(b"first").unwrap();
    assert_eq!(view.pending_links().unwrap(), [(b"second".to_vec(), None)]);
}

#[test]
fn a_pending_link_survives_a_reopen() {
    let db = TempDb::new("pending-links");
    let lock = lock();
    {
        let mut view = View::open(&db.0, &lock).unwrap();
        view.put_pending_link(b"kept").unwrap();
    }
    let view = View::open(&db.0, &lock).unwrap();
    assert_eq!(view.pending_links().unwrap(), [(b"kept".to_vec(), None)]);
}
