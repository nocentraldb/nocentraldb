//! RFC 0015, "The account's identity": a lost device keeps its old
//! certificate, whose statement names the same recovery key with a lower
//! number. The relay keeps, per identity, the highest statement number it
//! has seen in a genesis, an add or an account key statement entry, and
//! refuses with `not_allowed` a genesis whose statement carries a lower
//! number (NOC-420). Until the relay has seen the newer statement, the old
//! certificate still creates spaces.

mod support;

use std::collections::BTreeSet;

use ed25519_dalek::SigningKey;
use ncdb_core::change::Change;
use ncdb_core::keys::RecoveryKey;
use ncdb_core::membership::{Entry, Role};
use ncdb_relay::sqlite::SqliteStore;
use ncdb_relay::{ErrorCode, Pushed, Relay};
use support::genesis::{certified, genesis_of, statement};
use support::space::{Space, Who, account};
use support::{TempDir, config, push, push_one};

/// The host of a shared space, a member of the listed simulated account.
const HOST: Who = 0;
/// A reader of the shared space.
const READER: Who = 2;
/// The lost device, certified by the old account key.
const LOST: Who = 3;
/// A device certified by the new account key.
const NEW: Who = 4;
/// Another device certified by the old account key.
const OLD_TWO: Who = 5;

fn recovery() -> RecoveryKey {
    RecoveryKey::from_code(&[0x53; 16])
}

/// The account key the lost device was certified by, statement 1.
fn old_key() -> SigningKey {
    SigningKey::from_bytes(&[0x81; 32])
}

/// The account key that replaced it, statement 2.
fn new_key() -> SigningKey {
    SigningKey::from_bytes(&[0x82; 32])
}

/// A relay that lists the simulated account and the identity under test.
fn relay() -> (Relay<SqliteStore>, TempDir) {
    let dir = TempDir::new();
    let mut config = config();
    config.allowed_accounts =
        BTreeSet::from([account().verifying_key().to_bytes(), recovery().public()]);
    (Relay::new(dir.store(), config), dir)
}

/// The lost device's genesis, certified by the old account key.
fn lost_genesis() -> Change {
    genesis_of(LOST, certified(LOST, &old_key(), &recovery(), 1), [1; 16])
}

/// Pushes a space the host created, then `rest` written in it.
fn push_shared(relay: &Relay<SqliteStore>, space: &Space, rest: &[&Change]) {
    let mut changes = vec![&space.genesis];
    changes.extend_from_slice(rest);
    let pushed = push(relay, HOST, space.space, &changes);
    assert!(
        pushed.iter().all(|p| matches!(p, Pushed::Accepted(_))),
        "{pushed:?}"
    );
}

#[test]
fn an_older_statement_is_refused_once_a_newer_one_came_in_a_genesis() {
    let (relay, _dir) = relay();
    let newer = genesis_of(NEW, certified(NEW, &new_key(), &recovery(), 2), [2; 16]);
    assert_eq!(push_one(&relay, NEW, &newer), Pushed::Accepted(1));
    assert_eq!(
        push_one(&relay, LOST, &lost_genesis()),
        Pushed::Rejected(ErrorCode::NotAllowed)
    );
}

#[test]
fn an_older_statement_is_refused_once_a_newer_one_came_in_an_add() {
    let (relay, _dir) = relay();
    let mut shared = Space::new(HOST);
    let genesis = shared.genesis.clone();
    let certificate = certified(NEW, &new_key(), &recovery(), 2);
    let add = shared.add_certified(HOST, NEW, Role::Writer, certificate, &[&genesis]);
    push_shared(&relay, &shared, &[&add]);
    assert_eq!(
        push_one(&relay, LOST, &lost_genesis()),
        Pushed::Rejected(ErrorCode::NotAllowed)
    );
}

#[test]
fn an_older_statement_is_refused_once_a_newer_one_came_in_a_statement_entry() {
    let (relay, _dir) = relay();
    let mut shared = Space::new(HOST);
    let genesis = shared.genesis.clone();
    // The space knows the identity from a member the old key certified.
    let certificate = certified(OLD_TWO, &old_key(), &recovery(), 1);
    let old = shared.add_certified(HOST, OLD_TWO, Role::Writer, certificate, &[&genesis]);
    let add = shared.add(HOST, READER, Role::Reader, &[&old]);
    let newer = Entry::AccountKeyStatement(statement(&recovery(), &new_key(), 2));
    let entry = shared.write(READER, newer, &[&add]);
    push_shared(&relay, &shared, &[&old, &add]);
    assert_eq!(push_one(&relay, READER, &entry), Pushed::Accepted(4));
    assert_eq!(
        push_one(&relay, LOST, &lost_genesis()),
        Pushed::Rejected(ErrorCode::NotAllowed)
    );
}

#[test]
fn an_older_statement_creates_spaces_until_a_newer_one_comes() {
    let (relay, _dir) = relay();
    assert_eq!(push_one(&relay, LOST, &lost_genesis()), Pushed::Accepted(1));
    // Another device of the same account key, with the same number.
    let same = genesis_of(
        OLD_TWO,
        certified(OLD_TWO, &old_key(), &recovery(), 1),
        [3; 16],
    );
    assert_eq!(push_one(&relay, OLD_TWO, &same), Pushed::Accepted(1));
    let newer = genesis_of(NEW, certified(NEW, &new_key(), &recovery(), 2), [2; 16]);
    assert_eq!(push_one(&relay, NEW, &newer), Pushed::Accepted(1));
}
