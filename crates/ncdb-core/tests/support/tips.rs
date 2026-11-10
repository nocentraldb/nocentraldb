//! Test support for the tips of a log (NOC-138): a pool of real, signed
//! changes whose sealed changes depend on each other, with revocations
//! that un-fold some of them and fold them again; checks that count how
//! often a change is opened; and the full recount the tips must equal,
//! which reads and opens every change of the log, as the Rust SDK did
//! before NOC-138.
//!
//! Each test file includes this module with `#[path]`, together with
//! `space.rs` as `space`; not every file uses every helper.
#![allow(dead_code)]

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};

use ncdb_core::change::{Change, Kind, Payload};
use ncdb_core::checks::SealChecks;
use ncdb_core::clock::Hlc;
use ncdb_core::field_op;
use ncdb_core::ids::{ChangeId, MemberId, ObjectId, SpaceId};
use ncdb_core::log::{Log, Position, State};
use ncdb_core::membership::{ControlBody, Role};
use ncdb_core::receive::{Checks, Stop};
use ncdb_core::seal;
use ncdb_core::tips::Tips;

use crate::space::{self, Space, Who};

pub const ALICE: Who = 1;
pub const BOB: Who = 2;
pub const CAROL: Who = 3;
pub const DAN: Who = 4;
pub const ERIN: Who = 5;
/// Every author of the pool.
pub const AUTHORS: [Who; 5] = [ALICE, BOB, CAROL, DAN, ERIN];

/// The collection the pool's operations create objects in, which both the
/// in-memory fold and the view's test model know.
pub const COLLECTION: &str = "task";

/// The pool, and the changes the tests name.
pub struct Pool {
    pub space: SpaceId,
    /// Every change, in the order it was written.
    pub changes: Vec<Change>,
    /// The sealed changes.
    pub sealed: Vec<Change>,
    /// The revocations: Bob's removal of Dan, Alice's demotion of Dan and
    /// Erin's removal of Bob.
    pub revocations: Vec<Change>,
}

/// A sealed change of `who` at the clock `clock`, after `deps`: one create
/// of the object `object`.
fn sealed(s: &mut Space, who: Who, clock: u64, object: u8, deps: &[&Change]) -> Change {
    s.skip(who, 1);
    let payload = Payload::new(
        Hlc(clock),
        space::dependencies(deps),
        vec![field_op::create(ObjectId([object; 16]), COLLECTION)],
    );
    seal::seal(
        &space::key(who),
        &space::space_key(),
        s.space,
        s.sequence(who),
        s.first_generation,
        &payload,
    )
}

/// Alice founds the space with Bob and Erin as admins, Dan as a writer
/// and Carol as a reader, and wraps the space key to Carol. The sealed
/// changes form a graph with a fork and a join: Alice's first, Dan's
/// first and second and Bob's one after it, Dan's third joining them,
/// and Alice's last after it. Bob removes Dan after his first change, and
/// Alice demotes him after his second; Erin removes Bob with cutoff 0,
/// which excludes Bob's removal of Dan and Bob's change (RFC 0008,
/// seniority). Each copy is Carol's.
pub fn pool() -> &'static Pool {
    static POOL: std::sync::OnceLock<Pool> = std::sync::OnceLock::new();
    POOL.get_or_init(|| {
        let mut s = Space::new(ALICE);
        let g = s.genesis.clone();
        let bob = s.add(ALICE, BOB, Role::Admin, &[&g]);
        let dan = s.add(ALICE, DAN, Role::Writer, &[&bob]);
        let carol = s.add(ALICE, CAROL, Role::Reader, &[&dan]);
        let erin = s.add(ALICE, ERIN, Role::Admin, &[&carol]);
        let to_carol = s.envelope(ALICE, CAROL, &[&erin]);
        let a1 = sealed(&mut s, ALICE, 10, 1, &[]);
        let d1 = sealed(&mut s, DAN, 20, 2, &[&a1]);
        let d2 = sealed(&mut s, DAN, 30, 3, &[&d1]);
        let b1 = sealed(&mut s, BOB, 25, 4, &[&a1]);
        let d3 = sealed(&mut s, DAN, 40, 5, &[&d2, &b1]);
        let a2 = sealed(&mut s, ALICE, 35, 6, &[&d3]);
        let bob_removes_dan = s.remove(BOB, DAN, d1.header().sequence, &[&to_carol]);
        let demotion = s.set_role(ALICE, DAN, Role::Reader, d2.header().sequence, &[&to_carol]);
        let erin_removes_bob = s.remove(ERIN, BOB, 0, &[&erin]);
        let sealed = vec![a1, d1, d2, b1, d3, a2];
        let revocations = vec![bob_removes_dan, demotion, erin_removes_bob];
        let changes = [g, bob, dan, carol, erin, to_carol]
            .into_iter()
            .chain(sealed.iter().cloned())
            .chain(revocations.iter().cloned())
            .collect();
        Pool {
            space: s.space,
            changes,
            sealed,
            revocations,
        }
    })
}

