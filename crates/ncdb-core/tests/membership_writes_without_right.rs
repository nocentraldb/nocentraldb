//! RFC 0008's M-C7 with the convergence harness: writes without the right
//! never come back, and re-granted writes do count ("How the convergence
//! harness checks it"). The simulated members of
//! `tests/membership_convergence.rs` add, raise, lower, remove and re-grant
//! each other, among them two admins who raise a member to different roles
//! at once. On top of that, every member, revoked or not, keeps writing
//! sealed changes under every generation it holds, often after skipping
//! sequence numbers, as a device offline for a while would, and admins
//! write key chain entries that reuse a generation id the space has.
//!
//! After every delivery, each copy must hold to these consequences of
//! "Sealed changes" and "Generation ids":
//!
//! - a sealed change whose author holds no accepted grant that writes, or
//!   whose generation descends from no such grant's from generation, is not
//!   admitted;
//! - a sealed change above the cutoff of an accepted revocation of its
//!   author's right to write is not admitted, unless an accepted grant that
//!   writes saw that revocation;
//! - a sealed change its author wrote after seeing an accepted re-grant
//!   (a grant that saw a revocation of it), under the re-grant's fresh
//!   generation, is admitted while every accepted revocation of its author
//!   is one that re-grant saw;
//! - a key chain entry that reuses a generation id is not accepted, and a
//!   generation that two key chain entries name is never current.
//!
//! Control changes are not judged here beyond `resolve`'s own verdicts:
//! whether a control change had the right depends on the order of step 4,
//! where a revocation's edge can be skipped to settle a cycle, so the
//! final state alone does not say it (RFC 0008, "Two admins who remove
//! each other"). M-C6 checks the order against a reference.

mod support;

#[path = "support/control.rs"]
mod control;
#[path = "support/space.rs"]
mod space;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use ncdb_core::change::{Change, Kind, Payload};
use ncdb_core::clock::Hlc;
use ncdb_core::ids::ChangeId;
use ncdb_core::membership::{
    self, Action, Certificate, ControlBody, Entry, KeyChainEntry, MemberKind, MembershipEntry,
    PreviousKey, Resolved, Role, Verdict,
};
use ncdb_core::seal;
use proptest::prelude::*;
use support::convergence::{self, Author, Config, Generator, Mergeable, Op, OpId, Outcome};

use control::{Copy, Members, base};

thread_local! {
    /// Per sealed change the generator wrote, the control changes its
    /// author held when it wrote it: what it had seen.
    static SEEN: RefCell<BTreeMap<ChangeId, BTreeSet<ChangeId>>> =
        const { RefCell::new(BTreeMap::new()) };
}

/// One copy: the members' copy of `tests/membership_convergence.rs`, whose
/// every delivery is checked.
#[derive(Clone, Debug, Default)]
struct Checked(Copy);

/// Whether `role` writes sealed changes.
fn writes(role: Option<Role>) -> bool {
    role >= Some(Role::Writer)
}

/// The first M-C7 consequence `change`, a sealed change the copy holds,
/// breaks in `resolved`, if any.
fn broken(resolved: &Resolved, change: &Change) -> Option<String> {
    let header = change.header();
    let admitted = resolved.admits(header);
    let state = resolved.member(header.author);
    let grants: Vec<_> = state
        .into_iter()
        .flat_map(|s| &s.grants)
        .filter(|g| writes(Some(g.role)))
        .collect();
    let revocations: Vec<_> = state
        .into_iter()
        .flat_map(|s| &s.revocations)
        .filter(|r| !writes(r.role))
        .collect();
    let saw = |grant: ChangeId, revocation: ChangeId| {
        resolved
            .seen()
            .get(&grant)
            .is_some_and(|seen| seen.contains(&revocation))
    };
    if admitted && grants.is_empty() {
        return Some("admitted without an accepted grant that writes".into());
    }
    let descends = grants.iter().any(|g| {
        g.from_generation
            .is_some_and(|from| resolved.descends(header.key_generation, from))
    });
    if admitted && !descends {
        return Some("admitted under a generation no grant that writes counts from".into());
    }
    for revocation in &revocations {
        let re_granted = grants.iter().any(|g| saw(g.change, revocation.change));
        if admitted && header.sequence > revocation.cutoff && !re_granted {
            return Some(format!(
                "admitted above the cutoff {} of the revocation {:?}, which no grant saw",
                revocation.cutoff, revocation.change
            ));
        }
    }
    let held = SEEN.with(|seen| seen.borrow().get(&change.id()).cloned())?;
    for grant in &grants {
        let regrant = revocations.iter().any(|r| saw(grant.change, r.change));
        let covers_all = revocations.iter().all(|r| saw(grant.change, r.change));
        let fresh = grant.from_generation == Some(header.key_generation);
        if regrant && covers_all && fresh && held.contains(&grant.change) && !admitted {
            return Some(format!(
                "not admitted, though written after the re-grant {:?} under its generation",
                grant.change
            ));
        }
    }
    None
}

