//! What `resolve` folds: the control changes of one space that decode,
//! verify and, for a genesis, meet RFC 0008's rules for one ("How the relay
//! and members recognise it"). Every other change gets its verdict here.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::change::{Change, Kind};
use crate::ids::{ChangeId, MemberId, SpaceId};
use crate::keys::MemberPublicKey;
use crate::seal;

use super::body::{Action, BodyError, ControlBody, Entry, MembershipEntry};
use super::certificate::MemberKind;
use super::past::Past;
use super::state::{Rejection, Verdict};

/// One control change that takes part in the fold.
pub(crate) struct Node {
    pub(crate) id: ChangeId,
    pub(crate) author: MemberId,
    pub(crate) sequence: u64,
    /// Its number in the [`Memo`], which its causal past is a set of.
    pub(crate) number: u32,
    pub(crate) body: ControlBody,
    /// Its control dependencies that the holder holds as nodes.
    pub(crate) deps: BTreeSet<ChangeId>,
    /// Its causal past: the numbers of every node its dependencies reach.
    /// Shared with the [`Memo`], which hands the same set to the next call
    /// while it holds.
    pub(crate) past: Arc<Past>,
    /// Whether it or a node of its causal past names a dependency the
    /// holder does not hold: the holder does not hold its whole causal
    /// past yet.
    pub(crate) gap: bool,
}

impl Node {
    pub(crate) fn is_genesis(&self) -> bool {
        matches!(self.body.entry, Entry::Genesis(_))
    }

    pub(crate) fn membership(&self) -> Option<&MembershipEntry> {
        match &self.body.entry {
            Entry::Membership(entry) => Some(entry),
            _ => None,
        }
    }
}

/// The nodes of a space, and the verdicts of the changes that are not.
pub(crate) struct Input {
    pub(crate) space: SpaceId,
    pub(crate) nodes: BTreeMap<ChangeId, Node>,
    pub(crate) verdicts: BTreeMap<ChangeId, Verdict>,
    /// Dependencies that name no change the holder holds.
    pub(crate) gaps: BTreeSet<ChangeId>,
    /// The change id of each node, by its number.
    ids: BTreeMap<u32, ChangeId>,
    /// The public keys the geneses and add entries name, by member id.
    keys: BTreeMap<MemberId, MemberPublicKey>,
}

/// What [`Input::new`] remembers from one call to the next (NOC-424): what
/// a change gives on its own, and each node's causal past. Every entry is
/// a function of what it is keyed by, so a call that uses it builds the
/// input a call without it builds.
#[derive(Default)]
pub(crate) struct Memo {
    /// What each change's header gives: its body, or its verdict. A change
    /// id hashes its header, so the id names it.
    read: BTreeMap<ChangeId, Read>,
    /// Whether each change verifies under its author's key, and for a
    /// genesis meets the rules for one, with the signature it was checked
    /// by: the change id does not hash the signature. Kept once the
    /// author's key is known; a member id hashes its key, so the key is
    /// the same in every call.
    signed: BTreeMap<ChangeId, (Vec<u8>, Option<Rejection>)>,
    /// Each node's causal past, with the pasts of its dependencies among
    /// the nodes that it was built from.
    pasts: BTreeMap<ChangeId, Built>,
    /// Each node's number, given when the node is first seen, after its
    /// dependencies (NOC-442): a number names one change id in every call.
    numbers: BTreeMap<ChangeId, u32>,
}

impl Memo {
    /// Whether `change` verifies under `key`, its author's, and for a
    /// genesis meets RFC 0008's rules for one, as [`signed`] says, kept
    /// with the signature it was checked by.
    fn signed(
        &mut self,
        space: SpaceId,
        change: &Change,
        body: &ControlBody,
        key: &MemberPublicKey,
    ) -> Option<Rejection> {
        let id = change.id();
        match self.signed.get(&id) {
            Some((signature, verdict)) if signature == change.signature() => *verdict,
            _ => {
                let verdict = signed(space, change, body, key);
                let signature = change.signature().to_vec();
                self.signed.insert(id, (signature, verdict));
                verdict
            }
        }
    }

