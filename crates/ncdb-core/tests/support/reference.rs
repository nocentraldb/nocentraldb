//! A reference implementation of steps 2 to 5 of RFC 0008's `resolve`
//! ("The function"), written from the RFC's text for membership entries
//! and acknowledgements, for M-C6 ("Seniority and the order are total").
//! Step 1, whether a change is valid when written, comes from the core's
//! verdicts: the reference covers what the RFC asks it to, seniority, the
//! revocation edges, the order and the apply step.
//!
//! The rights each entry needs are those of RFC 0008's "Roles and rights":
//! a membership entry needs admin, except that a member may remove itself
//! as a reader and set its own role to a role it would hold; an
//! acknowledgement needs reader.
//!
//! Each test file includes this module with `#[path]`; not every file uses
//! every helper.
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};

use ncdb_core::change::{Change, Kind};
use ncdb_core::ids::{ChangeId, MemberId};
use ncdb_core::membership::{
    ACKNOWLEDGEMENT, Action, ControlBody, Entry, MembershipEntry, Rejection, Resolved, Role,
    Verdict,
};

/// One control change, as the reference reads it.
#[derive(Clone, Debug)]
pub struct Node {
    pub id: ChangeId,
    pub author: MemberId,
    pub sequence: u64,
    /// Its dependencies the holder holds.
    pub deps: BTreeSet<ChangeId>,
    /// Its causal past: every change it depends on, directly or not, that
    /// the holder holds.
    pub past: BTreeSet<ChangeId>,
    pub entry: Entry,
}

impl Node {
    fn membership(&self) -> Option<&MembershipEntry> {
        match &self.entry {
            Entry::Membership(entry) => Some(entry),
            _ => None,
        }
    }

    /// The role a membership entry sets: an add or a set role its role, a
    /// remove none. `None` outside membership entries.
    fn sets(&self) -> Option<Option<Role>> {
        self.membership().map(|entry| match entry.action {
            Action::Add { role, .. } | Action::SetRole { role, .. } => Some(role),
            Action::Remove { .. } => None,
        })
    }

    /// A revocation (step 3): a set role or a remove, with the role it
    /// sets and its cutoff.
    fn revocation(&self) -> Option<(MemberId, Option<Role>, u64)> {
        let entry = self.membership()?;
        match entry.action {
            Action::SetRole { role, cutoff, .. } => Some((entry.subject, Some(role), cutoff)),
            Action::Remove { cutoff } => Some((entry.subject, None, cutoff)),
            Action::Add { .. } => None,
        }
    }

    /// The right the entry needs ("Roles and rights").
    fn needed(&self) -> Role {
        match &self.entry {
            Entry::Genesis(_) => Role::Reader,
            Entry::Membership(MembershipEntry { subject, action }) if *subject == self.author => {
                match action {
                    Action::Remove { .. } => Role::Reader,
                    Action::SetRole { role, .. } => *role,
                    Action::Add { .. } => Role::Admin,
                }
            }
            Entry::Other { entry_type, .. } if *entry_type == ACKNOWLEDGEMENT => Role::Reader,
            Entry::PersonKey(_) | Entry::Shred(_) => Role::Writer,
            Entry::AccountKeyStatement(_) => Role::Reader,
            _ => Role::Admin,
        }
    }
}

/// What the reference computes: steps 2, 4 and 5.
#[derive(Debug, PartialEq, Eq)]
pub struct Outcome {
    pub seniority: Vec<MemberId>,
    pub order: Vec<ChangeId>,
    pub verdicts: BTreeMap<ChangeId, Verdict>,
}

/// The control changes among `changes`, as nodes.
pub fn nodes<'a>(changes: impl IntoIterator<Item = &'a Change>) -> BTreeMap<ChangeId, Node> {
    let mut nodes: BTreeMap<ChangeId, Node> = BTreeMap::new();
    let mut bodies = BTreeMap::new();
    for change in changes {
        if change.header().kind != Kind::CONTROL {
            continue;
        }
        if let Ok(body) = ControlBody::decode(&change.header().payload) {
            bodies.insert(change.id(), (change.header().clone(), body));
        }
    }
    let held: BTreeSet<ChangeId> = bodies.keys().copied().collect();
    for (id, (header, body)) in bodies {
        nodes.insert(
            id,
            Node {
                id,
                author: header.author,
                sequence: header.sequence,
                deps: body.dependencies.intersection(&held).copied().collect(),
                past: BTreeSet::new(),
                entry: body.entry,
            },
        );
    }
    let ids: Vec<ChangeId> = nodes.keys().copied().collect();
    for id in ids {
        let mut past = BTreeSet::new();
        let mut stack: Vec<ChangeId> = nodes[&id].deps.iter().copied().collect();
        while let Some(dep) = stack.pop() {
            if past.insert(dep) {
                stack.extend(nodes[&dep].deps.iter().copied());
            }
        }
        nodes.get_mut(&id).expect("a node").past = past;
    }
    nodes
}

