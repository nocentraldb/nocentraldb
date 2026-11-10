//! NOC-78 with the convergence harness: copies of one member that receive
//! the same real, signed changes (the genesis, membership entries, key
//! envelopes and sealed changes) in any order and with duplicates end with
//! the same state for every change, the same folded operations and the
//! same control state. A change rejected for an unknown author, a missing
//! membership or a missing key is folded once the control change it lacked
//! arrives, whatever came first (D11).
//!
//! The pool holds no revocation that excludes a sealed change: a change
//! folded before such a revocation arrives stays folded until un-folding
//! exists (NOC-49).

mod support;

#[path = "support/member.rs"]
mod member;
#[path = "support/space.rs"]
mod space;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeMap;

use ncdb_core::change::Change;
use ncdb_core::checks::SealChecks;
use ncdb_core::ids::{ChangeId, SpaceId};
use ncdb_core::keys::SpaceKey;
use ncdb_core::log::{Log, Position, State};
use ncdb_core::membership::Role;
use ncdb_core::seal;
use proptest::prelude::*;
use support::convergence::{self, Config, Mergeable, Op, OpId, Outcome, independent};

use member::{Recorder, set_title};
use space::Space;

const ALICE: u8 = 1;
const BOB: u8 = 2;
const CAROL: u8 = 3;
const DAN: u8 = 4;

/// Every change a member may receive: Alice's genesis, Bob added as a
/// writer, Carol as a reader, envelopes to Carol and to Bob, and sealed
/// changes that fold, that are not a member's, that have an unknown author
/// or that do not decrypt. Each copy is Carol's.
fn pool() -> &'static (SpaceId, Vec<Change>) {
    static POOL: std::sync::OnceLock<(SpaceId, Vec<Change>)> = std::sync::OnceLock::new();
    POOL.get_or_init(|| {
        let mut s = Space::new(ALICE);
        let g = s.genesis.clone();
        let bob = s.add(ALICE, BOB, Role::Writer, &[&g]);
        let carol = s.add(ALICE, CAROL, Role::Reader, &[&bob]);
        let to_carol = s.envelope(ALICE, CAROL, &[&carol]);
        let to_bob = s.envelope(ALICE, BOB, &[&carol]);
        let mut changes = vec![g, bob, carol, to_carol, to_bob];
        for (who, object) in [(ALICE, 1), (BOB, 2), (BOB, 3), (CAROL, 4), (DAN, 5)] {
            changes.push(s.sealed(who, vec![set_title(object), set_title(object + 8)]));
        }
        let other = s.first_generation ^ 1;
        changes.push(s.sealed_under(ALICE, other, vec![set_title(6)]));
        let payload = ncdb_core::change::Payload::new(
            ncdb_core::clock::Hlc(1),
            Default::default(),
            vec![set_title(7)],
        );
        let wrong_key = SpaceKey::from_bytes([0xa5; 32]);
        let generation = s.first_generation;
        let author = space::key(BOB);
        changes.push(seal::seal(
            &author, &wrong_key, s.space, 9, generation, &payload,
        ));
        (s.space, changes)
    })
}

/// Carol's copy: the frames it received, and its log, checks and fold
/// after them. Cloning replays the frames, since checks hold Carol's
/// secret key and are not `Clone`.
struct Copy {
    received: Vec<Vec<u8>>,
    log: Log,
    checks: SealChecks,
    fold: Recorder,
}

impl Default for Copy {
    fn default() -> Copy {
        let (space, _) = pool();
        Copy {
            received: Vec::new(),
            log: Log::new(*space),
            checks: SealChecks::new(*space, space::key(CAROL)),
            fold: Recorder::default(),
        }
    }
}

impl Clone for Copy {
    fn clone(&self) -> Copy {
        let mut copy = Copy::default();
        for frame in &self.received {
            copy.receive(frame);
        }
        copy
    }
}

impl Copy {
    fn receive(&mut self, frame: &[u8]) {
        self.log
            .receive(frame, &mut self.checks, &mut self.fold)
            .expect("every pooled frame is appended");
        self.received.push(frame.to_vec());
    }

    /// The state of each change, by change id, so that the order in which
    /// the log holds them does not count.
    fn states(&self) -> BTreeMap<ChangeId, Option<State>> {
        let log = &self.log;
        log.after(Position::START)
            .map(|(_, change)| (change.id(), log.state(change.id()).cloned()))
            .collect()
    }
}

impl std::fmt::Debug for Copy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Copy")
            .field("states", &self.states())
            .field("folded", &self.fold.folded)
            .field("checks", &self.checks)
            .finish()
    }
}

impl Mergeable for Copy {
    type Change = Vec<u8>;
    type Read = String;

    fn apply(&mut self, op: &Op<Vec<u8>>) {
        self.receive(&op.body);
    }

    /// The state of every change, the folded operations, the keys the
    /// checks hold and the control state: equal copies, equal bytes.
    fn state_bytes(&self) -> Vec<u8> {
        let mut bytes = format!("{:?}", (self.states(), &self.fold, &self.checks)).into_bytes();
        bytes.extend(self.checks.resolved().encode());
        bytes
    }

    fn read(&self) -> String {
        format!("{:?}", self.states())
    }

    /// The log keeps every change it receives, folded, parked or rejected:
    /// none is replaced or lost.
    fn outcome(&self, _: OpId) -> Outcome {
        Outcome::Visible
    }
}

/// Any change of the pool, by any member.
fn change(_member: u8) -> BoxedStrategy<Vec<u8>> {
    let (_, changes) = pool();
    (0..changes.len())
        .prop_map(|i| pool().1[i].bytes().to_vec())
        .boxed()
}

#[test]
fn every_copy_folds_what_its_control_changes_admit_in_any_order() {
    let config = Config {
        members: 4,
        max_changes_per_member: 6,
        cases: 32,
        ..Config::default()
    };
    let result = convergence::check::<Copy, _>(&config, &independent(change));
    assert!(result.is_ok(), "{}", result.unwrap_err());
}

#[test]
fn carol_folds_every_admitted_change_once_she_holds_them_all() {
    let (_, changes) = pool();
    let mut copy = Copy::default();
    // Sealed changes first, then the control changes in reverse.
    for change in changes.iter().rev() {
        copy.receive(change.bytes());
    }
    let folded: Vec<bool> = changes
        .iter()
        .map(|c| matches!(copy.log.state(c.id()), Some(State::Folded { .. })))
        .collect();
    // Five control changes, Alice's and Bob's two sealed changes folded;
    // Carol's, Dan's, the other generation's and the wrong key's not.
    let expected = [
        true, true, true, true, true, true, true, true, false, false, false, false,
    ];
    assert_eq!(folded, expected);
}
