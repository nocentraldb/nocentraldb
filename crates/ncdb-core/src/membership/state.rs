//! The membership state that [`super::resolve`] computes (RFC 0008, "The
//! state"), with the role rule of "Roles" and the canonical encoding the
//! convergence harness compares.

use std::collections::{BTreeMap, BTreeSet};

use crate::cbor::{self, Value};
use crate::ids::{ChangeId, MemberId};
use crate::keys::MemberPublicKey;

use super::body::Role;
use super::certificate::Certificate;
use super::fields;
use super::generations::Generations;

/// What `resolve` says about one control change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Accepted: it changes the state.
    Accepted,
    /// An accepted grant that a revocation it did not see capped below the
    /// role it gives (RFC 0008, "Concurrent add and remove of one member").
    Lost,
    /// Rejected, with its reason (RFC 0002 Amendment 1, D11): it stays in
    /// the log and changes nothing.
    Rejected(Rejection),
    /// Kept but not folded: an entry type or a key this core does not
    /// understand, a newer header format or another suite.
    Parked,
}

/// Why `resolve` rejects a control change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rejection {
    /// Its body is not a control body of the format.
    Malformed,
    /// No control change the holder has names its author's keys.
    UnknownAuthor,
    /// Its signature does not verify with its author's keys.
    BadSignature,
    /// The holder has no genesis for the space.
    NoGenesis,
    /// A genesis that does not meet RFC 0008's rules for one.
    InvalidGenesis,
    /// A second genesis of the creator, with the higher change id.
    EquivocatedGenesis,
    /// Its author lacked the right in its causal past, or its own rules
    /// failed there (RFC 0008, step 1).
    NotValidWhenWritten,
    /// Its author does not hold the right at its place in the order.
    NoRight,
    /// The accepted revocation with this change id excludes it.
    Excluded(ChangeId),
    /// A key chain entry that names the same generation id as another
    /// accepted one: both are rejected (RFC 0008, decision M23).
    DuplicateGeneration,
}

/// One member of the state, current or former.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemberState {
    /// Its public keys, from its genesis or add entry.
    pub key: MemberPublicKey,
    /// Its certificate, from the last accepted genesis or add entry.
    pub certificate: Certificate,
    /// Its role, or none when it is not a member.
    pub role: Option<Role>,
    /// Its accepted grants, in the order.
    pub grants: Vec<Grant>,
    /// Its accepted revocations, in the order.
    pub revocations: Vec<Revocation>,
}

/// An accepted grant: a genesis, an add or a set role.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Grant {
    /// The change that grants.
    pub change: ChangeId,
    /// The role it gives.
    pub role: Role,
    /// From which generation its sealed changes count; none for a reader.
    pub from_generation: Option<u64>,
    /// The role it still gives, lowered by the entries it did not see.
    pub effective: Option<Role>,
}

/// An accepted revocation: a set role or a remove.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Revocation {
    /// The change that revokes.
    pub change: ChangeId,
    /// The role it sets; none for a remove.
    pub role: Option<Role>,
    /// The subject's highest author sequence its author held.
    pub cutoff: u64,
}

/// The members as the changes accepted so far make them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Members(pub(crate) BTreeMap<MemberId, MemberState>);

impl Members {
    /// Adds an accepted grant, and the member if it is new.
    pub(crate) fn grant(
        &mut self,
        subject: MemberId,
        key: Option<(MemberPublicKey, &Certificate)>,
        grant: Grant,
    ) {
        if let Some((key, certificate)) = key {
            let state = self.0.entry(subject).or_insert_with(|| MemberState {
                key,
                certificate: certificate.clone(),
                role: None,
                grants: Vec::new(),
                revocations: Vec::new(),
            });
            state.certificate = certificate.clone();
        }
        if let Some(state) = self.0.get_mut(&subject) {
            state.grants.push(grant);
        }
    }

    /// Adds an accepted revocation.
    pub(crate) fn revoke(&mut self, subject: MemberId, revocation: Revocation) {
        if let Some(state) = self.0.get_mut(&subject) {
            state.revocations.push(revocation);
        }
    }

