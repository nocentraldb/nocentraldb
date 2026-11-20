//! `SealChecks` keeps every key that the envelopes to its member unwrap to
//! for a generation, and tries each when it opens a sealed change
//! (Carsten, 2026-11-20, NOC-389). A key envelope that holds a wrong key
//! therefore no longer decides, by arriving last, whether a member reads a
//! generation: the verdict is a function of the set of changes it holds
//! (principle 2; RFC 0008, "What the fold may read").
//!
//! The key a member writes under, [`SealChecks::space_key`], is the one the
//! generation's own control change confirms when the envelopes disagree:
//! the genesis's sealed `model.id` for the first generation, the key chain
//! entry for a later one. A wrong envelope is a local report, with the
//! author whose signature covers it.

mod support;

#[path = "support/member.rs"]
mod member;
#[path = "support/space.rs"]
mod space;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeSet;

use ncdb_core::change::{Change, Payload};
use ncdb_core::checks::{SealChecks, WrongEnvelope};
use ncdb_core::clock::Hlc;
use ncdb_core::envelope;
use ncdb_core::keys::SpaceKey;
use ncdb_core::log::{Log, RejectReason, State};
use ncdb_core::membership::{self, Entry, KeyChainEntry, PreviousKey, Role};
use ncdb_core::seal;

use member::{Recorder, set_title};
use space::{Space, Who, id, key, space_key};

const ALICE: Who = 1;
const BOB: Who = 2;
const CAROL: Who = 3;

fn wrong_key() -> SpaceKey {
    SpaceKey::from_bytes([0xee; 32])
}

fn second_key() -> SpaceKey {
    SpaceKey::from_bytes([0x42; 32])
}

/// Whether `a` is `b`, told without reading either key: a change sealed
/// under `a` opens under `b`.
fn same_key(space: &Space, generation: u64, a: &SpaceKey, b: &SpaceKey) -> bool {
    let payload = Payload::new(Hlc(1 << 16), BTreeSet::new(), vec![set_title(9)]);
    let change = seal::seal(&key(CAROL), a, space.space, 99, generation, &payload);
    seal::open(&change, &key(CAROL).public(), b).is_ok()
}

/// Carol's copy after receiving `order`: its log and its checks.
fn receive(space: &Space, order: &[&Change]) -> (Log, SealChecks) {
    let mut log = Log::new(space.space);
    let mut checks = SealChecks::new(space.space, key(CAROL));
    let mut fold = Recorder::default();
    for change in order {
        log.receive(change.bytes(), &mut checks, &mut fold)
            .expect("every change is appended");
    }
    (log, checks)
}

/// `author`'s envelope to Carol for `generation` that wraps `wrapped`.
fn envelope_to_carol(
    space: &mut Space,
    author: Who,
    generation: u64,
    wrapped: &SpaceKey,
    ephemeral: u8,
    deps: &[&Change],
) -> Change {
    let sealed = envelope::seal_with_ephemeral(
        wrapped,
        space.space,
        generation,
        &key(CAROL).public(),
        [ephemeral; 32],
    )
    .expect("Carol's key takes an envelope");
    space.write(author, Entry::KeyEnvelope(sealed), deps)
}

/// Alice's space with Carol a reader, Alice's right envelope to Carol for
/// the first generation, her wrong one, and a sealed change of hers.
struct FirstGeneration {
    space: Space,
    before: Vec<Change>,
    right: Change,
    wrong: Change,
    sealed: Change,
}

fn first_generation() -> FirstGeneration {
    let mut space = Space::new(ALICE);
    let genesis = space.genesis.clone();
    let carol = space.add(ALICE, CAROL, Role::Reader, &[&genesis]);
    let right = space.envelope(ALICE, CAROL, &[&carol]);
    let first = space.first_generation;
    let wrong = envelope_to_carol(&mut space, ALICE, first, &wrong_key(), 0x77, &[&carol]);
    let sealed = space.sealed(ALICE, vec![set_title(1)]);
    FirstGeneration {
        space,
        before: vec![genesis, carol],
        right,
        wrong,
        sealed,
    }
}

impl FirstGeneration {
    /// The orders of the two envelopes and the sealed change after the
    /// genesis and Carol's add.
    fn orders(&self) -> Vec<Vec<&Change>> {
        let tails: [[&Change; 3]; 4] = [
            [&self.right, &self.wrong, &self.sealed],
            [&self.wrong, &self.right, &self.sealed],
            [&self.sealed, &self.right, &self.wrong],
            [&self.sealed, &self.wrong, &self.right],
        ];
        tails
            .iter()
            .map(|tail| self.before.iter().chain(tail.iter().copied()).collect())
            .collect()
    }
}

#[test]
fn a_sealed_change_folds_in_every_order_when_an_envelope_holds_a_wrong_key() {
    let f = first_generation();
    for (n, order) in f.orders().iter().enumerate() {
        let (log, _) = receive(&f.space, order);
        assert_eq!(
            log.state(f.sealed.id()),
            Some(&State::Folded { parked: vec![] }),
            "order {n}"
        );
    }
}

