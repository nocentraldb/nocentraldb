//! `members().remove` and `members().set_role` on the device (RFC 0009,
//! "Invitations and members"; RFC 0008, "Membership entry", "The cutoff",
//! "Key chain entry" and "A member demoted and raised again"; NOC-203).

use ed25519_dalek::SigningKey;
use ncdb_core::membership::Resolved;

use super::tests::{copy, joined, now, of_account};
use super::{MemberId, NoticeCode, Role};
use crate::db::Db;
use crate::error::ErrorCode;
use crate::space::Space;
use crate::value::Value;

/// The id of the member of `invitation`, which `list` gives an app.
fn id_of(invitation: &super::Invitation) -> MemberId {
    MemberId(invitation.key.id())
}

/// The id of `db`'s own member.
fn own(db: &Db) -> MemberId {
    MemberId(db.lock().member.key().id())
}

/// The membership state `db` folded for `space`.
fn resolved(db: &Db, space: &Space) -> Resolved {
    crate::db::lock(&db.inner).spaces[&space.id().0]
        .checks
        .resolved()
        .clone()
}

/// The space's current generation on `db`.
fn current(db: &Db, space: &Space) -> u64 {
    resolved(db, space).current_generation().unwrap()
}

/// The number of changes `db`'s log of `space` holds.
fn changes(db: &Db, space: &Space) -> usize {
    let inner = crate::db::lock(&db.inner);
    let log = inner.view.log(space.id().0).expect("the space's log");
    log.after(ncdb_core::log::Position::START).count()
}

#[test]
fn removing_a_member_returns_the_notice_that_it_keeps_its_copy() {
    let account = SigningKey::from_bytes(&[9; 32]);
    let (admin, project, _, invitation) = joined(&account, Role::Writer);
    let id = id_of(&invitation);

    let notice = now(project.members().remove(id)).unwrap();

    assert_eq!(notice.code, NoticeCode::MemberRemoved);
    assert_eq!(
        notice.text,
        "The removed member keeps its copy of everything it received before now."
    );
    let state = resolved(&admin, &project);
    assert_eq!(state.role(invitation.key.id()), None);
    assert_eq!(now(project.members().list()).unwrap().len(), 1);
}

/// RFC 0003's eager rotation: the removal is answered by a key chain entry
/// for a fresh generation, with an envelope for every remaining member and
/// none for the removed one, so the new generation is not exposed.
#[test]
fn a_removal_rotates_the_key_without_an_envelope_for_the_removed_member() {
    let account = SigningKey::from_bytes(&[9; 32]);
    let (admin, project, _, invitation) = joined(&account, Role::Writer);
    let before = current(&admin, &project);
    let id = id_of(&invitation);

    now(project.members().remove(id)).unwrap();

    let state = resolved(&admin, &project);
    let after = state.current_generation().unwrap();
    assert!(after != before && state.descends(after, before));
    let recipients = &state.generation(after).unwrap().recipients;
    assert!(recipients.contains(&own(&admin).0));
    assert!(!recipients.contains(&invitation.key.id()));
    assert!(!state.exposed(after));
}

/// The cutoff is the member's highest author sequence the admin held
/// (RFC 0008, "The cutoff").
#[test]
fn the_removal_names_the_members_highest_sequence_the_admin_held_as_its_cutoff() {
    let account = SigningKey::from_bytes(&[9; 32]);
    let (admin, project, member, invitation) = joined(&account, Role::Writer);
    let on_member = Space::handle(member.inner.clone(), project.id());
    for title in ["one", "two"] {
        now(on_member.write(|tx| {
            tx.create("tasks", [("title", Value::from(title))])?;
            Ok(())
        }))
        .unwrap();
    }
    copy(&member, &admin, &project);
    let id = id_of(&invitation);

    now(project.members().remove(id)).unwrap();

    let state = resolved(&admin, &project);
    let removal = state.member(invitation.key.id()).unwrap().revocations.last().copied();
    assert_eq!(removal.map(|r| (r.role, r.cutoff)), Some((None, 2)));
}

#[test]
fn what_remove_refuses_writes_nothing() {
    let account = SigningKey::from_bytes(&[9; 32]);
    let (admin, project, member, invitation) = joined(&account, Role::Writer);
    let other = of_account(3, &account);
    let third = now(other.invitation());
    now(project.members().add(&third, super::tests::writer())).unwrap();
    copy(&admin, &member, &project);
    let on_member = Space::handle(member.inner.clone(), project.id());
    let me = own(&admin);
    let them = id_of(&third);
    let before = changes(&admin, &project);

    let not_admin = now(on_member.members().remove(them)).unwrap_err();
    let itself = now(project.members().remove(me)).unwrap_err();
    now(project.members().remove(them)).unwrap();
    let removed = changes(&admin, &project);
    let twice = now(project.members().remove(them)).unwrap_err();

    assert_eq!(not_admin.code(), ErrorCode::NotAllowed);
    assert_eq!(itself.code(), ErrorCode::InvalidValue);
    assert_eq!(twice.code(), ErrorCode::NotFound);
    assert!(removed > before);
    assert_eq!(changes(&admin, &project), removed);
    let _ = invitation;
}

