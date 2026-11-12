//! The person keys of the control state (RFC 0008, "The state"): per
//! person-key id its commitment, the records its accepted person-key
//! entries announce, and the shred that erased it, provisionally or for
//! good, with the decision that ended a provisional one; and the rules of
//! step 5 that apply to the whole set ("The function").

use std::collections::{BTreeMap, BTreeSet};

use crate::cbor::Value;
use crate::ids::{ChangeId, MemberId};

use super::body::{ACKNOWLEDGEMENT, Entry};
use super::fields;
use super::input::Node;
use super::person_key::PersonKeyEntry;
use super::state::{Rejection, Verdict};

/// One person key of a space: created by its first accepted person-key
/// entry, or named by an accepted shred alone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PersonKey {
    /// The commitment of the first accepted person-key entry of its id;
    /// none when no accepted entry of its id is left, which happens only to
    /// a person key an accepted shred destroyed (its entries lost to a
    /// concurrent first entry, which the shred then rejected).
    pub commitment: Option<[u8; 32]>,
    /// The records the accepted person-key entries of its id announce: the
    /// generation each is wrapped under, and its record hash.
    pub records: BTreeSet<(u64, [u8; 32])>,
    /// The accepted shred that erased it, the first in the order.
    pub shred: Option<ChangeId>,
    /// Whether that shred is provisional: the performer's own account
    /// signed it off, in a space whose members all belonged to it (RFC
    /// 0008, "A provisional erasure in a space with one account").
    pub provisional: bool,
    /// The first accepted erasure decision that finalizes that shred.
    pub finalized: Option<ChangeId>,
    /// The first accepted erasure decision that takes that shred back. None
    /// is accepted once the set holds an accepted finalize.
    pub taken_back: Option<ChangeId>,
    /// The member that performed that shred, whose account decides it.
    pub(crate) performer: Option<MemberId>,
    /// The members whose accepted acknowledgements name that shred, and
    /// whether one of them says "deleted" (RFC 0008, "Acknowledgement"):
    /// the audit's evidence of erasure. A member whose acknowledgement the
    /// fold rejected is not listed, as one that never confirmed.
    pub acknowledged: BTreeMap<MemberId, bool>,
}

impl PersonKey {
    fn new() -> PersonKey {
        PersonKey {
            commitment: None,
            records: BTreeSet::new(),
            shred: None,
            provisional: false,
            acknowledged: BTreeMap::new(),
            finalized: None,
            taken_back: None,
            performer: None,
        }
    }

    /// Whether the view treats the person's fields as erased: an accepted
    /// shred erased them, and no accepted take-back restored them.
    pub fn erased(&self) -> bool {
        self.shred.is_some() && self.taken_back.is_none()
    }

    /// Whether members destroy the person key and the relay deletes its
    /// record: a shred is final, or a provisional one is finalized. A
    /// provisional shred alone never destroys the key (RFC 0008, "A shred is
    /// final when accepted").
    pub fn destroyed(&self) -> bool {
        self.shred.is_some() && (!self.provisional || self.finalized.is_some())
    }
}

/// The person keys, by person-key id.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct PersonKeys(pub(crate) BTreeMap<[u8; 16], PersonKey>);

impl PersonKeys {
    /// Whether a change may be accepted here: a person-key entry repeats
    /// the commitment of its id, once the id exists.
    pub(crate) fn admits(&self, node: &Node) -> bool {
        match &node.body.entry {
            Entry::PersonKey(entry) => self.repeats(entry),
            _ => true,
        }
    }

    /// Whether an entry repeats the commitment of its id, if it exists.
    pub(crate) fn repeats(&self, entry: &PersonKeyEntry) -> bool {
        self.0
            .get(&entry.person_key)
            .and_then(|key| key.commitment)
            .is_none_or(|commitment| commitment == entry.commitment)
    }

    /// What an accepted change does to the person keys: the first entry of
    /// an id creates it, every entry adds its record, and the first shred
    /// erases it, provisionally when `provisional` says so, and keeps its id
    /// in the state even when no accepted entry of the id is left. The first
    /// decision of each kind on that shred records itself; a decision on
    /// another shred of the id changes nothing.
    pub(crate) fn apply(&mut self, node: &Node, provisional: bool) {
        match &node.body.entry {
            Entry::PersonKey(entry) => {
                let key = self
                    .0
                    .entry(entry.person_key)
                    .or_insert_with(PersonKey::new);
                key.commitment.get_or_insert(entry.commitment);
                key.records.insert((entry.generation, entry.record_hash));
            }
            Entry::Shred(shred) => {
                let key = self
                    .0
                    .entry(shred.person_key)
                    .or_insert_with(PersonKey::new);
                if key.shred.is_none() {
                    key.shred = Some(node.id);
                    key.provisional = provisional;
                    key.performer = Some(node.author);
                }
            }
            Entry::ErasureDecision(decision) => {
                let key = self
                    .0
                    .values_mut()
                    .find(|key| key.shred == Some(decision.shred) && key.provisional);
                if let Some(key) = key {
                    let slot = match decision.finalize {
                        true => &mut key.finalized,
                        false => &mut key.taken_back,
                    };
                    slot.get_or_insert(node.id);
                }
            }
            Entry::Other { entry_type, fields } if *entry_type == ACKNOWLEDGEMENT => {
                let named = match fields.get(&8) {
                    Some(Value::Bytes(id)) => <[u8; 32]>::try_from(id.as_slice()).ok(),
                    _ => None,
                };
                let deleted = matches!(fields.get(&9), Some(Value::Bool(true)));
                let key = named.and_then(|named| {
                    self.0
                        .values_mut()
                        .find(|key| key.shred == Some(ChangeId(named)))
                });
                if let Some(key) = key {
                    *key.acknowledged.entry(node.author).or_insert(false) |= deleted;
                }
            }
            _ => {}
        }
    }

