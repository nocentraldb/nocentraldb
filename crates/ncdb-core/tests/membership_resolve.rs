//! `resolve` of RFC 0008 ("Folding the membership state"): the genesis,
//! adds and rights, what the fold keeps but does not fold, and the
//! certificates it checks. Revocations, cutoffs and concurrent changes are
//! in membership_revocation.rs.

#[path = "support/space.rs"]
mod space;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::change::{Change, Header};
use ncdb_core::membership::{
    self, Certificate, ControlBody, Entry, MemberKind, Rejection, Role, Verdict, resolve,
};
use ncdb_core::seal;

use space::{Space, account, id, key};

const ALICE: u8 = 1;
const BOB: u8 = 2;
const CAROL: u8 = 3;
const DAN: u8 = 4;

#[test]
fn the_genesis_makes_its_creator_the_only_admin() {
    let s = Space::new(ALICE);
    let state = resolve(s.space, [&s.genesis]);
    assert_eq!(state.genesis(), Some(s.genesis.id()));
    assert_eq!(state.verdict(s.genesis.id()), Some(&Verdict::Accepted));
    assert_eq!(state.first_generation(), Some(s.first_generation));
    assert_eq!(
        state.members().collect::<Vec<_>>(),
        [(id(ALICE), Role::Admin)]
    );
    assert_eq!(state.seniority(), [id(ALICE)]);
}

#[test]
fn an_admin_adds_members_with_their_roles() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let bob = s.add(ALICE, BOB, Role::Writer, &[&g]);
    let carol = s.add(ALICE, CAROL, Role::Reader, &[&bob]);
    let state = resolve(s.space, [&g, &bob, &carol]);
    assert_eq!(state.verdict(bob.id()), Some(&Verdict::Accepted));
    assert_eq!(state.verdict(carol.id()), Some(&Verdict::Accepted));
    assert_eq!(state.role(id(BOB)), Some(Role::Writer));
    assert_eq!(state.role(id(CAROL)), Some(Role::Reader));
    let bob_state = state.member(id(BOB)).unwrap();
    assert_eq!(bob_state.key, key(BOB).public());
    assert_eq!(bob_state.grants.len(), 1);
    assert_eq!(bob_state.grants[0].change, bob.id());
    assert_eq!(
        bob_state.grants[0].from_generation,
        Some(s.first_generation)
    );
}

#[test]
fn only_an_admin_adds_and_never_a_member_twice() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let bob = s.add(ALICE, BOB, Role::Writer, &[&g]);
    let by_writer = s.add(BOB, CAROL, Role::Reader, &[&bob]);
    let again = s.add(ALICE, BOB, Role::Admin, &[&bob]);
    let state = resolve(s.space, [&g, &bob, &by_writer, &again]);
    let not_valid = Some(&Verdict::Rejected(Rejection::NotValidWhenWritten));
    assert_eq!(state.verdict(by_writer.id()), not_valid);
    assert_eq!(state.verdict(again.id()), not_valid);
    assert_eq!(state.role(id(CAROL)), None);
    assert_eq!(state.role(id(BOB)), Some(Role::Writer));
}

#[test]
fn any_member_leaves_or_lowers_its_own_role_but_raises_nobody() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let bob = s.add(ALICE, BOB, Role::Writer, &[&g]);
    let carol = s.add(ALICE, CAROL, Role::Writer, &[&bob]);
    let lower = s.set_role(BOB, BOB, Role::Reader, 0, &[&carol]);
    let leave = s.remove(CAROL, CAROL, 0, &[&carol]);
    let raise = s.set_role(BOB, BOB, Role::Admin, 0, &[&lower]);
    let state = resolve(s.space, [&g, &bob, &carol, &lower, &leave, &raise]);
    assert_eq!(state.verdict(lower.id()), Some(&Verdict::Accepted));
    assert_eq!(state.verdict(leave.id()), Some(&Verdict::Accepted));
    assert_eq!(
        state.verdict(raise.id()),
        Some(&Verdict::Rejected(Rejection::NotValidWhenWritten))
    );
    assert_eq!(state.role(id(BOB)), Some(Role::Reader));
    assert_eq!(state.role(id(CAROL)), None);
    assert_eq!(state.members().count(), 2, "Alice and Bob");
}

