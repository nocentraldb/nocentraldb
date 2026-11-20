//! RFC 0008's M-C6 with the convergence harness: seniority and the order
//! are total ("How the convergence harness checks it"). Simulated admins
//! and members add, raise, lower and remove each other, themselves
//! included, from their own copies and with dependencies chosen apart from
//! what they hold, so that mutual removals, rings and overlapping cycles
//! of concurrent revocations, ties of depth between admins the creator
//! added at once, and missing dependencies arise. Members also write
//! acknowledgements.
//!
//! After every delivery each copy checks two things:
//!
//! - `resolve`'s seniority, order and verdicts equal those of the
//!   reference implementation of steps 2 to 5 in `support/reference.rs`,
//!   written from the RFC's text;
//! - for the set S before the delivery and S′ after it, a verdict of a
//!   change of S differs only for a cause RFC 0008's "What a new change
//!   can change" lists. The delivered change can be none of those causes
//!   when it is no missing dependency of S and is an acknowledgement or a
//!   change that is not valid when written: then no verdict of S may
//!   change.

mod support;

#[path = "support/reference.rs"]
mod reference;
#[path = "support/space.rs"]
mod space;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::cell::OnceCell;
use std::collections::{BTreeMap, BTreeSet};

use ncdb_core::cbor::Value;
use ncdb_core::change::Change;
use ncdb_core::ids::{ChangeId, SpaceId};
use ncdb_core::membership::{
    self, ACKNOWLEDGEMENT, Action, Certificate, ControlBody, Entry, MemberKind, MembershipEntry,
    Rejection, Resolved, Role, Verdict, resolve,
};
use proptest::prelude::*;
use support::convergence::{self, Author, Config, Generator, Mergeable, Op, OpId, Outcome};

/// The members who write: harness member `m` is `space::Who` `m + 1`.
/// Subjects reach two ids beyond them.
const SUBJECTS: u8 = 6;

/// The genesis by member 1, who then adds members 2 and 3 as admins at
/// once, both at depth 1, and member 4 as a writer.
fn base() -> &'static (SpaceId, Vec<Change>) {
    static BASE: std::sync::OnceLock<(SpaceId, Vec<Change>)> = std::sync::OnceLock::new();
    BASE.get_or_init(|| {
        let mut s = space::Space::with_nonce(1, [6; 16]);
        let g = s.genesis.clone();
        let two = s.add(1, 2, Role::Admin, &[&g]);
        let three = s.add(1, 3, Role::Admin, &[&g]);
        let four = s.add(1, 4, Role::Writer, &[&two, &three]);
        (s.space, vec![g, two, three, four])
    })
}

/// One copy: the changes it holds.
#[derive(Clone, Default)]
struct Copy {
    changes: BTreeMap<OpId, Change>,
    resolved: OnceCell<Resolved>,
}

impl Copy {
    fn all(&self) -> impl Iterator<Item = &Change> {
        base().1.iter().chain(self.changes.values())
    }

    fn resolved(&self) -> &Resolved {
        self.resolved.get_or_init(|| resolve(base().0, self.all()))
    }

    fn highest(&self, who: u8) -> u64 {
        let member = space::id(who);
        let own = self.all().filter(|c| c.header().author == member);
        own.map(|c| c.header().sequence).max().unwrap_or(0)
    }

    /// M-C6 against the reference.
    fn check_reference(&self) {
        let core = self.resolved();
        let nodes = reference::nodes(self.all());
        let valid = reference::valid(&nodes, core);
        let set: BTreeSet<ChangeId> = nodes.keys().copied().collect();
        let genesis = core.genesis().expect("the base's genesis");
        let outcome = reference::reference(&nodes, &set, &valid, genesis);
        assert_eq!(core.seniority(), outcome.seniority, "M-C6: seniority");
        assert_eq!(core.order(), outcome.order, "M-C6: the order");
        for (id, verdict) in &outcome.verdicts {
            assert_eq!(core.verdict(*id), Some(verdict), "M-C6: verdict of {id:?}");
        }
    }
}

impl std::fmt::Debug for Copy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Copy")
            .field("members", &self.resolved().members().collect::<Vec<_>>())
            .field("verdicts", self.resolved().verdicts())
            .finish()
    }
}

/// Whether `change` can be none of the causes of "What a new change can
/// change" for the set `before`: it is no missing dependency there, and it
/// is an acknowledgement or not valid when written in `after`.
fn no_cause(change: &Change, before: &Resolved, after: &Resolved) -> bool {
    if before.gaps().contains(&change.id()) {
        return false;
    }
    let invalid =
        after.verdict(change.id()) == Some(&Verdict::Rejected(Rejection::NotValidWhenWritten));
    let acknowledgement = ControlBody::decode(&change.header().payload).is_ok_and(|body| {
        matches!(body.entry, Entry::Other { entry_type, .. } if entry_type == ACKNOWLEDGEMENT)
    });
    invalid || acknowledgement
}

