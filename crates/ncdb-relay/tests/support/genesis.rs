//! Geneses and certificates of accounts with a recovery key, for the tests
//! of admission by the account's identity (RFC 0015, "The account's
//! identity").

use std::collections::BTreeSet;

use ed25519_dalek::SigningKey;
use ncdb_core::change::Change;
use ncdb_core::keys::RecoveryKey;
use ncdb_core::membership::{
    self, AccountKeyStatement, Certificate, ControlBody, Entry, Genesis, MemberKind,
};

use super::space::{MODEL, Who, key, space_key};

/// The statement, under `recovery`, that names `account` with `number`.
pub fn statement(recovery: &RecoveryKey, account: &SigningKey, number: u64) -> AccountKeyStatement {
    AccountKeyStatement::sign(recovery, account.verifying_key().to_bytes(), number)
}

/// A device certificate of `who` by `account`, with the statement that
/// names `account` under `recovery` with `number`.
pub fn certified(
    who: Who,
    account: &SigningKey,
    recovery: &RecoveryKey,
    number: u64,
) -> Certificate {
    Certificate::sign(account, key(who).id(), MemberKind::Device)
        .with_statement(statement(recovery, account, number))
}

/// A genesis that `creator`, certified by `certificate`, writes with
/// `nonce`.
pub fn genesis_of(creator: Who, certificate: Certificate, nonce: [u8; 16]) -> Change {
    let creator_key = key(creator);
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
