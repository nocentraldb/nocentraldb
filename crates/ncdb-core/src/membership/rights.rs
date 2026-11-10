//! The rights of RFC 0008 ("Roles and rights", "The control bodies") and
//! steps 1 to 4 of "The function": which right a control change needs, the
//! own rules it must meet in its causal past, seniority, the revocation
//! edges and the order.

use std::collections::{BTreeMap, BTreeSet};

use crate::ids::{ChangeId, MemberId, SpaceId};

use super::body::{ACKNOWLEDGEMENT, Action, Entry, MembershipEntry, Role};
use super::certificate::{Certificate, MemberKind};
use super::genesis::generation_id;
use super::input::Node;
use super::person_key::ShredEntry;
use super::resolve::{Judged, Resolved};

/// The right a change's author needs. A membership entry needs admin,
/// except a member's own leave (reader) or lowering of its own role (the
/// role it sets). Key envelopes and key chain entries need admin,
/// person-key and shred entries writer, acknowledgements reader.
pub(crate) fn needed(node: &Node) -> Role {
    match &node.body.entry {
        Entry::Genesis(_) => Role::Reader,
        Entry::Membership(MembershipEntry { subject, action }) if *subject == node.author => {
            match action {
                Action::Remove { .. } => Role::Reader,
                Action::SetRole { role, .. } => *role,
                Action::Add { .. } => Role::Admin,
            }
        }
        Entry::Membership(_) | Entry::KeyEnvelope(_) | Entry::KeyChain(_) => Role::Admin,
        Entry::PersonKey(_) | Entry::Shred(_) => Role::Writer,
        Entry::Other { entry_type, .. } => match *entry_type {
            ACKNOWLEDGEMENT => Role::Reader,
            _ => Role::Admin,
        },
    }
}

/// Whether a sub-agent's certificate carries its parent's account's
/// statement, as RFC 0003 asks: both certificates carry a statement under
/// the same recovery key, or neither carries one. Key 6 is not under key
/// 5's signature, so a mismatch is refused rather than read either way.
fn same_recovery_key(parent: &Certificate, sub_agent: &Certificate) -> bool {
    match (&parent.statement, &sub_agent.statement) {
        (Some(parent), Some(sub_agent)) => parent.recovery_key == sub_agent.recovery_key,
        (None, None) => true,
        _ => false,
    }
}

/// A change's own rules in the state of its causal past (step 1): a key
/// chain entry's previous generations exist and its generation id derives
/// from its space, author and previous generations; an envelope's recipient
/// is a member and its generation exists; a person-key entry repeats the
/// commitment of its id; a shred names a person key and carries a valid
/// sign-off ([`signed_off`]); an add's subject is not a member
/// and its certificate holds; a set role's or a remove's subject is a
/// member. A re-grant, an add of a former member or a set role that gives
/// the right to write to a member without it, names a fresh generation as
/// its from generation ([`fresh`]). The state of its causal past holds the
/// revocation it re-grants after, so it has seen it.
pub(crate) fn own_rules(space: SpaceId, node: &Node, past: &Resolved) -> bool {
    let entry = match &node.body.entry {
        Entry::Membership(entry) => entry,
        Entry::KeyChain(chain) => {
            let previous: Vec<u64> = chain.previous_generations().collect();
            return previous.iter().all(|p| past.generation(*p).is_some())
                && chain.generation == generation_id(space, node.author, &previous);
        }
        Entry::KeyEnvelope(envelope) => {
            return past.role(envelope.recipient).is_some()
                && past.generation(envelope.generation).is_some();
        }
        Entry::PersonKey(entry) => {
            return past
                .person_key(entry.person_key)
                .and_then(|key| key.commitment)
                .is_none_or(|commitment| commitment == entry.commitment);
        }
        Entry::Shred(shred) => return signed_off(space, node, shred, past),
        _ => return true,
    };
    let MembershipEntry { subject, action } = entry;
    match action {
        Action::Add {
            certificate,
            from_generation,
            ..
        } => {
            let former = past.member(*subject).is_some();
            past.role(*subject).is_none()
                && (!former || fresh(past, node.author, *subject, *from_generation))
                && certificate.member == *subject
                && match certificate.kind {
                    MemberKind::Device | MemberKind::Agent => {
                        certificate.verify(&certificate.account_key)
                    }
                    MemberKind::SubAgent => certificate.parent.is_some_and(|parent| {
                        past.role(parent).is_some()
                            && past.member(parent).is_some_and(|p| {
                                p.certificate.kind != MemberKind::Device
                                    && p.certificate.account_key == certificate.account_key
                                    && same_recovery_key(&p.certificate, certificate)
                                    && certificate.verify(&p.key.signing)
                            })
                    }),
                }
        }
        Action::SetRole {
            role,
            from_generation,
            ..
        } => {
            let current = past.role(*subject);
            let re_grant = *role >= Role::Writer && current < Some(Role::Writer);
            current.is_some()
                && (!re_grant
                    || from_generation.is_some_and(|g| fresh(past, node.author, *subject, g)))
        }
        Action::Remove { .. } => past.role(*subject).is_some(),
    }
}

