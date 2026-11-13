//! `Account::remove_member` (RFC 0009, "Accounts and devices"; RFC 0010,
//! "Losing a device"; NOC-268): another device of the account removes the
//! lost phone with the recovery code, which renews the account key with
//! the next number. The remaining tablet takes the new key from the
//! account space, and both carry the new statement into their spaces, so
//! that an add certified by the old key is refused. A code that is not the
//! account's makes no new account key and removes nothing.
//!
//! Every simulated member holds the test account's key, statement number
//! 1, and the recovery code [`Keys::of_account`](crate::keys::Keys)
//! derives from it.

use ncdb_core::membership::AccountKeyStatement;
use ncdb_core::recovery::RecoveryCode;

use super::support::*;
use crate::account;
use crate::db::{Db, lock};
use crate::error::ErrorCode;
use crate::id::SpaceId;
use crate::keys;
use crate::members::{AddMember, MemberId, Notice, Role};

/// The account's laptop, which removes the phone.
const LAPTOP: Who = 1;
/// The account's phone, which is lost.
const PHONE: Who = 2;
/// A member of another person.
const OTHER: Who = 3;
/// The account's tablet, which remains.
const TABLET: Who = 4;
/// A database that still holds the account's old key.
const STALE: Who = 5;

/// The words of the test account's recovery code.
fn words() -> String {
    let bits: [u8; 16] = account().to_bytes()[..16].try_into().unwrap();
    RecoveryCode::from_bits(bits).words().to_string()
}

fn account_space(db: &Db) -> SpaceId {
    SpaceId(account::space(&lock(&db.inner)).expect("the account space"))
}

fn member_id(db: &Db) -> [u8; 32] {
    lock(&db.inner).member.key().id().0
}

fn member(db: &Db) -> MemberId {
    MemberId(ncdb_core::ids::MemberId(member_id(db)))
}

/// The statement the database holds, and its account key.
fn held(db: &Db) -> (AccountKeyStatement, [u8; 32]) {
    match &lock(&db.inner).account {
        keys::Account::Holds { key, statement } => {
            (statement.clone(), key.verifying_key().to_bytes())
        }
        _ => panic!("the database holds no account key"),
    }
}

fn add(by: &Db, space: SpaceId, to: &Db, role: Role) {
    let invitation = now(to.invitation());
    let options = AddMember {
        role,
        fingerprint: None,
    };
    now(handle(by, space.0).members().add(&invitation, options)).unwrap();
}

fn role_in(db: &Db, space: SpaceId, member: [u8; 32]) -> Option<ncdb_core::membership::Role> {
    let inner = lock(&db.inner);
    let resolved = inner.spaces[&space.0].checks.resolved();
    resolved.role(ncdb_core::ids::MemberId(member))
}

/// A laptop with an app space, a phone and a tablet in the account space,
/// and the phone and the tablet in the app space too, all recorded.
async fn devices(relay: &LocalRelay) -> (Db, Db, Db, SpaceId) {
    let laptop = open(LAPTOP, &relay.url()).await;
    let phone = open_joining(PHONE, &relay.url()).await;
    let tablet = open_joining(TABLET, &relay.url()).await;
    let shared = now(laptop.spaces().create()).unwrap().id();
    for device in [&phone, &tablet] {
        add(&laptop, account_space(&laptop), device, Role::Admin);
        add(&laptop, shared, device, Role::Writer);
    }
    synced(&laptop).await;
    let ids = [member_id(&laptop), member_id(&phone), member_id(&tablet)];
    eventually("the laptop reads every device's records", || {
        let records = account::memberships(&lock(&laptop.inner)).unwrap();
        ids.iter().all(|id| {
            records
                .iter()
                .any(|m| m.member == *id && m.space == shared.0.0)
        })
    })
    .await;
    (laptop, phone, tablet, shared)
}

