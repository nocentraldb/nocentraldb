//! RFC 0008's M-C11, "The relay reaches the members' state" (NOC-382):
//! after every append, the relay's membership table equals `resolve` of its
//! own log (RFC 0008, "How the relay applies the rules": "Its membership
//! table is `resolve` of its log, recomputed with each append of a control
//! change").
//!
//! Alice creates the space and makes Bob an admin. Bob adds Xavier. Alice
//! removes Bob with a cutoff below Bob's add, so `resolve` excludes the add
//! and Xavier is no member at all: no row of the table may name him as a
//! current member.

mod support;

use ncdb_core::membership::{self, Role};
use ncdb_relay::conformance::block_on;
use ncdb_relay::record;
use ncdb_relay::store::RelayStore;
use ncdb_relay::{ErrorCode, Pushed};
use support::space::{Space, id};
use support::{push, push_one, relay};

const ALICE: u8 = 0;
const BOB: u8 = 1;
const XAVIER: u8 = 5;

#[test]
fn m_c11_a_member_whose_add_a_removal_excludes_is_no_current_member_on_the_relay() {
    let (relay, _dir) = relay();
    let mut s = Space::new(ALICE);
    let genesis = s.genesis.clone();
    let bob = s.add(ALICE, BOB, Role::Admin, &[&genesis]);
    let cutoff = s.sequence(BOB);
    let xavier = s.add(BOB, XAVIER, Role::Writer, &[&bob]);
    let remove_bob = s.remove(ALICE, BOB, cutoff, &[&bob]);
    let pushed = push(
        &relay,
        ALICE,
        s.space,
        &[&genesis, &bob, &xavier, &remove_bob],
    );
    assert!(pushed.iter().all(|p| matches!(p, Pushed::Accepted(_))));

    let resolved = membership::resolve(s.space, [&genesis, &bob, &xavier, &remove_bob]);
    assert_eq!(resolved.role(id(XAVIER)), None);
    assert!(
        !record::records(&resolved)
            .iter()
            .any(|row| row.member == id(XAVIER))
    );
    let row = block_on(relay.store().member(s.space, id(XAVIER))).unwrap();
    assert!(
        row.is_none_or(|row| !row.current),
        "the relay's table names Xavier as a current member; resolve of its log names him not at all"
    );
    assert!(
        block_on(relay.store().members_of(id(XAVIER)))
            .unwrap()
            .is_empty()
    );
}

/// A former member's row goes too: Bob adds Xavier, Xavier writes, Bob
/// removes Xavier with a cutoff that keeps that change. Alice then removes
/// Bob, concurrently, with a cutoff below Bob's add, so `resolve` excludes the add and the
/// removal and names Xavier not at all. The relay may no longer admit the
/// change Xavier's old row let in.
#[test]
fn a_former_member_whose_add_a_removal_excludes_has_no_late_changes_on_the_relay() {
    let (relay, _dir) = relay();
    let mut s = Space::new(ALICE);
    let genesis = s.genesis.clone();
    let bob = s.add(ALICE, BOB, Role::Admin, &[&genesis]);
    let cutoff = s.sequence(BOB);
    let xavier = s.add(BOB, XAVIER, Role::Writer, &[&bob]);
    let written = s.sealed(XAVIER, Vec::new());
    let remove_xavier = s.remove(BOB, XAVIER, s.sequence(XAVIER), &[&xavier]);
    let pushed = push(
        &relay,
        ALICE,
        s.space,
        &[&genesis, &bob, &xavier, &remove_xavier],
    );
    assert!(pushed.iter().all(|p| matches!(p, Pushed::Accepted(_))));
    let remove_bob = s.remove(ALICE, BOB, cutoff, &[&bob]);
    assert!(matches!(
        push_one(&relay, ALICE, &remove_bob),
        Pushed::Accepted(_)
    ));

    let resolved = membership::resolve(
        s.space,
        [&genesis, &bob, &xavier, &remove_xavier, &remove_bob],
    );
    assert!(resolved.member(id(XAVIER)).is_none());
    assert_eq!(
        push_one(&relay, ALICE, &written),
        Pushed::Rejected(ErrorCode::AuthorNotMember),
        "the relay admits a change of Xavier, whom resolve of its log names not at all"
    );
}