#[test]
fn other_entry_types_need_the_right_their_type_names() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let bob = s.add(ALICE, BOB, Role::Reader, &[&g]);
    let ack = s.other(BOB, membership::ACKNOWLEDGEMENT, &[&bob]);
    let person_key = s.other(BOB, membership::PERSON_KEY, &[&bob]);
    let chain = s.other(ALICE, membership::KEY_CHAIN, &[&bob]);
    let state = resolve(s.space, [&g, &bob, &ack, &person_key, &chain]);
    assert_eq!(state.verdict(ack.id()), Some(&Verdict::Accepted));
    assert_eq!(
        state.verdict(person_key.id()),
        Some(&Verdict::Rejected(Rejection::NotValidWhenWritten))
    );
    assert_eq!(state.verdict(chain.id()), Some(&Verdict::Accepted));
}

#[test]
fn without_a_genesis_nothing_folds() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let bob = s.add(ALICE, BOB, Role::Writer, &[&g]);
    let ack = s.other(BOB, membership::ACKNOWLEDGEMENT, &[&bob]);
    let state = resolve(s.space, [&bob, &ack]);
    assert_eq!(state.genesis(), None);
    // Only the genesis names Alice's keys; Bob's come with his add.
    let unknown = Some(&Verdict::Rejected(Rejection::UnknownAuthor));
    assert_eq!(state.verdict(bob.id()), unknown);
    let no_genesis = Some(&Verdict::Rejected(Rejection::NoGenesis));
    assert_eq!(state.verdict(ack.id()), no_genesis);
    assert_eq!(state.members().count(), 0);
    assert!(state.gaps().contains(&g.id()));
}

#[test]
fn the_lower_of_two_geneses_is_the_genesis_in_any_order() {
    let a = space::genesis(ALICE, [7; 16], [1; 16]).0;
    let b = space::genesis(ALICE, [7; 16], [2; 16]).0;
    let (low, high) = if a.id() < b.id() { (a, b) } else { (b, a) };
    let space = low.header().space;
    for set in [[&low, &high], [&high, &low]] {
        let state = resolve(space, set);
        assert_eq!(state.genesis(), Some(low.id()));
        assert_eq!(state.verdict(low.id()), Some(&Verdict::Accepted));
        assert_eq!(
            state.verdict(high.id()),
            Some(&Verdict::Rejected(Rejection::EquivocatedGenesis))
        );
    }
    // A member that holds only the higher one takes it.
    assert_eq!(resolve(space, [&high]).genesis(), Some(high.id()));
}

#[test]
fn nobody_else_writes_a_genesis_for_a_space() {
    let s = Space::new(ALICE);
    // Bob's genesis, with Alice's nonce, derives another space id; placed
    // into Alice's space, it is not a valid genesis there.
    let (bob_genesis, bob_space, _) = space::genesis(BOB, [7; 16], [1; 16]);
    assert_ne!(bob_space, s.space);
    let mut header = bob_genesis.header().clone();
    header.space = s.space;
    let forged = seal::sign(&key(BOB), header);
    let state = resolve(s.space, [&forged, &s.genesis]);
    assert_eq!(state.genesis(), Some(s.genesis.id()));
    assert_eq!(
        state.verdict(forged.id()),
        Some(&Verdict::Rejected(Rejection::InvalidGenesis))
    );
}

#[test]
fn a_genesis_needs_sequence_one_and_its_derived_generation() {
    let s = Space::new(ALICE);
    let body = ControlBody::decode(&s.genesis.header().payload).unwrap();
    let late = membership::write(&key(ALICE), s.space, 2, s.first_generation, &body);
    let Entry::Genesis(mut genesis) = body.entry.clone() else {
        unreachable!()
    };
    genesis.first_generation ^= 1;
    let chosen = ControlBody::new(Default::default(), Entry::Genesis(genesis));
    let chosen = membership::write(&key(ALICE), s.space, 1, s.first_generation, &chosen);
    for change in [&late, &chosen] {
        let state = resolve(s.space, [change]);
        assert_eq!(state.genesis(), None);
        assert_eq!(
            state.verdict(change.id()),
            Some(&Verdict::Rejected(Rejection::InvalidGenesis))
        );
    }
}

#[test]
fn a_change_keeps_its_place_but_folds_nothing_when_not_understood() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let newer = s.other(ALICE, 9, &[&g]);
    let state = resolve(s.space, [&g, &newer]);
    assert_eq!(state.verdict(newer.id()), Some(&Verdict::Parked));
}

