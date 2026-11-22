//! The device-local table of rich-text client ids (RFC 0013, "Reusing a
//! client id"): per field, the client ids this member used, the nonce of
//! each and the clock it reached. A binding, or an agent's write, leases
//! one for as long as it writes; two at once lease two different ids, and
//! a released id is leased again with the clock it reached. The table is
//! no part of the view: the file keeps it, and no fold touches it.
#![cfg(ncdb_rich_text)]

mod support;

#[path = "support/space.rs"]
mod space;
#[path = "support/view.rs"]
mod view_support;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::ids::{MemberId, ObjectId, SpaceId};
use ncdb_core::rich_text::client_id;
use ncdb_core::view::View;

use view_support::*;

const SPACE: SpaceId = SpaceId([5; 16]);
const T: ObjectId = ObjectId([0x11; 16]);
const ME: MemberId = MemberId([9; 32]);

fn view() -> View {
    View::open_in_memory(&lock()).unwrap()
}

#[test]
fn a_lease_gives_a_client_id_derived_from_the_member_and_its_nonce() {
    let mut view = view();
    let lease = view.lease_rich_text(ME, SPACE, TASK, T, "body").unwrap();
    assert_eq!(lease.client, client_id(&ME, &lease.nonce));
    assert_eq!(lease.clock, 0);
}

#[test]
fn two_leases_at_once_get_two_client_ids() {
    let mut view = view();
    let one = view.lease_rich_text(ME, SPACE, TASK, T, "body").unwrap();
    let two = view.lease_rich_text(ME, SPACE, TASK, T, "body").unwrap();
    assert_ne!(one.client, two.client);
    assert_ne!(one.nonce, two.nonce);
}

#[test]
fn a_released_client_id_is_leased_again_with_the_clock_it_reached() {
    let mut view = view();
    let one = view.lease_rich_text(ME, SPACE, TASK, T, "body").unwrap();
    view.release_rich_text(&one, 12).unwrap();
    let again = view.lease_rich_text(ME, SPACE, TASK, T, "body").unwrap();
    assert_eq!(
        (again.client, again.nonce, again.clock),
        (one.client, one.nonce, 12)
    );
    // A clock below the one reached never moves it back.
    view.release_rich_text(&again, 3).unwrap();
    let third = view.lease_rich_text(ME, SPACE, TASK, T, "body").unwrap();
    assert_eq!(third.clock, 12);
}

#[test]
fn each_field_has_its_own_client_ids() {
    let mut view = view();
    let body = view.lease_rich_text(ME, SPACE, TASK, T, "body").unwrap();
    view.release_rich_text(&body, 4).unwrap();
    let notes = view.lease_rich_text(ME, SPACE, TASK, T, "notes").unwrap();
    assert_ne!(body.client, notes.client);
    let other = view
        .lease_rich_text(ME, SPACE, TASK, ObjectId([0x22; 16]), "body")
        .unwrap();
    assert_ne!(body.client, other.client);
}

#[test]
fn leases_are_kept_in_the_file_and_are_no_part_of_the_view() {
    let db = TempDb::new("rich-text-lease");
    let one = {
        let mut view = View::open(&db.0, &lock()).unwrap();
        let one = view.lease_rich_text(ME, SPACE, TASK, T, "body").unwrap();
        view.release_rich_text(&one, 7).unwrap();
        one
    };
    let mut view = View::open(&db.0, &lock()).unwrap();
    let again = view.lease_rich_text(ME, SPACE, TASK, T, "body").unwrap();
    assert_eq!((again.client, again.clock), (one.client, 7));
}