#[test]
fn the_key_a_member_writes_under_is_the_one_the_genesis_confirms() {
    let f = first_generation();
    for (n, order) in f.orders().iter().enumerate() {
        let (_, checks) = receive(&f.space, order);
        let key = checks
            .space_key(f.space.first_generation)
            .unwrap_or_else(|| panic!("order {n}: a key to write under"));
        let first = f.space.first_generation;
        assert!(same_key(&f.space, first, key, &space_key()), "order {n}");
        assert_eq!(checks.model_id(), Some(space::MODEL), "order {n}");
    }
}

#[test]
fn a_wrong_key_envelope_is_reported_locally_with_its_author() {
    let f = first_generation();
    let report = WrongEnvelope {
        change: f.wrong.id(),
        author: id(ALICE),
        generation: f.space.first_generation,
    };
    for (n, order) in f.orders().iter().enumerate() {
        let (_, checks) = receive(&f.space, order);
        assert_eq!(checks.wrong_envelopes(), vec![report.clone()], "order {n}");
    }
}

#[test]
fn a_lone_wrong_key_envelope_is_reported_and_reads_nothing() {
    let f = first_generation();
    let order: Vec<&Change> = f.before.iter().chain([&f.wrong, &f.sealed]).collect();
    let (log, checks) = receive(&f.space, &order);

    assert_eq!(
        log.state(f.sealed.id()),
        Some(&State::Rejected(RejectReason::Undecryptable))
    );
    assert_eq!(checks.wrong_envelopes().len(), 1);
}

/// Alice's space rotated by Alice to a second generation, whose key chain
/// entry encrypts the first generation's key. Carol is added after it,
/// with a right and a wrong envelope for the second generation only. Bob
/// sealed a change under the first generation before the rotation.
struct SecondGeneration {
    space: Space,
    before: Vec<Change>,
    rotation: Change,
    right: Change,
    wrong: Change,
    by_bob: Change,
    second: u64,
}

fn second_generation() -> SecondGeneration {
    let mut space = Space::new(ALICE);
    let genesis = space.genesis.clone();
    let add_bob = space.add(ALICE, BOB, Role::Writer, &[&genesis]);
    let to_bob = space.envelope(ALICE, BOB, &[&add_bob]);
    let by_bob = space.sealed(BOB, vec![set_title(1)]);
    let first = space.first_generation;
    let second = membership::generation_id(space.space, id(ALICE), &[first]);
    let chain = Entry::KeyChain(KeyChainEntry {
        generation: second,
        previous: vec![PreviousKey {
            generation: first,
            key: membership::seal_previous_key(
                &second_key(),
                space.space,
                second,
                first,
                &space_key(),
            ),
        }],
        answers: BTreeSet::new(),
    });
    let rotation = space.write(ALICE, chain, &[&to_bob]);
    let add_carol = space.add_at(ALICE, CAROL, Role::Reader, second, &[&rotation]);
    let right = envelope_to_carol(
        &mut space,
        ALICE,
        second,
        &second_key(),
        0x3c,
        &[&add_carol],
    );
    let wrong = envelope_to_carol(&mut space, ALICE, second, &wrong_key(), 0x77, &[&add_carol]);
    SecondGeneration {
        space,
        before: vec![genesis, add_bob, to_bob, add_carol],
        rotation,
        right,
        wrong,
        by_bob,
        second,
    }
}

#[test]
fn the_key_chain_opens_the_first_generation_in_every_order_of_a_wrong_envelope() {
    let s = second_generation();
    let tails: [[&Change; 4]; 4] = [
        [&s.rotation, &s.right, &s.wrong, &s.by_bob],
        [&s.rotation, &s.wrong, &s.right, &s.by_bob],
        [&s.by_bob, &s.right, &s.wrong, &s.rotation],
        [&s.wrong, &s.by_bob, &s.rotation, &s.right],
    ];
    for (n, tail) in tails.iter().enumerate() {
        let order: Vec<&Change> = s.before.iter().chain(tail.iter().copied()).collect();
        let (log, checks) = receive(&s.space, &order);

        assert_eq!(
            log.state(s.by_bob.id()),
            Some(&State::Folded { parked: vec![] }),
            "order {n}"
        );
        let key = checks.space_key(s.second).expect("the confirmed key");
        assert!(
            same_key(&s.space, s.second, key, &second_key()),
            "order {n}"
        );
        assert_eq!(checks.wrong_envelopes().len(), 1, "order {n}");
    }
}

#[test]
fn two_envelope_keys_without_their_key_chain_entry_give_no_key_to_write_under() {
    let s = second_generation();
    let order: Vec<&Change> = s.before.iter().chain([&s.right, &s.wrong]).collect();
    let (_, checks) = receive(&s.space, &order);

    assert!(checks.space_key(s.second).is_none());
    assert!(checks.wrong_envelopes().is_empty());
}
