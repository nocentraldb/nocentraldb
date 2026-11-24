//! `SpaceStatus::only_member` (RFC 0009, "Sync and status"; RFC 0010,
//! "What it restores"): a space whose only member is this member says so,
//! because losing this member's copy loses the space, and the status
//! changes when a second member is added or removed (NOC-494).

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so these tests run on both targets (NOC-175).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ed25519_dalek::SigningKey;
use futures_core::Stream;

use super::SpaceStatus;
use crate::members::tests::{copy, joined, now, of_account, space, writer};
use crate::members::{Invitation, MemberId, Role};
use crate::space::Space;

/// The status `stream` holds now; it panics when the stream has none.
fn next(stream: &mut (impl Stream<Item = SpaceStatus> + Unpin)) -> SpaceStatus {
    let item = std::future::poll_fn(|cx| std::pin::Pin::new(&mut *stream).poll_next(cx));
    now(item).expect("the stream does not end")
}

/// The admin's handle `project` removes the member of `invitation`.
fn remove(project: &Space, invitation: &Invitation) {
    now(project.members().remove(MemberId(invitation.key.id()))).unwrap();
}

#[test]
fn a_space_this_member_created_alone_has_it_as_its_only_member() {
    let account = SigningKey::from_bytes(&[9; 32]);
    let admin = of_account(1, &account);

    let project = space(&admin);

    assert!(project.status().current().only_member);
}

#[test]
fn a_second_member_ends_it_on_both_copies() {
    let account = SigningKey::from_bytes(&[9; 32]);

    let (_, project, member, _) = joined(&account, Role::Writer);

    assert!(!project.status().current().only_member);
    let on_member = now(member.spaces().get(project.id())).unwrap();
    assert!(!on_member.status().current().only_member);
}

#[test]
fn removing_the_second_member_brings_it_back() {
    let account = SigningKey::from_bytes(&[9; 32]);
    let (_, project, _, invitation) = joined(&account, Role::Writer);

    remove(&project, &invitation);

    assert!(project.status().current().only_member);
}

#[test]
fn a_removed_member_is_not_the_only_member() {
    let account = SigningKey::from_bytes(&[9; 32]);
    let (admin, project, member, invitation) = joined(&account, Role::Writer);
    let on_member = now(member.spaces().get(project.id())).unwrap();

    remove(&project, &invitation);
    copy(&admin, &member, &project);

    assert_eq!(on_member.status().current().role, None);
    assert!(!on_member.status().current().only_member);
}

#[test]
fn the_report_follows_a_member_added_and_removed() {
    let account = SigningKey::from_bytes(&[9; 32]);
    let project = space(&of_account(1, &account));
    let mut statuses = project.status().subscribe();
    assert!(next(&mut statuses).only_member);
    let joining = of_account(2, &account);
    let invitation = now(joining.invitation());

    now(project.members().add(&invitation, writer())).unwrap();

    assert!(!next(&mut statuses).only_member);
    remove(&project, &invitation);

    assert!(next(&mut statuses).only_member);
}
