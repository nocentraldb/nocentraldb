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
    pub performer: Option<MemberId>,
    /// The account key that signed that shred off (RFC 0008, "Shred
    /// entry").
    pub sign_off: Option<[u8; 32]>,
    /// The members of the state of that shred's causal past: those that
    /// could have held the key, whom the audit of erasures lists (RFC 0009,
    /// "Erasures"; Carsten, 2026-11-12, NOC-246).
    pub past_members: BTreeSet<MemberId>,
    /// The members whose accepted acknowledgements name that shred, and
    /// whether one of them says "deleted" (RFC 0008, "Acknowledgement"):
    /// the audit's evidence of erasure. A member whose acknowledgement the
    /// fold rejected is not listed, as one that never confirmed.
    pub acknowledged: BTreeMap<MemberId, bool>,
    /// The shreds of its id that took effect before that shred and were
    /// taken back, in the order: of two accepted shreds of one person-key
    /// id, the first in the order that is not taken back takes effect
    /// (RFC 0008, "The function"; Carsten, 2026-11-12, NOC-242).
    pub earlier: Vec<EarlierShred>,
}

/// A provisional shred of a person key that was taken back, and that a
/// later shred of the same id replaced: what [`PersonKey`] held of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EarlierShred {
    /// The shred's change id.
    pub id: ChangeId,
    /// The first accepted erasure decision that took it back.
    pub taken_back: Option<ChangeId>,
    /// The member that performed it.
    pub performer: Option<MemberId>,
    /// The account key that signed it off.
    pub sign_off: Option<[u8; 32]>,
    /// The members of the state of its causal past.
    pub past_members: BTreeSet<MemberId>,
    /// The members whose accepted acknowledgements name it, and whether one
    /// of them says "deleted".
    pub acknowledged: BTreeMap<MemberId, bool>,
}

