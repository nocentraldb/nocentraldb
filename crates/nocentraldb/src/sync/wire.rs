//! The member's side of RFC 0007's messages ("The keys of each message"):
//! the requests the SDK writes and the answers it reads, each one CBOR map
//! with the type at key 1, the request id at key 2 when it has one, then
//! its fields at keys 3, 4, ... in the RFC's order.

use std::collections::BTreeMap;

use ncdb_core::cbor::{self, Key, Value};
use ncdb_core::ids::SpaceId;

/// The protocol versions the SDK speaks.
pub(crate) const VERSIONS: [u64; 1] = [1];

/// The largest message, in bytes (RFC 0007, decision R4).
pub(crate) const MAX_MESSAGE: usize = 128 * 1024;

/// The most frames one push carries (RFC 0007, decision R4).
pub(crate) const MAX_PUSH: usize = 32;

/// What a message holds besides its frames: the type, the request id, the
/// space id and CBOR's own bytes, with room to spare.
const OVERHEAD: usize = 1024;

/// The error codes RFC 0007 lists ("The errors").
const CODES: [&str; 20] = [
    "unsupported_version",
    "unauthenticated",
    "not_member",
    "removed_member",
    "unknown_space",
    "space_exists",
    "not_allowed",
    "malformed",
    "too_large",
    "unsupported_suite",
    "bad_signature",
    "author_not_member",
    "membership_rule",
    "too_early",
    "epoch_changed",
    "bad_record",
    "compacted",
    "bad_cursor",
    "rate_limited",
    "unavailable",
];

/// What the SDK keeps of a code the relay sent that RFC 0007 does not
/// list (NOC-146).
const UNKNOWN_ERROR: &str = "unknown_error";

/// What the SDK keeps of `code`, which the relay sent: the code itself
/// when RFC 0007 lists it, else [`UNKNOWN_ERROR`], so the relay's own text
/// never reaches the app. The one mapping for every code the relay sends:
/// the sync status shows it (NOC-146), and the log records a refusal of a
/// change by it (NOC-188).
pub(crate) fn mapped_code(code: &str) -> String {
    let shown = if CODES.contains(&code) {
        code
    } else {
        UNKNOWN_ERROR
    };
    shown.to_owned()
}

/// The client string `hello` carries (RFC 0007, "Opening a connection").
const CLIENT: &str = concat!("ncdb-rs/", env!("CARGO_PKG_VERSION"));

fn message(kind: &str, req: Option<u64>, fields: Vec<Value>) -> Vec<u8> {
    let mut map = BTreeMap::new();
    map.insert(Key::Unsigned(1), Value::Text(kind.to_owned()));
    if let Some(req) = req {
        map.insert(Key::Unsigned(2), Value::Unsigned(req));
    }
    for (key, value) in (3..).zip(fields) {
        map.insert(Key::Unsigned(key), value);
    }
    cbor::encode(&Value::Map(map))
}

pub(crate) fn hello() -> Vec<u8> {
    let versions = VERSIONS.iter().map(|v| Value::Unsigned(*v)).collect();
    message(
        "hello",
        None,
        vec![Value::Array(versions), Value::Text(CLIENT.to_owned())],
    )
}

pub(crate) fn auth(signing: [u8; 32], agreement: [u8; 32], signature: [u8; 64]) -> Vec<u8> {
    message(
        "auth",
        None,
        vec![
            Value::Bytes(signing.to_vec()),
            Value::Bytes(agreement.to_vec()),
            Value::Bytes(signature.to_vec()),
        ],
    )
}

pub(crate) fn push(req: u64, space: SpaceId, frames: &[Vec<u8>]) -> Vec<u8> {
    let frames = frames.iter().map(|f| Value::Bytes(f.clone())).collect();
    message(
        "push",
        Some(req),
        vec![Value::Bytes(space.0.to_vec()), Value::Array(frames)],
    )
}

pub(crate) fn pull(req: u64, space: SpaceId, epoch: [u8; 16], after: u64) -> Vec<u8> {
    message(
        "pull",
        Some(req),
        vec![
            Value::Bytes(space.0.to_vec()),
            Value::Bytes(epoch.to_vec()),
            Value::Unsigned(after),
            Value::Unsigned(256),
        ],
    )
}

pub(crate) fn subscribe(req: u64, space: SpaceId, epoch: [u8; 16], after: u64) -> Vec<u8> {
    message(
        "subscribe",
        Some(req),
        vec![
            Value::Bytes(space.0.to_vec()),
            Value::Bytes(epoch.to_vec()),
            Value::Unsigned(after),
        ],
    )
}

