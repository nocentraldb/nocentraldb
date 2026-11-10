//! RFC 0007, "What the relay checks": the uploader is a current member, a
//! signature verifies with the author's key, a sealed change's author may
//! write it, and a control change obeys the membership rules, by RFC 0008's
//! `resolve` from the core.

mod support;

use ncdb_core::change::{Change, Kind};
use ncdb_core::membership::{self, Role, SHRED};
use ncdb_relay::conformance::block_on;
use ncdb_relay::store::RelayStore;
use ncdb_relay::{ErrorCode, Pushed};
use support::space::{Space, id};
use support::{push, push_one, relay};

/// The same header and a signature with one bit flipped.
fn badly_signed(change: &Change) -> Change {
    let mut signature = change.signature().to_vec();
    signature[0] ^= 1;
    Change::new(change.header().clone(), signature)
}

/// A space on the relay where 0 created it and added 1 as a writer and 2
/// as a reader.
fn held() -> (
    ncdb_relay::Relay<ncdb_relay::sqlite::SqliteStore>,
    support::TempDir,
    Space,
    Vec<Change>,
) {
    let (relay, dir) = relay();
    let mut space = Space::new(0);
    let genesis = space.genesis.clone();
    let add_1 = space.add(0, 1, Role::Writer, &[&genesis]);
    let add_2 = space.add(0, 2, Role::Reader, &[&add_1]);
    let pushed = push(&relay, 0, space.space, &[&genesis, &add_1, &add_2]);
    assert_eq!(
        pushed,
        [
            Pushed::Accepted(1),
            Pushed::Accepted(2),
            Pushed::Accepted(3)
        ]
    );
    (relay, dir, space, vec![genesis, add_1, add_2])
}

#[test]
fn an_accepted_membership_entry_writes_the_membership_table() {
    let (relay, _dir, space, _) = held();
    let store = relay.store();
    for who in 0..3 {
        let row = block_on(store.member(space.space, id(who))).unwrap();
        assert!(row.is_some_and(|row| row.current), "member {who}");
    }
    assert_eq!(block_on(store.members_of(id(2))).unwrap(), [space.space]);
}

#[test]
fn a_writer_s_sealed_change_is_accepted() {
    let (relay, _dir, mut space, _) = held();
    let change = space.sealed(1, Vec::new());
    assert_eq!(push_one(&relay, 1, &change), Pushed::Accepted(4));
}

#[test]
fn any_current_member_may_forward_another_member_s_change() {
    let (relay, _dir, mut space, _) = held();
    let change = space.sealed(1, Vec::new());
    assert_eq!(push_one(&relay, 2, &change), Pushed::Accepted(4));
}

#[test]
fn an_uploader_that_is_not_a_member_is_refused() {
    let (relay, _dir, mut space, _) = held();
    let change = space.sealed(1, Vec::new());
    assert_eq!(
        push_one(&relay, 5, &change),
        Pushed::Rejected(ErrorCode::NotMember)
    );
}

#[test]
fn a_sealed_change_with_a_bad_signature_is_refused() {
    let (relay, _dir, mut space, _) = held();
    let change = badly_signed(&space.sealed(1, Vec::new()));
    assert_eq!(
        push_one(&relay, 1, &change),
        Pushed::Rejected(ErrorCode::BadSignature)
    );
}

#[test]
fn a_sealed_change_of_a_reader_or_a_stranger_is_refused() {
    let (relay, _dir, mut space, _) = held();
    let reader = space.sealed(2, Vec::new());
    assert_eq!(
        push_one(&relay, 0, &reader),
        Pushed::Rejected(ErrorCode::AuthorNotMember)
    );
    let stranger = space.sealed(5, Vec::new());
    assert_eq!(
        push_one(&relay, 0, &stranger),
        Pushed::Rejected(ErrorCode::AuthorNotMember)
    );
}

#[test]
fn a_sealed_change_under_another_generation_is_refused() {
    let (relay, _dir, mut space, _) = held();
    let change = space.sealed_under(1, space.first_generation ^ 1, Vec::new());
    assert_eq!(
        push_one(&relay, 1, &change),
        Pushed::Rejected(ErrorCode::AuthorNotMember)
    );
}