/// Lowering a role writes a set role with a cutoff and no from generation
/// for a reader, and does not rotate: a reader still reads the space.
#[test]
fn lowering_a_writer_to_a_reader_returns_role_set_and_does_not_rotate() {
    let account = SigningKey::from_bytes(&[9; 32]);
    let (admin, project, _, invitation) = joined(&account, Role::Writer);
    let before = current(&admin, &project);
    let id = id_of(&invitation);

    let notice = now(project.members().set_role(id, Role::Reader)).unwrap();

    assert_eq!(notice.code, NoticeCode::RoleSet);
    assert_eq!(
        notice.text,
        "A member whose role was lowered keeps its copy of everything it received before now."
    );
    let state = resolved(&admin, &project);
    assert_eq!(state.current_generation(), Some(before));
    let subject = state.member(invitation.key.id()).unwrap();
    assert_eq!(subject.role, Some(Role::Reader.into()));
    assert_eq!(subject.grants.last().unwrap().from_generation, None);
    assert_eq!(subject.revocations.last().unwrap().role, Some(Role::Reader.into()));
}

/// Raising a reader to a writer is a re-grant (RFC 0008, "A member demoted
/// and raised again"): the admin rotates first, the raise names the fresh
/// generation, and the subject's envelope for it follows the raise. The
/// subject then holds the current key, and the fold accepts it all.
#[test]
fn raising_a_reader_to_a_writer_rotates_first_and_grants_from_the_fresh_generation() {
    let account = SigningKey::from_bytes(&[9; 32]);
    let (admin, project, member, invitation) = joined(&account, Role::Reader);
    let before = current(&admin, &project);
    let id = id_of(&invitation);

    let notice = now(project.members().set_role(id, Role::Writer)).unwrap();

    assert_eq!(notice.code, NoticeCode::RoleSet);
    let state = resolved(&admin, &project);
    let after = state.current_generation().unwrap();
    assert!(after != before && state.descends(after, before));
    let subject = state.member(invitation.key.id()).unwrap();
    assert_eq!(subject.role, Some(Role::Writer.into()));
    assert_eq!(subject.grants.last().unwrap().from_generation, Some(after));
    copy(&admin, &member, &project);
    let on_member = crate::db::lock(&member.inner);
    let checks = &on_member.spaces[&project.id().0].checks;
    assert_eq!(checks.resolved().current_generation(), Some(after));
    assert!(checks.space_key(after).is_some());
}

/// Raising a writer to an admin keeps its right to write, so it is no
/// re-grant and does not rotate; the raise names the current generation.
#[test]
fn raising_a_writer_to_an_admin_does_not_rotate() {
    let account = SigningKey::from_bytes(&[9; 32]);
    let (admin, project, _, invitation) = joined(&account, Role::Writer);
    let before = current(&admin, &project);
    let id = id_of(&invitation);

    now(project.members().set_role(id, Role::Admin)).unwrap();

    let state = resolved(&admin, &project);
    assert_eq!(state.current_generation(), Some(before));
    let subject = state.member(invitation.key.id()).unwrap();
    assert_eq!(subject.role, Some(Role::Admin.into()));
    assert_eq!(subject.grants.last().unwrap().from_generation, Some(before));
}

/// Any member may lower its own role (RFC 0008, "Roles and rights"); only
/// an admin changes another's or raises one; the last admin may not lower
/// its own role, as it may not leave (RFC 0009, "Spaces").
#[test]
fn who_may_set_a_role() {
    let account = SigningKey::from_bytes(&[9; 32]);
    let (admin, project, member, invitation) = joined(&account, Role::Writer);
    let on_member = Space::handle(member.inner.clone(), project.id());
    let me = own(&admin);
    let them = id_of(&invitation);

    let lowers_another = now(on_member.members().set_role(me, Role::Reader)).unwrap_err();
    let raises_itself = now(on_member.members().set_role(them, Role::Admin)).unwrap_err();
    let last_admin = now(project.members().set_role(me, Role::Writer)).unwrap_err();
    let lowers_itself = now(on_member.members().set_role(them, Role::Reader));

    assert_eq!(lowers_another.code(), ErrorCode::NotAllowed);
    assert_eq!(raises_itself.code(), ErrorCode::NotAllowed);
    assert_eq!(last_admin.code(), ErrorCode::LastAdmin);
    assert_eq!(lowers_itself.unwrap().code, NoticeCode::RoleSet);
}
