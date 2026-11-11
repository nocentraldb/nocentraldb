//! `SealChecks` learns members and space keys only from control changes
//! (NOC-78): it folds the space's genesis and membership entries through
//! `membership::resolve`, decides step 4 for a sealed change with RFC
//! 0008's membership verdict ("Sealed changes"), and opens the key
//! envelopes addressed to its member with `envelope::open`. A change
//! rejected for an unknown author, a missing membership or a missing key is
//! folded once the control change it lacked arrives (RFC 0002, "Rejected
//! changes").

mod support;

#[path = "support/member.rs"]
mod member;
#[path = "support/space.rs"]
mod space;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::change::Change;
use ncdb_core::checks::SealChecks;
use ncdb_core::log::{Log, RejectReason, State};
use ncdb_core::membership::Role;

use member::{Recorder, set_title};
use space::{Space, id, key};

const ALICE: u8 = 1;
const BOB: u8 = 2;
const CAROL: u8 = 3;

/// A member's copy of one space: its log, its checks and its fold.
struct Copy {
    log: Log,
    checks: SealChecks,
    fold: Recorder,
}

impl Copy {
    fn of(space: &Space, who: u8) -> Copy {
        Copy {
            log: Log::new(space.space),
            checks: SealChecks::new(space.space, key(who)),
            fold: Recorder::default(),
        }
    }

    fn receive(&mut self, changes: &[&Change]) {
        for change in changes {
            self.log
                .receive(change.bytes(), &mut self.checks, &mut self.fold)
                .expect("a change of the space is appended");
        }
    }

    fn state(&self, change: &Change) -> State {
        self.log.state(change.id()).cloned().expect("a held change")
    }
}

fn folded() -> State {
    State::Folded { parked: vec![] }
}

fn rejected(reason: RejectReason) -> State {
    State::Rejected(reason)
}

/// Alice creates the space, adds Bob as a writer and Carol as a reader,
/// and wraps the space key to both.
struct Founded {
    space: Space,
    genesis: Change,
    bob: Change,
    carol: Change,
    to_bob: Change,
    to_carol: Change,
}

fn founded() -> Founded {
    let mut space = Space::new(ALICE);
    let genesis = space.genesis.clone();
    let bob = space.add(ALICE, BOB, Role::Writer, &[&genesis]);
    let carol = space.add(ALICE, CAROL, Role::Reader, &[&bob]);
    let to_bob = space.envelope(ALICE, BOB, &[&carol]);
    let to_carol = space.envelope(ALICE, CAROL, &[&to_bob]);
    Founded {
        space,
        genesis,
        bob,
        carol,
        to_bob,
        to_carol,
    }
}

impl Founded {
    fn controls(&self) -> [&Change; 5] {
        [
            &self.genesis,
            &self.bob,
            &self.carol,
            &self.to_bob,
            &self.to_carol,
        ]
    }
}

#[test]
fn the_genesis_and_the_membership_entries_make_the_checks_membership() {
    let f = founded();
    let mut carol = Copy::of(&f.space, CAROL);
    carol.receive(&f.controls());
    let resolved = carol.checks.resolved();
    assert_eq!(resolved.role(id(ALICE)), Some(Role::Admin));
    assert_eq!(resolved.role(id(BOB)), Some(Role::Writer));
    assert_eq!(resolved.role(id(CAROL)), Some(Role::Reader));
    for control in f.controls() {
        assert_eq!(carol.state(control), folded());
    }
}

#[test]
fn a_change_by_a_writer_is_folded_with_the_key_of_the_envelope_to_the_member() {
    let mut f = founded();
    let by_bob = f.space.sealed(BOB, vec![set_title(1)]);
    let by_alice = f.space.sealed(ALICE, vec![set_title(2)]);
    let mut carol = Copy::of(&f.space, CAROL);
    carol.receive(&f.controls());
    carol.receive(&[&by_bob, &by_alice]);
    assert_eq!(carol.state(&by_bob), folded());
    assert_eq!(carol.state(&by_alice), folded());
    assert_eq!(carol.fold.folded.len(), 2);
}

#[test]
fn an_envelope_to_another_member_gives_no_key() {
    let mut f = founded();
    let by_bob = f.space.sealed(BOB, vec![set_title(1)]);
    let mut carol = Copy::of(&f.space, CAROL);
    carol.receive(&[&f.genesis, &f.bob, &f.carol, &f.to_bob, &by_bob]);
    assert_eq!(carol.state(&by_bob), rejected(RejectReason::NoKey));
}

