//! RFC 0008's M-C4 with the convergence harness: the fold reads only
//! control bodies and headers ("What the fold may read"). Each copy of the
//! harness holds three holders of the same space, which receive every
//! control change but different subsets of the sealed changes, through a
//! real log and its checks: two admins, each holding the space key, and a
//! holder that holds no key at all, as the relay does. After every
//! delivery, each holder's control state must equal `resolve` of every
//! control change delivered so far, and the step-4 verdict its log reached
//! for every sealed change it holds must be the membership verdict of
//! that change's header (RFC 0008, "Sealed changes"), whatever keys and
//! sealed changes the holder has.
//!
//! M-C5, "Local checks never split the state", runs on the same holders:
//! members also write key envelopes and key chain entries that hold wrong
//! keys. The control state must stay equal on every holder, the harness
//! compares each holder's verdicts and reports across orders of arrival,
//! and only the holder a wrong envelope is addressed to reports it
//! (`SealChecks::wrong_envelopes`). Person-key records are checked in the
//! SDK, not in the core's log and checks, so they are not mixed in here.

mod support;

#[path = "support/control.rs"]
mod control;
#[path = "support/member.rs"]
mod member;
#[path = "support/space.rs"]
mod space;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use ncdb_core::change::{Change, Kind};
use ncdb_core::checks::SealChecks;
use ncdb_core::envelope;
use ncdb_core::ids::ChangeId;
use ncdb_core::keys::SpaceKey;
use ncdb_core::log::{Log, Position, RejectReason, State};
use ncdb_core::membership::{self, ControlBody, Entry, KeyChainEntry, PreviousKey};
use proptest::prelude::*;
use support::convergence::{self, Author, Config, Generator, Mergeable, Op, OpId, Outcome};

use control::{Copy, Members, base};
use member::Recorder;

/// The members whose copies hold keys: two admins of the base. The third
/// holder is a member id that no change names, so it never holds a key.
const KEYED: [u8; 2] = [2, 3];
const KEYLESS: u8 = 0x70;

thread_local! {
    /// The envelopes with a wrong key the generator wrote (M-C5), with
    /// their recipient and generation.
    static WRONG: RefCell<BTreeMap<ChangeId, (u8, u64)>> = const { RefCell::new(BTreeMap::new()) };
}

/// The envelopes that give the two keyed holders the first generation's
/// key, by the creator after the base. Every copy holds them, so the
/// members' control changes see them too.
fn envelopes() -> &'static [Change; 2] {
    static ENVELOPES: std::sync::OnceLock<[Change; 2]> = std::sync::OnceLock::new();
    ENVELOPES.get_or_init(|| {
        let (space, first_generation, base) = base();
        let last = base.last().expect("the base").id();
        let creator = space::id(1);
        let after_base = base.iter().filter(|c| c.header().author == creator).count() as u64;
        KEYED.map(|who| {
            let sealed = envelope::seal_with_ephemeral(
                &space::space_key(),
                *space,
                *first_generation,
                &space::key(who).public(),
                [0x5e ^ who; 32],
            )
            .expect("a simulated member's key takes an envelope");
            let body = ControlBody::new([last].into(), Entry::KeyEnvelope(sealed));
            let sequence = after_base + u64::from(who) - 1;
            membership::write(&space::key(1), *space, sequence, *first_generation, &body)
        })
    })
}

/// One holder: its log, its checks and its fold. `bit` picks the sealed
/// changes it receives.
struct Holder {
    bit: u8,
    log: Log,
    checks: SealChecks,
    fold: Recorder,
}

impl Holder {
    fn new(bit: u8, who: u8) -> Holder {
        let (space, _, base) = base();
        let mut holder = Holder {
            bit,
            log: Log::new(*space),
            checks: SealChecks::new(*space, space::key(who)),
            fold: Recorder::default(),
        };
        for change in base.iter().chain(envelopes()) {
            holder.receive(change);
        }
        holder
    }

