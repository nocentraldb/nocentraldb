//! RFC 0015, "The account's identity": the guest's one space (RFC 0007,
//! "Who may create a space") is keyed by the account's identity, the
//! recovery key, so a guest whose account recovered or replaced a lost
//! device gets no second space; and a space that an account listed by its
//! identity created admits its members' accounts as guests (NOC-420).

mod support;

use std::collections::BTreeSet;

use ed25519_dalek::SigningKey;
use ncdb_core::change::Change;
use ncdb_core::keys::RecoveryKey;
use ncdb_core::membership::{
    self, AccountKeyStatement, Certificate, ControlBody, Entry, Genesis, MemberKind, Role,
};
use ncdb_relay::sqlite::SqliteStore;
use ncdb_relay::{ErrorCode, Pushed, Relay};
use support::space::{MODEL, Space, Who, account, key, other_account, space_key};
use support::{TempDir, config, push, push_one};

/// The listed account's member, which creates the shared space.
const HOST: Who = 0;
/// The guest: a device of an account the relay does not list.
const GUEST: Who = 1;
/// The guest's device after its account recovered.
const GUEST_TWO: Who = 2;

/// The recovery key of the host's account.
fn host_recovery() -> RecoveryKey {
    RecoveryKey::from_code(&[0x41; 16])
}

/// The recovery key of the guest's account.
fn guest_recovery() -> RecoveryKey {
    RecoveryKey::from_code(&[0x47; 16])
}

/// A certificate of `who` by `account`, with the statement that names it
/// under `recovery` with `number`.
fn certified(who: Who, account: &SigningKey, recovery: &RecoveryKey, number: u64) -> Certificate {
    let statement = AccountKeyStatement::sign(recovery, account.verifying_key().to_bytes(), number);
    Certificate::sign(account, key(who).id(), MemberKind::Device).with_statement(statement)
}

/// A genesis that `creator`, certified by `certificate`, writes with
/// `nonce`.
fn genesis_of(creator: Who, certificate: Certificate, nonce: [u8; 16]) -> Change {
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

/// A relay whose allowlist holds `listed`, holding a space the host
/// created with `certificate`, with `guests` added to it as writers.
fn hosting(
    listed: [u8; 32],
    certificate: Certificate,
    guests: &[Who],
) -> (Relay<SqliteStore>, TempDir) {
    let dir = TempDir::new();
    let mut config = config();
    config.allowed_accounts = BTreeSet::from([listed]);
    let relay = Relay::new(dir.store(), config);
    let mut shared = Space::new(HOST);
    // The same space, created by the host with this certificate.
    shared.genesis = genesis_of(HOST, certificate, [7; 16]);
    let mut changes = vec![shared.genesis.clone()];
    for guest in guests {
        shared.certify_by(*guest, other_account(1));
        let previous = changes.last().expect("the genesis").clone();
        changes.push(shared.add(HOST, *guest, Role::Writer, &[&previous]));
    }
    let refs: Vec<&Change> = changes.iter().collect();
    let pushed = push(&relay, HOST, shared.space, &refs);
    assert!(
        pushed.iter().all(|p| matches!(p, Pushed::Accepted(_))),
        "{pushed:?}"
    );
    (relay, dir)
}

/// A relay that lists the host's account key, whose certificate carries
/// no statement.
fn hosting_by_account_key(guests: &[Who]) -> (Relay<SqliteStore>, TempDir) {
    let certificate = Certificate::sign(&account(), key(HOST).id(), MemberKind::Device);
    hosting(account().verifying_key().to_bytes(), certificate, guests)
}

#[test]
fn a_member_of_a_space_a_listed_identity_created_may_create_one_space() {
    let certificate = certified(HOST, &account(), &host_recovery(), 1);
    let (relay, _dir) = hosting(host_recovery().public(), certificate, &[GUEST]);
    let genesis = genesis_of(
        GUEST,
        certified(GUEST, &other_account(1), &guest_recovery(), 1),
        [1; 16],
    );
    assert_eq!(push_one(&relay, GUEST, &genesis), Pushed::Accepted(1));
}

#[test]
fn a_guest_creates_no_second_space_after_its_account_recovers() {
    let (relay, _dir) = hosting_by_account_key(&[GUEST, GUEST_TWO]);
    let first = genesis_of(
        GUEST,
        certified(GUEST, &other_account(1), &guest_recovery(), 1),
        [1; 16],
    );
    assert_eq!(push_one(&relay, GUEST, &first), Pushed::Accepted(1));

    // The recovered account key, under the same recovery key.
    let second = genesis_of(
        GUEST_TWO,
        certified(GUEST_TWO, &other_account(2), &guest_recovery(), 2),
        [2; 16],
    );
    assert_eq!(
        push_one(&relay, GUEST_TWO, &second),
        Pushed::Rejected(ErrorCode::NotAllowed)
    );
}
