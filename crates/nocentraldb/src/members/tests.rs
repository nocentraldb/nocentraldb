//! What `members().add` refuses (RFC 0009, "Invitations and members" and
//! "Errors"), on the device, without a relay.

use ed25519_dalek::SigningKey;

use super::{AddMember, Role};
use crate::db::{Db, OpenOptions};
use crate::error::ErrorCode;
use crate::space::Space;

const LOCK: &str = include_str!("../../tests/model/model.lock.json");

/// The output of an SDK future, which is ready at its first poll.
fn now<F: std::future::Future>(future: F) -> F::Output {
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
    now(Db::open_as(options, [who; 32], [who ^ 0x80; 32], account.clone()))
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
