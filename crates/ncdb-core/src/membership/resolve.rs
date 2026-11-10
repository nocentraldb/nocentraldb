//! `resolve` of RFC 0008 ("The function"): the control state of a space as
//! a function of the set of its control changes, in five steps, the same on
//! every holder in any order of arrival.
//!
//! 1. **Validity in the causal past.** A change is valid when written if
//!    the fold of its causal past gives its author the right and its own
//!    rules hold there ([`judge`]).
//! 2. **Seniority** of member ids: the creator, then the holders of admin
//!    grants by (depth, change id), then every other id by its bytes.
//! 3. **Revocation edges** put each valid revocation before the changes it
//!    excludes.
//! 4. **The order**: dependencies and edges first, then the senior author,
//!    then the lower change id.
//! 5. **Apply** in that order: a change is accepted when it is valid when
//!    written, its author holds the right, and no accepted revocation
//!    before it excludes it.

use std::collections::{BTreeMap, BTreeSet};

use crate::change::{Change, Header};
use crate::ids::{ChangeId, MemberId, SpaceId};

use super::body::{Action, Entry, Role};
use super::generations::{Generation, Generations};
use super::input::{Input, Node};
use super::person_keys::{self, PersonKey, PersonKeys};
use super::rights;
use super::state::{self, Grant, MemberState, Members, Rejection, Revocation, Verdict};

/// The control state of a space: what `resolve` computes. Its `Debug`
/// output shows ids, roles and verdicts, never content.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolved {
    genesis: Option<ChangeId>,
    first_generation: Option<u64>,
    members: Members,
    verdicts: BTreeMap<ChangeId, Verdict>,
    order: Vec<ChangeId>,
    seniority: Vec<MemberId>,
    gaps: BTreeSet<ChangeId>,
    /// For each accepted grant, the accepted revocations of its subject in
    /// its causal past.
    seen: BTreeMap<ChangeId, BTreeSet<ChangeId>>,
    generations: Generations,
    person_keys: PersonKeys,
}

/// Folds the control changes of `space` among `changes` into its control
/// state. Changes of other spaces and sealed changes are left out; every
/// control change of the space gets a verdict.
pub fn resolve<'a>(space: SpaceId, changes: impl IntoIterator<Item = &'a Change>) -> Resolved {
    let input = Input::new(space, changes);
    let mut judged = Judged::default();
    // Smaller causal pasts first: a node's past holds only nodes with
    // smaller pasts, so every fold below finds their judgement.
    let mut nodes: Vec<&Node> = input.nodes.values().collect();
    nodes.sort_by_key(|node| (node.past.len(), node.id));
    for node in nodes {
        let past = fold(&input, &judged, &node.past);
        let judgement = judge(&input, &judged, node, &past);
        judged.0.insert(node.id, judgement);
    }
    let all: BTreeSet<ChangeId> = input.nodes.keys().copied().collect();
    let mut resolved = fold(&input, &judged, &all);
    resolved.seen = seen(&input, &resolved.members);
    resolved.verdicts.extend(input.verdicts);
    resolved.gaps = input.gaps;
    resolved
}

/// For each accepted grant, the accepted revocations of its subject that it
/// saw.
fn seen(input: &Input, members: &Members) -> BTreeMap<ChangeId, BTreeSet<ChangeId>> {
    let mut seen = BTreeMap::new();
    for state in members.0.values() {
        for grant in &state.grants {
            let past = input.nodes.get(&grant.change).map(|node| &node.past);
            let revocations = state
                .revocations
                .iter()
                .map(|r| r.change)
                .filter(|r| past.is_some_and(|past| past.contains(r)));
            seen.insert(grant.change, revocations.collect());
        }
    }
    seen
}

impl Resolved {
    /// The genesis the state starts from.
    pub fn genesis(&self) -> Option<ChangeId> {
        self.genesis
    }

    /// The generation id of the space's first generation.
    pub fn first_generation(&self) -> Option<u64> {
        self.first_generation
    }

    /// The verdict of a control change of the space.
    pub fn verdict(&self, change: ChangeId) -> Option<&Verdict> {
        self.verdicts.get(&change)
    }

    /// Every control change's verdict, by change id.
    pub fn verdicts(&self) -> &BTreeMap<ChangeId, Verdict> {
        &self.verdicts
    }

    /// A member's role; none when it is not a member.
    pub fn role(&self, member: MemberId) -> Option<Role> {
        self.members.role(member)
    }