    /// The causal past of the node `id`, given the past of each of its
    /// dependencies: the one built before while each dependency has the
    /// very same past, else a new one, built and kept.
    fn past(&mut self, id: ChangeId, deps: &BTreeMap<ChangeId, Arc<Past>>) -> Arc<Past> {
        let same = self.pasts.get(&id).filter(|before| {
            before.deps.len() == deps.len()
                && deps.iter().all(|(dep, past)| {
                    before
                        .deps
                        .get(dep)
                        .is_some_and(|before| Arc::ptr_eq(before, past))
                })
        });
        if let Some(before) = same {
            return Arc::clone(&before.past);
        }
        let mut past = Past::default();
        for (dep, of_dep) in deps {
            past.insert(self.numbers[dep]);
            past.extend(of_dep);
        }
        let past = Arc::new(past);
        let built = Built {
            deps: deps.clone(),
            past: Arc::clone(&past),
        };
        self.pasts.insert(id, built);
        past
    }
}

/// A node's causal past, and the past of each of its dependencies it was
/// built from: while each dependency has the very same past, so has the
/// node.
struct Built {
    deps: BTreeMap<ChangeId, Arc<Past>>,
    past: Arc<Past>,
}

/// What a change's header gives on its own.
#[derive(Clone)]
enum Read {
    Body(Box<ControlBody>),
    Verdict(Verdict),
}

/// What `change`'s header gives on its own: its body, or the verdict of a
/// change `resolve` does not fold.
fn read(change: &Change) -> Read {
    let header = change.header();
    if !header.foldable() || header.suite != seal::SUITE {
        return Read::Verdict(Verdict::Parked);
    }
    match ControlBody::decode(&header.payload) {
        Ok(body) => Read::Body(Box::new(body)),
        Err(BodyError::NotUnderstood) => Read::Verdict(Verdict::Parked),
        Err(BodyError::Malformed(_)) => Read::Verdict(Verdict::Rejected(Rejection::Malformed)),
    }
}

/// Whether `change` verifies under `key`, its author's, and for a genesis
/// meets RFC 0008's rules for one.
fn signed(
    space: SpaceId,
    change: &Change,
    body: &ControlBody,
    key: &MemberPublicKey,
) -> Option<Rejection> {
    if seal::verify(change, key).is_err() {
        return Some(Rejection::BadSignature);
    }
    match &body.entry {
        Entry::Genesis(_) if !valid_genesis(space, change, body) => Some(Rejection::InvalidGenesis),
        _ => None,
    }
}