    /// The canonical value of the person keys, in id order.
    pub(crate) fn to_value(&self) -> Value {
        let keys = self.0.iter().map(|(id, key)| {
            let records = key.records.iter().map(|(generation, hash)| {
                Value::Array(vec![Value::Unsigned(*generation), fields::bytes(hash)])
            });
            Value::Array(vec![
                fields::bytes(id),
                key.commitment.map_or(Value::Null, |c| fields::bytes(&c)),
                Value::Array(records.collect()),
                key.shred.map_or(Value::Null, |s| fields::bytes(&s.0)),
                Value::Bool(key.provisional),
                key.finalized.map_or(Value::Null, |s| fields::bytes(&s.0)),
                key.taken_back.map_or(Value::Null, |s| fields::bytes(&s.0)),
                Value::Array(
                    key.acknowledged
                        .iter()
                        .map(|(member, deleted)| {
                            Value::Array(vec![fields::bytes(&member.0), Value::Bool(*deleted)])
                        })
                        .collect(),
                ),
            ])
        });
        Value::Array(keys.collect())
    }
}

/// The rules of step 5 for the whole set, after the order is applied: a
/// person-key entry that an accepted shred of its id did not see is
/// rejected, wherever it falls in the order; and a take-back of a
/// provisional shred is rejected when the set holds an accepted finalize of
/// it, the first in the order (Carsten, 2026-11-11, NOC-230). Returns the
/// person keys of the changes accepted then, applied in the order.
/// `saw(a, b)` is whether `b` is in `a`'s causal past, and
/// `provisional(shred)` whether a shred is provisional.
pub(crate) fn settle(
    order: &[ChangeId],
    nodes: &BTreeMap<ChangeId, &Node>,
    verdicts: &mut BTreeMap<ChangeId, Verdict>,
    saw: impl Fn(ChangeId, ChangeId) -> bool,
    provisional: impl Fn(ChangeId) -> bool,
) -> PersonKeys {
    let accepted =
        |verdicts: &BTreeMap<ChangeId, Verdict>, id| verdicts.get(id) == Some(&Verdict::Accepted);
    let mut shreds: BTreeMap<[u8; 16], Vec<ChangeId>> = BTreeMap::new();
    for id in order.iter().filter(|id| accepted(verdicts, id)) {
        if let Entry::Shred(shred) = &nodes[id].body.entry {
            shreds.entry(shred.person_key).or_default().push(*id);
        }
    }
    for id in order {
        let Entry::PersonKey(entry) = &nodes[id].body.entry else {
            continue;
        };
        let unseen = shreds
            .get(&entry.person_key)
            .and_then(|shreds| shreds.iter().find(|shred| !saw(**shred, *id)));
        if let Some(shred) = unseen.filter(|_| accepted(verdicts, id)) {
            verdicts.insert(*id, Verdict::Rejected(Rejection::Shredded(*shred)));
        }
    }
    let mut finalizes: BTreeMap<ChangeId, ChangeId> = BTreeMap::new();
    for id in order.iter().filter(|id| accepted(verdicts, id)) {
        if let Entry::ErasureDecision(decision) = &nodes[id].body.entry
            && decision.finalize
        {
            finalizes.entry(decision.shred).or_insert(*id);
        }
    }
    for id in order {
        let Entry::ErasureDecision(decision) = &nodes[id].body.entry else {
            continue;
        };
        let finalize = finalizes
            .get(&decision.shred)
            .filter(|_| !decision.finalize);
        if let Some(finalize) = finalize.filter(|_| accepted(verdicts, id)) {
            verdicts.insert(*id, Verdict::Rejected(Rejection::Finalized(*finalize)));
        }
    }
    let mut keys = PersonKeys::default();
    for id in order.iter().filter(|id| accepted(verdicts, id)) {
        keys.apply(nodes[id], provisional(*id));
    }
    keys
}
