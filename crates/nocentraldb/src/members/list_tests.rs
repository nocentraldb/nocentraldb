//! `members().list()` (RFC 0009, "Invitations and members"; NOC-203): the
//! current members of a space, each with its id, its account's
//! fingerprint, its kind, its role and whether it is this member.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so these tests run on both targets (NOC-175).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ed25519_dalek::SigningKey;

use super::tests::{joined, now, of_account, remove, space};
use super::{MemberInfo, MemberKind, Role};
use crate::db::Db;
use crate::space::Space;

/// The members `db` lists in `space`.
fn listed(db: &Db, space: &Space) -> Vec<MemberInfo> {
    let space = crate::space::Space::handle(db.inner.clone(), space.id());
    now(space.members().list()).unwrap()
}

#[test]
fn the_creator_lists_itself_as_the_spaces_admin() {
    let account = SigningKey::from_bytes(&[9; 32]);
    let admin = of_account(1, &account);
    let project = space(&admin);
    let own = now(admin.invitation()).fingerprint();

    let members = listed(&admin, &project);

    assert_eq!(members.len(), 1);
    let me = &members[0];
    assert_eq!(me.role, Some(Role::Admin));
    assert!(me.is_self);
    assert_eq!(me.kind, MemberKind::Device);
    assert_eq!(me.account, own);
}

#[test]
fn every_current_member_is_listed_and_each_database_marks_itself() {
    let account = SigningKey::from_bytes(&[9; 32]);
    let (admin, project, member, invitation) = joined(&account, Role::Writer);

    let on_admin = listed(&admin, &project);
    let on_member = listed(&member, &project);

    assert_eq!(on_admin.len(), 2);
    let added = on_admin.iter().find(|m| !m.is_self).unwrap();
    assert_eq!(added.role, Some(Role::Writer));
    assert_eq!(added.account, invitation.fingerprint());
    let ids = |list: &[MemberInfo]| list.iter().map(|m| m.id).collect::<Vec<_>>();
    assert_eq!(ids(&on_admin), ids(&on_member));
    let itself = on_member.iter().find(|m| m.is_self).unwrap();
    assert_eq!(itself.id, added.id);
}

#[test]
fn a_removed_member_is_not_listed() {
    let account = SigningKey::from_bytes(&[9; 32]);
    let (admin, project, _, invitation) = joined(&account, Role::Writer);

    remove(&admin, &project, &invitation);

    let members = listed(&admin, &project);
    assert_eq!(members.len(), 1);
    assert!(members[0].is_self);
}

/// The id prints no key bytes: it is opaque until it gets a text form.
#[test]
fn a_member_id_is_opaque_in_debug() {
    let account = SigningKey::from_bytes(&[9; 32]);
    let admin = of_account(1, &account);
    let project = space(&admin);

    let id = listed(&admin, &project)[0].id;

    assert_eq!(format!("{id:?}"), "MemberId(..)");
}
