//! `SealChecks` walks the key chain (RFC 0003, "Key-chain subkey"; RFC
//! 0008, "Key chain entry"): from every generation whose key it holds, it
//! opens the previous generations' keys that the generation's key chain
//! entry encrypts, back to the first generation. A member added after a
//! rotation, with an envelope for the new generation only, so reads the
//! changes sealed before it, in whichever order the envelope and the key
//! chain entry arrive. Only a key chain entry that the generation graph
//! holds, valid when written, gives keys, and of two that give different
//! keys for one generation, every copy keeps the same one, in whichever
//! order they arrive.

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
use ncdb_core::checks::SealChecks;
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

/// The key of the second generation; the first is [`space_key`].
fn second_key() -> SpaceKey {
    SpaceKey::from_bytes([0x42; 32])
}

/// Carol's copy of the space: its log, its checks and its fold.
struct Copy {
    log: Log,
    checks: SealChecks,
    fold: Recorder,
}

impl Copy {
    fn of(space: &Space) -> Copy {
        Copy {
            log: Log::new(space.space),
            checks: SealChecks::new(space.space, key(CAROL)),
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

/// Alice's space: Bob a writer who seals a change under the first
/// generation, then `rotator`'s rotation to a second generation with a key
/// of its own, then Carol added from it, with an envelope for the second
/// generation only.
struct Rotated {
    space: Space,
    first: Vec<Change>,
    by_bob: Change,
    rotation: Change,
    second: u64,
    add_carol: Change,
    to_carol: Change,
}

/// A key chain entry for `second`, sealed under [`second_key`], that gives
/// `previous` as the key of the first generation.
fn chain_entry(space: &Space, second: u64, previous: &SpaceKey) -> Entry {
    let first = space.first_generation;
    Entry::KeyChain(KeyChainEntry {
        generation: second,
        previous: vec![PreviousKey {
            generation: first,
            key: membership::seal_previous_key(&second_key(), space.space, second, first, previous),
        }],
        answers: BTreeSet::new(),
    })
}

fn rotated(rotator: Who) -> Rotated {
    let mut space = Space::new(ALICE);
    let genesis = space.genesis.clone();
    let add_bob = space.add(ALICE, BOB, Role::Writer, &[&genesis]);
    let to_bob = space.envelope(ALICE, BOB, &[&add_bob]);
    let by_bob = space.sealed(BOB, vec![set_title(1)]);
    let second = membership::generation_id(space.space, id(rotator), &[space.first_generation]);
    let chain = chain_entry(&space, second, &space_key());
    let rotation = space.write(rotator, chain, &[&to_bob]);
    let add_carol = space.add_at(ALICE, CAROL, Role::Reader, second, &[&rotation]);
    let sealed = envelope::seal(&second_key(), space.space, second, &key(CAROL).public())
        .expect("a simulated member's key takes an envelope");
    let to_carol = space.write(ALICE, Entry::KeyEnvelope(sealed), &[&add_carol]);
    Rotated {
        space,
        first: vec![genesis, add_bob, to_bob],
        by_bob,
        rotation,
        second,
        add_carol,
        to_carol,
    }
}

impl Rotated {
    fn first(&self) -> Vec<&Change> {
        self.first.iter().collect()
    }
}

#[test]
fn the_key_chain_entry_opens_the_first_generation_for_a_member_added_after_it() {
    let r = rotated(ALICE);
    let mut carol = Copy::of(&r.space);
    carol.receive(&r.first());
    carol.receive(&[&r.by_bob, &r.rotation, &r.add_carol, &r.to_carol]);

    assert_eq!(carol.state(&r.by_bob), State::Folded { parked: vec![] });
    assert!(carol.checks.space_key(r.space.first_generation).is_some());
    assert_eq!(carol.checks.model_id(), Some(space::MODEL));
}

#[test]
fn a_key_chain_entry_that_arrives_after_the_envelope_opens_the_first_generation_too() {
    let r = rotated(ALICE);
    let mut carol = Copy::of(&r.space);
    carol.receive(&r.first());
    carol.receive(&[&r.add_carol, &r.to_carol, &r.by_bob]);
    assert_eq!(carol.state(&r.by_bob), State::Rejected(RejectReason::NoKey));

    carol.receive(&[&r.rotation]);

    assert_eq!(carol.state(&r.by_bob), State::Folded { parked: vec![] });
}

#[test]
fn a_key_chain_entry_the_generation_graph_does_not_hold_gives_no_key() {
    // Bob is a writer, so his rotation is not valid when written.
    let r = rotated(BOB);
    let mut carol = Copy::of(&r.space);
    carol.receive(&r.first());
    carol.receive(&[&r.rotation, &r.to_carol, &r.by_bob]);

    assert!(carol.checks.resolved().generation(r.second).is_none());
    assert!(carol.checks.space_key(r.space.first_generation).is_none());
    assert_eq!(carol.state(&r.by_bob), State::Rejected(RejectReason::NoKey));
}

/// Alice's space in which Carol holds the second generation's key from an
/// envelope before either key chain entry arrives, and Bob's change under
/// the first generation. Alice then writes two key chain entries for the
/// second generation: `real` with the first generation's real key, and
/// `other`, sealed with `nonce`, with another key.
struct Disagreeing {
    space: Space,
    before: Vec<Change>,
    by_bob: Change,
    second: u64,
    real: Change,
    other: Change,
}

fn disagreeing(nonce: u8) -> Disagreeing {
    let mut space = Space::new(ALICE);
    let genesis = space.genesis.clone();
    let add_bob = space.add(ALICE, BOB, Role::Writer, &[&genesis]);
    let to_bob = space.envelope(ALICE, BOB, &[&add_bob]);
    let by_bob = space.sealed(BOB, vec![set_title(1)]);
    let add_carol = space.add(ALICE, CAROL, Role::Reader, &[&to_bob]);
    let first = space.first_generation;
    let second = membership::generation_id(space.space, id(ALICE), &[first]);
    let to_carol = envelope::seal_with_ephemeral(
        &second_key(),
        space.space,
        second,
        &key(CAROL).public(),
        [0x3c; 32],
    )
    .expect("a simulated member's key takes an envelope");
    let to_carol = space.write(ALICE, Entry::KeyEnvelope(to_carol), &[&add_carol]);
    let entry = |previous: &SpaceKey, nonce: u8| {
        Entry::KeyChain(KeyChainEntry {
            generation: second,
            previous: vec![PreviousKey {
                generation: first,
                key: membership::seal_previous_key_with_nonce(
                    &second_key(),
                    space.space,
                    second,
                    first,
                    previous,
                    [nonce; 24],
                ),
            }],
            answers: BTreeSet::new(),
        })
    };
    let real = entry(&space_key(), 0);
    let other = entry(&SpaceKey::from_bytes([0x77; 32]), nonce);
    let real = space.write(ALICE, real, &[&to_carol]);
    let other = space.write(ALICE, other, &[&to_carol]);
    Disagreeing {
        before: vec![genesis, add_bob, to_bob, add_carol, to_carol],
        space,
        by_bob,
        second,
        real,
        other,
    }
}

/// The first [`disagreeing`] space, by nonce, in which the real entry's
/// change id is below the other's exactly when `real_lower`.
fn disagreeing_with(real_lower: bool) -> Disagreeing {
    (1..=u8::MAX)
        .map(disagreeing)
        .find(|d| (d.real.id() < d.other.id()) == real_lower)
        .expect("a nonce gives each order of the change ids")
}

#[test]
fn two_key_chain_entries_that_disagree_give_every_copy_the_same_key_in_either_order() {
    for real_lower in [true, false] {
        let d = disagreeing_with(real_lower);
        let before: Vec<&Change> = d.before.iter().chain([&d.by_bob]).collect();
        let mut one = Copy::of(&d.space);
        one.receive(&before);
        one.receive(&[&d.real, &d.other]);
        let mut two = Copy::of(&d.space);
        two.receive(&before);
        two.receive(&[&d.other, &d.real]);

        let generation = one.checks.resolved().generation(d.second);
        assert_eq!(generation.map(|g| g.entries.len()), Some(2));
        assert_eq!(one.checks.model_id(), two.checks.model_id(), "{real_lower}");
        assert_eq!(one.state(&d.by_bob), two.state(&d.by_bob), "{real_lower}");
        assert_eq!(one.fold, two.fold, "{real_lower}");
    }
}

#[test]
fn a_change_folded_under_a_replaced_key_is_unfolded_and_its_field_folded_again_from_the_rest() {
    // The other entry's change id is the lower one, so it wins: copy one
    // folds Bob's change under the real key, then un-folds it when the
    // other entry arrives. Alice's change under the second generation sets
    // the same field and stays folded on both copies.
    let d = disagreeing_with(false);
    let payload = Payload::new(Hlc(1 << 32), BTreeSet::new(), vec![set_title(1)]);
    let by_alice = seal::seal(
        &key(ALICE),
        &second_key(),
        d.space.space,
        100,
        d.second,
        &payload,
    );
    let before: Vec<&Change> = d.before.iter().chain([&d.by_bob, &by_alice]).collect();
    let mut one = Copy::of(&d.space);
    one.receive(&before);
    one.receive(&[&d.real]);
    assert_eq!(one.state(&d.by_bob), State::Folded { parked: vec![] });
    assert!(one.fold.folded.contains(&d.by_bob.operation_id(0)));

    one.receive(&[&d.other]);
    let mut two = Copy::of(&d.space);
    two.receive(&before);
    two.receive(&[&d.other, &d.real]);

    for copy in [&one, &two] {
        assert_eq!(
            copy.state(&d.by_bob),
            State::Rejected(RejectReason::Undecryptable)
        );
        assert_eq!(copy.state(&by_alice), State::Folded { parked: vec![] });
        let folded = &copy.fold.folded;
        assert_eq!(*folded, BTreeSet::from([by_alice.operation_id(0)]));
    }
    assert_eq!(one.fold, two.fold);
}
