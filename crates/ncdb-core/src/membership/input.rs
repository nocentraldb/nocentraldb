//! What `resolve` folds: the control changes of one space that decode,
//! verify and, for a genesis, meet RFC 0008's rules for one ("How the relay
//! and members recognise it"). Every other change gets its verdict here.

use std::collections::{BTreeMap, BTreeSet};

use crate::change::{Change, Kind};
use crate::ids::{ChangeId, MemberId, SpaceId};
use crate::keys::MemberPublicKey;
use crate::seal;

use super::body::{Action, BodyError, ControlBody, Entry, MembershipEntry};
use super::certificate::MemberKind;
use super::state::{Rejection, Verdict};

/// One control change that takes part in the fold.
pub(crate) struct Node {
    pub(crate) id: ChangeId,
    pub(crate) author: MemberId,
    pub(crate) sequence: u64,
    pub(crate) body: ControlBody,
    /// Its control dependencies that the holder holds as nodes.
    pub(crate) deps: BTreeSet<ChangeId>,
    /// Its causal past: every node its dependencies reach.
    pub(crate) past: BTreeSet<ChangeId>,
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
    pub(crate) nodes: BTreeMap<ChangeId, Node>,
    pub(crate) verdicts: BTreeMap<ChangeId, Verdict>,
    /// Dependencies that name no change the holder holds.
    pub(crate) gaps: BTreeSet<ChangeId>,
}

impl Input {
    pub(crate) fn new<'a>(space: SpaceId, changes: impl IntoIterator<Item = &'a Change>) -> Input {
        let changes: BTreeMap<ChangeId, &Change> = changes
            .into_iter()
            .filter(|c| c.header().kind == Kind::CONTROL && c.header().space == space)
            .map(|c| (c.id(), c))
            .collect();
        let mut verdicts = BTreeMap::new();
        let mut bodies = BTreeMap::new();
        for (id, change) in &changes {
            let header = change.header();
            if !header.foldable() || header.suite != seal::SUITE {
                verdicts.insert(*id, Verdict::Parked);
                continue;
            }
            match ControlBody::decode(&header.payload) {
                Ok(body) => {
                    bodies.insert(*id, body);
                }
                Err(BodyError::NotUnderstood) => {
                    verdicts.insert(*id, Verdict::Parked);
                }
                Err(BodyError::Malformed(_)) => {
                    verdicts.insert(*id, Verdict::Rejected(Rejection::Malformed));
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
                Some(key) if seal::verify(change, key).is_err() => Some(Rejection::BadSignature),
                Some(_) => match &body.entry {
                    Entry::Genesis(_) if !valid_genesis(space, change, &body) => {
                        Some(Rejection::InvalidGenesis)
                    }
                    _ => None,
                },
            };
            if let Some(rejection) = verdict {
                verdicts.insert(id, Verdict::Rejected(rejection));
                continue;
            }
            let node = Node {
                id,
                author: header.author,
                sequence: header.sequence,
                deps: BTreeSet::new(),
                past: BTreeSet::new(),
                body,
            };
            nodes.insert(id, node);
        }
        let held: BTreeSet<ChangeId> = nodes.keys().copied().collect();
        for node in nodes.values_mut() {
            node.deps = node.body.dependencies.intersection(&held).copied().collect();
        }
        let pasts = pasts(&nodes);
        for (id, past) in pasts {
            nodes.get_mut(&id).expect("a node").past = past;
        }
        Input {
            nodes,
            verdicts,
            gaps,
        }
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
/// hashes the dependencies it names.
fn pasts(nodes: &BTreeMap<ChangeId, Node>) -> BTreeMap<ChangeId, BTreeSet<ChangeId>> {
    let mut pasts: BTreeMap<ChangeId, BTreeSet<ChangeId>> = BTreeMap::new();
    let mut stack: Vec<(ChangeId, bool)> = nodes.keys().map(|id| (*id, false)).collect();
    while let Some((id, expanded)) = stack.pop() {
        if pasts.contains_key(&id) {
            continue;
        }
        let deps = &nodes[&id].deps;
        if expanded {
            let mut past = BTreeSet::new();
            for dep in deps {
                past.insert(*dep);
                past.extend(pasts[dep].iter().copied());
            }
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
