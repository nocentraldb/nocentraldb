//! The rights of RFC 0008 ("Roles and rights", "The control bodies") and
//! steps 1 to 4 of "The function": which right a control change needs, the
//! own rules it must meet in its causal past, seniority, the revocation
//! edges and the order.

use std::collections::{BTreeMap, BTreeSet};

use crate::ids::{ChangeId, MemberId};

use super::body::{
    ACKNOWLEDGEMENT, Action, Entry, KEY_CHAIN, KEY_ENVELOPE, MembershipEntry, PERSON_KEY, Role,
    SHRED,
};
use super::certificate::MemberKind;
use super::input::Node;
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
        Entry::Membership(_) => Role::Admin,
        Entry::Other { entry_type, .. } => match *entry_type {
            ACKNOWLEDGEMENT => Role::Reader,
            PERSON_KEY | SHRED => Role::Writer,
            KEY_ENVELOPE | KEY_CHAIN => Role::Admin,
            _ => Role::Admin,
        },
    }
}

/// A membership entry's own rules in the state of its causal past (step
/// 1): an add's subject is not a member and its certificate holds; a set
/// role's or a remove's subject is a member.
///
/// A re-grant, an add of a former member or a set role that gives the
/// right to write to a member without it, needs a fresh generation, which
/// only key chain entries and envelopes can show. This core does not fold
/// them yet, so it never finds one, and a re-grant is not valid: a
/// revocation always holds.
pub(crate) fn own_rules(node: &Node, past: &Resolved) -> bool {
    let Some(MembershipEntry { subject, action }) = node.membership() else {
        return true;
    };
    match action {
        Action::Add { certificate, .. } => {
            past.member(*subject).is_none()
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
                                    && certificate.verify(&p.key.signing)
                            })
                    }),
                }
        }
        Action::SetRole { role, .. } => {
            let current = past.role(*subject);
            current.is_some() && !(*role >= Role::Writer && current < Some(Role::Writer))
        }
        Action::Remove { .. } => past.role(*subject).is_some(),
    }
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
/// excludes, as `from -> to`.
pub(crate) fn edges(
    _nodes: &BTreeMap<ChangeId, &Node>,
    _judged: &Judged,
    _rank: &BTreeMap<MemberId, usize>,
) -> BTreeMap<ChangeId, BTreeSet<ChangeId>> {
    BTreeMap::new()
}

/// Whether the revocation `r` excludes the change `y` (step 3).
pub(crate) fn excludes(
    _r: &Node,
    _y: &Node,
    _nodes: &BTreeMap<ChangeId, Node>,
    _judged: &Judged,
) -> bool {
    false
}

/// Step 4: every node after its dependencies and the revocations whose
/// edges point at it; among those that may come next, the senior author,
/// then the lower change id.
pub(crate) fn order(
    nodes: &BTreeMap<ChangeId, &Node>,
    edges: &BTreeMap<ChangeId, BTreeSet<ChangeId>>,
    rank: &BTreeMap<MemberId, usize>,
) -> Vec<ChangeId> {
    let mut after: BTreeMap<ChangeId, BTreeSet<ChangeId>> = edges.clone();
    for node in nodes.values() {
        for dep in node.deps.iter().filter(|d| nodes.contains_key(d)) {
            after.entry(*dep).or_default().insert(node.id);
        }
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
