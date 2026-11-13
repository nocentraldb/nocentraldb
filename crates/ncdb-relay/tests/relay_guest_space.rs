//! RFC 0007, "Who may create a space" (Carsten, 2026-11-12, NOC-286): a
//! relay that admits listed accounts also accepts exactly one space from an
//! account it does not list, once a member of that account belongs to a
//! space a listed account created: the guest's account space (RFC 0010).
//! A second space of the guest's account is refused with `not_allowed`, as
//! is a space of an account that belongs to no such space. The relay cannot
//! tell what kind of space the guest creates, since the genesis seals the
//! model id, and does not need to.

mod support;

use std::collections::BTreeSet;

use ed25519_dalek::SigningKey;
use ncdb_core::change::Change;
use ncdb_core::ids::SpaceId;
use ncdb_core::membership::{self, Certificate, ControlBody, Entry, Genesis, MemberKind, Role};
use ncdb_relay::{ErrorCode, Pushed};
use support::space::{MODEL, Space, Who, key, other_account, space_key};
use support::{push, push_one, relay};

/// The admitted account's member, which creates the shared space.
const HOST: Who = 0;
/// The guest: a device of an account the relay does not list.
const GUEST: Who = 1;
/// Another device of the guest's account.
const GUEST_TWO: Who = 2;

fn guest_account() -> SigningKey {
    other_account(1)
}

/// A genesis that `creator`, certified by `account`, writes with `nonce`.
fn genesis_of(creator: Who, account: &SigningKey, nonce: [u8; 16]) -> (Change, SpaceId) {
    let creator_key = key(creator);
    let space = membership::space_id(creator_key.id(), &nonce);
    let first_generation = membership::generation_id(space, creator_key.id(), &[]);
    let genesis = Genesis {
        nonce,
        creator: creator_key.public(),
        certificate: Certificate::sign(account, creator_key.id(), MemberKind::Device),
        first_generation,
        model_id: membership::seal_model_id(&space_key(), space, creator_key.id(), MODEL),
    };
    let body = ControlBody::new(BTreeSet::new(), Entry::Genesis(genesis));
    let change = membership::write(&creator_key, space, 1, first_generation, &body);
    (change, space)
}

/// A relay that holds a space the admitted host created, with `guests` of
/// the guest's account added to it as writers.
fn hosting(
    guests: &[Who],
) -> (
    ncdb_relay::Relay<ncdb_relay::sqlite::SqliteStore>,
    support::TempDir,
) {
    let (relay, dir) = relay();
    let mut shared = Space::new(HOST);
    let genesis = shared.genesis.clone();
    let mut changes = vec![genesis];
    for guest in guests {
        shared.certify_by(*guest, guest_account());
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

#[test]
fn a_member_of_an_admitted_space_may_create_one_space() {
    let (relay, _dir) = hosting(&[GUEST]);
    let (genesis, _) = genesis_of(GUEST, &guest_account(), [1; 16]);
    assert_eq!(push_one(&relay, GUEST, &genesis), Pushed::Accepted(1));
}

#[test]
fn its_account_creates_no_second_space() {
    let (relay, _dir) = hosting(&[GUEST, GUEST_TWO]);
    let (first, _) = genesis_of(GUEST, &guest_account(), [1; 16]);
    assert_eq!(push_one(&relay, GUEST, &first), Pushed::Accepted(1));

    let (second, _) = genesis_of(GUEST, &guest_account(), [2; 16]);
    assert_eq!(
        push_one(&relay, GUEST, &second),
        Pushed::Rejected(ErrorCode::NotAllowed)
    );
    // Another device of the same account counts against the same one.
    let (other, _) = genesis_of(GUEST_TWO, &guest_account(), [3; 16]);
    assert_eq!(
        push_one(&relay, GUEST_TWO, &other),
        Pushed::Rejected(ErrorCode::NotAllowed)
    );
}

#[test]
fn an_account_in_no_admitted_space_creates_none() {
    let (relay, _dir) = hosting(&[]);
    let (genesis, _) = genesis_of(GUEST, &guest_account(), [1; 16]);
    assert_eq!(
        push_one(&relay, GUEST, &genesis),
        Pushed::Rejected(ErrorCode::NotAllowed)
    );
}
