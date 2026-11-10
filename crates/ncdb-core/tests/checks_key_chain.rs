//! `SealChecks` walks the key chain (RFC 0003, "Key-chain subkey"; RFC
//! 0008, "Key chain entry"): from every generation whose key it holds, it
//! opens the previous generations' keys that the generation's key chain
//! entry encrypts, back to the first generation. A member added after a
//! rotation, with an envelope for the new generation only, so reads the
//! changes sealed before it, in whichever order the envelope and the key
//! chain entry arrive. Only a key chain entry that the generation graph
//! holds, valid when written, gives keys.

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

use ncdb_core::change::Change;
use ncdb_core::checks::SealChecks;
use ncdb_core::envelope;
use ncdb_core::keys::SpaceKey;
use ncdb_core::log::{Log, RejectReason, State};
use ncdb_core::membership::{self, Entry, KeyChainEntry, PreviousKey, Role};

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

fn rotated(rotator: Who) -> Rotated {
    let mut space = Space::new(ALICE);
    let genesis = space.genesis.clone();
    let add_bob = space.add(ALICE, BOB, Role::Writer, &[&genesis]);
    let to_bob = space.envelope(ALICE, BOB, &[&add_bob]);
    let by_bob = space.sealed(BOB, vec![set_title(1)]);
    let first = space.first_generation;
    let second = membership::generation_id(space.space, id(rotator), &[first]);
    let chain = KeyChainEntry {
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
    };
    let rotation = space.write(rotator, Entry::KeyChain(chain), &[&to_bob]);
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
    assert_eq!(carol.checks.model_id(), Some([1; 16]));
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
