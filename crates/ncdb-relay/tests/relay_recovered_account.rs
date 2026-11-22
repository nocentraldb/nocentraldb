//! RFC 0015, "The account's identity": the relay admits a genesis by the
//! identity of the account that certified its creator, the recovery key
//! that the certificate's account key statement names. RFC 0010 answers a
//! recovery or a lost device with a new account key under the same
//! recovery key, so the account stays the same account after either
//! (NOC-420). An entry of the allowlist that holds an account key keeps
//! matching that key: an account without a statement, and an account whose
//! statement still names that key, until its first recovery.

mod support;

use std::collections::BTreeSet;

use ed25519_dalek::SigningKey;
use ncdb_core::keys::RecoveryKey;
use ncdb_core::membership::{Certificate, MemberKind};
use ncdb_relay::{ErrorCode, Pushed, Relay};
use support::genesis;
use support::space::{Who, key};
use support::{TempDir, config, push_one};

/// The recovery code of the listed account.
const CODE: u8 = 0x52;
/// The device recovered from the code: a member of nothing yet.
const RECOVERED: Who = 3;
/// Recovery numbers the new key with the current time (NOC-285).
const RECOVERED_NUMBER: u64 = 1_800_000_000;

fn recovery() -> RecoveryKey {
    RecoveryKey::from_code(&[CODE; 16])
}

/// The account key the account drew first, named by statement 1.
fn first_key() -> SigningKey {
    SigningKey::from_bytes(&[0x71; 32])
}

/// The account key recovery drew, named by a statement with a higher
/// number under the same recovery key (RFC 0010, "Recovering").
fn recovered_key() -> SigningKey {
    SigningKey::from_bytes(&[0x72; 32])
}

/// A certificate of `who` by `account`, with the statement that names
/// `account` under the listed account's recovery key with `number`.
fn certified(who: Who, account: &SigningKey, number: u64) -> Certificate {
    genesis::certified(who, account, &recovery(), number)
}

/// A genesis that `creator` writes, certified by `certificate`.
fn genesis_of(creator: Who, certificate: Certificate) -> ncdb_core::change::Change {
    genesis::genesis_of(creator, certificate, [9; 16])
}

/// A relay whose allowlist holds `listed` alone.
fn listing(listed: [u8; 32]) -> (Relay<ncdb_relay::sqlite::SqliteStore>, TempDir) {
    let dir = TempDir::new();
    let mut config = config();
    config.allowed_accounts = BTreeSet::from([listed]);
    (Relay::new(dir.store(), config), dir)
}

#[test]
fn a_listed_account_may_create_a_space_after_recovering_with_a_new_account_key() {
    let (relay, _dir) = listing(recovery().public());
    let genesis = genesis_of(
        RECOVERED,
        certified(RECOVERED, &recovered_key(), RECOVERED_NUMBER),
    );
    assert_eq!(push_one(&relay, RECOVERED, &genesis), Pushed::Accepted(1));
}

#[test]
fn a_listed_account_key_admits_an_account_without_a_statement() {
    let (relay, _dir) = listing(first_key().verifying_key().to_bytes());
    let certificate = Certificate::sign(&first_key(), key(RECOVERED).id(), MemberKind::Device);
    let genesis = genesis_of(RECOVERED, certificate);
    assert_eq!(push_one(&relay, RECOVERED, &genesis), Pushed::Accepted(1));
}

#[test]
fn a_listed_account_key_admits_the_account_while_its_statement_names_that_key() {
    let (relay, _dir) = listing(first_key().verifying_key().to_bytes());
    let genesis = genesis_of(RECOVERED, certified(RECOVERED, &first_key(), 1));
    assert_eq!(push_one(&relay, RECOVERED, &genesis), Pushed::Accepted(1));
}

#[test]
fn a_listed_account_key_no_longer_admits_the_account_after_it_recovers() {
    let (relay, _dir) = listing(first_key().verifying_key().to_bytes());
    let genesis = genesis_of(
        RECOVERED,
        certified(RECOVERED, &recovered_key(), RECOVERED_NUMBER),
    );
    assert_eq!(
        push_one(&relay, RECOVERED, &genesis),
        Pushed::Rejected(ErrorCode::NotAllowed)
    );
}
