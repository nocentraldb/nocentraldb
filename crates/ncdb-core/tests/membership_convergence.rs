//! RFC 0008's M-C1 and M-C2 with the convergence harness (NOC-9, NOC-19):
//! the same control changes, in any order of arrival and with duplicates,
//! resolve to the same membership state and the same verdicts on every
//! copy. Simulated members write genesis-based membership entries, key
//! chain entries, key envelopes and entries of the other types from their
//! own view of the space, so that concurrent grants, revocations, cutoffs,
//! cycles of removals, rotations and re-grants under a fresh generation
//! arise (NOC-71).

mod support;

#[path = "support/space.rs"]
mod space;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::cell::OnceCell;
use std::collections::{BTreeMap, BTreeSet};

use ncdb_core::change::Change;
use ncdb_core::envelope;
use ncdb_core::ids::{ChangeId, MemberId, SpaceId};
use ncdb_core::membership::{
    self, Action, Certificate, ControlBody, Entry, KeyChainEntry, MemberKind, MembershipEntry,
    PreviousKey, Resolved, Role, Verdict, resolve,
};
use proptest::prelude::*;
use support::convergence::{self, Author, Config, Generator, Mergeable, Op, OpId, Outcome};

/// The simulated members: harness member `m` is `space::Who` `m + 1`.
/// Subjects reach two ids beyond the writers, so some members are added
/// that never write.
const SUBJECTS: u8 = 6;

/// What every copy starts from: the genesis by harness member 0, which
/// then adds members 1 and 2 as admins and member 3 as a writer, so that
/// every simulated member can write from the start.
fn base() -> &'static (SpaceId, u64, Vec<Change>) {
    static BASE: std::sync::OnceLock<(SpaceId, u64, Vec<Change>)> = std::sync::OnceLock::new();
    BASE.get_or_init(|| {
        let mut s = space::Space::with_nonce(1, [9; 16]);
        let g = s.genesis.clone();
        let two = s.add(1, 2, Role::Admin, &[&g]);
        let three = s.add(1, 3, Role::Admin, &[&two]);
        let four = s.add(1, 4, Role::Writer, &[&three]);
        (s.space, s.first_generation, vec![g, two, three, four])
    })
}

/// One copy: the control changes it holds, and their fold, computed once.
#[derive(Clone, Default)]
struct Copy {
    changes: BTreeMap<OpId, Change>,
    resolved: OnceCell<Resolved>,
}

impl Copy {
    fn resolved(&self) -> &Resolved {
        self.resolved.get_or_init(|| {
            let (space, _, base) = base();
            resolve(*space, base.iter().chain(self.changes.values()))
        })
    }

    /// The heads of the copy's control DAG: the changes no other names.
    fn heads(&self) -> BTreeSet<ChangeId> {
        let mut heads: BTreeSet<ChangeId> = self.changes.values().map(Change::id).collect();
        heads.insert(base().2.last().expect("the base").id());
        for change in self.changes.values() {
            if let Ok(body) = ControlBody::decode(&change.header().payload) {
                for dep in &body.dependencies {
                    heads.remove(dep);
                }
            }
        }
        heads
    }

    /// The heads of the copy's generation graph: the generations no other
    /// lists as previous.
    fn generation_heads(&self) -> Vec<u64> {
        let generations: Vec<_> = self.resolved().generations().collect();
        let listed: BTreeSet<u64> = generations
            .iter()
            .flat_map(|(_, g)| g.previous.iter().copied())
            .collect();
        let ids = generations.iter().map(|(id, _)| *id);
        ids.filter(|id| !listed.contains(id)).collect()
    }

    /// The newest generation `author` rotated to with no envelope to
    /// `subject` in the copy, which a re-grant may name; otherwise the
    /// newest generation of the copy.
    fn generation_for(&self, author: MemberId, subject: MemberId) -> u64 {
        let generations: Vec<_> = self.resolved().generations().collect();
        let fresh = generations.iter().filter(|(_, g)| {
            g.entries.values().any(|by| *by == author) && !g.recipients.contains(&subject)
        });
        let newest = generations.iter().map(|(id, _)| *id).max();
        fresh
            .map(|(id, _)| *id)
            .max()
            .or(newest)
            .expect("the first generation")
    }

