//! RFC 0008's account key statement entry (control entry type 7; Carsten,
//! 2026-11-11, NOC-187) at the relay, which runs the same `resolve` as the
//! members (RFC 0008, "How the relay applies the rules"): it appends a
//! valid entry from any member, a reader too, and refuses an entry that is
//! not valid when written and an add whose certificate names an account
//! key the entry superseded, with `membership_rule`.

mod support;

use ed25519_dalek::SigningKey;
use ncdb_core::change::Change;
use ncdb_core::keys::RecoveryKey;
use ncdb_core::membership::{AccountKeyStatement, Certificate, Entry, MemberKind, Role};
use ncdb_relay::{ErrorCode, Pushed};
use support::space::{Space, id};
use support::{push, push_one, relay};

/// The recovery code of the account whose device is lost.
const CODE: u8 = 0x51;

fn old_key() -> SigningKey {
    SigningKey::from_bytes(&[0x61; 32])
}

fn new_key() -> SigningKey {
    SigningKey::from_bytes(&[0x62; 32])
}

fn statement(account: &SigningKey, number: u64) -> AccountKeyStatement {
    let recovery = RecoveryKey::from_code(&[CODE; 16]);
    AccountKeyStatement::sign(&recovery, account.verifying_key().to_bytes(), number)
}

fn certified(who: u8, account: &SigningKey, number: u64) -> Certificate {
    Certificate::sign(account, id(who), MemberKind::Device)
        .with_statement(statement(account, number))
}

/// A space on the relay where 0 created it, added 1 as a writer certified
/// by the old account key with statement 1, and 2 as a reader.
fn held() -> (
    ncdb_relay::Relay<ncdb_relay::sqlite::SqliteStore>,
    support::TempDir,
    Space,
    Change,
) {
    let (relay, dir) = relay();
    let mut space = Space::new(0);
    let genesis = space.genesis.clone();
    let add_1 = space.add_certified(0, 1, Role::Writer, certified(1, &old_key(), 1), &[&genesis]);
    let add_2 = space.add(0, 2, Role::Reader, &[&add_1]);
    let pushed = push(&relay, 0, space.space, &[&genesis, &add_1, &add_2]);
    assert!(
        pushed.iter().all(|p| matches!(p, Pushed::Accepted(_))),
        "{pushed:?}"
    );
    (relay, dir, space, add_2)
}

#[test]
fn a_readers_statement_entry_with_a_newer_number_is_appended() {
    let (relay, _dir, mut space, last) = held();
    let entry = space.write(
        2,
        Entry::AccountKeyStatement(statement(&new_key(), 2)),
        &[&last],
    );
    assert_eq!(push_one(&relay, 2, &entry), Pushed::Accepted(4));
}

#[test]
fn a_statement_entry_that_is_not_newer_is_refused() {
    let (relay, _dir, mut space, last) = held();
    let entry = space.write(
        2,
        Entry::AccountKeyStatement(statement(&new_key(), 1)),
        &[&last],
    );
    assert_eq!(
        push_one(&relay, 2, &entry),
        Pushed::Rejected(ErrorCode::MembershipRule)
    );
}

#[test]
fn an_add_under_a_superseded_account_key_is_refused() {
    let (relay, _dir, mut space, last) = held();
    let entry = space.write(
        2,
        Entry::AccountKeyStatement(statement(&new_key(), 2)),
        &[&last],
    );
    assert_eq!(push_one(&relay, 2, &entry), Pushed::Accepted(4));
    let old = space.add_certified(0, 3, Role::Writer, certified(3, &old_key(), 1), &[&entry]);
    assert_eq!(
        push_one(&relay, 0, &old),
        Pushed::Rejected(ErrorCode::MembershipRule)
    );
    let new = space.add_certified(0, 4, Role::Writer, certified(4, &new_key(), 2), &[&entry]);
    assert_eq!(push_one(&relay, 0, &new), Pushed::Accepted(5));
}
