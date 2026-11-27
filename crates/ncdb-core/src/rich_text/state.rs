//! A rich-text field's state (RFC 0013, "Folding", "The state"): the
//! document as one Yjs update of its whole state, encoded v1 with garbage
//! collection and every map in key order, which holds pending items too; for each operation it took,
//! its author, its stamp, the runs of ids it added and the ids its delete
//! set named; the same of each rejected operation it holds as
//! placeholders (RFC 0013, "Un-folding a rejected change"); and the client
//! ids with their nonces. Its canonical CBOR is what `fs_<collection>`
//! stores.

use std::collections::BTreeMap;
use std::fmt;

use super::edit::Edit;
use super::update::{Range, Run};
use crate::cbor::{self, Key, Value};
use crate::clock::Hlc;
use crate::ids::{ChangeId, MemberId, OperationId};
use crate::redact::{Hidden, Size};

/// The state of one rich-text field. `Default` is a field nobody wrote.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct State {
    /// The document's whole state as a Yjs update, empty before the first
    /// operation.
    pub(super) document: Vec<u8>,
    /// What each operation the state took added and deleted.
    pub(super) edits: BTreeMap<OperationId, Record>,
    /// Each client id the operations added items under, with its nonce.
    pub(super) clients: BTreeMap<u64, [u8; 8]>,
    /// What each rejected operation the state holds as placeholders added
    /// and deleted.
    pub(super) placeholders: BTreeMap<OperationId, Record>,
}

/// What the state keeps of one operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Record {
    pub(super) author: MemberId,
    pub(super) stamp: Hlc,
    pub(super) runs: Vec<Run>,
    pub(super) deletes: Vec<Range>,
}

impl Record {
    pub(super) fn of(edit: &Edit) -> Record {
        Record {
            author: edit.author,
            stamp: edit.stamp,
            runs: edit.runs().to_vec(),
            deletes: edit.deletes().to_vec(),
        }
    }

    /// Stamp order, as [`Edit::order`].
    pub(super) fn order(&self, id: OperationId) -> (Hlc, MemberId, OperationId) {
        (self.stamp, self.author, id)
    }
}

/// Bytes that are no rich-text state the core wrote.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Corrupt;

const DOCUMENT: u64 = 1;
const EDITS: u64 = 2;
const CLIENTS: u64 = 3;
const PLACEHOLDERS: u64 = 4;

impl State {
    /// The document's whole state as a Yjs update, encoding v1, empty for a
    /// field nobody wrote: what a binding loads its `Y.Doc` from.
    pub fn document(&self) -> &[u8] {
        &self.document
    }

    /// Whether the document holds a node: one that no deletion removed.
    /// A field nobody wrote, and one whose every block was deleted, hold
    /// none (RFC 0013, "The first content").
    pub fn holds_content(&self) -> bool {
        super::document::holds_content(&self.document)
    }

    /// The canonical CBOR: `{1: document, 2: [[[change, position], author,
    /// stamp, [[client, clock, len, digest]…], [[client, clock, len]…]]…],
    /// 3: {client: nonce}, 4: [placeholders, as 2]}`, operations in id
    /// order, and key 4 only when the state holds a placeholder. Not a
    /// wire format.
    pub fn bytes(&self) -> Vec<u8> {
        let clients = self
            .clients
            .iter()
            .map(|(client, nonce)| (Key::Unsigned(*client), Value::Bytes(nonce.to_vec())))
            .collect();
        let mut map: BTreeMap<Key, Value> = [
            (Key::Unsigned(DOCUMENT), Value::Bytes(self.document.clone())),
            (Key::Unsigned(EDITS), records_value(&self.edits)),
            (Key::Unsigned(CLIENTS), Value::Map(clients)),
        ]
        .into();
        if !self.placeholders.is_empty() {
            map.insert(
                Key::Unsigned(PLACEHOLDERS),
                records_value(&self.placeholders),
            );
        }
        cbor::encode(&Value::Map(map))
    }