/// Carol's checks, which count every change they open.
pub struct Counting {
    pub checks: SealChecks,
    pub opened: Cell<usize>,
}

impl Counting {
    /// Carol's checks in the pool's space, which hold no control change yet.
    pub fn new() -> Counting {
        Counting {
            checks: SealChecks::new(pool().space, space::key(CAROL)),
            opened: Cell::new(0),
        }
    }

    /// How many changes they opened since the last call.
    pub fn take_opened(&self) -> usize {
        self.opened.replace(0)
    }
}

impl Checks for Counting {
    fn knows_suite(&self, suite: u64) -> bool {
        self.checks.knows_suite(suite)
    }

    fn verify(&self, change: &Change) -> Result<(), Stop> {
        self.checks.verify(change)
    }

    fn is_member(&self, change: &Change) -> bool {
        self.checks.is_member(change)
    }

    fn open(&self, change: &Change) -> Result<Payload, Stop> {
        self.opened.set(self.opened.get() + 1);
        self.checks.open(change)
    }

    fn add_control(&mut self, change: &Change) {
        self.checks.add_control(change);
    }
}

/// What a full recount of a log gives: the heads among the folded
/// changes, the heads among the folded control changes, the next author
/// sequence of each author and the highest clock among the folded sealed
/// changes.
#[derive(Debug, PartialEq, Eq)]
pub struct Recount {
    pub heads: BTreeSet<ChangeId>,
    pub control_heads: BTreeSet<ChangeId>,
    pub sequences: BTreeMap<MemberId, u64>,
    pub seen: Hlc,
}

/// Reads and opens every change of `log`, as the Rust SDK did on each
/// write before NOC-138. A change the log holds but did not fold is no
/// head; a folded sealed change that does not open is skipped.
pub fn recount(log: &Log, checks: &impl Checks) -> Recount {
    let mut heads = BTreeSet::new();
    let mut control_heads = BTreeSet::new();
    let mut covered = BTreeSet::new();
    let mut controls_covered = BTreeSet::new();
    let mut sequences = BTreeMap::new();
    let mut seen = Hlc::default();
    for (_, change) in log.after(Position::START) {
        let header = change.header();
        let next = sequences.entry(header.author).or_insert(1);
        *next = (*next).max(header.sequence + 1);
        if !matches!(log.state(change.id()), Some(State::Folded { .. })) {
            continue;
        }
        let dependencies = match header.kind {
            Kind::CONTROL => {
                let dependencies = ControlBody::decode(&header.payload)
                    .map(|body| body.dependencies)
                    .unwrap_or_default();
                controls_covered.extend(dependencies.iter().copied());
                control_heads.insert(change.id());
                dependencies
            }
            _ => match checks.open(change) {
                Ok(payload) => {
                    seen = seen.max(payload.clock);
                    payload.dependencies
                }
                Err(_) => continue,
            },
        };
        covered.extend(dependencies);
        heads.insert(change.id());
    }
    heads.retain(|id| !covered.contains(id));
    control_heads.retain(|id| !controls_covered.contains(id));
    Recount {
        heads,
        control_heads,
        sequences,
        seen,
    }
}

/// The recount the tips give, for every author of the pool.
pub fn of(tips: &Tips) -> Recount {
    Recount {
        heads: tips.heads().clone(),
        control_heads: tips.control_heads().clone(),
        sequences: AUTHORS
            .iter()
            .map(|who| (space::id(*who), tips.next_sequence(space::id(*who))))
            .filter(|(_, next)| *next > 1)
            .collect(),
        seen: tips.seen(),
    }
}
