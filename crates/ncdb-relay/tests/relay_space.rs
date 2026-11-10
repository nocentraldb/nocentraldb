//! RFC 0007, "What the relay checks": a space starts with its genesis, the
//! frame is well-formed, canonical, not too large and of a known suite, and
//! a change the log holds is a duplicate.

mod support;

use ncdb_core::change::{Change, Header, Kind};
use ncdb_core::membership::{Certificate, MemberKind, Role};
use ncdb_core::seal;
use ncdb_relay::conformance::block_on;
use ncdb_relay::store::RelayStore;
use ncdb_relay::{ErrorCode, Pushed, Relay};
use support::space::{self, Space, id, key};
use support::{TempDir, config, push, push_one, relay};

#[test]
fn a_genesis_its_creator_pushes_creates_the_space() {
    let (relay, _dir) = relay();
    let space = Space::new(0);
    assert_eq!(push_one(&relay, 0, &space.genesis), Pushed::Accepted(1));
    let store = relay.store();
    assert_eq!(block_on(store.head(space.space)).unwrap(), Some(1));
    let row = block_on(store.member(space.space, id(0))).unwrap().unwrap();
    assert!(row.current);
    assert_eq!(block_on(store.members_of(id(0))).unwrap(), [space.space]);
}

#[test]
fn a_genesis_someone_else_pushes_is_refused() {
    let (relay, _dir) = relay();
    let space = Space::new(0);
    let pushed = push_one(&relay, 1, &space.genesis);
    assert_eq!(pushed, Pushed::Rejected(ErrorCode::NotMember));
    assert_eq!(block_on(relay.store().head(space.space)).unwrap(), None);
}

#[test]
fn a_genesis_of_an_account_the_relay_does_not_allow_is_refused() {
    let dir = TempDir::new();
    let mut config = config();
    config.allowed_accounts.clear();
    let relay = Relay::new(dir.store(), config);
    let space = Space::new(0);
    let pushed = push_one(&relay, 0, &space.genesis);
    assert_eq!(pushed, Pushed::Rejected(ErrorCode::NotAllowed));
}

#[test]
fn a_space_needs_its_genesis_first() {
    let (relay, _dir) = relay();
    let mut space = Space::new(0);
    let genesis = space.genesis.clone();
    let add = space.add(0, 1, Role::Writer, &[&genesis]);
    let pushed = push_one(&relay, 0, &add);
    assert_eq!(pushed, Pushed::Rejected(ErrorCode::UnknownSpace));
}

#[test]
fn a_second_genesis_of_a_held_space_is_refused() {
    let (relay, _dir) = relay();
    let space = Space::new(0);
    push_one(&relay, 0, &space.genesis);
    let (other, other_space, _) = space::genesis(0, [7; 16], [2; 16]);
    assert_eq!(other_space, space.space);
    assert_ne!(other.id(), space.genesis.id());
    let pushed = push_one(&relay, 0, &other);
    assert_eq!(pushed, Pushed::Rejected(ErrorCode::SpaceExists));
}

#[test]
fn a_change_the_log_holds_is_a_duplicate_with_its_position() {
    let (relay, _dir) = relay();
    let mut space = Space::new(0);
    let genesis = space.genesis.clone();
    let add = space.add(0, 1, Role::Writer, &[&genesis]);
    let pushed = push(&relay, 0, space.space, &[&genesis, &add, &genesis]);
    assert_eq!(
        pushed,
        [
            Pushed::Accepted(1),
            Pushed::Accepted(2),
            Pushed::Duplicate(1)
        ]
    );
    assert_eq!(push_one(&relay, 0, &add), Pushed::Duplicate(2));
    // A current member that forwards it learns its position too.
    assert_eq!(push_one(&relay, 1, &add), Pushed::Duplicate(2));
}