/// Whether a shred names a person key of its causal past and carries a
/// valid sign-off: it verifies for the space, the person key and the
/// performer, its account holds an admin in the causal past, and it is not
/// the account of the performer's certificate (RFC 0008, "Shred entry"). A
/// sub-agent's certificate carries its parent's account key, so a sub-agent
/// counts as its parent's account.
fn signed_off(space: SpaceId, node: &Node, shred: &ShredEntry, past: &Resolved) -> bool {
    let account = shred.sign_off.account_key;
    let performer = past.member(node.author);
    past.person_key(shred.person_key).is_some()
        && shred.sign_off.verify(space, shred.person_key, node.author)
        && performer.is_some_and(|p| p.certificate.account_key != account)
        && past.members().any(|(member, role)| {
            role == Role::Admin
                && past
                    .member(member)
                    .is_some_and(|m| m.certificate.account_key == account)
        })
}

/// Whether `generation` is fresh for a re-grant of `subject` by `author`
/// in the state of the re-grant's causal past: `author` wrote a key chain
/// entry for it, and no key envelope to `subject` for it is there.
fn fresh(past: &Resolved, author: MemberId, subject: MemberId, generation: u64) -> bool {
    past.generation(generation).is_some_and(|g| {
        g.entries.values().any(|by| *by == author) && !g.recipients.contains(&subject)
    })
}

/// Step 2: the creator, then every member with a valid admin grant by its
/// smallest (depth, change id), then every other member id by its bytes.
pub(crate) fn seniority(
    genesis: &Node,
    nodes: &BTreeMap<ChangeId, &Node>,
    judged: &Judged,
) -> Vec<MemberId> {
    let mut admins: BTreeMap<MemberId, (u64, ChangeId)> = BTreeMap::new();
    let mut others = BTreeSet::new();
    for node in nodes.values() {
        others.insert(node.author);
        let Some(entry) = node.membership() else {
            continue;
        };
        others.insert(entry.subject);
        let depth = judged.0.get(&node.id).and_then(|j| j.depth);
        if let Some(depth) = depth.filter(|_| entry.role() == Some(Role::Admin)) {
            let rank = admins.entry(entry.subject).or_insert((depth, node.id));
            *rank = (*rank).min((depth, node.id));
        }
    }
    admins.remove(&genesis.author);
    let mut ranked: Vec<(&(u64, ChangeId), &MemberId)> =
        admins.iter().map(|(member, rank)| (rank, member)).collect();
    ranked.sort();
    let mut seniority = vec![genesis.author];
    seniority.extend(ranked.into_iter().map(|(_, member)| *member));
    others.retain(|member| !seniority.contains(member));
    seniority.extend(others);
    seniority
}

/// Step 3: the edges from each valid revocation to the changes it
/// excludes, as `from -> to`. Revocations in the order of their authors'
/// seniority, then author sequence, then change id; within one, its edges
/// by the excluded change's id; an edge that would close a cycle with the
/// dependencies and the edges before it is skipped.
pub(crate) fn edges(
    nodes: &BTreeMap<ChangeId, &Node>,
    all: &BTreeMap<ChangeId, Node>,
    judged: &Judged,
    rank: &BTreeMap<MemberId, usize>,
) -> BTreeMap<ChangeId, BTreeSet<ChangeId>> {
    let mut revocations: Vec<&Node> = nodes
        .values()
        .copied()
        .filter(|n| judged.valid(n.id) && n.membership().is_some_and(|m| m.cutoff().is_some()))
        .collect();
    revocations.sort_by_key(|r| (rank.get(&r.author).copied(), r.sequence, r.id));
    let mut after = successors(nodes);
    let mut edges: BTreeMap<ChangeId, BTreeSet<ChangeId>> = BTreeMap::new();
    for r in revocations {
        for y in nodes.values().filter(|y| excludes(r, y, all, judged)) {
            if reaches(&after, y.id, r.id) {
                continue;
            }
            after.entry(r.id).or_default().insert(y.id);
            edges.entry(r.id).or_default().insert(y.id);
        }
    }
    edges
}

