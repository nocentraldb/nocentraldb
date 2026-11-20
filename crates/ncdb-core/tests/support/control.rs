//! Simulated members that write control changes and sealed changes from
//! their own copy of a space, for the convergence harness (RFC 0008, "How
//! the convergence harness checks it"): the base every copy starts from,
//! a copy that resolves what it holds ([`Copy`]), and the generator of the
//! members' changes ([`Members`]). `tests/membership_convergence.rs` runs
//! them for M-C1, M-C2, M-C3 and M-C8; other files reuse them.
//!
//! Each test file includes this module with `#[path]`, next to
//! `support/space.rs` as `space`; not every file uses every helper.
#![allow(dead_code)]

use std::cell::OnceCell;
use std::collections::{BTreeMap, BTreeSet};

use ncdb_core::change::{Change, Kind, Payload};
use ncdb_core::clock::Hlc;
use ncdb_core::envelope;
use ncdb_core::ids::{ChangeId, MemberId, SpaceId};
use ncdb_core::membership::{
    self, Action, Certificate, ControlBody, Entry, KeyChainEntry, MemberKind, MembershipEntry,
    PersonKeyEntry, PreviousKey, Resolved, Role, ShredEntry, SignOff, Verdict, resolve,
};
use ncdb_core::seal;
use proptest::prelude::*;

use crate::space;
use crate::support::convergence::{Author, Generator, Mergeable, Op, OpId, Outcome};

/// The simulated members: harness member `m` is `space::Who` `m + 1`.
/// Subjects reach two ids beyond the writers, so some members are added
/// that never write.
pub const SUBJECTS: u8 = 6;

/// What every copy starts from: the genesis by harness member 0, which
/// then adds members 1 and 2 as admins and member 3 as a writer, so that
/// every simulated member can write from the start, and writes the keys of
/// persons 1 and 2.
pub fn base() -> &'static (SpaceId, u64, Vec<Change>) {
    static BASE: std::sync::OnceLock<(SpaceId, u64, Vec<Change>)> = std::sync::OnceLock::new();
    BASE.get_or_init(|| {
        let mut s = space::Space::with_nonce(1, [9; 16]);
        // The genesis seals its model id under a fixed nonce rather than a
        // random one, so that every run, native or wasm32, writes the same
        // changes with the same ids (M-C3).
        let mut g = s.genesis.clone();
        let mut body = ControlBody::decode(&g.header().payload).expect("the genesis body");
        if let Entry::Genesis(genesis) = &mut body.entry {
            genesis.model_id = membership::seal_model_id_with_nonce(
                &space::space_key(),
                s.space,
                space::id(1),
                space::MODEL,
                [9; 24],
            );
        }
        g = membership::write(&space::key(1), s.space, 1, s.first_generation, &body);
        s.genesis = g.clone();
        let two = s.add(1, 2, Role::Admin, &[&g]);
        // Member 3's account is a second account, which signs off shreds.
        s.certify_by(3, space::other_account(1));
        let three = s.add(1, 3, Role::Admin, &[&two]);
        let four = s.add(1, 4, Role::Writer, &[&three]);
        // Persons 1 and 2 have keys, so that a shred can be valid from the
        // start (M-C8).
        let one = s.person_key(1, 1, 1, &[&four]);
        let other = s.person_key(1, 2, 2, &[&one]);
        (
            s.space,
            s.first_generation,
            vec![g, two, three, four, one, other],
        )
    })
}

/// One copy: the control changes it holds, and their fold, computed once.
#[derive(Clone, Default)]
pub struct Copy {
    pub changes: BTreeMap<OpId, Change>,
    resolved: OnceCell<Resolved>,
}

impl Copy {
    pub fn resolved(&self) -> &Resolved {
        self.resolved.get_or_init(|| {
            let (space, _, base) = base();
            resolve(*space, base.iter().chain(self.changes.values()))
        })
    }