/// Step 1, from the core: whether each node is valid when written.
pub fn valid(nodes: &BTreeMap<ChangeId, Node>, core: &Resolved) -> BTreeSet<ChangeId> {
    nodes
        .keys()
        .copied()
        .filter(|id| {
            !matches!(
                core.verdict(*id),
                Some(Verdict::Rejected(
                    Rejection::NotValidWhenWritten
                        | Rejection::Malformed
                        | Rejection::UnknownAuthor
                        | Rejection::BadSignature
                        | Rejection::NoGenesis
                        | Rejection::InvalidGenesis
                        | Rejection::EquivocatedGenesis
                )) | Some(Verdict::Parked)
                    | None
            )
        })
        .collect()
}

/// Steps 2 to 5 over the nodes in `set`, a set closed under the holder's
/// dependencies, with step 1's `valid` and the genesis `genesis`.
pub fn reference(
    nodes: &BTreeMap<ChangeId, Node>,
    set: &BTreeSet<ChangeId>,
    valid: &BTreeSet<ChangeId>,
    genesis: ChangeId,
) -> Outcome {
    let creator = nodes[&genesis].author;
    let seniority = seniority(nodes, set, valid, genesis, creator);
    let rank = |member: &MemberId| {
        seniority
            .iter()
            .position(|m| m == member)
            .unwrap_or(usize::MAX)
    };
    let edges = edges(nodes, set, valid, &rank);
    let order = order(nodes, set, &edges, &rank);
    let verdicts = apply(nodes, &order, valid, genesis, creator);
    Outcome {
        seniority,
        order,
        verdicts,
    }
}

/// Step 2: the creator, then the members with a valid grant of admin by
/// the smallest (depth, change id) over those grants, then every other
/// member id by its bytes.
fn seniority(
    nodes: &BTreeMap<ChangeId, Node>,
    set: &BTreeSet<ChangeId>,
    valid: &BTreeSet<ChangeId>,
    genesis: ChangeId,
    creator: MemberId,
) -> Vec<MemberId> {
    let mut depths: BTreeMap<ChangeId, u64> = BTreeMap::new();
    let mut best: BTreeMap<MemberId, (u64, ChangeId)> = BTreeMap::new();
    // Smaller causal pasts first, so a grant's author's grants have depths.
    let mut grants: Vec<&Node> = set
        .iter()
        .map(|id| &nodes[id])
        .filter(|n| valid.contains(&n.id) && n.sets() == Some(Some(Role::Admin)))
        .collect();
    grants.sort_by_key(|n| (n.past.len(), n.id));
    for grant in grants {
        // The admin grants its author holds in the state of its causal
        // past: those the fold of that past accepts with admin in effect.
        let past = reference(nodes, &grant.past, valid, genesis);
        let held = admin_grants_in_effect(nodes, &past, genesis, grant.author);
        let depth = held
            .iter()
            .filter_map(|g| match *g == genesis {
                true => Some(0),
                false => depths.get(g).copied(),
            })
            .min();
        let Some(depth) = depth.map(|d| d + 1) else {
            continue;
        };
        depths.insert(grant.id, depth);
        let subject = grant.membership().expect("a grant").subject;
        let entry = best.entry(subject).or_insert((depth, grant.id));
        *entry = (*entry).min((depth, grant.id));
    }
    let mut admins: Vec<(u64, ChangeId, MemberId)> = best
        .into_iter()
        .filter(|(m, _)| *m != creator)
        .map(|(m, (d, id))| (d, id, m))
        .collect();
    admins.sort();
    let mut seniority = vec![creator];
    seniority.extend(admins.iter().map(|(_, _, m)| *m));
    let mut others: BTreeSet<MemberId> = BTreeSet::new();
    for id in set {
        let node = &nodes[id];
        others.insert(node.author);
        if let Some(entry) = node.membership() {
            others.insert(entry.subject);
        }
    }
    let ranked: BTreeSet<MemberId> = seniority.iter().copied().collect();
    seniority.extend(others.into_iter().filter(|m| !ranked.contains(m)));
    seniority
}

