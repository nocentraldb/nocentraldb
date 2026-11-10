//! What `members().add` refuses (RFC 0009, "Invitations and members" and
//! "Errors"), on the device, without a relay.

use ed25519_dalek::SigningKey;

use super::{AddMember, Role};
use crate::db::{Db, OpenOptions};
use crate::error::ErrorCode;
use crate::space::Space;

const LOCK: &str = include_str!("../../tests/model/model.lock.json");

/// The output of an SDK future, which is ready at its first poll.
pub(crate) fn now<F: std::future::Future>(future: F) -> F::Output {
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    match std::pin::pin!(future).poll(&mut cx) {
        std::task::Poll::Ready(output) => output,
        std::task::Poll::Pending => panic!("an SDK call waited"),
    }
}

/// A database in memory, of an account of its own.
fn database() -> Db {
    now(Db::open(OpenOptions::new(LOCK).in_memory())).expect("the test lock opens")
}

/// A database in memory of the member `who` of `account`.
fn of_account(who: u8, account: &SigningKey) -> Db {
    let options = OpenOptions::new(LOCK).in_memory();
    now(Db::open_as(
        options,
        [who; 32],
        [who ^ 0x80; 32],
        account.clone(),
    ))
    .expect("the test lock opens")
}

/// A space the admin's database creates.
fn space(admin: &Db) -> Space {
    now(admin.spaces().create()).unwrap()
}

fn writer() -> AddMember {
    AddMember {
        role: Role::Writer,
        fingerprint: None,
    }
}

#[test]
fn a_member_of_an_account_new_to_the_database_is_refused_without_its_fingerprint() {
    let admin = database();
    let project = space(&admin);
    let invitation = now(database().invitation());

    let refused = now(project.members().add(&invitation, writer())).unwrap_err();

    assert_eq!(refused.code(), ErrorCode::UnknownAccount);
    let confirmed = AddMember {
        fingerprint: Some(invitation.fingerprint()),
        ..writer()
    };
    now(project.members().add(&invitation, confirmed)).unwrap();
}

#[test]
fn a_fingerprint_of_another_account_does_not_confirm_the_invitations() {
    let admin = database();
    let project = space(&admin);
    let invitation = now(database().invitation());
    let other = now(database().invitation()).fingerprint();

    let options = AddMember {
        fingerprint: Some(other),
        ..writer()
    };
    let refused = now(project.members().add(&invitation, options)).unwrap_err();

    assert_eq!(refused.code(), ErrorCode::UnknownAccount);
}

#[test]
fn a_member_of_the_databases_own_account_needs_no_fingerprint() {
    let account = SigningKey::from_bytes(&[9; 32]);
    let admin = of_account(1, &account);
    let project = space(&admin);
    let invitation = now(of_account(2, &account).invitation());

    now(project.members().add(&invitation, writer())).unwrap();
}

/// The invitation of a fresh database, its certificate changed by `spoil`.
fn spoiled(spoil: impl FnOnce(&mut super::Invitation)) -> super::Invitation {
    let mut invitation = now(database().invitation());
    spoil(&mut invitation);
    invitation
}

#[test]
fn an_invitation_whose_certificate_does_not_hold_is_refused() {
    let admin = database();
    let project = space(&admin);
    let forged = spoiled(|invitation| invitation.certificate.signature[0] ^= 1);
    let other = now(database().invitation());
    let of_another_member = spoiled(|invitation| invitation.key = other.key);
    let of_a_sub_agent = spoiled(|invitation| {
        invitation.certificate.kind = ncdb_core::membership::MemberKind::SubAgent;
    });

    for invitation in [forged, of_another_member, of_a_sub_agent] {
        let options = AddMember {
            fingerprint: Some(invitation.fingerprint()),
            ..writer()
        };
        let refused = now(project.members().add(&invitation, options)).unwrap_err();

        assert_eq!(refused.code(), ErrorCode::InvalidValue);
    }
}

/// Gives `to` every change of `space` that `from` holds, as a pull would.
pub(crate) fn copy(from: &Db, to: &Db, space: &Space) {
    let changes: Vec<Vec<u8>> = {
        let inner = from.lock();
        let log = inner.view.log(space.id().0).expect("the space's log");
        log.after(ncdb_core::log::Position::START)
            .map(|(_, change)| change.bytes().to_vec())
            .collect()
    };
    let mut inner = to.lock();
    let member = inner.member.key();
    let crate::db::Inner { view, spaces, .. } = &mut *inner;
    let state = spaces
        .entry(space.id().0)
        .or_insert_with(|| crate::db::SpaceState::new(space.id().0, member));
    for bytes in changes {
        view.receive(&bytes, &mut state.checks)
            .expect("the change is received");
    }
}