/// Each node's dependants among `nodes`.
fn successors(nodes: &BTreeMap<ChangeId, &Node>) -> BTreeMap<ChangeId, BTreeSet<ChangeId>> {
    let mut after: BTreeMap<ChangeId, BTreeSet<ChangeId>> = BTreeMap::new();
    for node in nodes.values() {
        for dep in node.deps.iter().filter(|d| nodes.contains_key(d)) {
            after.entry(*dep).or_default().insert(node.id);
        }
    }
    after
}

/// Whether `to` comes after `from` through `after`.
fn reaches(after: &BTreeMap<ChangeId, BTreeSet<ChangeId>>, from: ChangeId, to: ChangeId) -> bool {
    let mut seen = BTreeSet::from([from]);
    let mut stack = vec![from];
    while let Some(id) = stack.pop() {
        if id == to {
            return true;
        }
        for next in after.get(&id).into_iter().flatten() {
            if seen.insert(*next) {
                stack.push(*next);
            }
        }
    }
    false
}

/// Whether the revocation `r` excludes the change `y` (step 3): `y` is its
/// subject's, needs a right above the role `r` sets, has an author
/// sequence above `r`'s cutoff, and no valid grant of that right to its
/// author that saw `r` is in `y`'s causal past.
pub(crate) fn excludes(
    r: &Node,
    y: &Node,
    all: &BTreeMap<ChangeId, Node>,
    judged: &Judged,
) -> bool {
    let Some(revocation) = r.membership() else {
        return false;
    };
    let Some(cutoff) = revocation.cutoff() else {
        return false;
    };
    let right = needed(y);
    if y.is_genesis()
        || y.id == r.id
        || y.author != revocation.subject
        || Some(right) <= revocation.role()
        || y.sequence <= cutoff
    {
        return false;
    }
    let covered = y.past.iter().filter_map(|id| all.get(id)).any(|g| {
        g.past.contains(&r.id)
            && judged.valid(g.id)
            && g.membership()
                .is_some_and(|grant| grant.subject == y.author && grant.role() >= Some(right))
    });
    !covered
}

/// Step 4: every node after its dependencies and the revocations whose
/// edges point at it; among those that may come next, the senior author,
/// then the lower change id.
pub(crate) fn order(
    nodes: &BTreeMap<ChangeId, &Node>,
    edges: &BTreeMap<ChangeId, BTreeSet<ChangeId>>,
    rank: &BTreeMap<MemberId, usize>,
) -> Vec<ChangeId> {
    let mut after = successors(nodes);
    for (from, targets) in edges {
        after.entry(*from).or_default().extend(targets);
    }
    let mut waiting: BTreeMap<ChangeId, usize> = nodes.keys().map(|id| (*id, 0)).collect();
    for targets in after.values() {
        for target in targets {
            *waiting.get_mut(target).expect("an edge between nodes") += 1;
        }
    }
    let key = |id: ChangeId| {
        (
            rank.get(&nodes[&id].author).copied().unwrap_or(usize::MAX),
            id,
        )
    };
    let mut ready: BTreeSet<(usize, ChangeId)> = waiting
        .iter()
        .filter(|(_, n)| **n == 0)
        .map(|(id, _)| key(*id))
        .collect();
    let mut order = Vec::with_capacity(nodes.len());
    while let Some((_, id)) = ready.pop_first() {
        order.push(id);
        for target in after.get(&id).into_iter().flatten() {
            let count = waiting.get_mut(target).expect("a node");
            *count -= 1;
            if *count == 0 {
                ready.insert(key(*target));
            }
        }
    }
    order
}