    fn receive(&mut self, change: &Change) {
        self.log
            .receive(change.bytes(), &mut self.checks, &mut self.fold)
            .expect("every simulated change is appended");
    }

    /// Per change the holder holds, its state.
    fn states(&self) -> BTreeMap<ChangeId, Option<State>> {
        let log = &self.log;
        log.after(Position::START)
            .map(|(_, change)| (change.id(), log.state(change.id()).cloned()))
            .collect()
    }
}

/// One copy of the harness: every change delivered so far, resolved, and
/// the three holders.
struct Holders {
    full: Copy,
    received: Vec<Op<(Change, u8)>>,
    holders: [Holder; 3],
}

impl Default for Holders {
    fn default() -> Holders {
        let mut full = Copy::default();
        for (i, change) in envelopes().iter().enumerate() {
            full.apply(&Op {
                id: OpId::MAX - i as OpId,
                author: 0,
                body: change.clone(),
            });
        }
        Holders {
            full,
            received: Vec::new(),
            holders: [
                Holder::new(0, KEYED[0]),
                Holder::new(1, KEYED[1]),
                Holder::new(2, KEYLESS),
            ],
        }
    }
}

impl Clone for Holders {
    /// Replays what the copy received, since checks hold a secret key and
    /// are not `Clone`.
    fn clone(&self) -> Holders {
        let mut copy = Holders::default();
        for op in &self.received {
            copy.apply(op);
        }
        copy
    }
}

impl std::fmt::Debug for Holders {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let states: Vec<_> = self.holders.iter().map(Holder::states).collect();
        f.debug_struct("Holders")
            .field("full", &self.full)
            .field("states", &states)
            .finish()
    }
}

/// The step-4 verdict a holder's log reached for a sealed change: `true`
/// when it passed the membership check, whatever step 5 then found.
fn passed_membership(state: &State) -> bool {
    match state {
        State::Rejected(RejectReason::NotMember) => false,
        State::Rejected(RejectReason::NoKey | RejectReason::Undecryptable) => true,
        State::Folded { .. } => true,
        other => panic!("a simulated sealed change stopped before step 4: {other:?}"),
    }
}

impl Holders {
    /// M-C4: every holder's control state is `resolve` of every control
    /// change, and each sealed change it holds has the membership verdict
    /// of its header.
    fn check(&self) {
        let resolved = self.full.resolved();
        let expected = resolved.encode();
        for (i, holder) in self.holders.iter().enumerate() {
            assert!(
                holder.checks.resolved().encode() == expected,
                "M-C4: holder {i}'s control state differs from resolve of the control changes"
            );
            for (_, change) in holder.log.after(Position::START) {
                if change.header().kind != Kind::SEALED {
                    continue;
                }
                let state = holder.log.state(change.id()).expect("a held change");
                assert_eq!(
                    passed_membership(state),
                    resolved.admits(change.header()),
                    "M-C4: holder {i} reached {state:?} for sealed change {:?}",
                    change.id()
                );
            }
            self.check_reports(i, holder);
        }
    }

    /// M-C5: only the local reports differ, and only on the holders that
    /// can tell. A holder reports a wrong envelope only when it is addressed
    /// to it and holds a wrong key, and it reports every such envelope of
    /// the first generation it holds, which the genesis confirms. The keyless
    /// holder, which no envelope is addressed to, reports none. Later
    /// generations are left out: the simulated key chain entries carry
    /// stand-in previous keys, which refute every key of their generation.
    fn check_reports(&self, i: usize, holder: &Holder) {
        let (_, first, _) = base();
        let who = [KEYED[0], KEYED[1], KEYLESS][i];
        let reported: BTreeSet<ChangeId> = holder
            .checks
            .wrong_envelopes()
            .into_iter()
            .filter(|w| w.generation == *first)
            .map(|w| w.change)
            .collect();
        let held: BTreeSet<ChangeId> = holder
            .log
            .after(Position::START)
            .map(|(_, c)| c.id())
            .collect();
        let wrong: BTreeSet<ChangeId> = WRONG.with(|wrong| {
            wrong
                .borrow()
                .iter()
                .filter(|(id, (to, generation))| {
                    *to == who && generation == first && held.contains(id)
                })
                .map(|(id, _)| *id)
                .collect()
        });
        assert_eq!(
            reported, wrong,
            "M-C5: holder {i} reports other wrong envelopes than those addressed to it"
        );
    }
}