/// A person-key record (RFC 0007, `record`): `{1: person-key id, 2:
/// generation, 3: writer member id, 4: wrapped, 5: signature}`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Record {
    pub(crate) person_key: [u8; 16],
    pub(crate) generation: u64,
    pub(crate) writer: [u8; 32],
    pub(crate) wrapped: Vec<u8>,
    pub(crate) signature: [u8; 64],
}

impl Record {
    fn value(&self) -> Value {
        let fields = [
            Value::Bytes(self.person_key.to_vec()),
            Value::Unsigned(self.generation),
            Value::Bytes(self.writer.to_vec()),
            Value::Bytes(self.wrapped.clone()),
            Value::Bytes(self.signature.to_vec()),
        ];
        Value::Map((1..).map(Key::Unsigned).zip(fields).collect())
    }
}

pub(crate) fn person_keys(req: u64, space: SpaceId) -> Vec<u8> {
    message(
        "person-keys",
        Some(req),
        vec![Value::Bytes(space.0.to_vec())],
    )
}

pub(crate) fn put_person_keys(req: u64, space: SpaceId, records: &[Record]) -> Vec<u8> {
    message(
        "put-person-keys",
        Some(req),
        vec![
            Value::Bytes(space.0.to_vec()),
            Value::Array(records.iter().map(Record::value).collect()),
        ],
    )
}

pub(crate) fn ping(req: u64) -> Vec<u8> {
    message("ping", Some(req), Vec::new())
}

/// The frames of `frames`, in order, in groups that each fit into one
/// push: at most [`MAX_PUSH`] frames, and at most [`MAX_MESSAGE`] bytes. A
/// frame too large for any push goes alone, and the relay answers
/// `too_large`.
pub(crate) fn batches(frames: Vec<Vec<u8>>) -> Vec<Vec<Vec<u8>>> {
    let mut batches: Vec<Vec<Vec<u8>>> = Vec::new();
    let mut size = 0;
    for frame in frames {
        let fits = batches.last().is_some_and(|batch| {
            batch.len() < MAX_PUSH && size + frame.len() + 16 <= MAX_MESSAGE - OVERHEAD
        });
        if fits {
            size += frame.len() + 16;
            batches.last_mut().expect("a batch").push(frame);
        } else {
            size = frame.len() + 16;
            batches.push(vec![frame]);
        }
    }
    batches
}

/// One entry of a space's log, as `entries` carries it.
pub(crate) struct Entry {
    pub(crate) position: u64,
    pub(crate) frame: Vec<u8>,
}

/// The result of one frame of a push.
pub(crate) enum Pushed {
    Accepted(u64),
    Duplicate(u64),
    Rejected(String),
}

/// An answer, or a message the relay sends on its own.
pub(crate) enum Answer {
    Challenge {
        relay_id: Vec<u8>,
        nonce: [u8; 32],
    },
    Welcome {
        epoch: [u8; 16],
        spaces: Vec<SpaceId>,
    },
    Pushed {
        req: u64,
        results: Vec<Pushed>,
    },
    Entries {
        req: Option<u64>,
        space: SpaceId,
        entries: Vec<Entry>,
        head: u64,
    },
    /// The member was added to `space` while connected (RFC 0007,
    /// decision R13).
    Added {
        space: SpaceId,
    },
    Error {
        req: Option<u64>,
        code: String,
        /// How many seconds to wait before retrying, for `rate_limited`.
        retry_after: Option<u64>,
    },
    /// The person-key records of a space (RFC 0007, `person-keys`).
    PersonKeys {
        req: u64,
        records: Vec<Record>,
    },
    /// `pong`, `ok` or a type this SDK does not read.
    Other {
        req: Option<u64>,
    },
}

impl Answer {
    /// The request id it answers.
    pub(crate) fn req(&self) -> Option<u64> {
        match self {
            Answer::Pushed { req, .. } | Answer::PersonKeys { req, .. } => Some(*req),
            Answer::Entries { req, .. } | Answer::Error { req, .. } | Answer::Other { req } => *req,
            Answer::Challenge { .. } | Answer::Welcome { .. } | Answer::Added { .. } => None,
        }
    }
}

/// A message the relay sent that the SDK cannot read.
#[derive(Debug)]
pub(crate) struct Unreadable;