    /// The current members and their roles, by member id.
    pub fn members(&self) -> impl Iterator<Item = (MemberId, Role)> + '_ {
        let members = self.members.0.iter();
        members.filter_map(|(id, state)| state.role.map(|role| (*id, role)))
    }

    /// A current or former member: its keys, certificate, grants and
    /// revocations.
    pub fn member(&self, member: MemberId) -> Option<&MemberState> {
        self.members.0.get(&member)
    }

    /// The order of step 4, of the changes that take part in the fold.
    pub fn order(&self) -> &[ChangeId] {
        &self.order
    }

    /// A generation of the generation graph.
    pub fn generation(&self, generation: u64) -> Option<&Generation> {
        self.generations.0.get(&generation)
    }

    /// The generation graph, by generation id.
    pub fn generations(&self) -> impl Iterator<Item = (u64, &Generation)> + '_ {
        self.generations.0.iter().map(|(id, g)| (*id, g))
    }

    /// Whether `generation` is in the generation graph and is `from` or
    /// descends from it.
    pub fn descends(&self, generation: u64, from: u64) -> bool {
        self.generations.descends(generation, from)
    }

    /// The seniority of step 2, most senior first.
    pub fn seniority(&self) -> &[MemberId] {
        &self.seniority
    }

    /// Control dependencies the holder does not hold: the SDK asks for
    /// them (RFC 0002).
    pub fn gaps(&self) -> &BTreeSet<ChangeId> {
        &self.gaps
    }

    /// RFC 0008's membership verdict for a sealed change ("Sealed
    /// changes"), from its header alone: [`MemberState::admits`] of its
    /// author, over this space's generation graph.
    pub fn admits(&self, header: &Header) -> bool {
        self.members.0.get(&header.author).is_some_and(|state| {
            state.admits(header, &self.seen, |generation, from| {
                self.descends(generation, from)
            })
        })
    }

    /// A person key of the space: its commitment, its records and the shred
    /// that destroyed it.
    pub fn person_key(&self, id: [u8; 16]) -> Option<&PersonKey> {
        self.person_keys.0.get(&id)
    }

    /// The person keys of the space, by person-key id.
    pub fn person_keys(&self) -> impl Iterator<Item = ([u8; 16], &PersonKey)> + '_ {
        self.person_keys.0.iter().map(|(id, key)| (*id, key))
    }

    /// For each accepted grant, the accepted revocations of its subject in
    /// its causal past.
    pub fn seen(&self) -> &BTreeMap<ChangeId, BTreeSet<ChangeId>> {
        &self.seen
    }

    /// The canonical bytes of the state: equal states, equal bytes.
    pub fn encode(&self) -> Vec<u8> {
        state::encode(
            self.genesis,
            self.first_generation,
            &self.members,
            &self.verdicts,
            &self.gaps,
            &self.generations,
            &self.person_keys,
        )
    }
}

/// What step 1 found for a node, which depends only on its causal past.
#[derive(Clone, Copy)]
pub(crate) struct Judgement {
    pub(crate) valid: bool,
    /// For a valid grant of admin: its depth (step 2).
    pub(crate) depth: Option<u64>,
}

/// The judgement of every node judged so far.
#[derive(Default)]
pub(crate) struct Judged(pub(crate) BTreeMap<ChangeId, Judgement>);

impl Judged {
    pub(crate) fn valid(&self, id: ChangeId) -> bool {
        self.0.get(&id).is_some_and(|j| j.valid)
    }
}