impl Checked {
    /// M-C7 on the copy as it stands.
    fn check(&self) {
        let resolved = self.0.resolved();
        for change in self.0.changes.values() {
            if change.header().kind == Kind::SEALED
                && let Some(why) = broken(resolved, change)
            {
                panic!("M-C7: sealed change {:?}: {why}", change.id());
            }
        }
        // An entry whose id is not the one its author and previous
        // generations derive is never valid; one that matches can stand
        // alone on a copy that does not hold the other entry yet.
        let (space, _, _) = base();
        for change in self.0.changes.values() {
            let Ok(body) = ControlBody::decode(&change.header().payload) else {
                continue;
            };
            let Entry::KeyChain(chain) = body.entry else {
                continue;
            };
            let previous: Vec<u64> = chain.previous.iter().map(|p| p.generation).collect();
            let derived = membership::generation_id(*space, change.header().author, &previous);
            if chain.generation != derived {
                assert_ne!(
                    resolved.verdict(change.id()),
                    Some(&Verdict::Accepted),
                    "M-C7: key chain entry {:?} names another's generation id and is accepted",
                    change.id()
                );
            }
        }
        for (id, generation) in resolved.generations() {
            if generation.entries.len() < 2 {
                continue;
            }
            assert_ne!(
                resolved.current_generation(),
                Some(id),
                "M-C7: generation {id:x}, which two key chain entries name, is current"
            );
            for entry in generation.entries.keys() {
                assert_ne!(
                    resolved.verdict(*entry),
                    Some(&Verdict::Accepted),
                    "M-C7: key chain entry {entry:?} reuses generation {id:x} and is accepted"
                );
            }
        }
    }
}

impl Mergeable for Checked {
    type Change = Change;
    type Read = control::Read;

    fn apply(&mut self, op: &Op<Change>) {
        self.0.apply(op);
        self.check();
    }

    fn state_bytes(&self) -> Vec<u8> {
        self.0.state_bytes()
    }

    fn read(&self) -> control::Read {
        self.0.read()
    }

    fn outcome(&self, id: OpId) -> Outcome {
        self.0.outcome(id)
    }
}

/// What a member does next: what the members of
/// `tests/membership_convergence.rs` do, or, by `extra`, a sealed change
/// under a generation it holds or a key chain entry that reuses a
/// generation id. `pick` chooses the generation and the sequences skipped.
#[derive(Clone, Debug)]
struct Raw {
    members: control::Raw,
    extra: u8,
    pick: u8,
}

/// The generators of M-C7.
struct Writers(Members);

impl Writers {
    /// The generations `author` holds in its copy: the first, those it
    /// rotated to, those an envelope wraps to it, and the from generations
    /// of its grants, a re-grant's fresh one among them, whose key an
    /// envelope after the grant brings.
    fn held(copy: &Copy, author: u8) -> Vec<u64> {
        let (_, first, _) = base();
        let me = space::id(author);
        let resolved = copy.resolved();
        let mut held: BTreeSet<u64> = resolved
            .generations()
            .filter(|(_, g)| g.recipients.contains(&me) || g.entries.values().any(|a| *a == me))
            .map(|(id, _)| id)
            .collect();
        let grants = resolved.member(me).into_iter().flat_map(|s| &s.grants);
        held.extend(grants.filter_map(|g| g.from_generation));
        held.insert(*first);
        held.into_iter().collect()
    }

