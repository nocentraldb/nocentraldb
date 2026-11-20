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

use std::collections::BTreeMap;

use ncdb_core::change::{Change, Kind};
use ncdb_core::checks::SealChecks;
use ncdb_core::envelope;
use ncdb_core::ids::ChangeId;
use ncdb_core::log::{Log, Position, RejectReason, State};
use ncdb_core::membership::{self, ControlBody, Entry};
use proptest::prelude::*;
use support::convergence::{self, Author, Config, Generator, Mergeable, Op, OpId, Outcome};

use control::{Copy, Members, base};
use member::Recorder;

/// The members whose copies hold keys: two admins of the base. The third
/// holder is a member id that no change names, so it never holds a key.
const KEYED: [u8; 2] = [2, 3];
const KEYLESS: u8 = 0x70;

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
        }
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
