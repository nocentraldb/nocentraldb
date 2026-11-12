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
//!    before it excludes it. A shred needs only to be valid when written,
//!    and no revocation excludes it (RFC 0008, "A shred is final when
//!    accepted").

use std::collections::{BTreeMap, BTreeSet};

use crate::cbor::{self, Value};
use crate::change::{Change, Header};
use crate::ids::{ChangeId, MemberId, SpaceId};

use super::body::{Action, Entry, MembershipEntry, Role};
use super::current::{self, MergeRotation};
use super::fields;
use super::generations::{Generation, Generations};
use super::input::{Input, Node};
use super::judge::{Judged, judge};
use super::person_keys::{self, PersonKey, PersonKeys};
use super::rights;
use super::state::{self, Grant, MemberState, Members, Rejection, Revocation, Verdict};
use super::statement::AccountKeyStatement;

/// The control state of a space: what `resolve` computes. Its `Debug`
/// output shows ids, roles and verdicts, never content.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolved {
    space: SpaceId,
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
    heads: BTreeSet<u64>,
    current: Option<u64>,
    person_keys: PersonKeys,
    /// The accepted account key statement entries, by change id.
    statements: BTreeMap<ChangeId, AccountKeyStatement>,
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
        if judgement.valid && matches!(node.body.entry, Entry::Shred(_)) {
            let members = past.members().map(|(member, _)| member);
            judged.shred_past_members.insert(node.id, members.collect());
        }
        judged.judgements.insert(node.id, judgement);
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

    /// The certificate of every current or former member: each one's last
    /// accepted genesis or add entry.
    pub(crate) fn certificates(&self) -> impl Iterator<Item = &super::Certificate> + '_ {
        self.members.0.values().map(|state| &state.certificate)
    }

    /// The statements of the accepted account key statement entries, in
    /// the order of their change ids (RFC 0008, "Account key statement
    /// entry").
    pub fn statements(&self) -> impl Iterator<Item = &AccountKeyStatement> + '_ {
        self.statements.values()
    }

    /// Every account key statement of the state: those the certificates of
    /// current and former members carry, and those of the accepted
    /// statement entries.
    pub fn all_statements(&self) -> impl Iterator<Item = &AccountKeyStatement> + '_ {
        let certified = self
            .certificates()
            .filter_map(|certificate| certificate.statement.as_deref());
        certified.chain(self.statements())
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

    /// The heads of the generation graph: the generations no accepted key
    /// chain entry lists as previous, leaving out a generation that two
    /// entries name (RFC 0008, "Generation ids").
    pub fn heads(&self) -> BTreeSet<u64> {
        self.heads.clone()
    }

    /// The current generation, which a writer seals under (RFC 0008, "Two
    /// removals that rotate the key at once"): the head with the highest
    /// ordinal, then the one whose key chain entry comes first in the
    /// order. None without a genesis.
    pub fn current_generation(&self) -> Option<u64> {
        self.current
    }

    /// Whether a generation of the graph is exposed: someone who may hold
    /// it is not a member of the state (RFC 0008, "Two removals that
    /// rotate the key at once", step 4).
    pub fn exposed(&self, generation: u64) -> bool {
        self.generation(generation)
            .is_some_and(|g| g.holders.iter().any(|m| self.role(*m).is_none()))
    }

    /// The merge rotation `author` writes, when the current generation is
    /// exposed and `author` is an admin (RFC 0008, "Two removals that
    /// rotate the key at once", step 5): every head as previous, and an
    /// envelope for each current member. When to write it is the SDK's
    /// (RFC 0008, "Two removals that rotate the key at once", step 5).
    pub fn merge_rotation(&self, author: MemberId) -> Option<MergeRotation> {
        let current = self.current?;
        if self.role(author) != Some(Role::Admin) || !self.exposed(current) {
            return None;
        }
        let recipients = self.members().map(|(member, _)| member).collect();
        Some(MergeRotation::new(
            self.space,
            author,
            &self.heads,
            recipients,
        ))
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

    /// Whether the members `a` and `b`, current or former, belong to one
    /// account: their certificates name one account key, or the state's
    /// statements attach one recovery key to both (RFC 0008, "Roles and
    /// rights"). False when either is unknown.
    pub fn same_account(&self, a: MemberId, b: MemberId) -> bool {
        match (self.member(a), self.member(b)) {
            (Some(a), Some(b)) => {
                rights::same_account(self, a.certificate.account_key, b.certificate.account_key)
            }
            _ => false,
        }
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
        let mut entries = state::fields(
            self.genesis,
            self.first_generation,
            &self.members,
            &self.verdicts,
            &self.gaps,
            &self.generations,
            self.current,
        );
        entries.push((8, self.person_keys.to_value()));
        let statements = self
            .statements
            .iter()
            .map(|(id, statement)| Value::Array(vec![fields::bytes(&id.0), statement.to_value()]));
        entries.push((9, Value::Array(statements.collect())));
        cbor::encode(&fields::map(entries))
    }
}

