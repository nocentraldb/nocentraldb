//! Losing a device (RFC 0010, "Losing a device"; NOC-268), the account
//! space's side: another device of the account removes the lost member
//! from every space where it is an admin, which rotates each space key,
//! and from the account space, whose record of the lost member then says
//! `removed`. The spaces where the lost member is a member and no device
//! of the account is an admin are listed, for the app to ask their admins.
//!
//! Pairing is NOC-267's: here the laptop adds the phone to the account
//! space directly, as an admin adds any member (RFC 0008, "Adding a
//! member"), as `account_space.rs` does.

use super::support::*;
use crate::account;
use crate::db::{Db, lock};
use crate::error::ErrorCode;
use crate::id::SpaceId;
use crate::members::{AddMember, MemberId, Notice, Role};

/// The account's laptop, which runs the removal.
const LAPTOP: Who = 1;
/// The account's phone, which is lost.
const PHONE: Who = 2;
/// Another person, who adds the phone to a space of their own.
const OTHER: Who = 3;

fn account_space(db: &Db) -> SpaceId {
    SpaceId(account::space(&lock(&db.inner)).expect("the account space"))
}

fn member_id(db: &Db) -> [u8; 32] {
    lock(&db.inner).member.key().id().0
}

/// The spaces `member` belongs to, as the account space of `db` records
/// them.
fn recorded_spaces(db: &Db, member: [u8; 32]) -> Vec<SpaceId> {
    let mut spaces: Vec<SpaceId> = account::memberships(&lock(&db.inner))
        .unwrap()
        .into_iter()
        .filter(|m| m.member == member)
        .map(|m| SpaceId(ncdb_core::ids::SpaceId(m.space)))
        .collect();
    spaces.sort();
    spaces
}

fn role_in(db: &Db, space: SpaceId, member: [u8; 32]) -> Option<ncdb_core::membership::Role> {
    let inner = lock(&db.inner);
    let resolved = inner.spaces[&space.0].checks.resolved();
    resolved.role(ncdb_core::ids::MemberId(member))
}

fn generation(db: &Db, space: SpaceId) -> Option<u64> {
    let inner = lock(&db.inner);
    inner.spaces[&space.0].checks.resolved().current_generation()
}

/// Adds the database `to` to `space` of `by`, with `role`.
fn add(by: &Db, space: SpaceId, to: &Db, role: Role) {
    let invitation = now(to.invitation());
    let options = AddMember {
        role,
        fingerprint: None,
    };
    now(handle(by, space.0).members().add(&invitation, options)).unwrap();
}

/// A laptop with an app space, and a phone of the same account in the
/// account space and in that app space, both recorded.
struct Account {
    laptop: Db,
    phone: Db,
    shared: SpaceId,
}

async fn laptop_and_phone(relay: &LocalRelay) -> Account {
    let laptop = open(LAPTOP, &relay.url()).await;
    let phone = open_joining(PHONE, &relay.url()).await;
    let shared = now(laptop.spaces().create()).unwrap().id();
    add(&laptop, account_space(&laptop), &phone, Role::Admin);
    add(&laptop, shared, &phone, Role::Writer);
    synced(&laptop).await;
    let lost = member_id(&phone);
    eventually("the laptop reads the phone's records", || {
        recorded_spaces(&laptop, lost) == [shared]
    })
    .await;
    Account {
        laptop,
        phone,
        shared,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_lost_device_is_removed_from_the_spaces_the_account_administers() {
    let relay = LocalRelay::start().await;
    let Account {
        laptop,
        phone,
        shared,
    } = laptop_and_phone(&relay).await;
    let lost = member_id(&phone);
    let before = generation(&laptop, shared);

    let removal = now(laptop.remove_lost(MemberId(ncdb_core::ids::MemberId(lost)))).unwrap();

    assert_eq!(removal.removed, [(shared, Notice::MEMBER_REMOVED)]);
    assert!(removal.ask_admins.is_empty(), "{:?}", removal.ask_admins);
    assert_eq!(role_in(&laptop, shared, lost), None);
    assert_ne!(generation(&laptop, shared), before, "the space key rotates");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_lost_device_leaves_the_account_space_marked_removed() {
    let relay = LocalRelay::start().await;
    let Account { laptop, phone, .. } = laptop_and_phone(&relay).await;
    let lost = member_id(&phone);
    let accounts = account_space(&laptop);
    let before = generation(&laptop, accounts);

    now(laptop.remove_lost(MemberId(ncdb_core::ids::MemberId(lost)))).unwrap();

    assert_eq!(role_in(&laptop, accounts, lost), None);
    assert_ne!(generation(&laptop, accounts), before, "the account space's key rotates");
    let records = account::members(&lock(&laptop.inner)).unwrap();
    let record = records.iter().find(|m| m.member == lost).expect("its record");
    assert!(record.removed, "{record:?}");
    let laptop_record = records.iter().find(|m| m.member == member_id(&laptop));
    assert!(!laptop_record.expect("the laptop's record").removed);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_space_no_device_of_the_account_administers_is_listed_for_its_admins() {
    let relay = LocalRelay::start().await;
    let Account {
        laptop,
        phone,
        shared,
    } = laptop_and_phone(&relay).await;
    let lost = member_id(&phone);
    // Another person adds the phone to a space of their own, as a writer.
    let other = open(OTHER, &relay.url()).await;
    let theirs = now(other.spaces().create()).unwrap().id();
    add(&other, theirs, &phone, Role::Writer);
    synced(&other).await;
    eventually("the laptop reads that the phone is in their space", || {
        recorded_spaces(&laptop, lost).contains(&theirs)
    })
    .await;

    let removal = now(laptop.remove_lost(MemberId(ncdb_core::ids::MemberId(lost)))).unwrap();

    assert_eq!(removal.removed, [(shared, Notice::MEMBER_REMOVED)]);
    assert_eq!(removal.ask_admins, [theirs]);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_device_does_not_remove_itself_or_a_member_of_no_record() {
    let relay = LocalRelay::start().await;
    let Account { laptop, .. } = laptop_and_phone(&relay).await;
    let me = MemberId(ncdb_core::ids::MemberId(member_id(&laptop)));
    let refused = now(laptop.remove_lost(me)).unwrap_err();
    assert_eq!(refused.code(), ErrorCode::InvalidValue);

    let stranger = MemberId(ncdb_core::ids::MemberId([0x77; 32]));
    let refused = now(laptop.remove_lost(stranger)).unwrap_err();
    assert_eq!(refused.code(), ErrorCode::NotFound);
}