fn field(map: &BTreeMap<Key, Value>, key: u64) -> Result<&Value, Unreadable> {
    map.get(&Key::Unsigned(key)).ok_or(Unreadable)
}

fn uint(value: &Value) -> Result<u64, Unreadable> {
    match value {
        Value::Unsigned(n) => Ok(*n),
        _ => Err(Unreadable),
    }
}

fn bytes(value: &Value) -> Result<&[u8], Unreadable> {
    match value {
        Value::Bytes(bytes) => Ok(bytes),
        _ => Err(Unreadable),
    }
}

fn fixed<const N: usize>(value: &Value) -> Result<[u8; N], Unreadable> {
    bytes(value)?.try_into().map_err(|_| Unreadable)
}

fn array(value: &Value) -> Result<&[Value], Unreadable> {
    match value {
        Value::Array(items) => Ok(items),
        _ => Err(Unreadable),
    }
}

fn map(value: &Value) -> Result<&BTreeMap<Key, Value>, Unreadable> {
    match value {
        Value::Map(map) => Ok(map),
        _ => Err(Unreadable),
    }
}

fn text(value: &Value) -> Result<&str, Unreadable> {
    match value {
        Value::Text(text) => Ok(text),
        _ => Err(Unreadable),
    }
}

/// Reads one message from the relay.
pub(crate) fn decode(message: &[u8]) -> Result<Answer, Unreadable> {
    let value = cbor::decode(message).map_err(|_| Unreadable)?;
    let m = map(&value)?;
    let req = match m.get(&Key::Unsigned(2)) {
        Some(req) => Some(uint(req)?),
        None => None,
    };
    Ok(match text(field(m, 1)?)? {
        "challenge" => Answer::Challenge {
            relay_id: bytes(field(m, 4)?)?.to_vec(),
            nonce: fixed(field(m, 5)?)?,
        },
        "welcome" => Answer::Welcome {
            epoch: fixed(field(m, 4)?)?,
            spaces: array(field(m, 5)?)?
                .iter()
                .map(|space| fixed(space).map(SpaceId))
                .collect::<Result<_, _>>()?,
        },
        "pushed" => Answer::Pushed {
            req: req.ok_or(Unreadable)?,
            results: array(field(m, 3)?)?
                .iter()
                .map(result)
                .collect::<Result<_, _>>()?,
        },
        "entries" => Answer::Entries {
            req,
            space: SpaceId(fixed(field(m, 3)?)?),
            entries: array(field(m, 4)?)?
                .iter()
                .map(entry)
                .collect::<Result<_, _>>()?,
            head: uint(field(m, 5)?)?,
        },
        "person-keys" => Answer::PersonKeys {
            req: req.ok_or(Unreadable)?,
            records: array(field(m, 4)?)?
                .iter()
                .map(record)
                .collect::<Result<_, _>>()?,
        },
        "added" => Answer::Added {
            space: SpaceId(fixed(field(m, 3)?)?),
        },
        "error" => Answer::Error {
            req,
            code: text(field(m, 3)?)?.to_owned(),
            retry_after: match m.get(&Key::Unsigned(4)) {
                Some(seconds) => Some(uint(seconds)?),
                None => None,
            },
        },
        _ => Answer::Other { req },
    })
}

fn record(value: &Value) -> Result<Record, Unreadable> {
    let m = map(value)?;
    Ok(Record {
        person_key: fixed(field(m, 1)?)?,
        generation: uint(field(m, 2)?)?,
        writer: fixed(field(m, 3)?)?,
        wrapped: bytes(field(m, 4)?)?.to_vec(),
        signature: fixed(field(m, 5)?)?,
    })
}

fn result(value: &Value) -> Result<Pushed, Unreadable> {
    let m = map(value)?;
    if let Some(position) = m.get(&Key::Unsigned(1)) {
        return Ok(Pushed::Accepted(uint(position)?));
    }
    if let Some(position) = m.get(&Key::Unsigned(2)) {
        return Ok(Pushed::Duplicate(uint(position)?));
    }
    Ok(Pushed::Rejected(text(field(m, 3)?)?.to_owned()))
}

/// An entry whose key 3 holds the frame. A download link, which a relay
/// that carries large changes sends instead, is not read yet.
fn entry(value: &Value) -> Result<Entry, Unreadable> {
    let m = map(value)?;
    Ok(Entry {
        position: uint(field(m, 1)?)?,
        frame: bytes(field(m, 3)?)?.to_vec(),
    })
}