/// Steps 2 to 5 over `set`, a set of nodes closed under dependencies or
/// all of them.
fn fold(input: &Input, judged: &Judged, set: &BTreeSet<ChangeId>) -> Resolved {
    let mut resolved = Resolved {
        genesis: None,
        first_generation: None,
        members: Members::default(),
        verdicts: BTreeMap::new(),
        order: Vec::new(),
        seniority: Vec::new(),
        gaps: BTreeSet::new(),
        seen: BTreeMap::new(),
        generations: Generations::default(),
        person_keys: PersonKeys::default(),
    };
    let nodes: BTreeMap<ChangeId, &Node> = set.iter().map(|id| (*id, &input.nodes[id])).collect();
    // The genesis with the lowest change id; the others fold nothing.
    let geneses: Vec<&Node> = nodes.values().copied().filter(|n| n.is_genesis()).collect();
    let Some((genesis, others)) = geneses.split_first() else {
        for id in nodes.keys() {
            resolved
                .verdicts
                .insert(*id, Verdict::Rejected(Rejection::NoGenesis));
        }
        return resolved;
    };
    for other in others {
        let verdict = Verdict::Rejected(Rejection::EquivocatedGenesis);
        resolved.verdicts.insert(other.id, verdict);
    }
    let Entry::Genesis(body) = &genesis.body.entry else {
        unreachable!("a genesis node")
    };
    resolved.genesis = Some(genesis.id);
    resolved.first_generation = Some(body.first_generation);
    let folded: BTreeMap<ChangeId, &Node> = nodes
        .into_iter()
        .filter(|(id, node)| !node.is_genesis() || *id == genesis.id)
        .collect();
    resolved.generations =
        Generations::build(body.first_generation, folded.values().copied(), judged);
    resolved.seniority = rights::seniority(genesis, &folded, judged);
    let rank: BTreeMap<MemberId, usize> = resolved
        .seniority
        .iter()
        .enumerate()
        .map(|(i, m)| (*m, i))
        .collect();
    let edges = rights::edges(&folded, &input.nodes, judged, &rank);
    resolved.order = rights::order(&folded, &edges, &rank);

    let saw = |a: ChangeId, b: ChangeId| input.nodes.get(&a).is_some_and(|n| n.past.contains(&b));
    let creator = Grant {
        change: genesis.id,
        role: Role::Admin,
        from_generation: Some(body.first_generation),
        effective: Some(Role::Admin),
    };
    let members = &mut resolved.members;
    members.grant(
        genesis.author,
        Some((body.creator, &body.certificate)),
        creator,
    );
    members.settle(saw);
    resolved.verdicts.insert(genesis.id, Verdict::Accepted);
    let mut accepted_revocations: Vec<ChangeId> = Vec::new();
    let mut keys = PersonKeys::default();
    for id in resolved.order.iter().filter(|id| **id != genesis.id) {
        let node = folded[id];
        let verdict = if !judged.valid(node.id) {
            Verdict::Rejected(Rejection::NotValidWhenWritten)
        } else if let Some(by) = accepted_revocations
            .iter()
            .find(|r| rights::excludes(folded[r], node, &input.nodes, judged))
        {
            // Reported before a missing right, which the exclusion explains.
            Verdict::Rejected(Rejection::Excluded(*by))
        } else if members.role(node.author) < Some(rights::needed(node)) {
            Verdict::Rejected(Rejection::NoRight)
        } else if !keys.admits(node) {
            Verdict::Rejected(Rejection::Commitment)
        } else {
            apply(members, node);
            keys.apply(node);
            members.settle(saw);
            if node.membership().is_some_and(|m| m.cutoff().is_some()) {
                accepted_revocations.push(node.id);
            }
            Verdict::Accepted
        };
        resolved.verdicts.insert(node.id, verdict);
    }
    for state in members.0.values() {
        for grant in &state.grants {
            if grant.effective < Some(grant.role) {
                resolved.verdicts.insert(grant.change, Verdict::Lost);
            }
        }
    }
    reject_duplicate_generations(&mut resolved.verdicts, &folded);
    resolved.person_keys =
        person_keys::settle(&resolved.order, &folded, &mut resolved.verdicts, saw);
    resolved
}

/// Decision M23: when two accepted key chain entries name the same
/// generation id, both are rejected.
fn reject_duplicate_generations(
    verdicts: &mut BTreeMap<ChangeId, Verdict>,
    folded: &BTreeMap<ChangeId, &Node>,
) {
    let mut by_generation: BTreeMap<u64, Vec<ChangeId>> = BTreeMap::new();
    for (id, node) in folded {
        if let Entry::KeyChain(chain) = &node.body.entry
            && verdicts.get(id) == Some(&Verdict::Accepted)
        {
            by_generation.entry(chain.generation).or_default().push(*id);
        }
    }
    for ids in by_generation.into_values().filter(|ids| ids.len() > 1) {
        for id in ids {
            verdicts.insert(id, Verdict::Rejected(Rejection::DuplicateGeneration));
        }
    }
}

/// What an accepted change does to the members.
fn apply(members: &mut Members, node: &Node) {
    let Some(entry) = node.membership() else {
        return;
    };
    let grant = |role: Role, from_generation| Grant {
        change: node.id,
        role,
        from_generation,
        effective: Some(role),
    };
    match &entry.action {
        Action::Add {
            role,
            key,
            certificate,
            from_generation,
        } => {
            let grant = grant(*role, Some(*from_generation));
            members.grant(entry.subject, Some((*key, certificate)), grant);
        }
        Action::SetRole {
            role,
            from_generation,
            cutoff,
        } => {
            members.grant(entry.subject, None, grant(*role, *from_generation));
            let revocation = Revocation {
                change: node.id,
                role: Some(*role),
                cutoff: *cutoff,
            };
            members.revoke(entry.subject, revocation);
        }
        Action::Remove { cutoff } => {
            let revocation = Revocation {
                change: node.id,
                role: None,
                cutoff: *cutoff,
            };
            members.revoke(entry.subject, revocation);
        }
    }
}

/// Step 1 for one node, given the fold of its causal past.
fn judge(input: &Input, judged: &Judged, node: &Node, past: &Resolved) -> Judgement {
    if node.is_genesis() {
        return Judgement {
            valid: true,
            depth: Some(0),
        };
    }
    let valid = past.genesis.is_some()
        && past.role(node.author) >= Some(rights::needed(node))
        && rights::own_rules(input.space, node, past);
    let depth = match (valid, node.membership()) {
        (true, Some(entry)) if entry.role() == Some(Role::Admin) => {
            let held = past.member(node.author).into_iter().flat_map(|s| &s.grants);
            held.filter(|g| g.effective == Some(Role::Admin))
                .filter_map(|g| match input.nodes.get(&g.change) {
                    Some(n) if n.is_genesis() => Some(0),
                    _ => judged.0.get(&g.change).and_then(|j| j.depth),
                })
                .min()
                .map(|depth| depth + 1)
        }
        _ => None,
    };
    Judgement { valid, depth }
}