    /// The heads of the copy's control DAG: the control changes no other
    /// names.
    pub fn heads(&self) -> BTreeSet<ChangeId> {
        let mut heads: BTreeSet<ChangeId> = self
            .changes
            .values()
            .filter(|c| c.header().kind == Kind::CONTROL)
            .map(Change::id)
            .collect();
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

    /// The newest generation `author` rotated to with no envelope to
    /// `subject` in the copy, which a re-grant may name; otherwise the
    /// newest generation of the copy.
    pub fn generation_for(&self, author: MemberId, subject: MemberId) -> u64 {
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
    pub fn highest(&self, member: MemberId) -> u64 {
        let own = base().2.iter().chain(self.changes.values());
        own.filter(|c| c.header().author == member)
            .map(|c| c.header().sequence)
            .max()
            .unwrap_or(0)
    }

    /// The shred entries the copy accepts.
    pub fn accepted_shreds(&self) -> BTreeSet<ChangeId> {
        let resolved = self.resolved();
        self.changes
            .values()
            .filter(|c| c.header().kind == Kind::CONTROL)
            .filter(|c| {
                ControlBody::decode(&c.header().payload)
                    .is_ok_and(|body| matches!(body.entry, Entry::Shred(_)))
            })
            .map(Change::id)
            .filter(|id| resolved.verdict(*id) == Some(&Verdict::Accepted))
            .collect()
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
    type Read = Read;

    /// Adds a change. RFC 0008's M-C8: a shred the copy accepted stays
    /// accepted whatever arrives next (RFC 0008, "A shred is final when
    /// accepted"). Copies grow one change at a time in every order the
    /// harness draws, equivocations and cutoffs that leave a shred out
    /// included, so each step is checked.
    fn apply(&mut self, op: &Op<Change>) {
        if !self.changes.contains_key(&op.id) {
            let shreds = self.accepted_shreds();
            self.changes.insert(op.id, op.body.clone());
            self.resolved = OnceCell::new();
            let after = self.accepted_shreds();
            let lost: Vec<_> = shreds.difference(&after).collect();
            assert!(
                lost.is_empty(),
                "M-C8: accepted shreds {lost:?} were rejected later"
            );
        }
    }

    fn state_bytes(&self) -> Vec<u8> {
        self.resolved().encode()
    }

    fn read(&self) -> Read {
        let resolved = self.resolved();
        let current = resolved.current_generation();
        Read {
            members: resolved.members().collect(),
            current,
            exposed: current.is_some_and(|c| resolved.exposed(c)),
            heads: resolved.heads(),
            admitted: self
                .changes
                .values()
                .filter(|c| c.header().kind == Kind::SEALED && resolved.admits(c.header()))
                .map(Change::id)
                .collect(),
        }
    }

    fn outcome(&self, id: OpId) -> Outcome {
        let change = &self.changes[&id];
        let visible = match change.header().kind {
            Kind::SEALED => self.resolved().admits(change.header()),
            _ => self.resolved().verdict(change.id()) == Some(&Verdict::Accepted),
        };
        match visible {
            true => Outcome::Visible,
            false => Outcome::Lost,
        }
    }
}

/// What the SDK reads of a copy: its members, its current generation and
/// whether that is exposed, the heads a merge rotation would list, and the
/// sealed changes the membership rule admits.
#[derive(Debug, PartialEq)]
pub struct Read {
    members: Vec<(MemberId, Role)>,
    current: Option<u64>,
    exposed: bool,
    heads: BTreeSet<u64>,
    admitted: BTreeSet<ChangeId>,
}

/// What a member does next, before its view decides the details.
#[derive(Clone, Debug)]
pub struct Raw {
    pub action: u8,
    pub subject: u8,
    pub role: u8,
    /// How far below the subject's highest sequence the cutoff lies.
    pub slack: u8,
    /// Sequences the member uses in between, as sealed changes would.
    pub skip: u8,
    /// What kind of change it is: 0 to 3 a control change under the next
    /// sequence, 4 a control change that equivocates, 5 a sealed change, 6
    /// a sealed change that equivocates.
    pub kind: u8,
    /// For an equivocation, how far below the member's highest sequence the
    /// sequence it reuses lies.
    pub back: u8,
    /// For an equivocating control change, its dependencies are the base's
    /// last change only, rather than the heads of the member's copy.
    pub bare: bool,
}

/// Members that add, raise, lower and remove each other, leave, rotate
/// (with a merge rotation when their current generation is exposed), wrap
/// keys, and write entries of the other types, all from their own copy.
/// `forks` makes most changes equivocations or sealed changes. `shreds`
/// makes every change a person-key entry, a shred or a revocation of the
/// two writers that perform shreds, so that revocations their copies did
/// not see meet accepted shreds (M-C8).
pub struct Members {
    pub forks: bool,
    pub shreds: bool,
}

impl Generator<Copy> for Members {
    type Raw = Raw;

    fn raw(&self, _member: u8) -> BoxedStrategy<Raw> {
        let kinds = match self.forks {
            true => 3..7u8,
            false => 0..7u8,
        };
        let (actions, subjects) = match self.shreds {
            true => (
                prop_oneof![Just(2u8), Just(3), Just(5), Just(6), Just(6)].boxed(),
                (2..=3u8).boxed(),
            ),
            false => ((0..9u8).boxed(), (2..=SUBJECTS).boxed()),
        };
        (
            actions,
            subjects,
            1..=3u8,
            0..2u8,
            0..2u8,
            kinds,
            0..3u8,
            any::<bool>(),
        )
            .prop_map(
                |(action, subject, role, slack, skip, kind, back, bare)| Raw {
                    action,
                    subject,
                    role,
                    slack,
                    skip,
                    kind,
                    back,
                    bare,
                },
            )
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
                if let Some(merge) = copy.resolved().merge_rotation(space::id(who)) {
                    let key = space::space_key();
                    let entry = merge.entry(&key, |_| Some(&key));
                    Entry::KeyChain(entry.expect("every simulated generation has the same key"))
                } else {
                    let previous: Vec<u64> = copy.resolved().heads().into_iter().collect();
                    Entry::KeyChain(KeyChainEntry {
                        generation: membership::generation_id(space, space::id(who), &previous),
                        previous: previous
                            .into_iter()
                            .map(|generation| PreviousKey {
                                generation,
                                key: [0xc4; 72],
                            })
                            .collect(),
                        answers: BTreeSet::new(),
                    })
                }
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
            // A record of person 1 or 2, under the generation the member
            // writes with, committing to key 1 or 2 by the role drawn.
            (None, 5) => Entry::PersonKey(PersonKeyEntry {
                person_key: space::person(raw.subject % 2 + 1),
                commitment: [raw.role % 2 + 1; 32],
                generation,
                record_hash: [raw.subject; 32],
            }),
            // A shred of person 1 or 2, signed off by member 3's account.
            (None, 6) if raw.slack == 0 => {
                let person_key = space::person(raw.subject % 2 + 1);
                Entry::Shred(ShredEntry {
                    person_key,
                    sign_off: SignOff::sign(
                        &space::other_account(1),
                        space,
                        person_key,
                        space::id(who),
                    ),
                })
            }
            (None, n) => Entry::Other {
                entry_type: membership::ACKNOWLEDGEMENT,
                fields: BTreeMap::from([(8, ncdb_core::cbor::Value::Unsigned(u64::from(n)))]),
            },
        };
        // An equivocation reuses a sequence the member already wrote under,
        // as a device restored from a backup or a stolen one would; a member
        // that wrote nothing yet writes its first change instead.
        let own = copy.highest(space::id(who));
        let equivocates = matches!(raw.kind, 4 | 6) && own > 0;
        let sequence = match equivocates {
            true => own.saturating_sub(u64::from(raw.back)).max(1),
            false => own + 1 + u64::from(raw.skip),
        };
        if matches!(raw.kind, 5 | 6) {
            // The clock carries the operation's id, so that a sealed change
            // that reuses a sequence differs from the first one.
            let clock = Hlc((sequence << 16) | u64::from(author.id & 0xffff));
            let payload = Payload::new(clock, BTreeSet::new(), vec![]);
            // A nonce from the operation's id, unique in a case, rather
            // than a random one, so that every run writes the same changes
            // (M-C3).
            let mut nonce = [who; 24];
            nonce[..4].copy_from_slice(&author.id.to_le_bytes());
            return seal::seal_with_nonce(
                &space::key(who),
                &space::space_key(),
                space,
                sequence,
                generation,
                &payload,
                nonce,
            );
        }
        let dependencies = match equivocates && raw.bare {
            true => BTreeSet::from([base().2.last().expect("the base").id()]),
            false => copy.heads(),
        };
        let body = ControlBody::new(dependencies, entry);
        membership::write(&space::key(who), space, sequence, first_generation, &body)
    }
}
