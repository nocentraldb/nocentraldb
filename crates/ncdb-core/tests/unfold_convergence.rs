//! Un-folding with the convergence harness (NOC-49; RFC 0006, "Why the end
//! state depends only on the set"): copies of one member that receive the
//! same real, signed changes in any order and with duplicates end with the
//! same state for every change and the same fold, whether a revocation
//! arrived before the changes it excludes or after the copy folded them.
//!
//! The pool holds two revocations of Dan, a writer: Bob's removal with a
//! cutoff after Dan's first change, and Alice's demotion with a cutoff after
//! his second. Erin, an admin junior to Bob, removes Bob concurrently with
//! cutoff 0, which excludes Bob's removal of Dan (RFC 0008, seniority). So a
//! copy can fold Dan's changes, un-fold them when Bob's removal arrives,
//! and fold all but the last again when Erin's removal arrives.

mod support;

#[path = "support/member.rs"]
mod member;
#[path = "support/space.rs"]
mod space;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::{BTreeMap, BTreeSet};

use ncdb_core::basic::Write;
use ncdb_core::cbor::Value;
use ncdb_core::change::{Change, Operation};
use ncdb_core::checks::SealChecks;
use ncdb_core::clock::Hlc;
use ncdb_core::field_op;
use ncdb_core::ids::{ChangeId, ObjectId, SpaceId};
use ncdb_core::log::{Log, Position, RejectReason, State};
use ncdb_core::membership::Role;
use proptest::prelude::*;
use support::convergence::{self, Config, Mergeable, Op, OpId, Outcome, independent};

use member::{COLLECTION, Recorder, set_title};
use space::Space;

const ALICE: u8 = 1;
const BOB: u8 = 2;
const CAROL: u8 = 3;
const DAN: u8 = 4;
const ERIN: u8 = 5;

fn object(n: u8) -> ObjectId {
    ObjectId([n; 16])
}

fn create(n: u8) -> Operation {
    field_op::create(object(n), COLLECTION)
}

fn delete(n: u8) -> Operation {
    let write = Write {
        id: ncdb_core::ids::OperationId {
            change: ChangeId([0; 32]),
            position: 0,
        },
        author: space::id(DAN),
        stamp: Hlc(0),
        value: Value::Bool(true),
        replaces: BTreeSet::new(),
    };
    field_op::delete(object(n), COLLECTION, &write)
}

/// The pool, and the changes the tests name.
struct Pool {
    space: SpaceId,
    changes: Vec<Change>,
    dan: [Change; 3],
    bob_removes_dan: Change,
    demotion: Change,
    erin_removes_bob: Change,
}