#[test]
fn a_frame_that_is_not_well_formed_or_not_canonical_is_malformed() {
    let (relay, _dir) = relay();
    let space = Space::new(0);
    push_one(&relay, 0, &space.genesis);
    let mut trailing = space.genesis.bytes().to_vec();
    trailing.push(0);
    // The frame version 1 as a two-byte integer: well-formed, not canonical.
    let mut long = vec![0x83, 0x18, 0x01];
    long.extend_from_slice(&space.genesis.bytes()[2..]);
    let frames = vec![vec![0xff], trailing, long, Vec::new()];
    let pushed = block_on(relay.push(id(0), space.space, frames));
    assert_eq!(pushed, [Pushed::Rejected(ErrorCode::Malformed); 4]);
}

#[test]
fn a_frame_of_another_space_is_malformed_in_this_push() {
    let (relay, _dir) = relay();
    let space = Space::new(0);
    let other = Space::with_nonce(0, [8; 16]);
    push_one(&relay, 0, &space.genesis);
    let pushed = push(&relay, 0, space.space, &[&other.genesis]);
    assert_eq!(pushed, [Pushed::Rejected(ErrorCode::Malformed)]);
    assert_eq!(block_on(relay.store().head(other.space)).unwrap(), None);
}

#[test]
fn a_frame_above_the_largest_change_is_too_large() {
    let dir = TempDir::new();
    let mut config = config();
    config.max_frame = 64;
    let relay = Relay::new(dir.store(), config);
    let space = Space::new(0);
    assert!(space.genesis.bytes().len() > 64);
    let pushed = push_one(&relay, 0, &space.genesis);
    assert_eq!(pushed, Pushed::Rejected(ErrorCode::TooLarge));
}

#[test]
fn the_largest_change_is_16_mib() {
    assert_eq!(ncdb_relay::MAX_FRAME, 16 * 1024 * 1024);
    assert_eq!(config().max_frame, ncdb_relay::MAX_FRAME);
}

#[test]
fn a_change_of_a_suite_the_relay_does_not_know_is_refused() {
    let (relay, _dir) = relay();
    let space = Space::new(0);
    push_one(&relay, 0, &space.genesis);
    let header = Header::new(
        space.space,
        id(0),
        2,
        space.first_generation,
        Kind::SEALED,
        vec![1, 2, 3],
        2,
    );
    let change = seal::sign(&key(0), header);
    let pushed = push_one(&relay, 0, &change);
    assert_eq!(pushed, Pushed::Rejected(ErrorCode::UnsupportedSuite));
}

#[test]
fn a_change_of_a_kind_the_relay_does_not_know_is_refused() {
    let (relay, _dir) = relay();
    let space = Space::new(0);
    push_one(&relay, 0, &space.genesis);
    let header = Header::new(
        space.space,
        id(0),
        2,
        space.first_generation,
        Kind(2),
        vec![1, 2, 3],
        seal::SUITE,
    );
    let change = seal::sign(&key(0), header);
    let pushed = push_one(&relay, 0, &change);
    assert_eq!(pushed, Pushed::Rejected(ErrorCode::UnsupportedSuite));
}

#[test]
fn a_genesis_with_a_bad_certificate_breaks_the_membership_rules() {
    let (relay, _dir) = relay();
    let creator = key(0);
    let nonce = [9; 16];
    let space_id = ncdb_core::membership::space_id(creator.id(), &nonce);
    let first_generation = ncdb_core::membership::generation_id(space_id, creator.id(), &[]);
    // The certificate names another member.
    let certificate = Certificate::sign(&space::account(), id(1), MemberKind::Agent);
    let genesis = ncdb_core::membership::Genesis {
        nonce,
        creator: creator.public(),
        certificate,
        first_generation,
        model_id: ncdb_core::membership::seal_model_id(
            &space::space_key(),
            space_id,
            creator.id(),
            [1; 16],
        ),
    };
    let body = ncdb_core::membership::ControlBody::new(
        Default::default(),
        ncdb_core::membership::Entry::Genesis(genesis),
    );
    let change: Change =
        ncdb_core::membership::write(&creator, space_id, 1, first_generation, &body);
    let pushed = push_one(&relay, 0, &change);
    assert_eq!(pushed, Pushed::Rejected(ErrorCode::MembershipRule));
}