    /// A sealed change by `who` under a generation it holds, after
    /// skipping up to three sequence numbers.
    fn sealed(copy: &Copy, who: u8, id: OpId, pick: u8) -> Change {
        let held = Writers::held(copy, who);
        let generation = held[usize::from(pick) % held.len()];
        Writers::sealed_under(copy, who, id, pick, generation)
    }

    /// A sealed change by `who` under the from generation of its newest
    /// grant in its copy, as a re-granted member writes once it holds the
    /// fresh generation's key.
    fn sealed_after_grant(copy: &Copy, who: u8, id: OpId, pick: u8) -> Change {
        let grants = copy.resolved().member(space::id(who)).map(|s| &s.grants);
        let newest = grants.and_then(|g| g.iter().rev().find_map(|g| g.from_generation));
        match newest {
            Some(generation) => Writers::sealed_under(copy, who, id, pick, generation),
            None => Writers::sealed(copy, who, id, pick),
        }
    }

    fn sealed_under(copy: &Copy, who: u8, id: OpId, pick: u8, generation: u64) -> Change {
        let (space, _, _) = base();
        let sequence = copy.highest(space::id(who)) + 1 + u64::from(pick % 4);
        let clock = Hlc((sequence << 16) | u64::from(id & 0xffff));
        let payload = Payload::new(clock, BTreeSet::new(), vec![]);
        let mut nonce = [who ^ 0x40; 24];
        nonce[..4].copy_from_slice(&id.to_le_bytes());
        seal::seal_with_nonce(
            &space::key(who),
            &space::space_key(),
            *space,
            sequence,
            generation,
            &payload,
            nonce,
        )
    }

    /// A key chain entry by `who` that names a generation the copy holds
    /// already. When `who` rotated before, it rotates again from the same
    /// previous generations, so the entry is valid when written and names
    /// the same id (RFC 0008, "Generation ids"); otherwise it names another
    /// author's generation, which is not valid when written.
    fn reuse(copy: &Copy, who: u8, pick: u8) -> Change {
        let (space, first, _) = base();
        let resolved = copy.resolved();
        let me = space::id(who);
        let own: Vec<(u64, BTreeSet<u64>)> = resolved
            .generations()
            .filter(|(_, g)| g.entries.values().any(|a| *a == me))
            .map(|(id, g)| (id, g.previous.clone()))
            .collect();
        let (generation, previous) = match own.is_empty() {
            false => own[usize::from(pick) % own.len()].clone(),
            true => {
                let all: Vec<u64> = resolved.generations().map(|(id, _)| id).collect();
                (all[usize::from(pick) % all.len()], resolved.heads())
            }
        };
        let entry = Entry::KeyChain(KeyChainEntry {
            generation,
            previous: previous
                .into_iter()
                .map(|generation| PreviousKey {
                    generation,
                    key: [0xc4; 72],
                })
                .collect(),
            answers: BTreeSet::new(),
        });
        let body = ControlBody::new(copy.heads(), entry);
        let sequence = copy.highest(me) + 1;
        membership::write(&space::key(who), *space, sequence, *first, &body)
    }
}