/// Alice founds the space with Bob as an admin, Dan as a writer, Carol as
/// a reader and Erin as an admin, and wraps the space key to Carol. Dan
/// writes three changes on objects 1 and 2, Alice one on object 1; then the
/// revocations. Each copy is Carol's.
fn pool() -> &'static Pool {
    static POOL: std::sync::OnceLock<Pool> = std::sync::OnceLock::new();
    POOL.get_or_init(|| {
        let mut s = Space::new(ALICE);
        let g = s.genesis.clone();
        let bob = s.add(ALICE, BOB, Role::Admin, &[&g]);
        let dan = s.add(ALICE, DAN, Role::Writer, &[&bob]);
        let carol = s.add(ALICE, CAROL, Role::Reader, &[&dan]);
        let erin = s.add(ALICE, ERIN, Role::Admin, &[&carol]);
        let to_carol = s.envelope(ALICE, CAROL, &[&erin]);
        let d1 = s.sealed(DAN, vec![create(1), set_title(1)]);
        let d2 = s.sealed(DAN, vec![set_title(1), delete(1), create(2)]);
        let d3 = s.sealed(DAN, vec![set_title(2), set_title(1)]);
        let a1 = s.sealed(ALICE, vec![set_title(1), create(1)]);
        let cutoff = d1.header().sequence;
        let bob_removes_dan = s.remove(BOB, DAN, cutoff, &[&to_carol]);
        let cutoff = d2.header().sequence;
        let demotion = s.set_role(ALICE, DAN, Role::Reader, cutoff, &[&to_carol]);
        let erin_removes_bob = s.remove(ERIN, BOB, 0, &[&erin]);
        let changes = vec![
            g,
            bob,
            dan,
            carol,
            erin,
            to_carol,
            d1.clone(),
            d2.clone(),
            d3.clone(),
            a1,
            bob_removes_dan.clone(),
            demotion.clone(),
            erin_removes_bob.clone(),
        ];
        Pool {
            space: s.space,
            changes,
            dan: [d1, d2, d3],
            bob_removes_dan,
            demotion,
            erin_removes_bob,
        }
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
        let space = pool().space;
        Copy {
            received: Vec::new(),
            log: Log::new(space),
            checks: SealChecks::new(space, space::key(CAROL)),
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

    fn state(&self, change: &Change) -> Option<State> {
        self.log.state(change.id()).cloned()
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
            .field("fold", &self.fold)
            .finish()
    }
}

impl Mergeable for Copy {
    type Change = Vec<u8>;
    type Read = String;

    fn apply(&mut self, op: &Op<Vec<u8>>) {
        self.receive(&op.body);
    }

    /// The state of every change, the fold and the control state: equal
    /// copies, equal bytes.
    fn state_bytes(&self) -> Vec<u8> {
        let mut bytes = format!("{:?}", (self.states(), &self.fold)).into_bytes();
        bytes.extend(self.checks.resolved().encode());
        bytes
    }

    fn read(&self) -> String {
        format!("{:?}", (self.states(), &self.fold))
    }

    /// The log keeps every change it receives, folded or rejected: none is
    /// replaced or lost.
    fn outcome(&self, _: OpId) -> Outcome {
        Outcome::Visible
    }
}

/// Any change of the pool, by any member.
fn change(_member: u8) -> BoxedStrategy<Vec<u8>> {
    (0..pool().changes.len())
        .prop_map(|i| pool().changes[i].bytes().to_vec())
        .boxed()
}

#[test]
fn every_copy_un_folds_and_folds_again_to_the_same_state_in_any_order() {
    let config = Config {
        members: 4,
        max_changes_per_member: 8,
        cases: 64,
        ..Config::default()
    };
    let result = convergence::check::<Copy, _>(&config, &independent(change));
    assert!(result.is_ok(), "{}", result.unwrap_err());
}

/// The pool's path through un-folding and folding again, in one order:
/// every change arrives before the revocations, then Bob's removal of
/// Dan, then Erin's removal of Bob and Alice's demotion of Dan.
#[test]
fn a_copy_folds_again_what_a_revocation_excluded_once_that_revocation_is_excluded() {
    let pool = pool();
    let folded = Some(State::Folded { parked: vec![] });
    let not_member = Some(State::Rejected(RejectReason::NotMember));
    let mut copy = Copy::default();
    let revocations = [
        &pool.bob_removes_dan,
        &pool.demotion,
        &pool.erin_removes_bob,
    ];
    for change in &pool.changes {
        if !revocations.contains(&change) {
            copy.receive(change.bytes());
        }
    }
    let [d1, d2, d3] = &pool.dan;
    assert_eq!(
        [copy.state(d1), copy.state(d2), copy.state(d3)],
        [folded.clone(), folded.clone(), folded.clone()]
    );

    copy.receive(pool.bob_removes_dan.bytes());
    assert_eq!(
        [copy.state(d1), copy.state(d2), copy.state(d3)],
        [folded.clone(), not_member.clone(), not_member.clone()]
    );

    copy.receive(pool.erin_removes_bob.bytes());
    copy.receive(pool.demotion.bytes());
    assert_eq!(
        [copy.state(d1), copy.state(d2), copy.state(d3)],
        [folded.clone(), folded, not_member]
    );

    // The same as a copy that received the revocations first.
    let mut early = Copy::default();
    for change in pool.changes.iter().rev() {
        early.receive(change.bytes());
    }
    assert_eq!(early.states(), copy.states());
    assert_eq!(early.fold, copy.fold);
}