#[test]
fn malformed_unsigned_and_unknown_authors_are_rejected() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let bob = s.add(ALICE, BOB, Role::Writer, &[&g]);
    // A membership entry without dependencies.
    let orphan = Header::new(
        s.space,
        id(ALICE),
        9,
        s.first_generation,
        ncdb_core::change::Kind::CONTROL,
        vec![0xa1, 0x01, 0x01],
        seal::SUITE,
    );
    let malformed = seal::sign(&key(ALICE), orphan);
    let mut forged_header = bob.header().clone();
    forged_header.sequence = 99;
    let forged = Change::new(forged_header, bob.signature().to_vec());
    let stranger = s.other(DAN, membership::ACKNOWLEDGEMENT, &[&g]);
    let state = resolve(s.space, [&g, &bob, &malformed, &forged, &stranger]);
    assert_eq!(
        state.verdict(malformed.id()),
        Some(&Verdict::Rejected(Rejection::Malformed))
    );
    assert_eq!(
        state.verdict(forged.id()),
        Some(&Verdict::Rejected(Rejection::BadSignature))
    );
    assert_eq!(
        state.verdict(stranger.id()),
        Some(&Verdict::Rejected(Rejection::UnknownAuthor))
    );
    assert_eq!(state.role(id(BOB)), Some(Role::Writer));
}

#[test]
fn changes_of_other_spaces_and_sealed_changes_are_not_judged() {
    let s = Space::new(ALICE);
    let other = Space::new(BOB);
    let state = resolve(s.space, [&s.genesis, &other.genesis]);
    assert_eq!(state.verdict(other.genesis.id()), None);
}

#[test]
fn a_certificate_that_does_not_verify_makes_the_add_invalid() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let mut certificate = Certificate::sign(&account(), id(BOB), MemberKind::Device);
    certificate.signature[0] ^= 1;
    let bad = s.add_certified(ALICE, BOB, Role::Writer, certificate, &[&g]);
    let other = Certificate::sign(&account(), id(DAN), MemberKind::Device);
    let wrong_member = s.add_certified(ALICE, CAROL, Role::Writer, other, &[&g]);
    let state = resolve(s.space, [&g, &bad, &wrong_member]);
    let not_valid = Some(&Verdict::Rejected(Rejection::NotValidWhenWritten));
    assert_eq!(state.verdict(bad.id()), not_valid);
    assert_eq!(state.verdict(wrong_member.id()), not_valid);
}

#[test]
fn a_sub_agent_is_added_on_its_parents_certificate() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    // Alice is an agent; her sub-agent Bob carries her account key.
    let account_key = account().verifying_key().to_bytes();
    let by_parent = Certificate::sign_sub_agent(&key(ALICE), account_key, id(BOB));
    let sub_agent = s.add_certified(ALICE, BOB, Role::Writer, by_parent, &[&g]);
    // Carol's certificate names Alice as parent but Dan signed it.
    let by_stranger = Certificate::sign_sub_agent(&key(DAN), account_key, id(CAROL));
    let mut forged = by_stranger.clone();
    forged.parent = Some(id(ALICE));
    let forged = s.add_certified(ALICE, CAROL, Role::Writer, forged, &[&g]);
    // Dan's parent Bob is not a member yet where Dan's add was written.
    let by_bob = Certificate::sign_sub_agent(&key(BOB), account_key, id(DAN));
    let early = s.add_certified(ALICE, DAN, Role::Writer, by_bob, &[&g]);
    let state = resolve(s.space, [&g, &sub_agent, &forged, &early]);
    assert_eq!(state.verdict(sub_agent.id()), Some(&Verdict::Accepted));
    let not_valid = Some(&Verdict::Rejected(Rejection::NotValidWhenWritten));
    assert_eq!(state.verdict(forged.id()), not_valid);
    assert_eq!(state.verdict(early.id()), not_valid);
}

#[test]
fn a_missing_dependency_is_a_gap_and_the_rest_still_counts() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let bob = s.add(ALICE, BOB, Role::Writer, &[&g]);
    let carol = s.add(ALICE, CAROL, Role::Reader, &[&bob]);
    let dan = s.add(ALICE, DAN, Role::Reader, &[&g, &bob]);
    let state = resolve(s.space, [&g, &carol, &dan]);
    assert!(state.gaps().contains(&bob.id()));
    // Carol's add reaches the genesis only through Bob's, so its held
    // causal past is empty. Dan's names the genesis too.
    let not_valid = Some(&Verdict::Rejected(Rejection::NotValidWhenWritten));
    assert_eq!(state.verdict(carol.id()), not_valid);
    assert_eq!(state.verdict(dan.id()), Some(&Verdict::Accepted));
    assert_eq!(state.role(id(BOB)), None);
}