    /// Reads the bytes [`State::bytes`] wrote.
    pub fn from_bytes(bytes: &[u8]) -> Result<State, Corrupt> {
        let Value::Map(mut map) = cbor::decode(bytes).map_err(|_| Corrupt)? else {
            return Err(Corrupt);
        };
        let mut take = |key| map.remove(&Key::Unsigned(key)).ok_or(Corrupt);
        let (Value::Bytes(document), Value::Array(edits), Value::Map(clients)) =
            (take(DOCUMENT)?, take(EDITS)?, take(CLIENTS)?)
        else {
            return Err(Corrupt);
        };
        let placeholders = match map.remove(&Key::Unsigned(PLACEHOLDERS)) {
            None => Vec::new(),
            // Written only when the state holds one.
            Some(Value::Array(placeholders)) if !placeholders.is_empty() => placeholders,
            Some(_) => return Err(Corrupt),
        };
        if !map.is_empty() {
            return Err(Corrupt);
        }
        Ok(State {
            document,
            edits: edits.into_iter().map(record).collect::<Result<_, _>>()?,
            placeholders: placeholders
                .into_iter()
                .map(record)
                .collect::<Result<_, _>>()?,
            clients: clients
                .into_iter()
                .map(|entry| match entry {
                    (Key::Unsigned(client), Value::Bytes(nonce)) => {
                        Ok((client, nonce.try_into().map_err(|_| Corrupt)?))
                    }
                    _ => Err(Corrupt),
                })
                .collect::<Result<_, _>>()?,
        })
    }
}

impl fmt::Debug for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("State")
            .field("document", &Hidden)
            .field("size", &Size(self.document.len(), "bytes"))
            .field("edits", &self.edits.len())
            .field("clients", &self.clients.len())
            .field("placeholders", &self.placeholders.len())
            .finish()
    }
}

fn records_value(records: &BTreeMap<OperationId, Record>) -> Value {
    Value::Array(
        records
            .iter()
            .map(|(id, record)| {
                Value::Array(vec![
                    Value::Array(vec![
                        Value::Bytes(id.change.0.to_vec()),
                        Value::Unsigned(id.position),
                    ]),
                    Value::Bytes(record.author.0.to_vec()),
                    Value::Unsigned(record.stamp.0),
                    Value::Array(record.runs.iter().map(run_value).collect()),
                    Value::Array(record.deletes.iter().map(range_value).collect()),
                ])
            })
            .collect(),
    )
}

fn run_value(run: &Run) -> Value {
    let Value::Array(mut items) = range_value(&run.range) else {
        unreachable!("a range is an array")
    };
    items.push(Value::Bytes(run.digest.to_vec()));
    Value::Array(items)
}

fn range_value(range: &Range) -> Value {
    Value::Array(vec![
        Value::Unsigned(range.client),
        Value::Unsigned(range.clock),
        Value::Unsigned(range.len),
    ])
}

fn record(value: Value) -> Result<(OperationId, Record), Corrupt> {
    let Value::Array(items) = value else {
        return Err(Corrupt);
    };
    let [id, author, stamp, runs, deletes] = <[Value; 5]>::try_from(items).map_err(|_| Corrupt)?;
    let id = match id {
        Value::Array(pair) => match <[Value; 2]>::try_from(pair) {
            Ok([Value::Bytes(change), Value::Unsigned(position)]) => OperationId {
                change: ChangeId(change.try_into().map_err(|_| Corrupt)?),
                position,
            },
            _ => return Err(Corrupt),
        },
        _ => return Err(Corrupt),
    };
    let (Value::Bytes(author), Value::Unsigned(stamp), Value::Array(runs), Value::Array(deletes)) =
        (author, stamp, runs, deletes)
    else {
        return Err(Corrupt);
    };
    let runs = runs
        .into_iter()
        .map(|value| match value {
            Value::Array(mut items) if items.len() == 4 => {
                let Some(Value::Bytes(digest)) = items.pop() else {
                    return Err(Corrupt);
                };
                Ok(Run {
                    range: range(Value::Array(items))?,
                    digest: digest.try_into().map_err(|_| Corrupt)?,
                })
            }
            _ => Err(Corrupt),
        })
        .collect::<Result<_, _>>()?;
    Ok((
        id,
        Record {
            author: MemberId(author.try_into().map_err(|_| Corrupt)?),
            stamp: Hlc(stamp),
            runs,
            deletes: deletes.into_iter().map(range).collect::<Result<_, _>>()?,
        },
    ))
}

fn range(value: Value) -> Result<Range, Corrupt> {
    match value {
        Value::Array(items) => match <[Value; 3]>::try_from(items) {
            Ok(
                [
                    Value::Unsigned(client),
                    Value::Unsigned(clock),
                    Value::Unsigned(len),
                ],
            ) => Ok(Range { client, clock, len }),
            _ => Err(Corrupt),
        },
        _ => Err(Corrupt),
    }
}