impl Mergeable for Holders {
    type Change = (Change, u8);
    type Read = control::Read;

    fn apply(&mut self, op: &Op<(Change, u8)>) {
        let (change, mask) = &op.body;
        let control = change.header().kind == Kind::CONTROL;
        self.full.apply(&Op {
            id: op.id,
            author: op.author,
            body: change.clone(),
        });
        for holder in &mut self.holders {
            if control || mask & (1 << holder.bit) != 0 {
                holder.receive(change);
            }
        }
        self.received.push(op.clone());
        self.check();
    }

    /// The control state, and per holder the state of every change it
    /// holds.
    fn state_bytes(&self) -> Vec<u8> {
        let mut bytes = self.full.state_bytes();
        for holder in &self.holders {
            bytes.extend(holder.checks.resolved().encode());
            bytes.extend(format!("{:?}", holder.states()).into_bytes());
            let mut reports = holder.checks.wrong_envelopes();
            reports.sort_by_key(|w| (w.generation, w.change));
            bytes.extend(format!("{reports:?}").into_bytes());
        }
        bytes
    }

    fn read(&self) -> control::Read {
        self.full.read()
    }

    fn outcome(&self, id: OpId) -> Outcome {
        self.full.outcome(id)
    }
}

/// The members of `tests/membership_convergence.rs`, whose sealed changes
/// each carry a random set of the holders that receive them.
struct Spread(Members);

impl Generator<Holders> for Spread {
    type Raw = (control::Raw, u8);

    fn raw(&self, member: u8) -> BoxedStrategy<(control::Raw, u8)> {
        (Generator::<Copy>::raw(&self.0, member), 0..8u8).boxed()
    }

    fn make(&self, author: &Author<'_, Holders>, raw: &(control::Raw, u8)) -> (Change, u8) {
        let (raw, mask) = raw;
        let author = Author {
            member: author.member,
            id: author.id,
            copy: &author.copy.full,
            seen: &[],
        };
        (self.0.make(&author, raw), *mask)
    }
}

/// The members of `Spread` that also write envelopes to the keyed holders,
/// with the right key or a wrong one, and key chain entries whose previous
/// keys open to a wrong key (M-C5).
struct Tamper(Members);

impl Generator<Holders> for Tamper {
    type Raw = (control::Raw, u8, u8, u8);

    fn raw(&self, member: u8) -> BoxedStrategy<(control::Raw, u8, u8, u8)> {
        let members = Generator::<Copy>::raw(&self.0, member);
        (members, 0..8u8, 0..8u8, any::<u8>()).boxed()
    }

    fn make(&self, author: &Author<'_, Holders>, raw: &(control::Raw, u8, u8, u8)) -> (Change, u8) {
        let (members, mask, tamper, pick) = raw;
        let copy = &author.copy.full;
        let who = author.member + 1;
        let change = match tamper {
            5 => envelope_to_holder(copy, who, author.id, *pick, true),
            6 => envelope_to_holder(copy, who, author.id, *pick, false),
            7 => wrong_chain(copy, who, *pick),
            _ => {
                let inner = Author {
                    member: author.member,
                    id: author.id,
                    copy,
                    seen: &[],
                };
                self.0.make(&inner, members)
            }
        };
        (change, *mask)
    }
}

/// A key that is not the space key: what a wrong envelope or key chain
/// entry holds.
fn wrong_key(pick: u8) -> SpaceKey {
    SpaceKey::from_bytes([0xe0 | (pick & 0x0f); 32])
}