    /// The highest author sequence of `member` the copy holds.
    fn highest(&self, member: MemberId) -> u64 {
        let own = base().2.iter().chain(self.changes.values());
        own.filter(|c| c.header().author == member)
            .map(|c| c.header().sequence)
            .max()
            .unwrap_or(0)
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

impl Mergeable for Copy {
    type Change = Change;
    type Read = Vec<(MemberId, Role)>;

    fn apply(&mut self, op: &Op<Change>) {
        if let std::collections::btree_map::Entry::Vacant(entry) = self.changes.entry(op.id) {
            entry.insert(op.body.clone());
            self.resolved = OnceCell::new();
        }
    }

    fn state_bytes(&self) -> Vec<u8> {
        self.resolved().encode()
    }

    fn read(&self) -> Vec<(MemberId, Role)> {
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

/// What a member does next, before its view decides the details.
#[derive(Clone, Debug)]
struct Raw {
    action: u8,
    subject: u8,
    role: u8,
    /// How far below the subject's highest sequence the cutoff lies.
    slack: u8,
    /// Sequences the member uses in between, as sealed changes would.
    skip: u8,
}

/// Members that add, raise, lower and remove each other, leave, and write
/// entries of the other types, all from their own copy.
struct Members;

impl Generator<Copy> for Members {
    type Raw = Raw;

    fn raw(&self, _member: u8) -> BoxedStrategy<Raw> {
        (0..9u8, 2..=SUBJECTS, 1..=3u8, 0..2u8, 0..2u8)
            .prop_map(|(action, subject, role, slack, skip)| Raw {
                action,
                subject,
                role,
                slack,
                skip,
            })
            .boxed()
    }

    fn make(&self, author: &Author<'_, Copy>, raw: &Raw) -> Change {
        let (space, first_generation, _) = *base();
        let who = author.member + 1;
        let copy = author.copy;
        let subject = space::id(raw.subject);
        let role = [Role::Reader, Role::Writer, Role::Admin][usize::from(raw.role - 1)];
        let cutoff = copy.highest(subject).saturating_sub(u64::from(raw.slack));
        let generation = copy.generation_for(space::id(who), subject);
        let from_generation = (role >= Role::Writer).then_some(generation);
        let action = match raw.action {
            0 | 1 => Some(Action::Add {
                role,
                key: space::key(raw.subject).public(),
                certificate: Box::new(Certificate::sign(
                    &space::account(),
                    subject,
                    MemberKind::Device,
                )),
                from_generation: generation,
            }),
            2 => Some(Action::SetRole {
                role,
                from_generation,
                cutoff,
            }),
            3 => Some(Action::Remove { cutoff }),
            _ => None,
        };
        let entry = match (action, raw.action) {
            (Some(action), _) => Entry::Membership(MembershipEntry { subject, action }),
            (None, 4) => Entry::Membership(MembershipEntry {
                subject: space::id(who),
                action: Action::Remove {
                    cutoff: copy.highest(space::id(who)),
                },
            }),
            (None, 7) => {
                let previous = copy.generation_heads();
                Entry::KeyChain(KeyChainEntry {
                    generation: membership::generation_id(space, space::id(who), &previous),
                    previous: previous
                        .into_iter()
                        .map(|generation| PreviousKey {
                            generation,
                            key: vec![0xc4; 72],
                        })
                        .collect(),
                    answers: BTreeSet::new(),
                })
            }
            (None, 8) => Entry::KeyEnvelope(
                envelope::seal_with_ephemeral(
                    &space::space_key(),
                    space,
                    generation,
                    &space::key(raw.subject).public(),
                    [who ^ (raw.subject << 4); 32],
                )
                .expect("a simulated member's key takes an envelope"),
            ),
            (None, n) => Entry::Other {
                entry_type: [membership::ACKNOWLEDGEMENT, membership::PERSON_KEY]
                    [usize::from(n % 2)],
                fields: BTreeMap::from([(8, ncdb_core::cbor::Value::Unsigned(u64::from(n)))]),
            },
        };
        let body = ControlBody::new(copy.heads(), entry);
        let sequence = copy.highest(space::id(who)) + 1 + u64::from(raw.skip);
        membership::write(&space::key(who), space, sequence, first_generation, &body)
    }
}

#[test]
fn m_c1_the_same_control_changes_resolve_to_the_same_state_in_any_order() {
    let config = Config {
        members: 3,
        max_changes_per_member: 3,
        cases: 48,
        ..Config::default()
    };
    let result = convergence::check::<Copy, _>(&config, &Members);
    assert!(result.is_ok(), "{}", result.unwrap_err());
}

/// Every order of `items`.
fn permutations<T: Clone>(items: &[T]) -> Vec<Vec<T>> {
    if items.len() <= 1 {
        return vec![items.to_vec()];
    }
    let mut all = Vec::new();
    for i in 0..items.len() {
        let mut rest = items.to_vec();
        let first = rest.remove(i);
        for mut tail in permutations(&rest) {
            tail.insert(0, first.clone());
            all.push(tail);
        }
    }
    all
}

/// The state of `base` and then `changes`, these arriving in every order:
/// one state, or the test fails.
fn in_every_order(space: SpaceId, base: &[Change], changes: &[Change]) -> Resolved {
    let first = resolve(space, base.iter().chain(changes));
    for order in permutations(changes) {
        let state = resolve(space, base.iter().chain(&order));
        assert_eq!(state.encode(), first.encode());
    }
    first
}

#[test]
fn a_re_grant_and_revocations_end_in_the_same_membership_in_any_order() {
    const ALICE: u8 = 1;
    const BOB: u8 = 2;
    const DAN: u8 = 4;
    let mut s = space::Space::with_nonce(ALICE, [9; 16]);
    let g = s.genesis.clone();
    let first = s.first_generation;
    let bob = s.add(ALICE, BOB, Role::Admin, &[&g]);
    let dan = s.add(ALICE, DAN, Role::Writer, &[&bob]);
    let cutoff = s.sequence(DAN);
    // Alice removes Dan, rotates, re-adds him under the fresh generation
    // and wraps it to him. Bob removes Dan too: once concurrently, once
    // after he has seen the re-add.
    let remove = s.remove(ALICE, DAN, cutoff, &[&dan]);
    let (rotation, fresh) = s.rotate(ALICE, &[first], &[&remove]);
    let again = s.add_at(ALICE, DAN, Role::Writer, fresh, &[&rotation]);
    let to_dan = s.envelope_at(ALICE, DAN, fresh, &[&again]);
    let concurrent = s.remove(BOB, DAN, cutoff, &[&dan]);
    let after = s.remove(BOB, DAN, cutoff, &[&again]);
    let base = [g, bob, dan];
    let set = |extra: &[&Change]| -> Vec<Change> {
        let tail = [&remove, &rotation, &again, &to_dan];
        tail.into_iter()
            .chain(extra.iter().copied())
            .cloned()
            .collect()
    };

    let re_granted = in_every_order(s.space, &base, &set(&[]));
    assert_eq!(re_granted.verdict(again.id()), Some(&Verdict::Accepted));
    assert_eq!(re_granted.role(space::id(DAN)), Some(Role::Writer));

    let lost = in_every_order(s.space, &base, &set(&[&concurrent]));
    assert_eq!(lost.verdict(again.id()), Some(&Verdict::Lost));
    assert_eq!(lost.role(space::id(DAN)), None);

    let removed = in_every_order(s.space, &base, &set(&[&after]));
    assert_eq!(removed.verdict(again.id()), Some(&Verdict::Lost));
    assert_eq!(removed.verdict(after.id()), Some(&Verdict::Accepted));
    assert_eq!(removed.role(space::id(DAN)), None);
}