/// Whether revocation `r` excludes node `y` (step 3), leaving aside
/// whether `r` is valid or accepted.
fn excludes(nodes: &BTreeMap<ChangeId, Node>, r: &Node, y: &Node) -> bool {
    let Some((subject, role, cutoff)) = r.revocation() else {
        return false;
    };
    if y.author != subject || y.id == r.id || matches!(y.entry, Entry::Shred(_)) {
        return false;
    }
    let needed = y.needed();
    if Some(needed) <= role || y.sequence <= cutoff {
        return false;
    }
    // Covered by a re-grant: y's causal past holds a grant of that right to
    // its author that saw r.
    let covered = y.past.iter().map(|id| &nodes[id]).any(|g| {
        g.membership().is_some_and(|e| e.subject == y.author)
            && g.sets().flatten() >= Some(needed)
            && g.past.contains(&r.id)
    });
    !covered
}

/// Step 3: the edges, one revocation at a time.
fn edges(
    nodes: &BTreeMap<ChangeId, Node>,
    set: &BTreeSet<ChangeId>,
    valid: &BTreeSet<ChangeId>,
    rank: &impl Fn(&MemberId) -> usize,
) -> BTreeMap<ChangeId, BTreeSet<ChangeId>> {
    let mut revocations: Vec<&Node> = set
        .iter()
        .map(|id| &nodes[id])
        .filter(|n| valid.contains(&n.id) && n.revocation().is_some())
        .collect();
    revocations.sort_by_key(|n| (rank(&n.author), n.sequence, n.id));
    // Successors: a node's dependents, then the edges added.
    let mut after: BTreeMap<ChangeId, BTreeSet<ChangeId>> = BTreeMap::new();
    for id in set {
        for dep in &nodes[id].deps {
            after.entry(*dep).or_default().insert(*id);
        }
    }
    let mut edges: BTreeMap<ChangeId, BTreeSet<ChangeId>> = BTreeMap::new();
    for r in revocations {
        for y in set.iter().map(|id| &nodes[id]) {
            if !excludes(nodes, r, y) || reaches(&after, y.id, r.id) {
                continue;
            }
            after.entry(r.id).or_default().insert(y.id);
            edges.entry(r.id).or_default().insert(y.id);
        }
    }
    edges
}

/// Whether `to` follows `from` in `after`.
fn reaches(after: &BTreeMap<ChangeId, BTreeSet<ChangeId>>, from: ChangeId, to: ChangeId) -> bool {
    let mut seen = BTreeSet::new();
    let mut stack = vec![from];
    while let Some(id) = stack.pop() {
        if id == to {
            return true;
        }
        if seen.insert(id) {
            stack.extend(after.get(&id).into_iter().flatten().copied());
        }
    }
    false
}

/// Step 4: after dependencies and edges; among the changes that may come
/// next, the more senior author first, then the lower change id.
fn order(
    nodes: &BTreeMap<ChangeId, Node>,
    set: &BTreeSet<ChangeId>,
    edges: &BTreeMap<ChangeId, BTreeSet<ChangeId>>,
    rank: &impl Fn(&MemberId) -> usize,
) -> Vec<ChangeId> {
    let mut before: BTreeMap<ChangeId, BTreeSet<ChangeId>> =
        set.iter().map(|id| (*id, nodes[id].deps.clone())).collect();
    for (r, ys) in edges {
        for y in ys {
            before.get_mut(y).expect("an excluded node").insert(*r);
        }
    }
    let mut order = Vec::with_capacity(set.len());
    let mut placed = BTreeSet::new();
    while order.len() < set.len() {
        let next = set
            .iter()
            .filter(|id| !placed.contains(*id))
            .filter(|id| before[*id].iter().all(|b| placed.contains(b)))
            .min_by_key(|id| (rank(&nodes[*id].author), **id))
            .copied()
            .expect("the dependencies and edges have no cycle");
        placed.insert(next);
        order.push(next);
    }
    order
}

