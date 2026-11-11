//! `space.leave()` on the device (RFC 0009, "Spaces"; RFC 0008, "Roles and
//! rights" and "Membership entry"; the notice text the owner decided on
//! NOC-219): the member writes a removal of itself with its cutoff, and does
//! not rotate, since it must not know the next key.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so these tests run on both targets (NOC-175).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ed25519_dalek::SigningKey;
use ncdb_core::membership::Resolved;

use super::tests::{copy, joined, now, remove};
use super::{NoticeCode, Role};
use crate::db::Db;
use crate::error::ErrorCode;
use crate::space::Space;
use crate::value::Value;

/// The membership state `db` folded for `space`.
fn resolved(db: &Db, space: &Space) -> Resolved {
    crate::db::lock(&db.inner).spaces[&space.id().0]
        .checks
        .resolved()
        .clone()
}

/// The number of changes `db`'s log of `space` holds.
fn changes(db: &Db, space: &Space) -> usize {
    let inner = crate::db::lock(&db.inner);
    let log = inner.view.log(space.id().0).expect("the space's log");
    log.after(ncdb_core::log::Position::START).count()
}

#[test]
fn leaving_returns_space_left_and_removes_only_the_member_itself() {
    let account = SigningKey::from_bytes(&[9; 32]);
    let (admin, project, member, invitation) = joined(&account, Role::Writer);
    let on_member = Space::handle(member.inner.clone(), project.id());
    let before = resolved(&member, &project).current_generation();

    let notice = now(on_member.leave()).unwrap();

    assert_eq!(notice.code, NoticeCode::SpaceLeft);
    assert_eq!(
        notice.text,
        "This member keeps its copy of everything it received before now. \
         Until an admin rotates the space key, the others write under a key it still holds."
    );
    let state = resolved(&member, &project);
    assert_eq!(state.role(invitation.key.id()), None);
    assert_eq!(state.current_generation(), before);
    copy(&member, &admin, &project);
    assert_eq!(resolved(&admin, &project).role(invitation.key.id()), None);
    assert_eq!(now(project.members().list()).unwrap().len(), 1);
}

/// The leaver writes no rotation, so the current generation is one it
/// holds: the admin's status reports it as exposed until an admin rotates
/// (RFC 0008, "Two removals that rotate the key at once", step 4).
#[test]
fn after_a_leave_the_admin_sees_the_current_generation_exposed() {
    let account = SigningKey::from_bytes(&[9; 32]);
    let (admin, project, member, _) = joined(&account, Role::Writer);
    let on_member = Space::handle(member.inner.clone(), project.id());
    assert!(!project.status().current().exposed);

    now(on_member.leave()).unwrap();
    copy(&member, &admin, &project);

    assert!(project.status().current().exposed);
}

/// The cutoff is the member's own highest author sequence (RFC 0008,
/// "Membership entry": a member that leaves names its own highest sequence).
#[test]
fn the_leave_names_the_members_own_highest_sequence_as_its_cutoff() {
    let account = SigningKey::from_bytes(&[9; 32]);
    let (_, project, member, invitation) = joined(&account, Role::Writer);
    let on_member = Space::handle(member.inner.clone(), project.id());
    for title in ["one", "two"] {
        now(on_member.write(|tx| {
            tx.create("tasks", [("title", Value::from(title))])?;
            Ok(())
        }))
        .unwrap();
    }

    now(on_member.leave()).unwrap();

    let state = resolved(&member, &project);
    let left = state
        .member(invitation.key.id())
        .unwrap()
        .revocations
        .last()
        .copied();
    assert_eq!(left.map(|r| (r.role, r.cutoff)), Some((None, 2)));
}

#[test]
fn the_last_admin_may_not_leave_and_nothing_is_written() {
    let account = SigningKey::from_bytes(&[9; 32]);
    let (admin, project, _, _) = joined(&account, Role::Writer);
    let before = changes(&admin, &project);

    let refused = now(project.leave()).unwrap_err();

    assert_eq!(refused.code(), ErrorCode::LastAdmin);
    assert_eq!(changes(&admin, &project), before);
}

/// An admin may leave while another admin stays.
#[test]
fn an_admin_leaves_when_another_admin_stays() {
    let account = SigningKey::from_bytes(&[9; 32]);
    let (admin, project, member, invitation) = joined(&account, Role::Admin);
    let on_member = Space::handle(member.inner.clone(), project.id());

    now(on_member.leave()).unwrap();

    copy(&member, &admin, &project);
    assert_eq!(resolved(&admin, &project).role(invitation.key.id()), None);
}

#[test]
fn a_member_that_was_removed_cannot_leave() {
    let account = SigningKey::from_bytes(&[9; 32]);
    let (admin, project, member, invitation) = joined(&account, Role::Writer);
    remove(&admin, &project, &invitation);
    copy(&admin, &member, &project);
    let on_member = Space::handle(member.inner.clone(), project.id());

    let refused = now(on_member.leave()).unwrap_err();

    assert_eq!(refused.code(), ErrorCode::NotMember);
}
