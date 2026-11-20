//! RFC 0008's M-C11, "The relay reaches the members' state" (NOC-382), in
//! generated runs: simulated admins and members add, raise, lower and
//! remove each other from their own partial views of the space, so that
//! revocations are concurrent with what they exclude, and members write
//! sealed changes. The changes reach the relay in a random order. After
//! every push:
//!
//! - the relay's membership table equals `resolve` of its own log: every
//!   member `resolve` names has the row `record::records` gives, and any
//!   other member has no row or one that is not current;
//! - every change the relay accepted is one the members accept for the
//!   same set, and every change it refused is one the members reject for
//!   the same set, or one RFC 0008 and RFC 0007 let it refuse on its own:
//!   a push by an uploader that is not a current member.
//!
//! The runs use a seeded generator of their own rather than proptest,
//! which the relay's tests do not depend on; each run names its seed when
//! it fails. Change ids differ from run to run, since the simulated space
//! seals under random nonces, so a failing seed may need several runs to
//! show again.

mod support;

use std::collections::{BTreeMap, BTreeSet};

use ncdb_core::change::{Change, Kind};
use ncdb_core::ids::{ChangeId, MemberId, SpaceId};
use ncdb_core::membership::{ControlBody, Resolved, Role, Verdict, resolve};
use ncdb_relay::conformance::block_on;
use ncdb_relay::sqlite::SqliteStore;
use ncdb_relay::store::RelayStore;
use ncdb_relay::{ErrorCode, Pushed, Relay, record};
use support::space::{Space, Who, id};
use support::{push_one, relay};

/// The members who write: the creator, two admins and a writer.
const WRITERS: [Who; 4] = [0, 1, 2, 3];
/// Every member id the runs name.
const EVERYONE: [Who; 6] = [0, 1, 2, 3, 4, 5];
/// Changes generated per run.
const STEPS: usize = 16;
/// Runs.
const SEEDS: u64 = 80;

/// splitmix64: a small seeded generator, the same on every machine.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// The changes of one run: the base, which reaches the relay first and in
/// order, and the generated changes.
struct Run {
    space: SpaceId,
    base: Vec<Change>,
    changes: Vec<Change>,
}

/// The heads of the control changes among `known`: those no other names.
fn heads<'a>(known: &BTreeSet<ChangeId>, all: &'a BTreeMap<ChangeId, Change>) -> Vec<&'a Change> {
    let controls: Vec<&Change> = known
        .iter()
        .map(|id| &all[id])
        .filter(|c| c.header().kind == Kind::CONTROL)
        .collect();
    let named: BTreeSet<ChangeId> = controls
        .iter()
        .filter_map(|c| ControlBody::decode(&c.header().payload).ok())
        .flat_map(|body| body.dependencies)
        .collect();
    controls
        .into_iter()
        .filter(|c| !named.contains(&c.id()))
        .collect()
}

/// The highest author sequence of `who` among `known`.
fn highest(who: Who, known: &BTreeSet<ChangeId>, all: &BTreeMap<ChangeId, Change>) -> u64 {
    known
        .iter()
        .map(|id| all[id].header())
        .filter(|h| h.author == id(who))
        .map(|h| h.sequence)
        .max()
        .unwrap_or(0)
}

fn generate(rng: &mut Rng) -> Run {
    let mut s = Space::new(0);
    let g = s.genesis.clone();
    let one = s.add(0, 1, Role::Admin, &[&g]);
    let two = s.add(0, 2, Role::Admin, &[&g]);
    let three = s.add(0, 3, Role::Writer, &[&one, &two]);
    let base = vec![g, one, two, three];
    let mut all: BTreeMap<ChangeId, Change> = base.iter().map(|c| (c.id(), c.clone())).collect();
    let mut known: Vec<BTreeSet<ChangeId>> = vec![all.keys().copied().collect(); WRITERS.len()];
    let mut changes: Vec<Change> = Vec::new();
    for _ in 0..STEPS {
        let w = rng.below(WRITERS.len());
        let who = WRITERS[w];
        // The author learns a few earlier changes, not always with their
        // dependencies.
        for _ in 0..rng.below(4) {
            if !changes.is_empty() {
                known[w].insert(changes[rng.below(changes.len())].id());
            }
        }
        let deps = heads(&known[w], &all);
        let subject = 1 + rng.below(5) as Who;
        let role = [Role::Reader, Role::Writer, Role::Admin][rng.below(3)];
        let cutoff = highest(subject, &known[w], &all).saturating_sub(rng.below(2) as u64);
        let change = match rng.below(6) {
            0 => s.add(who, subject.max(3), role, &deps),
            1 => s.set_role(who, subject, role, cutoff, &deps),
            2 | 3 => s.remove(who, subject, cutoff, &deps),
            _ => s.sealed(who, Vec::new()),
        };
        known[w].insert(change.id());
        all.insert(change.id(), change.clone());
        changes.push(change);
    }
    Run {
        space: s.space,
        base,
        changes,
    }
}