impl Input {
    pub(crate) fn new<'a>(
        space: SpaceId,
        changes: impl IntoIterator<Item = &'a Change>,
        memo: &mut Memo,
    ) -> Input {
        let changes: BTreeMap<ChangeId, &Change> = changes
            .into_iter()
            .filter(|c| c.header().kind == Kind::CONTROL && c.header().space == space)
            .map(|c| (c.id(), c))
            .collect();
        let mut verdicts = BTreeMap::new();
        let mut bodies = BTreeMap::new();
        for (id, change) in &changes {
            let read = memo.read.entry(*id).or_insert_with(|| read(change));
            match read {
                Read::Body(body) => {
                    bodies.insert(*id, (**body).clone());
                }
                Read::Verdict(verdict) => {
                    verdicts.insert(*id, *verdict);
                }
            }
        }
        // Every decoded body's dependencies count, so that the holder also
        // asks for what a change it cannot judge yet builds on.
        let gaps: BTreeSet<ChangeId> = bodies
            .values()
            .flat_map(|body| body.dependencies.iter().copied())
            .filter(|dep| !changes.contains_key(dep))
            .collect();
        let keys = public_keys(bodies.values());
        let mut nodes = BTreeMap::new();
        for (id, body) in bodies {
            let change = changes[&id];
            let header = change.header();
            let verdict = match keys.get(&header.author) {
                None => Some(Rejection::UnknownAuthor),
                Some(key) => memo.signed(space, change, &body, key),
            };
            if let Some(rejection) = verdict {
                verdicts.insert(id, Verdict::Rejected(rejection));
                continue;
            }
            let node = Node {
                id,
                author: header.author,
                sequence: header.sequence,
                number: 0,
                deps: BTreeSet::new(),
                past: Arc::default(),
                gap: false,
                body,
            };
            nodes.insert(id, node);
        }
        let held: BTreeSet<ChangeId> = nodes.keys().copied().collect();
        for node in nodes.values_mut() {
            node.deps = node
                .body
                .dependencies
                .intersection(&held)
                .copied()
                .collect();
        }
        let pasts = pasts(&nodes, memo);
        let missing: BTreeSet<ChangeId> = nodes
            .values()
            .filter(|node| {
                node.body
                    .dependencies
                    .iter()
                    .any(|d| !changes.contains_key(d))
            })
            .map(|node| node.id)
            .collect();
        // A node's dependencies have smaller pasts than it, so they come
        // first here; a node of its past misses a dependency exactly when
        // one of its dependencies or the node itself does.
        let mut order: Vec<(usize, ChangeId)> =
            pasts.iter().map(|(id, p)| (p.len(), *id)).collect();
        order.sort_unstable();
        for (_, id) in order {
            let node = &nodes[&id];
            let gap = missing.contains(&id) || node.deps.iter().any(|dep| nodes[dep].gap);
            let node = nodes.get_mut(&id).expect("a node");
            node.gap = gap;
        }
        let mut ids = BTreeMap::new();
        for (id, past) in pasts {
            let node = nodes.get_mut(&id).expect("a node");
            node.number = memo.numbers[&id];
            node.past = past;
            ids.insert(node.number, id);
        }
        Input {
            space,
            nodes,
            verdicts,
            gaps,
            ids,
            keys,
        }
    }

    /// How many control changes of the space it holds, nodes or not.
    pub(crate) fn len(&self) -> usize {
        self.nodes.len() + self.verdicts.len()
    }

    /// The node `change` would be if it were added to the changes this
    /// input was built from (NOC-497), when it is a person-key entry of the
    /// space that this input does not hold, that no change of it depends
    /// on, whose author's key it holds, that verifies, and whose
    /// dependencies are all nodes without a gap. None otherwise. It reads
    /// nothing of the other changes but those dependencies, and changes
    /// only `memo`, whose entries are functions of what they are keyed by.
    pub(crate) fn person_key_node(&self, change: &Change, memo: &mut Memo) -> Option<Node> {
        let header = change.header();
        let id = change.id();
        if header.kind != Kind::CONTROL
            || header.space != self.space
            || self.nodes.contains_key(&id)
            || self.verdicts.contains_key(&id)
            || self.gaps.contains(&id)
        {
            return None;
        }
        let Read::Body(body) = memo.read.entry(id).or_insert_with(|| read(change)).clone() else {
            return None;
        };
        if !matches!(body.entry, Entry::PersonKey(_)) {
            return None;
        }
        let deps = &body.dependencies;
        let held = deps
            .iter()
            .all(|dep| self.nodes.get(dep).is_some_and(|node| !node.gap));
        if !held {
            return None;
        }
        let key = self.keys.get(&header.author)?;
        if memo.signed(self.space, change, &body, key).is_some() {
            return None;
        }
        let next = u32::try_from(memo.numbers.len()).expect("fewer than 2^32 control changes");
        let number = *memo.numbers.entry(id).or_insert(next);
        let pasts: BTreeMap<ChangeId, Arc<Past>> = deps
            .iter()
            .map(|dep| (*dep, Arc::clone(&self.nodes[dep].past)))
            .collect();
        let past = memo.past(id, &pasts);
        Some(Node {
            id,
            author: header.author,
            sequence: header.sequence,
            number,
            deps: deps.clone(),
            past,
            gap: false,
            body: *body,
        })
    }

    /// Adds `node`, which [`Input::person_key_node`] gave.
    pub(crate) fn insert(&mut self, node: Node) {
        self.ids.insert(node.number, node.id);
        self.nodes.insert(node.id, node);
    }

    /// Whether the node `b` is in the causal past of the node `a`; false
    /// when either is no node.
    pub(crate) fn saw(&self, a: ChangeId, b: ChangeId) -> bool {
        match (self.nodes.get(&a), self.nodes.get(&b)) {
            (Some(a), Some(b)) => a.past.contains(b.number),
            _ => false,
        }
    }

    /// The nodes of `past`, by change id.
    pub(crate) fn nodes_of<'a>(&'a self, past: &'a Past) -> impl Iterator<Item = &'a Node> + 'a {
        past.iter().map(|number| &self.nodes[&self.ids[&number]])
    }
}