#[test]
fn a_change_by_a_reader_or_a_stranger_is_not_a_member() {
    let mut f = founded();
    let by_carol = f.space.sealed(CAROL, vec![set_title(1)]);
    let mut bob = Copy::of(&f.space, BOB);
    bob.receive(&f.controls());
    bob.receive(&[&by_carol]);
    assert_eq!(bob.state(&by_carol), rejected(RejectReason::NotMember));
}

#[test]
fn a_change_under_a_generation_its_author_was_not_granted_is_not_a_member() {
    let mut f = founded();
    let other = f.space.first_generation ^ 1;
    let by_bob = f.space.sealed_under(BOB, other, vec![set_title(1)]);
    let mut carol = Copy::of(&f.space, CAROL);
    carol.receive(&f.controls());
    carol.receive(&[&by_bob]);
    assert_eq!(carol.state(&by_bob), rejected(RejectReason::NotMember));
}

#[test]
fn a_removed_writer_keeps_its_changes_up_to_the_cutoff() {
    let mut f = founded();
    let inside = f.space.sealed(BOB, vec![set_title(1)]);
    let cutoff = f.space.sequence(BOB);
    let excluded = f.space.sealed(BOB, vec![set_title(2)]);
    let remove = f.space.remove(ALICE, BOB, cutoff, &[&f.to_carol]);
    let mut carol = Copy::of(&f.space, CAROL);
    carol.receive(&f.controls());
    carol.receive(&[&remove, &inside, &excluded]);
    assert_eq!(carol.state(&inside), folded());
    assert_eq!(carol.state(&excluded), rejected(RejectReason::NotMember));
}

#[test]
fn a_change_that_arrives_before_its_control_changes_is_folded_when_they_arrive() {
    let mut f = founded();
    let by_bob = f.space.sealed(BOB, vec![set_title(1)]);
    let mut carol = Copy::of(&f.space, CAROL);
    carol.receive(&[&by_bob]);
    assert_eq!(carol.state(&by_bob), rejected(RejectReason::UnknownAuthor));
    carol.receive(&[&f.genesis, &f.carol, &f.to_carol]);
    // Bob's add has not arrived: his keys are unknown.
    assert_eq!(carol.state(&by_bob), rejected(RejectReason::UnknownAuthor));
    // The envelope to Carol waits for nothing but the genesis.
    assert_eq!(carol.state(&f.to_carol), folded());
    carol.receive(&[&f.bob]);
    assert_eq!(carol.state(&by_bob), folded());
}

#[test]
fn a_change_without_its_envelope_is_folded_when_the_envelope_arrives() {
    let mut f = founded();
    let by_bob = f.space.sealed(BOB, vec![set_title(1)]);
    let mut carol = Copy::of(&f.space, CAROL);
    carol.receive(&[&f.genesis, &f.bob, &f.carol, &by_bob]);
    assert_eq!(carol.state(&by_bob), rejected(RejectReason::NoKey));
    carol.receive(&[&f.to_carol]);
    assert_eq!(carol.state(&by_bob), folded());
}

#[test]
fn a_control_change_whose_author_is_unknown_waits_for_the_entry_that_names_it() {
    let mut f = founded();
    // A control change by Bob, whose add Carol does not hold yet.
    let leave = f.space.remove(BOB, BOB, 0, &[&f.to_carol]);
    let mut carol = Copy::of(&f.space, CAROL);
    carol.receive(&[&f.genesis, &leave]);
    assert_eq!(carol.state(&leave), rejected(RejectReason::UnknownAuthor));
    carol.receive(&[&f.bob]);
    assert_eq!(carol.state(&leave), folded());
}

#[test]
fn a_change_under_a_rotated_generation_is_folded_with_its_envelope() {
    let mut f = founded();
    let first = f.space.first_generation;
    let (rotation, two) = f.space.rotate(ALICE, &[first], &[&f.to_carol]);
    let to_carol = f.space.envelope_at(ALICE, CAROL, two, &[&rotation]);
    let by_bob = f.space.sealed_under(BOB, two, vec![set_title(1)]);
    let mut carol = Copy::of(&f.space, CAROL);
    carol.receive(&f.controls());
    carol.receive(&[&rotation, &to_carol, &by_bob]);
    assert_eq!(carol.state(&by_bob), folded());
}