impl Writers {
    /// The next step by admin `who` towards re-granting member `subject`
    /// the right to write, from its copy: revoke it while it writes, then
    /// rotate to a fresh generation, then grant it a role that writes from
    /// that generation, which sees the revocation (RFC 0008, "A member
    /// demoted and raised again"). `pick` chooses between a demotion and a
    /// removal, the cutoff's slack and the role of the grant.
    fn re_grant_step(copy: &Copy, who: u8, subject: u8, pick: u8) -> Change {
        let (space, first, _) = base();
        let resolved = copy.resolved();
        let me = space::id(who);
        let target = space::id(subject);
        let role = resolved.role(target);
        let heads = copy.heads();
        let sequence = copy.highest(me) + 1;
        let write = |entry: Entry| {
            let body = ControlBody::new(heads.clone(), entry);
            membership::write(&space::key(who), *space, sequence, *first, &body)
        };
        let membership = |action: Action| {
            Entry::Membership(MembershipEntry {
                subject: target,
                action,
            })
        };
        if writes(role) {
            let cutoff = copy.highest(target).saturating_sub(u64::from(pick % 2));
            return write(membership(match pick & 2 {
                0 => Action::Remove { cutoff },
                _ => Action::SetRole {
                    role: Role::Reader,
                    from_generation: None,
                    cutoff,
                },
            }));
        }
        let fresh = copy.generation_for(me, target);
        let rotated = resolved
            .generation(fresh)
            .is_some_and(|g| g.entries.values().any(|a| *a == me));
        if !rotated {
            let previous: Vec<u64> = resolved.heads().into_iter().collect();
            return write(Entry::KeyChain(KeyChainEntry {
                generation: membership::generation_id(*space, me, &previous),
                previous: previous
                    .into_iter()
                    .map(|generation| PreviousKey {
                        generation,
                        key: [0xc4; 72],
                    })
                    .collect(),
                answers: BTreeSet::new(),
            }));
        }
        let granted = [Role::Writer, Role::Admin][usize::from(pick >> 7)];
        write(membership(match role {
            None => Action::Add {
                role: granted,
                key: space::key(subject).public(),
                certificate: Box::new(Certificate::sign(
                    &space::account(),
                    target,
                    MemberKind::Device,
                )),
                from_generation: fresh,
            },
            Some(_) => Action::SetRole {
                role: granted,
                from_generation: Some(fresh),
                cutoff: copy.highest(target),
            },
        }))
    }
}

impl Generator<Checked> for Writers {
    type Raw = Raw;

    fn raw(&self, member: u8) -> BoxedStrategy<Raw> {
        (
            Generator::<Copy>::raw(&self.0, member),
            0..12u8,
            any::<u8>(),
        )
            .prop_map(|(members, extra, pick)| Raw {
                members,
                extra,
                pick,
            })
            .boxed()
    }

    fn make(&self, author: &Author<'_, Checked>, raw: &Raw) -> Change {
        let copy = &author.copy.0;
        let who = author.member + 1;
        let change = match raw.extra {
            0..=4 => {
                let inner = Author {
                    member: author.member,
                    id: author.id,
                    copy,
                    seen: &[],
                };
                self.0.make(&inner, &raw.members)
            }
            5..=7 => Writers::sealed(copy, who, author.id, raw.pick),
            8 => Writers::reuse(copy, who, raw.pick),
            // Members 3 and 4, an admin and the writer, are re-granted by
            // the others.
            _ => {
                let subject = 3 + raw.pick % 2;
                match who == subject {
                    true => Writers::sealed_after_grant(copy, who, author.id, raw.pick),
                    false => Writers::re_grant_step(copy, who, subject, raw.pick),
                }
            }
        };
        if change.header().kind == Kind::SEALED {
            let (_, _, base) = base();
            let held = base.iter().chain(copy.changes.values()).map(Change::id);
            SEEN.with(|seen| seen.borrow_mut().insert(change.id(), held.collect()));
        }
        change
    }
}

/// Four members: the creator, two admins and a writer, who is the one
/// most often revoked, raised and re-granted.
fn config() -> Config {
    Config {
        members: 4,
        max_changes_per_member: 5,
        max_learned: 16,
        cases: 48,
        ..Config::default()
    }
}

#[test]
fn m_c7_no_sealed_change_without_the_right_is_admitted_and_re_granted_writes_count() {
    let writers = Writers(Members {
        forks: false,
        shreds: false,
    });
    let result = convergence::check::<Checked, _>(&config(), &writers);
    assert!(result.is_ok(), "{}", result.unwrap_err());
}

/// The same with equivocations: control and sealed changes that reuse an
/// author sequence, so that a revoked member's fork lands on both sides of
/// a cutoff.
#[test]
fn m_c7_with_equivocations_no_sealed_change_without_the_right_is_admitted() {
    let writers = Writers(Members {
        forks: true,
        shreds: false,
    });
    let result = convergence::check::<Checked, _>(&config(), &writers);
    assert!(result.is_ok(), "{}", result.unwrap_err());
}