/// The public keys that the geneses and add entries name, by member id.
/// Keys hash to the member id, so every entry that names one gives the
/// same keys.
fn public_keys<'a>(
    bodies: impl Iterator<Item = &'a ControlBody>,
) -> BTreeMap<MemberId, MemberPublicKey> {
    bodies
        .filter_map(|body| match &body.entry {
            Entry::Genesis(genesis) => Some(genesis.creator),
            Entry::Membership(MembershipEntry {
                action: Action::Add { key, .. },
                ..
            }) => Some(*key),
            _ => None,
        })
        .map(|key| (key.id(), key))
        .collect()
}

/// RFC 0008's rules for a genesis: author sequence 1, no dependencies (the
/// body checks that), the space id derived from its creator and nonce, its
/// author the creator, a certificate the account signed for the creator,
/// and the first generation id derived as "Generation ids" says.
fn valid_genesis(space: SpaceId, change: &Change, body: &ControlBody) -> bool {
    let Entry::Genesis(genesis) = &body.entry else {
        return false;
    };
    let header = change.header();
    let certificate = &genesis.certificate;
    header.sequence == 1
        && genesis.space_id() == space
        && genesis.creator.id() == header.author
        && certificate.member == header.author
        && certificate.kind != MemberKind::SubAgent
        && certificate.verify(&certificate.account_key)
        && genesis.first_generation == genesis.expected_first_generation()
}

/// The causal past of every node, in an order where a node's dependencies
/// come first; a cycle of dependencies cannot occur, since a change id
/// hashes the dependencies it names. A node seen for the first time gets
/// the next number, after its dependencies. A node whose dependencies have
/// the very pasts `memo` built its past from keeps that past: the same set,
/// not built again.
fn pasts(nodes: &BTreeMap<ChangeId, Node>, memo: &mut Memo) -> BTreeMap<ChangeId, Arc<Past>> {
    let mut pasts: BTreeMap<ChangeId, Arc<Past>> = BTreeMap::new();
    let mut stack: Vec<(ChangeId, bool)> = nodes.keys().map(|id| (*id, false)).collect();
    while let Some((id, expanded)) = stack.pop() {
        if pasts.contains_key(&id) {
            continue;
        }
        let deps = &nodes[&id].deps;
        if expanded {
            let next = u32::try_from(memo.numbers.len()).expect("fewer than 2^32 control changes");
            memo.numbers.entry(id).or_insert(next);
            let of_deps = deps.iter().map(|d| (*d, Arc::clone(&pasts[d]))).collect();
            let past = memo.past(id, &of_deps);
            pasts.insert(id, past);
        } else {
            stack.push((id, true));
            stack.extend(
                deps.iter()
                    .filter(|d| !pasts.contains_key(d))
                    .map(|d| (*d, false)),
            );
        }
    }
    pasts
}
