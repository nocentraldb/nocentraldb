//! RFC 0015, "Admission tokens": a member presents an admission with
//! `admit`; the relay checks it against `NCDB_ADMISSION_KEYS`, its own relay
//! id and its clock, and records the identity with until, spaces, the
//! admission key and a count of the spaces created. A genesis is accepted
//! when its identity is listed, or holds an admission that has not lapsed
//! and whose count is below spaces, and then the guest's one space (pm,
//! 2026-11-23, NOC-469). Bytes that are no admission are `malformed`; an
//! admission the relay does not take is `not_allowed`.

mod support;

use std::collections::BTreeSet;

use ed25519_dalek::SigningKey;
use ncdb_core::admission::Admission;
use ncdb_core::keys::RecoveryKey;
use ncdb_relay::store::RelayStore;
use ncdb_relay::{ErrorCode, Pushed, Relay};
use support::genesis;
use support::space::Who;
use ncdb_relay::conformance::block_on;
use support::{TempDir, config, push_one};

/// The relay's id in these tests.
const RELAY_ID: [u8; 16] = [0x0e; 16];
/// The relay's clock stands at this second (support::config).
const NOW: u64 = 1_700_000_000;
/// The admitted account's device, a member of nothing yet.
const DEVICE: Who = 3;

fn admission_key() -> SigningKey {
    SigningKey::from_bytes(&[0x5a; 32])
}

fn recovery() -> RecoveryKey {
    RecoveryKey::from_code(&[0x61; 16])
}

fn account() -> SigningKey {
    SigningKey::from_bytes(&[0x62; 32])
}

fn identity() -> [u8; 32] {
    recovery().public()
}

/// An admission of the account's identity, by the configured key.
fn admission(until: u64, spaces: u64) -> Vec<u8> {
    Admission::sign(&admission_key(), RELAY_ID.to_vec(), identity(), until, spaces).encode()
}

/// A genesis of the account's device, its `n`th space.
fn genesis(n: u8) -> ncdb_core::change::Change {
    let certificate = genesis::certified(DEVICE, &account(), &recovery(), 1);
    genesis::genesis_of(DEVICE, certificate, [n; 16])
}

/// A relay that lists no account of the test's and takes admissions
/// signed by the admission key.
fn admitting() -> (Relay<ncdb_relay::sqlite::SqliteStore>, TempDir) {
    let dir = TempDir::new();
    let mut config = config();
    config.admission_keys = BTreeSet::from([admission_key().verifying_key().to_bytes()]);
    (Relay::new(dir.store(), config), dir)
}

fn admit(relay: &Relay<ncdb_relay::sqlite::SqliteStore>, bytes: &[u8]) -> Result<(), ErrorCode> {
    block_on(relay.admit(&RELAY_ID, bytes))
}

#[test]
fn without_an_admission_the_genesis_is_not_allowed() {
    let (relay, _dir) = admitting();
    assert_eq!(
        push_one(&relay, DEVICE, &genesis(1)),
        Pushed::Rejected(ErrorCode::NotAllowed)
    );
}

#[test]
fn an_admitted_identity_creates_its_spaces_up_to_spaces() {
    let (relay, _dir) = admitting();
    assert_eq!(admit(&relay, &admission(NOW + 60, 2)), Ok(()));
    assert_eq!(push_one(&relay, DEVICE, &genesis(1)), Pushed::Accepted(1));
    assert_eq!(push_one(&relay, DEVICE, &genesis(2)), Pushed::Accepted(1));
    assert_eq!(
        push_one(&relay, DEVICE, &genesis(3)),
        Pushed::Rejected(ErrorCode::NotAllowed)
    );
}

#[test]
fn the_relay_records_until_spaces_the_key_and_the_count() {
    let (relay, _dir) = admitting();
    admit(&relay, &admission(NOW + 60, 2)).unwrap();
    push_one(&relay, DEVICE, &genesis(1));
    let record = block_on(relay.store().admission(identity()))
        .unwrap()
        .expect("the identity is recorded");
    assert_eq!(record.until, NOW + 60);
    assert_eq!(record.spaces, 2);
    assert_eq!(record.key, admission_key().verifying_key().to_bytes());
    assert_eq!(record.created, 1);
}