/// Steps 2 to 5 over `set`, a set of nodes closed under dependencies or
/// all of them.
fn fold(input: &Input, judged: &Judged, set: &BTreeSet<ChangeId>) -> Resolved {
    let mut resolved = Resolved {
        space: input.space,
        genesis: None,
        first_generation: None,
        members: Members::default(),
        verdicts: BTreeMap::new(),
        order: Vec::new(),
        seniority: Vec::new(),
        gaps: BTreeSet::new(),
        seen: BTreeMap::new(),
        generations: Generations::default(),
        heads: BTreeSet::new(),
        current: None,
        person_keys: PersonKeys::default(),
        statements: BTreeMap::new(),
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
    resolved.generations = Generations::build(
        body.first_generation,
        genesis.author,
        folded.values().copied(),
        judged,
    );
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
    let mut members = Members::default();
    members.grant(
        genesis.author,
        Some((body.creator, &body.certificate)),
        creator,
    );
    members.settle(saw);
    resolved.verdicts.insert(genesis.id, Verdict::Accepted);
    let (before, start) = (resolved.verdicts.clone(), members);
    // RFC 0008, step 5: an add concurrent with an accepted statement entry
    // that supersedes its account key is rejected wherever it falls in the
    // order. Rejecting it may change what else is accepted, so the order is
    // applied again with every add found so far rejected, until no new one
    // is found; the set only grows, so this ends.
    let mut superseded: BTreeMap<ChangeId, ChangeId> = BTreeMap::new();
    let members = loop {
        resolved.verdicts = before.clone();
        resolved.statements.clear();
        let members = apply_order(
            &mut resolved,
            &folded,
            input,
            judged,
            &superseded,
            start.clone(),
        );
        let found = concurrent_adds(&resolved, &members, &folded, input);
        let new: Vec<(ChangeId, ChangeId)> = found
            .into_iter()
            .filter(|(add, _)| !superseded.contains_key(add))
            .collect();
        if new.is_empty() {
            break members;
        }
        superseded.extend(new);
    };
    resolved.members = members;
    for state in resolved.members.0.values() {
        for grant in &state.grants {
            if lost(state, grant, saw) {
                resolved.verdicts.insert(grant.change, Verdict::Lost);
            }
        }
    }
    reject_duplicate_generations(&mut resolved.verdicts, &folded);
    resolved.heads = current::heads(&resolved.generations, &resolved.verdicts, &folded);
    resolved.current = current::current(&resolved.heads, &resolved.generations, &resolved.order);
    resolved.person_keys = person_keys::settle(
        &resolved.order,
        &folded,
        &mut resolved.verdicts,
        saw,
        |id| judged.provisional(id),
        |id| judged.past_members(id),
    );
    resolved
}

/// Step 5's walk through the order, from `members`, the state after the
/// genesis: each change accepted or rejected, the accepted ones applied.
/// An add in `superseded` is rejected, naming the statement entry that
/// supersedes it.
fn apply_order(
    resolved: &mut Resolved,
    folded: &BTreeMap<ChangeId, &Node>,
    input: &Input,
    judged: &Judged,
    superseded: &BTreeMap<ChangeId, ChangeId>,
    mut members: Members,
) -> Members {
    let saw = |a: ChangeId, b: ChangeId| input.nodes.get(&a).is_some_and(|n| n.past.contains(&b));
    let genesis = resolved.genesis;
    let mut accepted_revocations: Vec<ChangeId> = Vec::new();
    let mut keys = PersonKeys::default();
    for id in resolved
        .order
        .clone()
        .iter()
        .filter(|id| Some(**id) != genesis)
    {
        let node = folded[id];
        let verdict = if !judged.valid(node.id) {
            Verdict::Rejected(Rejection::NotValidWhenWritten)
        } else if let Some(entry) = superseded.get(&node.id) {
            Verdict::Rejected(Rejection::Superseded(*entry))
        } else if let Some(by) = accepted_revocations
            .iter()
            .filter(|_| !final_when_valid(node))
            .find(|r| rights::excludes(folded[r], node, &input.nodes, judged))
        {
            // Reported before a missing right, which the exclusion explains.
            Verdict::Rejected(Rejection::Excluded(*by))
        } else if !final_when_valid(node) && members.role(node.author) < Some(rights::needed(node))
        {
            // A shred, and a finalize, need only to be valid when written
            // (RFC 0008, "A shred is final when accepted"); every other
            // change also needs the right in the state so far.
            Verdict::Rejected(Rejection::NoRight)
        } else if !keys.admits(node) {
            Verdict::Rejected(Rejection::Commitment)
        } else {
            apply(&mut members, node);
            keys.apply(
                node,
                judged.provisional(node.id),
                &judged.past_members(node.id),
                &BTreeSet::new(),
            );
            if let Entry::AccountKeyStatement(statement) = &node.body.entry {
                resolved.statements.insert(node.id, statement.clone());
            }
            members.settle(saw);
            if node.membership().is_some_and(|m| m.cutoff().is_some()) {
                accepted_revocations.push(node.id);
            }
            Verdict::Accepted
        };
        resolved.verdicts.insert(node.id, verdict);
    }
    members
}

/// The accepted adds concurrent with an accepted account key statement
/// entry that supersedes the account key their certificate names, each
/// with the first such entry by change id (RFC 0008, step 5; pm, NOC-233,
/// for the owner's NOC-187 4B): neither is in the other's causal past, and
/// a statement of the state, or the add's own certificate, names that key
/// for the entry's recovery key with a lower number.
fn concurrent_adds(
    resolved: &Resolved,
    members: &Members,
    folded: &BTreeMap<ChangeId, &Node>,
    input: &Input,
) -> Vec<(ChangeId, ChangeId)> {
    let saw = |a: ChangeId, b: ChangeId| input.nodes.get(&a).is_some_and(|n| n.past.contains(&b));
    let certified = members
        .0
        .values()
        .filter_map(|state| state.certificate.statement.as_deref());
    let known: Vec<&AccountKeyStatement> = certified.chain(resolved.statements()).collect();
    let mut found = Vec::new();
    for (id, node) in folded {
        let Some(MembershipEntry {
            action: Action::Add { certificate, .. },
            ..
        }) = node.membership()
        else {
            continue;
        };
        if resolved.verdicts.get(id) != Some(&Verdict::Accepted) {
            continue;
        }
        let key = certificate.account_key;
        let named = |recovery: [u8; 32], number: u64| {
            known
                .iter()
                .copied()
                .chain(certificate.statement.as_deref())
                .any(|s| s.recovery_key == recovery && s.account_key == key && s.number < number)
        };
        let by = resolved.statements.iter().find(|(entry, statement)| {
            !saw(*id, **entry)
                && !saw(**entry, *id)
                && named(statement.recovery_key, statement.number)
        });
        if let Some((entry, _)) = by {
            found.push((*id, *entry));
        }
    }
    found
}

/// Whether `node` is a shred entry or an erasure decision that finalizes,
/// which step 5 accepts when it is valid when written: conditions 2 and 3
/// do not apply to it (RFC 0008, "The function", step 5).
fn final_when_valid(node: &Node) -> bool {
    match &node.body.entry {
        Entry::Shred(_) => true,
        Entry::ErasureDecision(decision) => decision.finalize,
        _ => false,
    }
}

/// Whether an accepted grant is lost (RFC 0008, "Concurrent add and remove
/// of one member"): an accepted entry about the same member caps it below
/// the role it gives, and the two are concurrent, neither in the other's
/// causal past. Every cap is a revocation (RFC 0008, "Roles"), an add's
/// too. A grant that a later removal or demotion saw was revoked on
/// purpose; it stays accepted, though its effective role is lowered all
/// the same.
fn lost(state: &MemberState, grant: &Grant, saw: impl Fn(ChangeId, ChangeId) -> bool) -> bool {
    let caps = state
        .grants
        .iter()
        .map(|g| (g.change, Some(g.role)))
        .chain(state.revocations.iter().map(|r| (r.change, r.role)));
    caps.into_iter().any(|(change, role)| {
        change != grant.change
            && role < Some(grant.role)
            && !saw(grant.change, change)
            && !saw(change, grant.change)
    })
}

/// RFC 0008, "Generation ids": when two accepted key chain entries name the same
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