/// The roles in effect over the accepted entries in `accepted`, by
/// member, with the creator's genesis as an admin grant: each grant's
/// role lowered to the lowest role an accepted entry of the same subject
/// it did not see sets, and the highest of those per member.
fn roles(
    nodes: &BTreeMap<ChangeId, Node>,
    accepted: &[ChangeId],
    genesis: ChangeId,
    creator: MemberId,
) -> (
    BTreeMap<MemberId, Option<Role>>,
    BTreeMap<ChangeId, Option<Role>>,
) {
    let mut by_subject: BTreeMap<MemberId, Vec<&Node>> = BTreeMap::new();
    for id in accepted {
        if let Some(entry) = nodes[id].membership() {
            by_subject
                .entry(entry.subject)
                .or_default()
                .push(&nodes[id]);
        }
    }
    let mut effective: BTreeMap<ChangeId, Option<Role>> = BTreeMap::new();
    let mut roles: BTreeMap<MemberId, Option<Role>> = BTreeMap::new();
    if accepted.contains(&genesis) {
        let caps = by_subject.get(&creator).into_iter().flatten();
        let cap = caps.map(|e| e.sets().flatten()).min();
        let role = cap.map_or(Some(Role::Admin), |cap| cap.min(Some(Role::Admin)));
        effective.insert(genesis, role);
        roles.insert(creator, role);
    }
    for (subject, entries) in &by_subject {
        for grant in entries.iter().filter(|g| g.sets().flatten().is_some()) {
            let cap = entries
                .iter()
                .filter(|e| e.id != grant.id && !grant.past.contains(&e.id))
                .map(|e| e.sets().flatten())
                .min();
            let own = grant.sets().flatten();
            let role = cap.map_or(own, |cap| cap.min(own));
            effective.insert(grant.id, role);
            let held = roles.entry(*subject).or_insert(None);
            *held = (*held).max(role);
        }
    }
    (roles, effective)
}

/// Step 5: accepts each node in the order when it is valid when written,
/// its author holds the right in the state so far, and no accepted
/// revocation before it excludes it. A grant accepted but capped below the
/// role it gives is lost.
fn apply(
    nodes: &BTreeMap<ChangeId, Node>,
    order: &[ChangeId],
    valid: &BTreeSet<ChangeId>,
    genesis: ChangeId,
    creator: MemberId,
) -> BTreeMap<ChangeId, Verdict> {
    let mut accepted: Vec<ChangeId> = Vec::new();
    let mut verdicts = BTreeMap::new();
    for id in order {
        let node = &nodes[id];
        let verdict = if *id == genesis {
            Verdict::Accepted
        } else if !valid.contains(id) {
            Verdict::Rejected(Rejection::NotValidWhenWritten)
        } else if let Some(r) = accepted
            .iter()
            .map(|r| &nodes[r])
            .find(|r| excludes(nodes, r, node))
        {
            Verdict::Rejected(Rejection::Excluded(r.id))
        } else {
            let (roles, _) = roles(nodes, &accepted, genesis, creator);
            match roles.get(&node.author).copied().flatten() >= Some(node.needed()) {
                true => Verdict::Accepted,
                false => Verdict::Rejected(Rejection::NoRight),
            }
        };
        if verdict == Verdict::Accepted {
            accepted.push(*id);
        }
        verdicts.insert(*id, verdict);
    }
    // Lost: a grant that an accepted entry concurrent with it caps below
    // its role. An entry that saw the grant caps it too, on purpose, and
    // the grant is not lost ("Concurrent add and remove of one member").
    let membership: Vec<&Node> = accepted
        .iter()
        .map(|id| &nodes[id])
        .filter(|n| n.membership().is_some())
        .collect();
    for grant in &membership {
        let own = grant.sets().flatten();
        let subject = grant.membership().expect("an entry").subject;
        let lost = membership.iter().any(|e| {
            e.id != grant.id
                && e.membership().expect("an entry").subject == subject
                && !grant.past.contains(&e.id)
                && !e.past.contains(&grant.id)
                && e.sets().flatten() < own
        });
        if own.is_some() && lost {
            verdicts.insert(grant.id, Verdict::Lost);
        }
    }
    verdicts
}

/// The grants of admin of `member` the fold accepted with admin in
/// effect, for step 2's depth.
pub fn admin_grants_in_effect(
    nodes: &BTreeMap<ChangeId, Node>,
    outcome: &Outcome,
    genesis: ChangeId,
    member: MemberId,
) -> Vec<ChangeId> {
    let creator = nodes[&genesis].author;
    let accepted: Vec<ChangeId> = outcome
        .order
        .iter()
        .copied()
        .filter(|id| {
            matches!(
                outcome.verdicts.get(id),
                Some(Verdict::Accepted | Verdict::Lost)
            )
        })
        .collect();
    let (_, effective) = roles(nodes, &accepted, genesis, creator);
    effective
        .into_iter()
        .filter(|(id, role)| {
            *role == Some(Role::Admin)
                && (*id == genesis || nodes[id].membership().is_some_and(|e| e.subject == member))
                && (*id != genesis || member == creator)
        })
        .map(|(id, _)| id)
        .collect()
}