impl Mergeable for Copy {
    type Change = Change;
    type Read = Vec<(ncdb_core::ids::MemberId, Role)>;

    fn apply(&mut self, op: &Op<Change>) {
        if self.changes.contains_key(&op.id) {
            return;
        }
        let before = self.resolved().clone();
        self.changes.insert(op.id, op.body.clone());
        self.resolved = OnceCell::new();
        let after = self.resolved();
        if no_cause(&op.body, &before, after) {
            for (id, verdict) in before.verdicts() {
                assert_eq!(
                    after.verdict(*id),
                    Some(verdict),
                    "M-C6: {:?}, which can be no listed cause, changed the verdict of {id:?}",
                    op.body.id()
                );
            }
        }
        self.check_reference();
    }

    fn state_bytes(&self) -> Vec<u8> {
        self.resolved().encode()
    }

    fn read(&self) -> Self::Read {
        self.resolved().members().collect()
    }

    fn outcome(&self, id: OpId) -> Outcome {
        let change = &self.changes[&id];
        match self.resolved().verdict(change.id()) {
            Some(Verdict::Accepted) => Outcome::Visible,
            _ => Outcome::Lost,
        }
    }
}

/// What a member does next.
#[derive(Clone, Debug)]
struct Raw {
    /// 0 add, 1 set role, 2 and 3 remove, 4 acknowledge.
    action: u8,
    subject: u8,
    role: u8,
    /// How far below the subject's highest sequence the cutoff lies.
    slack: u8,
    /// Which of the copy's changes the dependencies are: the heads, or a
    /// subset picked by these bits, which leaves later changes out.
    deps: Option<u16>,
}

struct Members;

impl Generator<Copy> for Members {
    type Raw = Raw;

    fn raw(&self, _member: u8) -> BoxedStrategy<Raw> {
        (
            0..5u8,
            1..=SUBJECTS,
            1..=3u8,
            0..2u8,
            prop::option::weighted(0.4, any::<u16>()),
        )
            .prop_map(|(action, subject, role, slack, deps)| Raw {
                action,
                subject,
                role,
                slack,
                deps,
            })
            .boxed()
    }

    fn make(&self, author: &Author<'_, Copy>, raw: &Raw) -> Change {
        let (space, base) = base();
        let copy = author.copy;
        let who = author.member + 1;
        let subject = space::id(raw.subject);
        let role = [Role::Reader, Role::Writer, Role::Admin][usize::from(raw.role - 1)];
        let cutoff = copy
            .highest(raw.subject)
            .saturating_sub(u64::from(raw.slack));
        let first = copy
            .resolved()
            .first_generation()
            .expect("the base's genesis");
        let entry = match raw.action {
            0 => Entry::Membership(MembershipEntry {
                subject,
                action: Action::Add {
                    role,
                    key: space::key(raw.subject).public(),
                    certificate: Box::new(Certificate::sign(
                        &space::account(),
                        subject,
                        MemberKind::Device,
                    )),
                    from_generation: first,
                },
            }),
            1 => Entry::Membership(MembershipEntry {
                subject,
                action: Action::SetRole {
                    role,
                    from_generation: (role >= Role::Writer).then_some(first),
                    cutoff,
                },
            }),
            2 | 3 => Entry::Membership(MembershipEntry {
                subject,
                action: Action::Remove { cutoff },
            }),
            _ => Entry::Other {
                entry_type: ACKNOWLEDGEMENT,
                fields: BTreeMap::from([(8, Value::Unsigned(u64::from(author.id)))]),
            },
        };
        let held: Vec<ChangeId> = copy.all().map(Change::id).collect();
        let dependencies: BTreeSet<ChangeId> = match raw.deps {
            None => {
                let mut heads: BTreeSet<ChangeId> = held.iter().copied().collect();
                for change in copy.all() {
                    if let Ok(body) = ControlBody::decode(&change.header().payload) {
                        for dep in &body.dependencies {
                            heads.remove(dep);
                        }
                    }
                }
                heads
            }
            Some(bits) => {
                let picked = held
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| bits & (1 << (i % 16)) != 0)
                    .map(|(_, id)| *id);
                let mut deps: BTreeSet<ChangeId> = picked.collect();
                deps.insert(base.last().expect("the base").id());
                deps
            }
        };
        let body = ControlBody::new(dependencies, entry);
        let sequence = copy.highest(who) + 1;
        membership::write(&space::key(who), *space, sequence, first, &body)
    }
}

#[test]
fn m_c6_resolve_follows_the_reference_and_verdicts_change_only_for_a_listed_cause() {
    let config = Config {
        members: 4,
        max_changes_per_member: 4,
        cases: 64,
        ..Config::default()
    };
    let result = convergence::check::<Copy, _>(&config, &Members);
    assert!(result.is_ok(), "{}", result.unwrap_err());
}