/// The admin removes the member of `invitation` from `space`, through the
/// crate's internals, since `members().remove` is not built. Returns the
/// removal, which the admin's database holds folded.
pub(crate) fn remove(
    admin: &Db,
    space: &Space,
    invitation: &super::Invitation,
) -> ncdb_core::change::Change {
    use ncdb_core::membership::{self, Action, ControlBody, Entry, MembershipEntry};

    let mut inner = admin.lock();
    let member = inner.member.key();
    let crate::db::Inner { view, spaces, .. } = &mut *inner;
    let state = spaces
        .get_mut(&space.id().0)
        .expect("the admin holds the space");
    let next = crate::next::next(view, space.id().0, &state.checks, member.id());
    let entry = Entry::Membership(MembershipEntry {
        subject: invitation.key.id(),
        action: Action::Remove { cutoff: 0 },
    });
    let removal = membership::write(
        &member,
        space.id().0,
        next.sequence,
        next.generation.expect("a generation"),
        &ControlBody::new(super::control_heads(view, space.id().0), entry),
    );
    crate::space::fold(view, &mut state.checks, &removal).unwrap();
    crate::sync::wake(&inner);
    removal
}

/// The admin adds the member of `invitation` again as a writer, after its
/// removal (RFC 0008, decision M5): it rotates to a fresh generation and
/// grants the member the right to write from it. The SDK does not rotate
/// yet, so the key chain entry carries stand-ins for the previous key,
/// which `resolve` does not open. Neither change is folded into the
/// admin's database; the test pushes them to the relay itself.
pub(crate) fn regrant(
    admin: &Db,
    space: &Space,
    invitation: &super::Invitation,
) -> [ncdb_core::change::Change; 2] {
    use ncdb_core::membership::{
        self, Action, ControlBody, Entry, KeyChainEntry, MembershipEntry, PreviousKey,
    };

    let inner = admin.lock();
    let member = inner.member.key();
    let id = space.id().0;
    let state = &inner.spaces[&id];
    let next = crate::next::next(&inner.view, id, &state.checks, member.id());
    let generation = next.generation.expect("a generation");
    let fresh = membership::generation_id(id, member.id(), &[generation]);
    let chain = KeyChainEntry {
        generation: fresh,
        previous: vec![PreviousKey {
            generation,
            key: [0xc4; membership::PREVIOUS_KEY_LEN],
        }],
        answers: std::collections::BTreeSet::new(),
    };
    let heads = super::control_heads(&inner.view, id);
    let rotation = membership::write(
        &member,
        id,
        next.sequence,
        generation,
        &ControlBody::new(heads, Entry::KeyChain(chain)),
    );
    let entry = Entry::Membership(MembershipEntry {
        subject: invitation.key.id(),
        action: Action::Add {
            role: Role::Writer.into(),
            key: invitation.key,
            certificate: Box::new(invitation.certificate.clone()),
            from_generation: fresh,
        },
    });
    let heads = std::collections::BTreeSet::from([rotation.id()]);
    let add = membership::write(
        &member,
        id,
        next.sequence + 1,
        generation,
        &ControlBody::new(heads, entry),
    );
    [rotation, add]
}

/// A space of the admin's, with the member `who` of the same account added
/// as `role`; the member's database holds the space.
pub(crate) fn joined(account: &SigningKey, role: Role) -> (Db, Space, Db, super::Invitation) {
    let admin = of_account(1, account);
    let project = space(&admin);
    let member = of_account(2, account);
    let invitation = now(member.invitation());
    let options = AddMember {
        role,
        fingerprint: None,
    };
    now(project.members().add(&invitation, options)).unwrap();
    copy(&admin, &member, &project);
    (admin, project, member, invitation)
}

#[test]
fn a_member_that_is_no_admin_may_not_add_one() {
    let account = SigningKey::from_bytes(&[9; 32]);
    let (_, project, member, _) = joined(&account, Role::Writer);
    let on_member = now(member.spaces().get(project.id())).unwrap();
    let invitation = now(of_account(3, &account).invitation());

    let refused = now(on_member.members().add(&invitation, writer())).unwrap_err();

    assert_eq!(refused.code(), ErrorCode::NotAllowed);
}

#[test]
fn a_member_that_was_removed_may_not_add_one() {
    let account = SigningKey::from_bytes(&[9; 32]);
    let (admin, project, member, invitation) = joined(&account, Role::Admin);
    remove(&admin, &project, &invitation);
    copy(&admin, &member, &project);
    let on_member = crate::space::Space::handle(member.inner.clone(), project.id());
    let other = now(of_account(3, &account).invitation());

    let refused = now(on_member.members().add(&other, writer())).unwrap_err();

    assert_eq!(refused.code(), ErrorCode::NotMember);
}

#[test]
fn a_member_or_a_former_member_is_not_added_again() {
    let account = SigningKey::from_bytes(&[9; 32]);
    let (admin, project, _, invitation) = joined(&account, Role::Writer);

    let again = now(project.members().add(&invitation, writer())).unwrap_err();
    remove(&admin, &project, &invitation);
    let re_grant = now(project.members().add(&invitation, writer())).unwrap_err();

    assert_eq!(again.code(), ErrorCode::InvalidValue);
    assert_eq!(re_grant.code(), ErrorCode::InvalidValue);
}
