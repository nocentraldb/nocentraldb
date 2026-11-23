//! Operation 5, the rich-text update (RFC 0013, "How an edit travels in a
//! change"): a Yjs update in encoding v1 at argument 1, and at argument 2
//! the nonce of each client id whose items it adds. Decoding checks the
//! update's format (NOC-468, B) and that every client id it adds derives
//! from the change's author and its nonce ("Client ids name their author").

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use super::client_id;
use super::update::{self, Range, Run, Shape};
use crate::cbor::{Key, Value};
use crate::change::Operation;
use crate::clock::Hlc;
use crate::field_op::Origin;
use crate::ids::{MemberId, OperationId};
use crate::log::OperationParkReason;
use crate::redact::{Hidden, Size};

/// Operation number 5: the rich-text update (RFC 0004, "Operation
/// numbers"; RFC 0013).
pub const UPDATE: u64 = 5;

/// Argument 1: the update.
const ARGUMENT_UPDATE: u64 = 1;
/// Argument 2: the client ids it adds, each with its nonce.
const ARGUMENT_CLIENTS: u64 = 2;
/// Keys 1 to 3 are RFC 0004's; one operation 5 does not define is
/// malformed. Keys from here to 63 are unknown ones a later rule may
/// define.
const FIRST_UNKNOWN_ARGUMENT: u64 = 4;
/// The last argument key a member must understand.
const LAST_MUST_UNDERSTAND_ARGUMENT: u64 = 63;

/// One rich-text update, with its id, author and stamp.
#[derive(Clone, PartialEq, Eq)]
pub struct Edit {
    /// The operation's id.
    pub id: OperationId,
    /// The member that signed the change.
    pub author: MemberId,
    /// The payload's clock.
    pub stamp: Hlc,
    /// The Yjs update, encoding v1.
    pub update: Vec<u8>,
    /// Each client id whose items the update adds, with its nonce.
    pub clients: BTreeMap<u64, [u8; 8]>,
    pub(super) shape: Shape,
}

impl Edit {
    /// The runs of ids the update adds.
    pub fn runs(&self) -> &[Run] {
        &self.shape.runs
    }

    /// The ids the update's delete set names.
    pub fn deletes(&self) -> &[Range] {
        &self.shape.deletes
    }

    /// Stamp order (RFC 0004, "Tie-break"): the stamp, the author, the
    /// operation id.
    pub fn order(&self) -> (Hlc, MemberId, OperationId) {
        (self.stamp, self.author, self.id)
    }
}

impl fmt::Debug for Edit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Edit")
            .field("id", &self.id)
            .field("author", &self.author)
            .field("stamp", &self.stamp)
            .field("update", &Size(self.update.len(), "bytes"))
            .field("clients", &Hidden)
            .finish()
    }
}

/// The arguments of `edit`: `{1: update, 2: {client id: nonce}}`.
pub(super) fn arguments(edit: &Edit) -> Value {
    let clients = edit
        .clients
        .iter()
        .map(|(client, nonce)| (Key::Unsigned(*client), Value::Bytes(nonce.to_vec())))
        .collect();
    Value::Map(
        [
            (
                Key::Unsigned(ARGUMENT_UPDATE),
                Value::Bytes(edit.update.clone()),
            ),
            (Key::Unsigned(ARGUMENT_CLIENTS), Value::Map(clients)),
        ]
        .into(),
    )
}

/// Reads `operation` as a rich-text update of `origin`'s author, or says
/// why it parks.
pub(super) fn decode(operation: &Operation, origin: Origin) -> Result<Edit, OperationParkReason> {
    use OperationParkReason::{MalformedArguments, UnknownArgument, UnknownOperation};
    if operation.op != UPDATE {
        return Err(UnknownOperation);
    }
    let Some(Value::Map(entries)) = operation.value() else {
        return Err(MalformedArguments);
    };
    let unknown = FIRST_UNKNOWN_ARGUMENT..=LAST_MUST_UNDERSTAND_ARGUMENT;
    if entries
        .keys()
        .any(|key| matches!(key, Key::Unsigned(key) if unknown.contains(key)))
    {
        return Err(UnknownArgument);
    }
    let (mut update, mut clients) = (None, None);
    for (key, value) in &entries {
        match (key, value) {
            (Key::Unsigned(ARGUMENT_UPDATE), Value::Bytes(bytes)) => update = Some(bytes),
            (Key::Unsigned(ARGUMENT_CLIENTS), Value::Map(map)) => clients = Some(map),
            (Key::Unsigned(key), _) if *key > LAST_MUST_UNDERSTAND_ARGUMENT => {}
            _ => return Err(MalformedArguments),
        }
    }
    let (Some(update), Some(clients)) = (update, clients) else {
        return Err(MalformedArguments);
    };
    let clients: BTreeMap<u64, [u8; 8]> = clients
        .iter()
        .map(|entry| match entry {
            (Key::Unsigned(client), Value::Bytes(nonce)) => {
                Some((*client, nonce.as_slice().try_into().ok()?))
            }
            _ => None,
        })
        .collect::<Option<_>>()
        .ok_or(MalformedArguments)?;
    let shape = update::read(update).map_err(|_| MalformedArguments)?;
    // Exactly the client ids the update adds, each its author's.
    let added: BTreeSet<u64> = shape.runs.iter().map(|run| run.range.client).collect();
    let named: BTreeSet<u64> = clients.keys().copied().collect();
    let authors = clients
        .iter()
        .all(|(client, nonce)| client_id(&origin.author, nonce) == *client);
    if added != named || !authors {
        return Err(MalformedArguments);
    }
    Ok(Edit {
        id: origin.id,
        author: origin.author,
        stamp: origin.stamp,
        update: update.clone(),
        clients,
        shape,
    })
}