/// An envelope by `who` to a keyed holder, for a generation of its copy,
/// that wraps the space key when `right`, else a wrong key. A right one
/// can arrive before or after a wrong one of the same generation.
fn envelope_to_holder(copy: &Copy, who: u8, id: OpId, pick: u8, right: bool) -> Change {
    let (space, first, _) = base();
    let to = KEYED[usize::from(pick % 2)];
    let generations: Vec<u64> = copy.resolved().generations().map(|(g, _)| g).collect();
    let generation = match pick & 2 {
        0 => *first,
        _ => generations[usize::from(pick >> 2) % generations.len()],
    };
    let mut ephemeral = [who ^ 0x2a; 32];
    ephemeral[..4].copy_from_slice(&id.to_le_bytes());
    let key = match right {
        true => space::space_key(),
        false => wrong_key(pick),
    };
    let sealed = envelope::seal_with_ephemeral(
        &key,
        *space,
        generation,
        &space::key(to).public(),
        ephemeral,
    )
    .expect("a simulated member's key takes an envelope");
    let body = ControlBody::new(copy.heads(), Entry::KeyEnvelope(sealed));
    let sequence = copy.highest(space::id(who)) + 1;
    let change = membership::write(&space::key(who), *space, sequence, *first, &body);
    if !right {
        WRONG.with(|wrong| wrong.borrow_mut().insert(change.id(), (to, generation)));
    }
    change
}

/// A rotation by `who` after the heads of its copy whose previous keys,
/// sealed under the space key, open to a wrong key.
fn wrong_chain(copy: &Copy, who: u8, pick: u8) -> Change {
    let (space, first, _) = base();
    let previous: Vec<u64> = copy.resolved().heads().into_iter().collect();
    let generation = membership::generation_id(*space, space::id(who), &previous);
    let entry = KeyChainEntry {
        generation,
        previous: previous
            .into_iter()
            .map(|p| PreviousKey {
                generation: p,
                key: membership::seal_previous_key_with_nonce(
                    &space::space_key(),
                    *space,
                    generation,
                    p,
                    &wrong_key(pick),
                    [pick; 24],
                ),
            })
            .collect(),
        answers: BTreeSet::new(),
    };
    let body = ControlBody::new(copy.heads(), Entry::KeyChain(entry));
    let sequence = copy.highest(space::id(who)) + 1;
    membership::write(&space::key(who), *space, sequence, *first, &body)
}

fn config() -> Config {
    Config {
        members: 3,
        max_changes_per_member: 4,
        cases: 32,
        ..Config::default()
    }
}

#[test]
fn m_c4_holders_with_other_keys_and_other_sealed_changes_reach_the_same_control_state() {
    let members = Members {
        forks: false,
        shreds: false,
    };
    let result = convergence::check::<Holders, _>(&config(), &Spread(members));
    assert!(result.is_ok(), "{}", result.unwrap_err());
}

/// The same with equivocations: sealed changes that reuse an author
/// sequence, which only some holders receive.
#[test]
fn m_c4_equivocated_sealed_changes_held_by_some_holders_only_change_no_control_state() {
    let members = Members {
        forks: true,
        shreds: false,
    };
    let result = convergence::check::<Holders, _>(&config(), &Spread(members));
    assert!(result.is_ok(), "{}", result.unwrap_err());
}

/// M-C5: envelopes and key chain entries that hold wrong keys, mixed into
/// the members' changes. The control state stays equal on every holder,
/// each holder's verdicts are the same in every order, and only the holder
/// a wrong envelope is addressed to reports it.
#[test]
fn m_c5_wrong_keys_split_no_state_and_only_their_recipient_reports_them() {
    let members = Members {
        forks: false,
        shreds: false,
    };
    let result = convergence::check::<Holders, _>(&config(), &Tamper(members));
    assert!(result.is_ok(), "{}", result.unwrap_err());
}