    /// Recomputes every role (RFC 0008, "Roles"): every accepted entry
    /// about a member sets a role (a grant its own, a remove none), and
    /// caps at it each of the member's grants that did not see it. Each
    /// grant is lowered to its lowest cap, and the member's role is the
    /// highest of them. `saw(a, b)` is whether `b` is in `a`'s causal past.
    pub(crate) fn settle(&mut self, saw: impl Fn(ChangeId, ChangeId) -> bool) {
        for state in self.0.values_mut() {
            let entries: Vec<(ChangeId, Option<Role>)> = state
                .grants
                .iter()
                .map(|g| (g.change, Some(g.role)))
                .chain(state.revocations.iter().map(|r| (r.change, r.role)))
                .collect();
            for grant in &mut state.grants {
                let cap = entries
                    .iter()
                    .filter(|(change, _)| *change != grant.change && !saw(grant.change, *change))
                    .map(|(_, role)| *role)
                    .min();
                grant.effective = match cap {
                    Some(cap) => cap.min(Some(grant.role)),
                    None => Some(grant.role),
                };
            }
            state.role = state.grants.iter().map(|g| g.effective).max().flatten();
        }
    }

    pub(crate) fn role(&self, member: MemberId) -> Option<Role> {
        self.0.get(&member).and_then(|state| state.role)
    }
}

/// The canonical bytes of a state: equal states give equal bytes.
pub(crate) fn encode(
    genesis: Option<ChangeId>,
    first_generation: Option<u64>,
    members: &Members,
    verdicts: &BTreeMap<ChangeId, Verdict>,
    gaps: &BTreeSet<ChangeId>,
    generations: &Generations,
    current: Option<u64>,
) -> Vec<u8> {
    let id = |id: &ChangeId| fields::bytes(&id.0);
    let role = |role: Option<Role>| Value::Unsigned(role.map_or(0, Role::number));
    let optional = |n: Option<u64>| n.map_or(Value::Null, Value::Unsigned);
    let members = members.0.iter().map(|(member, state)| {
        let grants = state.grants.iter().map(|g| {
            Value::Array(vec![
                id(&g.change),
                role(Some(g.role)),
                optional(g.from_generation),
                role(g.effective),
            ])
        });
        let revocations = state
            .revocations
            .iter()
            .map(|r| Value::Array(vec![id(&r.change), role(r.role), Value::Unsigned(r.cutoff)]));
        Value::Array(vec![
            fields::bytes(&member.0),
            role(state.role),
            fields::bytes(&state.key.signing),
            fields::bytes(&state.key.agreement),
            state.certificate.to_value(),
            Value::Array(grants.collect()),
            Value::Array(revocations.collect()),
        ])
    });
    let generations = generations.0.iter().map(|(generation, g)| {
        let previous = g.previous.iter().map(|p| Value::Unsigned(*p));
        let entries = g
            .entries
            .iter()
            .map(|(change, author)| Value::Array(vec![id(change), fields::bytes(&author.0)]));
        let recipients = g.recipients.iter().map(|r| fields::bytes(&r.0));
        let holders = g.holders.iter().map(|h| fields::bytes(&h.0));
        Value::Array(vec![
            Value::Unsigned(*generation),
            Value::Array(previous.collect()),
            Value::Array(entries.collect()),
            Value::Array(recipients.collect()),
            Value::Array(holders.collect()),
        ])
    });
    let verdicts = verdicts.iter().map(|(change, verdict)| {
        let (code, cause) = match verdict {
            Verdict::Accepted => (0, None),
            Verdict::Lost => (1, None),
            Verdict::Parked => (2, None),
            Verdict::Rejected(rejection) => (3, Some(*rejection)),
        };
        let cause = match cause {
            Some(Rejection::Excluded(by)) => id(&by),
            Some(other) => Value::Unsigned(rejection_code(other)),
            None => Value::Null,
        };
        Value::Array(vec![id(change), Value::Unsigned(code), cause])
    });
    cbor::encode(&fields::map([
        (1, genesis.as_ref().map_or(Value::Null, id)),
        (2, optional(first_generation)),
        (3, Value::Array(members.collect())),
        (4, Value::Array(verdicts.collect())),
        (5, Value::Array(gaps.iter().map(id).collect())),
        (6, Value::Array(generations.collect())),
        (7, optional(current)),
    ]))
}

fn rejection_code(rejection: Rejection) -> u64 {
    match rejection {
        Rejection::Malformed => 1,
        Rejection::UnknownAuthor => 2,
        Rejection::BadSignature => 3,
        Rejection::NoGenesis => 4,
        Rejection::InvalidGenesis => 5,
        Rejection::EquivocatedGenesis => 6,
        Rejection::NotValidWhenWritten => 7,
        Rejection::NoRight => 8,
        Rejection::Excluded(_) => 9,
        Rejection::DuplicateGeneration => 10,
    }
}