impl EarlierShred {
    /// Its canonical value, in the order of its fields.
    fn to_value(&self) -> Value {
        let members = |set: &BTreeSet<MemberId>| {
            Value::Array(set.iter().map(|m| fields::bytes(&m.0)).collect())
        };
        let acknowledged = self.acknowledged.iter().map(|(member, deleted)| {
            Value::Array(vec![fields::bytes(&member.0), Value::Bool(*deleted)])
        });
        Value::Array(vec![
            fields::bytes(&self.id.0),
            self.taken_back.map_or(Value::Null, |s| fields::bytes(&s.0)),
            self.performer.map_or(Value::Null, |m| fields::bytes(&m.0)),
            self.sign_off.map_or(Value::Null, |k| fields::bytes(&k)),
            members(&self.past_members),
            Value::Array(acknowledged.collect()),
        ])
    }
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
            sign_off: None,
            past_members: BTreeSet::new(),
            earlier: Vec::new(),
        }
    }

    /// Moves the shred that took effect into `earlier`, so that a later
    /// shred of the id can take effect.
    fn retire(&mut self, id: ChangeId) {
        self.earlier.push(EarlierShred {
            id,
            taken_back: self.taken_back.take(),
            performer: self.performer.take(),
            sign_off: self.sign_off.take(),
            past_members: std::mem::take(&mut self.past_members),
            acknowledged: std::mem::take(&mut self.acknowledged),
        });
        self.shred = None;
        self.provisional = false;
        self.finalized = None;
    }

    /// The member that performed the shred that erased it, whose account
    /// decides a provisional one.
    pub fn performer(&self) -> Option<MemberId> {
        self.performer
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
    /// that is not taken back erases it, provisionally when `provisional`
    /// says so, with `past_members` the members of its causal past, and
    /// keeps its id in the state even when no accepted entry of the id is
    /// left. `taken_back` holds the shreds the whole set takes back: a shred
    /// that took effect and is among them makes way for the next shred of
    /// its id, and moves to `earlier` (Carsten, 2026-11-12, NOC-242). The
    /// first decision of each kind on a shred records itself, on the shred
    /// that took effect or an earlier one; a decision on another shred of
    /// the id changes nothing.
    pub(crate) fn apply(
        &mut self,
        node: &Node,
        provisional: bool,
        past_members: &BTreeSet<MemberId>,
        taken_back: &BTreeSet<ChangeId>,
    ) {
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
                if let Some(current) = key.shred.filter(|s| taken_back.contains(s)) {
                    key.retire(current);
                }
                if key.shred.is_none() {
                    key.shred = Some(node.id);
                    key.provisional = provisional;
                    key.performer = Some(node.author);
                    key.sign_off = Some(shred.sign_off.account_key);
                    key.past_members = past_members.clone();
                }
            }
            Entry::ErasureDecision(decision) => {
                let shred = decision.shred;
                let current = self
                    .0
                    .values_mut()
                    .find(|key| key.shred == Some(shred) && key.provisional);
                if let Some(key) = current {
                    let slot = match decision.finalize {
                        true => &mut key.finalized,
                        false => &mut key.taken_back,
                    };
                    slot.get_or_insert(node.id);
                } else if !decision.finalize
                    && let Some(earlier) = self.earlier_mut(shred)
                {
                    earlier.taken_back.get_or_insert(node.id);
                }
            }
            Entry::Other { entry_type, fields } if *entry_type == ACKNOWLEDGEMENT => {
                let named = match fields.get(&8) {
                    Some(Value::Bytes(id)) => <[u8; 32]>::try_from(id.as_slice()).ok(),
                    _ => None,
                };
                let deleted = matches!(fields.get(&9), Some(Value::Bool(true)));
                let Some(shred) = named.map(ChangeId) else {
                    return;
                };
                let current = self.0.values_mut().find(|key| key.shred == Some(shred));
                let acknowledged = match current {
                    Some(key) => Some(&mut key.acknowledged),
                    None => self.earlier_mut(shred).map(|e| &mut e.acknowledged),
                };
                if let Some(acknowledged) = acknowledged {
                    *acknowledged.entry(node.author).or_insert(false) |= deleted;
                }
            }
            _ => {}
        }
    }

    /// The earlier shred of any person key with this change id.
    fn earlier_mut(&mut self, shred: ChangeId) -> Option<&mut EarlierShred> {
        self.0
            .values_mut()
            .flat_map(|key| key.earlier.iter_mut())
            .find(|earlier| earlier.id == shred)
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
                key.performer.map_or(Value::Null, |m| fields::bytes(&m.0)),
                key.sign_off.map_or(Value::Null, |k| fields::bytes(&k)),
                Value::Array(
                    key.past_members
                        .iter()
                        .map(|member| fields::bytes(&member.0))
                        .collect(),
                ),
                Value::Array(
                    key.acknowledged
                        .iter()
                        .map(|(member, deleted)| {
                            Value::Array(vec![fields::bytes(&member.0), Value::Bool(*deleted)])
                        })
                        .collect(),
                ),
                Value::Array(key.earlier.iter().map(EarlierShred::to_value).collect()),
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
/// person keys of the changes accepted then, applied in the order, where of
/// the shreds of one id the first that no accepted take-back names takes
/// effect (Carsten, 2026-11-12, NOC-242).
/// `saw(a, b)` is whether `b` is in `a`'s causal past, and
/// `provisional(shred)` whether a shred is provisional, and
/// `past_members(shred)` the members of its causal past.
pub(crate) fn settle(
    order: &[ChangeId],
    nodes: &BTreeMap<ChangeId, &Node>,
    verdicts: &mut BTreeMap<ChangeId, Verdict>,
    saw: impl Fn(ChangeId, ChangeId) -> bool,
    provisional: impl Fn(ChangeId) -> bool,
    past_members: impl Fn(ChangeId) -> BTreeSet<MemberId>,
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
    let taken_back: BTreeSet<ChangeId> = order
        .iter()
        .filter(|id| accepted(verdicts, id))
        .filter_map(|id| match &nodes[id].body.entry {
            Entry::ErasureDecision(decision) if !decision.finalize => Some(decision.shred),
            _ => None,
        })
        .collect();
    let mut keys = PersonKeys::default();
    for id in order.iter().filter(|id| accepted(verdicts, id)) {
        keys.apply(nodes[id], provisional(*id), &past_members(*id), &taken_back);
    }
    keys
}