#[test]
fn a_removed_author_s_changes_count_up_to_the_cutoff() {
    let (relay, _dir, mut space, held) = held();
    let early = space.sealed(1, Vec::new());
    let at_cutoff = space.sealed(1, Vec::new());
    let late = space.sealed(1, Vec::new());
    assert_eq!(at_cutoff.header().sequence, 2);
    let remove = space.remove(0, 1, 2, &[&held[2]]);
    assert_eq!(push_one(&relay, 0, &remove), Pushed::Accepted(4));
    // Changes it wrote before the removal arrive late, forwarded by 0.
    let pushed = push(&relay, 0, space.space, &[&late, &early, &at_cutoff]);
    assert_eq!(
        pushed,
        [
            Pushed::Rejected(ErrorCode::AuthorNotMember),
            Pushed::Accepted(5),
            Pushed::Accepted(6),
        ]
    );
    // It is no longer a member, so it can push nothing itself.
    let own = space.sealed(1, Vec::new());
    assert_eq!(
        push_one(&relay, 1, &own),
        Pushed::Rejected(ErrorCode::NotMember)
    );
    let row = block_on(relay.store().member(space.space, id(1))).unwrap();
    assert!(row.is_some_and(|row| !row.current));
    assert!(
        block_on(relay.store().members_of(id(1)))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn a_control_change_that_breaks_the_membership_rules_is_refused() {
    let (relay, _dir, mut space, held) = held();
    // A writer may not add a member.
    let add = space.add(1, 3, Role::Writer, &[&held[2]]);
    assert_eq!(
        push_one(&relay, 1, &add),
        Pushed::Rejected(ErrorCode::MembershipRule)
    );
    assert_eq!(
        block_on(relay.store().member(space.space, id(3))).unwrap(),
        None
    );
}

#[test]
fn a_control_change_with_a_bad_signature_is_refused() {
    let (relay, _dir, mut space, held) = held();
    let add = badly_signed(&space.add(0, 3, Role::Writer, &[&held[2]]));
    assert_eq!(
        push_one(&relay, 0, &add),
        Pushed::Rejected(ErrorCode::BadSignature)
    );
}

#[test]
fn a_control_change_of_an_author_nobody_added_is_refused() {
    let (relay, _dir, mut space, held) = held();
    let add = space.add(5, 3, Role::Writer, &[&held[2]]);
    assert_eq!(
        push_one(&relay, 0, &add),
        Pushed::Rejected(ErrorCode::AuthorNotMember)
    );
}

#[test]
fn a_control_body_that_is_not_one_is_malformed() {
    let (relay, _dir, space, _) = held();
    let header = ncdb_core::change::Header::new(
        space.space,
        id(0),
        9,
        space.first_generation,
        Kind::CONTROL,
        vec![0xa0],
        ncdb_core::seal::SUITE,
    );
    let change = ncdb_core::seal::sign(&support::space::key(0), header);
    assert_eq!(
        push_one(&relay, 0, &change),
        Pushed::Rejected(ErrorCode::Malformed)
    );
}

#[test]
fn a_control_body_the_relay_does_not_understand_waits_for_an_upgrade() {
    let (relay, _dir, mut space, held) = held();
    let future = space.other(0, 9, &[&held[2]]);
    assert_eq!(
        push_one(&relay, 0, &future),
        Pushed::Rejected(ErrorCode::UnsupportedSuite)
    );
}

#[test]
fn a_shred_entry_waits_until_the_relay_checks_its_sign_off() {
    let (relay, _dir, mut space, held) = held();
    let shred = space.other(0, SHRED, &[&held[2]]);
    assert_eq!(
        push_one(&relay, 0, &shred),
        Pushed::Rejected(ErrorCode::UnsupportedSuite)
    );
}

#[test]
fn the_relay_s_membership_is_the_cores_resolve_over_its_log() {
    let (relay, _dir, mut space, held) = held();
    let set_role = space.set_role(0, 1, Role::Reader, 0, &[&held[2]]);
    let envelope = space.envelope(0, 2, &[&set_role]);
    let pushed = push(&relay, 0, space.space, &[&set_role, &envelope]);
    assert_eq!(pushed, [Pushed::Accepted(4), Pushed::Accepted(5)]);
    let mut all = held.clone();
    all.extend([set_role, envelope]);
    let resolved = membership::resolve(space.space, &all);
    for who in 0..3 {
        let row = block_on(relay.store().member(space.space, id(who)))
            .unwrap()
            .unwrap();
        let row = ncdb_relay::record::decode(&row).unwrap();
        assert_eq!(Some(&row.state), resolved.member(id(who)), "member {who}");
    }
    // 1 is a reader now: its sealed changes above the cutoff are refused.
    let change = space.sealed(1, Vec::new());
    assert_eq!(
        push_one(&relay, 1, &change),
        Pushed::Rejected(ErrorCode::AuthorNotMember)
    );
}

#[test]
fn a_sealed_change_under_a_generation_that_descends_from_the_grant_is_accepted() {
    let (relay, _dir, mut space, held) = held();
    let first = space.first_generation;
    let (rotation, next) = space.rotate(0, &[first], &[&held[2]]);
    assert_eq!(push_one(&relay, 0, &rotation), Pushed::Accepted(4));
    let change = space.sealed_under(1, next, Vec::new());
    assert_eq!(push_one(&relay, 1, &change), Pushed::Accepted(5));
    // A generation the graph does not hold still counts for nobody.
    let unknown = space.sealed_under(1, next ^ 1, Vec::new());
    assert_eq!(
        push_one(&relay, 1, &unknown),
        Pushed::Rejected(ErrorCode::AuthorNotMember)
    );
    let mut all = held.clone();
    all.push(rotation);
    let resolved = membership::resolve(space.space, &all);
    assert!(resolved.admits(change.header()));
}