#[tokio::test(flavor = "multi_thread")]
async fn remove_member_removes_the_phone_and_renews_the_account_key() {
    let relay = LocalRelay::start().await;
    let (laptop, phone, _tablet, shared) = devices(&relay).await;
    let (old, old_key) = held(&laptop);

    let removed = now(laptop.account().remove_member(member(&phone), &words())).unwrap();

    assert_eq!(removed.removed, [(shared, Notice::MEMBER_REMOVED)]);
    assert!(removed.ask_admins.is_empty());
    assert_eq!(removed.notice, Notice::ACCOUNT_KEY_RENEWED);
    assert_eq!(role_in(&laptop, shared, member_id(&phone)), None);
    let (new, new_key) = held(&laptop);
    assert_eq!(new.number, old.number + 1, "the next number");
    assert_eq!(new.recovery_key, old.recovery_key, "the same account");
    assert_ne!(new_key, old_key);
    assert_eq!(new.account_key, new_key);
    // The laptop's certificate is now under the new account key.
    let invitation = now(laptop.invitation());
    let certificate = invitation.certificate().expect("a certificate");
    assert_eq!(certificate.account_key, new_key);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_tablet_takes_the_new_account_key_and_a_new_certificate() {
    let relay = LocalRelay::start().await;
    let (laptop, phone, tablet, _shared) = devices(&relay).await;

    now(laptop.account().remove_member(member(&phone), &words())).unwrap();
    let (statement, key) = held(&laptop);
    synced(&laptop).await;

    eventually("the tablet holds the new account key", || {
        held(&tablet) == (statement.clone(), key)
    })
    .await;
    let invitation = now(tablet.invitation());
    assert_eq!(invitation.certificate().unwrap().account_key, key);
    // The phone, removed from the account space, keeps the old key.
    assert_ne!(held(&phone).1, key);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_new_statement_is_carried_into_the_accounts_spaces() {
    let relay = LocalRelay::start().await;
    let (laptop, phone, _tablet, shared) = devices(&relay).await;

    now(laptop.account().remove_member(member(&phone), &words())).unwrap();
    let (statement, _) = held(&laptop);

    let spaces = [shared, account_space(&laptop)];
    let inner = lock(&laptop.inner);
    for space in spaces {
        let resolved = inner.spaces[&space.0].checks.resolved();
        assert!(
            resolved.statements().any(|s| *s == statement),
            "{space} holds the new statement"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_member_that_learnt_the_new_statement_refuses_an_add_certified_by_the_old_key() {
    let relay = LocalRelay::start().await;
    let (laptop, phone, _tablet, shared) = devices(&relay).await;
    now(laptop.account().remove_member(member(&phone), &words())).unwrap();

    // A database that still holds the old account key, as the lost phone
    // does, offers a member certified by it.
    let stale = open(STALE, &relay.url()).await;
    let invitation = now(stale.invitation());
    let options = AddMember {
        role: Role::Writer,
        fingerprint: Some(invitation.fingerprint()),
    };
    let refused = now(handle(&laptop, shared.0)
        .members()
        .add(&invitation, options))
    .unwrap_err();

    assert_eq!(refused.code(), ErrorCode::InvalidCertificate);
    assert_eq!(role_in(&laptop, shared, member_id(&stale)), None);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_lost_code_cannot_make_a_new_account_key() {
    let relay = LocalRelay::start().await;
    let (laptop, phone, _tablet, shared) = devices(&relay).await;
    let before = held(&laptop);

    // Twelve valid words of another code.
    let other = RecoveryCode::from_bits([0x11; 16]).words().to_string();
    let refused = now(laptop.account().remove_member(member(&phone), &other)).unwrap_err();
    assert_eq!(refused.code(), ErrorCode::NotAllowed);
    // Words that are no code at all.
    let refused = now(laptop.account().remove_member(member(&phone), "lost words")).unwrap_err();
    assert_eq!(refused.code(), ErrorCode::InvalidValue);

    // Nothing was removed or renewed.
    assert_eq!(held(&laptop), before);
    assert!(role_in(&laptop, shared, member_id(&phone)).is_some());
    assert!(
        account::statements(&lock(&laptop.inner))
            .unwrap()
            .is_empty()
    );
    let records = account::members(&lock(&laptop.inner)).unwrap();
    assert!(records.iter().all(|m| !m.removed));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_space_of_another_person_is_listed_for_its_admins() {
    let relay = LocalRelay::start().await;
    let (laptop, phone, _tablet, _shared) = devices(&relay).await;
    let other = open(OTHER, &relay.url()).await;
    let theirs = now(other.spaces().create()).unwrap().id();
    add(&other, theirs, &phone, Role::Writer);
    synced(&other).await;
    let lost = member_id(&phone);
    eventually("the laptop reads that the phone is in their space", || {
        account::memberships(&lock(&laptop.inner))
            .unwrap()
            .iter()
            .any(|m| m.member == lost && m.space == theirs.0.0)
    })
    .await;

    let removed = now(laptop.account().remove_member(member(&phone), &words())).unwrap();

    assert_eq!(removed.ask_admins, [theirs]);
}
