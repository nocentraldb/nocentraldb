//! A new account key after a lost device (RFC 0010, "Losing a device",
//! step 3; NOC-268), the account space's side: the device that removed the
//! lost member draws a new account key with the next number, signs its
//! statement with the recovery key, records the statement in the account
//! space, and hands the key to each remaining device there, sealed to that
//! device's agreement key. The lost device gets none, and no member of the
//! account space can read the key from its content.

use ed25519_dalek::SigningKey;
use ncdb_core::membership::AccountKeyStatement;
use ncdb_core::recovery::RecoveryCode;

use super::support::*;
use crate::account;
use crate::db::{Db, lock};
use crate::id::SpaceId;
use crate::members::{AddMember, MemberId, Role};

/// The account's laptop, which renews the account key.
const LAPTOP: Who = 1;
/// The account's phone, which is lost.
const PHONE: Who = 2;
/// The account's tablet, which remains.
const TABLET: Who = 4;

fn account_space(db: &Db) -> SpaceId {
    SpaceId(account::space(&lock(&db.inner)).expect("the account space"))
}

fn member_id(db: &Db) -> [u8; 32] {
    lock(&db.inner).member.key().id().0
}

fn add_to_account(laptop: &Db, device: &Db) {
    let invitation = now(device.invitation());
    let options = AddMember {
        role: Role::Admin,
        fingerprint: None,
    };
    let space = account_space(laptop);
    now(handle(laptop, space.0).members().add(&invitation, options)).unwrap();
}

/// A laptop, a phone and a tablet in one account space, each recorded,
/// and the phone removed as lost.
async fn phone_lost(relay: &LocalRelay) -> (Db, Db, Db) {
    let laptop = open(LAPTOP, &relay.url()).await;
    let phone = open_joining(PHONE, &relay.url()).await;
    let tablet = open_joining(TABLET, &relay.url()).await;
    now(laptop.spaces().create()).unwrap();
    add_to_account(&laptop, &phone);
    add_to_account(&laptop, &tablet);
    synced(&laptop).await;
    let devices = [member_id(&laptop), member_id(&phone), member_id(&tablet)];
    eventually("the laptop reads every device's record", || {
        let records = account::members(&lock(&laptop.inner)).unwrap();
        devices
            .iter()
            .all(|d| records.iter().any(|m| m.member == *d))
    })
    .await;
    let lost = MemberId(ncdb_core::ids::MemberId(member_id(&phone)));
    now(laptop.remove_lost(lost)).unwrap();
    (laptop, phone, tablet)
}

/// The recovery code of the test account, and the number of its current
/// statement.
fn code() -> RecoveryCode {
    RecoveryCode::from_bits([0x5a; 16])
}

fn renew(laptop: &Db) -> (AccountKeyStatement, SigningKey) {
    let recovery = code().recovery_key();
    account::renew(&mut lock(&laptop.inner), &recovery, 2).unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_new_account_key_with_the_next_number_reaches_each_remaining_device() {
    let relay = LocalRelay::start().await;
    let (laptop, _phone, tablet) = phone_lost(&relay).await;

    let (statement, key) = renew(&laptop);
    assert!(statement.verify());
    assert_eq!(statement.number, 2);
    assert_eq!(statement.recovery_key, code().recovery_key().public());
    assert_eq!(statement.account_key, key.verifying_key().to_bytes());
    synced(&laptop).await;

    eventually("the tablet opens the new account key", || {
        let held = account::new_key(&lock(&tablet.inner)).unwrap();
        held.is_some_and(|(s, k)| s == statement && k.to_bytes() == key.to_bytes())
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn the_lost_device_gets_no_new_account_key() {
    let relay = LocalRelay::start().await;
    let (laptop, phone, tablet) = phone_lost(&relay).await;

    renew(&laptop);

    let sealed_to = account::wrapped_to(&lock(&laptop.inner)).unwrap();
    assert_eq!(sealed_to, [(member_id(&tablet), 2)]);
    assert!(account::new_key(&lock(&phone.inner)).unwrap().is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn the_account_space_never_holds_the_new_account_key_readably() {
    let relay = LocalRelay::start().await;
    let (laptop, _phone, _tablet) = phone_lost(&relay).await;

    let (_, key) = renew(&laptop);

    let seed = key.to_bytes();
    let hex: String = seed.iter().map(|b| format!("{b:02x}")).collect();
    let base64 = data_encoding::BASE64URL_NOPAD.encode(&seed);
    let texts = account::texts(&lock(&laptop.inner)).unwrap();
    assert!(!texts.is_empty());
    for text in texts {
        assert!(!text.contains(&hex) && !text.contains(&base64), "{text}");
    }
}
