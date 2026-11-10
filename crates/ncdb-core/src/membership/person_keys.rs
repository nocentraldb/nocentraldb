//! The person keys of the control state (RFC 0008, "The state"): per
//! person-key id its commitment, the records its accepted person-key
//! entries announce, and the shred that destroyed it, with the two rules of
//! step 5 that apply to the whole set ("The function").

use std::collections::{BTreeMap, BTreeSet};

use crate::cbor::Value;
use crate::ids::ChangeId;

use super::body::Entry;
use super::fields;
use super::input::Node;
use super::person_key::PersonKeyEntry;
use super::state::{Rejection, Verdict};

/// One person key of a space.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PersonKey {
    /// The commitment of the first accepted person-key entry of its id.
    pub commitment: [u8; 32],
    /// The records the accepted person-key entries of its id announce: the
    /// generation each is wrapped under, and its record hash.
    pub records: BTreeSet<(u64, [u8; 32])>,
    /// The accepted shred that destroyed it, the first in the order.
    pub shred: Option<ChangeId>,
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
            .is_none_or(|key| key.commitment == entry.commitment)
    }

    /// What an accepted change does to the person keys: the first entry of
    /// an id creates it, every entry adds its record, and the first shred
    /// destroys it.
    pub(crate) fn apply(&mut self, node: &Node) {
        match &node.body.entry {
            Entry::PersonKey(entry) => {
                let key = self.0.entry(entry.person_key).or_insert(PersonKey {
                    commitment: entry.commitment,
                    records: BTreeSet::new(),
                    shred: None,
                });
                key.records.insert((entry.generation, entry.record_hash));
            }
            Entry::Shred(shred) => {
                if let Some(key) = self.0.get_mut(&shred.person_key) {
                    key.shred.get_or_insert(node.id);
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
                fields::bytes(&key.commitment),
                Value::Array(records.collect()),
                key.shred.map_or(Value::Null, |s| fields::bytes(&s.0)),
            ])
        });
        Value::Array(keys.collect())
    }
}

/// The rules of step 5 for the whole set, after the order is applied: a
/// person-key entry that an accepted shred of its id did not see is
/// rejected, wherever it falls in the order. Returns the person keys of the
/// changes accepted then, applied in the order. `saw(a, b)` is whether `b`
/// is in `a`'s causal past.
pub(crate) fn settle(
    order: &[ChangeId],
    nodes: &BTreeMap<ChangeId, &Node>,
    verdicts: &mut BTreeMap<ChangeId, Verdict>,
    saw: impl Fn(ChangeId, ChangeId) -> bool,
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
    let mut keys = PersonKeys::default();
    for id in order.iter().filter(|id| accepted(verdicts, id)) {
        keys.apply(nodes[id]);
    }
    keys
}