/// The control changes of the relay's log, in position order.
fn controls(relay: &Relay<SqliteStore>, space: SpaceId) -> Vec<Change> {
    let mut controls = Vec::new();
    let mut after = 0;
    loop {
        let page = block_on(relay.store().read_controls(space, after, 100)).unwrap();
        let Some(last) = page.last() else {
            return controls;
        };
        after = last.position;
        for entry in &page {
            controls.push(Change::decode(&entry.frame).expect("an appended frame"));
        }
    }
}

fn current(relay: &Relay<SqliteStore>, space: SpaceId, member: MemberId) -> bool {
    block_on(relay.store().member(space, member))
        .unwrap()
        .is_some_and(|row| row.current)
}

/// Whether the members accept `change` for the set `controls` plus it.
fn members_accept(space: SpaceId, controls: &[Change], change: &Change) -> bool {
    match change.header().kind {
        Kind::SEALED => resolve(space, controls).admits(change.header()),
        _ => {
            let resolved = resolve(space, controls.iter().chain([change]));
            matches!(
                resolved.verdict(change.id()),
                Some(Verdict::Accepted | Verdict::Lost)
            )
        }
    }
}

/// The table equals `resolve` of the relay's log.
fn check_table(relay: &Relay<SqliteStore>, space: SpaceId, resolved: &Resolved, at: &str) {
    let records = record::records(resolved);
    for who in EVERYONE {
        let member = id(who);
        let row = block_on(relay.store().member(space, member)).unwrap();
        match records.iter().find(|r| r.member == member) {
            Some(record) => assert!(
                row.as_ref() == Some(record),
                "M-C11 {at}: member {who}'s row is not resolve's"
            ),
            None => assert!(
                row.is_none_or(|row| !row.current),
                "M-C11 {at}: member {who} is current on the relay; resolve names him not at all"
            ),
        }
    }
}

fn run(seed: u64) {
    let mut rng = Rng(seed);
    let Run {
        space,
        base,
        mut changes,
    } = generate(&mut rng);
    let (relay, _dir) = relay();
    for change in &base {
        assert!(matches!(push_one(&relay, 0, change), Pushed::Accepted(_)));
    }
    // A random order of arrival.
    for i in (1..changes.len()).rev() {
        changes.swap(i, rng.below(i + 1));
    }
    for (step, change) in changes.iter().enumerate() {
        let at = format!("seed {seed}, push {step}");
        let before = controls(&relay, space);
        let author = change.header().author;
        let uploader = EVERYONE
            .into_iter()
            .find(|who| id(*who) == author && current(&relay, space, author))
            .or_else(|| {
                EVERYONE
                    .into_iter()
                    .find(|who| current(&relay, space, id(*who)))
            })
            .unwrap_or(0);
        let uploader_current = current(&relay, space, id(uploader));
        let pushed = push_one(&relay, uploader, change);
        let accept = members_accept(space, &before, change);
        match pushed {
            Pushed::Accepted(_) => assert!(
                accept,
                "M-C11 {at}: the relay accepted a change the members reject"
            ),
            Pushed::Rejected(ErrorCode::NotMember | ErrorCode::RemovedMember)
                if !uploader_current => {}
            Pushed::Rejected(code) => assert!(
                !accept,
                "M-C11 {at}: the relay refused with {code:?} a change the members accept"
            ),
            Pushed::Duplicate(_) => panic!("M-C11 {at}: each change is pushed once"),
        }
        check_table(
            &relay,
            space,
            &resolve(space, &controls(&relay, space)),
            &at,
        );
    }
}

#[test]
fn m_c11_the_relay_table_is_resolve_of_its_log_and_it_refuses_only_what_members_reject() {
    for seed in 0..SEEDS {
        run(seed);
    }
}