#[test]
fn a_later_admission_replaces_until_and_spaces_and_keeps_the_count() {
    let (relay, _dir) = admitting();
    admit(&relay, &admission(NOW + 60, 1)).unwrap();
    assert_eq!(push_one(&relay, DEVICE, &genesis(1)), Pushed::Accepted(1));
    assert_eq!(
        push_one(&relay, DEVICE, &genesis(2)),
        Pushed::Rejected(ErrorCode::NotAllowed)
    );
    // Renewing with the same spaces grants nothing new; raising them does.
    admit(&relay, &admission(NOW + 120, 1)).unwrap();
    assert_eq!(
        push_one(&relay, DEVICE, &genesis(2)),
        Pushed::Rejected(ErrorCode::NotAllowed)
    );
    admit(&relay, &admission(NOW + 120, 2)).unwrap();
    assert_eq!(push_one(&relay, DEVICE, &genesis(2)), Pushed::Accepted(1));
}

#[test]
fn a_lapsed_admission_creates_no_space() {
    let (relay, dir) = admitting();
    admit(&relay, &admission(NOW + 60, 5)).unwrap();
    // The same store, once the relay's clock reached until.
    let mut later = config();
    later.admission_keys = BTreeSet::from([admission_key().verifying_key().to_bytes()]);
    later.clock = || (NOW + 60) * 1000;
    let relay = Relay::new(dir.store(), later);
    assert_eq!(
        push_one(&relay, DEVICE, &genesis(1)),
        Pushed::Rejected(ErrorCode::NotAllowed)
    );
}

#[test]
fn an_admission_that_already_lapsed_is_not_allowed() {
    let (relay, _dir) = admitting();
    assert_eq!(
        admit(&relay, &admission(NOW, 5)),
        Err(ErrorCode::NotAllowed)
    );
    assert_eq!(block_on(relay.store().admission(identity())).unwrap(), None);
}

#[test]
fn an_admission_for_another_relay_is_not_allowed() {
    let (relay, _dir) = admitting();
    let other = Admission::sign(&admission_key(), vec![0x0f; 16], identity(), NOW + 60, 5);
    assert_eq!(admit(&relay, &other.encode()), Err(ErrorCode::NotAllowed));
}

#[test]
fn an_admission_by_a_key_the_relay_does_not_configure_is_not_allowed() {
    let (relay, _dir) = admitting();
    let stranger = SigningKey::from_bytes(&[0x5b; 32]);
    let other = Admission::sign(&stranger, RELAY_ID.to_vec(), identity(), NOW + 60, 5);
    assert_eq!(admit(&relay, &other.encode()), Err(ErrorCode::NotAllowed));
    let mut forged = Admission::decode(&admission(NOW + 60, 5)).unwrap();
    forged.spaces = 50;
    assert_eq!(admit(&relay, &forged.encode()), Err(ErrorCode::NotAllowed));
}

#[test]
fn a_relay_without_admission_keys_takes_no_admission() {
    let dir = TempDir::new();
    let relay = Relay::new(dir.store(), config());
    assert_eq!(
        admit(&relay, &admission(NOW + 60, 5)),
        Err(ErrorCode::NotAllowed)
    );
}

#[test]
fn bytes_that_are_no_admission_are_malformed() {
    let (relay, _dir) = admitting();
    let mut trailing = admission(NOW + 60, 5);
    trailing.push(0);
    for bytes in [vec![], vec![0xa0], trailing] {
        assert_eq!(admit(&relay, &bytes), Err(ErrorCode::Malformed));
    }
}

#[test]
fn a_listed_identity_does_not_spend_its_admission() {
    let dir = TempDir::new();
    let mut config = config();
    config.admission_keys = BTreeSet::from([admission_key().verifying_key().to_bytes()]);
    config.allowed_accounts.insert(identity());
    let relay = Relay::new(dir.store(), config);
    admit(&relay, &admission(NOW + 60, 1)).unwrap();
    assert_eq!(push_one(&relay, DEVICE, &genesis(1)), Pushed::Accepted(1));
    let record = block_on(relay.store().admission(identity())).unwrap().unwrap();
    assert_eq!(record.created, 0);
}
