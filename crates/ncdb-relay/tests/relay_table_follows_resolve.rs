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
use ncdb_relay::Pushed;
use ncdb_relay::conformance::block_on;
use ncdb_relay::record;
use ncdb_relay::store::RelayStore;
use support::space::{Space, id};
use support::{push, relay};

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
