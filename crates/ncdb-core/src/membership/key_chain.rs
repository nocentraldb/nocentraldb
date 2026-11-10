//! The key chain entry of RFC 0008 ("Key chain entry", entry type 3): a
//! rotation's new generation id, each previous generation's key encrypted
//! under the new generation, and the revocations the rotation answers.
//!
//! ```text
//! 8  generation   uint, the new generation id
//! 9  previous     array of {1: generation id, 2: encrypted key}, sorted by
//!                 generation id, at least one
//! 10 answers      array of change ids, sorted by bytes
//! ```
//!
//! Whether an encrypted key is the right one only a holder of the new key
//! can tell, a local check (RFC 0008, "What the fold may read"), so the fold
//! keeps the bytes as they are.

use std::collections::BTreeSet;
use std::fmt;

use crate::cbor::{Key, Value};
use crate::change::ChangeError;
use crate::ids::ChangeId;

use super::fields::{self, Fields};

/// The body keys of entry type 3.
const GENERATION: u64 = 8;
const PREVIOUS: u64 = 9;
const ANSWERS: u64 = 10;

/// A key chain entry (entry type 3). Its `Debug` output shows generation
/// ids and change ids, and each encrypted key only by its length.
#[derive(Clone, PartialEq, Eq)]
pub struct KeyChainEntry {
    /// Key 8: the new generation id.
    pub generation: u64,
    /// Key 9: the previous generations, sorted by generation id.
    pub previous: Vec<PreviousKey>,
    /// Key 10: the revocations this rotation answers.
    pub answers: BTreeSet<ChangeId>,
}

/// One previous generation of a key chain entry.
#[derive(Clone, PartialEq, Eq)]
pub struct PreviousKey {
    /// Key 1: its generation id.
    pub generation: u64,
    /// Key 2: its key, encrypted under the new generation's key-chain
    /// subkey (RFC 0003).
    pub key: Vec<u8>,
}

impl KeyChainEntry {
    /// The ids of the previous generations, in order.
    pub fn previous_generations(&self) -> impl Iterator<Item = u64> + '_ {
        self.previous.iter().map(|p| p.generation)
    }

    pub(crate) fn fields(&self) -> Vec<(u64, Value)> {
        let previous = self.previous.iter().map(|p| {
            fields::map([
                (1, Value::Unsigned(p.generation)),
                (2, fields::bytes(&p.key)),
            ])
        });
        let answers = self.answers.iter().map(|id| fields::bytes(&id.0));
        vec![
            (GENERATION, Value::Unsigned(self.generation)),
            (PREVIOUS, Value::Array(previous.collect())),
            (ANSWERS, Value::Array(answers.collect())),
        ]
    }

    /// Takes keys 8 to 10 out of the map. Every key is required; the
    /// previous generations are sorted by id, at least one and none twice,
    /// each a map of exactly keys 1 and 2, so each entry has one encoding.
    pub(crate) fn take(map: &mut Fields) -> Result<KeyChainEntry, ChangeError> {
        let generation = fields::unsigned(map.required(GENERATION)?)?;
        let previous = previous(map.required(PREVIOUS)?.1)?;
        let answers = fields::change_ids(map.required(ANSWERS)?)?;
        Ok(KeyChainEntry {
            generation,
            previous,
            answers,
        })
    }
}

fn previous(value: Value) -> Result<Vec<PreviousKey>, ChangeError> {
    let invalid = ChangeError::InvalidValue(PREVIOUS);
    let Value::Array(items) = value else {
        return Err(invalid);
    };
    let mut previous: Vec<PreviousKey> = Vec::with_capacity(items.len());
    for item in items {
        let Value::Map(entries) = item else {
            return Err(invalid);
        };
        let (Some(Value::Unsigned(generation)), Some(Value::Bytes(key)), 2) = (
            entries.get(&Key::Unsigned(1)),
            entries.get(&Key::Unsigned(2)),
            entries.len(),
        ) else {
            return Err(invalid);
        };
        if previous.last().is_some_and(|p| p.generation >= *generation) {
            return Err(invalid);
        }
        previous.push(PreviousKey {
            generation: *generation,
            key: key.clone(),
        });
    }
    if previous.is_empty() {
        return Err(invalid);
    }
    Ok(previous)
}

impl fmt::Debug for KeyChainEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KeyChainEntry")
            .field("generation", &self.generation)
            .field("previous", &self.previous)
            .field("answers", &self.answers)
            .finish()
    }
}

impl fmt::Debug for PreviousKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreviousKey")
            .field("generation", &self.generation)
            .field("key_len", &self.key.len())
            .finish()
    }
}
