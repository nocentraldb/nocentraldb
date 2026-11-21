//! RFC 0007, "Who may create a space": the relay accepts a genesis from a
//! member whose certificate comes from an account on the operator's
//! allowlist. RFC 0010 makes the recovery key the account's identity and
//! answers a recovery or a lost device with a new account key under it, so
//! the account stays the same account after either (NOC-413). The operator
//! listed the account before the recovery; a device recovered from the code
//! is certified by the new account key, and its genesis must still be
//! accepted.

mod support;

use std::collections::BTreeSet;

use ed25519_dalek::SigningKey;
use ncdb_core::change::Change;
use ncdb_core::keys::RecoveryKey;
use ncdb_core::membership::{
    self, AccountKeyStatement, Certificate, ControlBody, Entry, Genesis, MemberKind,
};
use ncdb_relay::{Pushed, Relay};
use support::space::{MODEL, Who, key, space_key};
use support::{TempDir, config, push_one};

/// The recovery code of the listed account.
const CODE: u8 = 0x52;
/// The device recovered from the code: a member of nothing yet.
const RECOVERED: Who = 3;

/// The account key the operator listed, named by statement 1.
fn listed_key() -> SigningKey {
    SigningKey::from_bytes(&[0x71; 32])
}

/// The account key recovery drew, named by a statement with a higher
/// number under the same recovery key (RFC 0010, "Recovering").
fn recovered_key() -> SigningKey {
    SigningKey::from_bytes(&[0x72; 32])
}

fn certified(who: Who, account: &SigningKey, number: u64) -> Certificate {
    let recovery = RecoveryKey::from_code(&[CODE; 16]);
    let statement =
        AccountKeyStatement::sign(&recovery, account.verifying_key().to_bytes(), number);
    Certificate::sign(account, key(who).id(), MemberKind::Device).with_statement(statement)
}

/// A genesis that `creator` writes, certified by `certificate`.
fn genesis_of(creator: Who, certificate: Certificate) -> Change {
    let creator_key = key(creator);
    let nonce = [9; 16];
    let space = membership::space_id(creator_key.id(), &nonce);
    let first_generation = membership::generation_id(space, creator_key.id(), &[]);
    let genesis = Genesis {
        nonce,
        creator: creator_key.public(),
        certificate,
        first_generation,
        model_id: membership::seal_model_id(&space_key(), space, creator_key.id(), MODEL),
    };
    let body = ControlBody::new(BTreeSet::new(), Entry::Genesis(genesis));
    membership::write(&creator_key, space, 1, first_generation, &body)
}

#[test]
fn a_listed_account_may_create_a_space_after_recovering_with_a_new_account_key() {
    let dir = TempDir::new();
    let mut config = config();
    config.allowed_accounts = BTreeSet::from([listed_key().verifying_key().to_bytes()]);
    let relay = Relay::new(dir.store(), config);
    // Recovery numbers the new key with the current time (NOC-285).
    let genesis = genesis_of(
        RECOVERED,
        certified(RECOVERED, &recovered_key(), 1_800_000_000),
    );
    assert_eq!(push_one(&relay, RECOVERED, &genesis), Pushed::Accepted(1));
}
