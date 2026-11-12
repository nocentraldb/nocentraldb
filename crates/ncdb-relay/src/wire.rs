//! RFC 0007's messages as bytes ("Transport and encoding"): one CBOR map
//! per binary WebSocket message, in RFC 0002's canonical profile. Key 1 is
//! the message's type, as text; key 2 the request id, absent when there is
//! none; the fields follow at keys 3, 4, ... in the order RFC 0007 lists
//! them, and the fields of a nested map at keys 1, 2, ... (the error map of
//! "The errors" is the model). A request with a key its type does not
//! define is malformed.

use std::collections::BTreeMap;

use crate::store::{Entry, Epoch, PersonKeyId, PersonKeyRecord, Position};
use crate::{ErrorCode, Keys, Pushed};
use ncdb_core::cbor::{self, Key, Value};
use ncdb_core::ids::{MemberId, SpaceId};
use ncdb_core::keys::MemberPublicKey;

/// The largest message, in bytes, on every backend (RFC 0007, "Transport
/// and encoding").
pub const MAX_MESSAGE: usize = 128 * 1024;

/// The most frames one push carries (RFC 0007, "Transport and encoding").
pub const MAX_PUSH: usize = 32;

/// The largest frame an entry carries inline: one whose entry still fits
/// into one `entries` message. A larger one comes as a download URL.
pub const MAX_INLINE: usize = MAX_MESSAGE - 1024;

/// The protocol versions this relay speaks.
pub const VERSIONS: [u64; 1] = [1];

/// A request a member sends.
#[derive(Debug, PartialEq, Eq)]
pub enum Request {
    /// The protocol versions the member speaks; its optional client string
    /// is read and dropped.
    Hello {
        /// The versions.
        versions: Vec<u64>,
    },
    /// The member's public keys and its signature over the challenge.
    Auth {
        /// The member's two public keys.
        key: MemberPublicKey,
        /// The Ed25519 signature.
        signature: Vec<u8>,
    },
    /// Frames to append to a space.
    Push {
        /// The request id.
        req: u64,
        /// The space.
        space: SpaceId,
        /// The frames, at most [`MAX_PUSH`].
        frames: Vec<Frame>,
    },
    /// A page of a space's log after a cursor.
    Pull(Cursor),
    /// Follow a space from a cursor; its `limit` is unused.
    Subscribe(Cursor),
    /// Stop following a space.
    Unsubscribe {
        /// The request id.
        req: u64,
        /// The space.
        space: SpaceId,
    },
    /// An upload URL for a large change (RFC 0007, "Large changes").
    Upload {
        /// The request id.
        req: u64,
        /// The space the change will be pushed to.
        space: SpaceId,
        /// The frame's size in bytes.
        size: u64,
    },
    /// `keys`: the member's key envelopes and the key chain (RFC 0007,
    /// "Keys and the person-key store").
    Keys {
        /// The request id.
        req: u64,
        /// The space.
        space: SpaceId,
    },
    /// `person-keys`: the space's person-key records.
    PersonKeys {
        /// The request id.
        req: u64,
        /// The space.
        space: SpaceId,
    },
    /// `put-person-keys`: records to store.
    PutPersonKeys {
        /// The request id.
        req: u64,
        /// The space.
        space: SpaceId,
        /// The records, each `{1: person-key id, 2: generation, 3: writer
        /// member id, 4: wrapped, 5: signature}`.
        records: Vec<PersonKeyRecord>,
    },
    /// Liveness.
    Ping {
        /// The request id.
        req: u64,
    },
}

impl Request {
    /// The request id, if the request has one.
    pub fn req(&self) -> Option<u64> {
        match self {
            Request::Hello { .. } | Request::Auth { .. } => None,
            Request::Push { req, .. }
            | Request::Pull(Cursor { req, .. })
            | Request::Subscribe(Cursor { req, .. })
            | Request::Unsubscribe { req, .. }
            | Request::Upload { req, .. }
            | Request::Keys { req, .. }
            | Request::PersonKeys { req, .. }
            | Request::PutPersonKeys { req, .. }
            | Request::Ping { req } => Some(*req),
        }
    }
}

/// One frame of a push.
#[derive(Debug, PartialEq, Eq)]
pub enum Frame {
    /// The frame's bytes.
    Change(Vec<u8>),
    /// `{1: upload id}`: a large change uploaded before.
    Upload(Vec<u8>),
}

/// Where a pull reads from.
#[derive(Debug, PartialEq, Eq)]
pub struct Cursor {
    /// The request id.
    pub req: u64,
    /// The space.
    pub space: SpaceId,
    /// The epoch the cursor was taken in.
    pub epoch: Epoch,
    /// The cursor.
    pub after: Position,
    /// The most entries the member wants.
    pub limit: u64,
}

/// A message the server cannot read: not canonical CBOR, not a map, an
/// unknown type, a missing or extra field. Its request id when it has one.
#[derive(Debug, PartialEq, Eq)]
pub struct Malformed {
    /// The request id, if the message names one.
    pub req: Option<u64>,
}

/// Reads one message.
pub fn decode(bytes: &[u8]) -> Result<Request, Malformed> {
    let Ok(Value::Map(map)) = cbor::decode(bytes) else {
        return Err(Malformed { req: None });
    };
    let mut fields = Fields(map);
    let req = match fields.take(2) {
        None => None,
        Some(Value::Unsigned(req)) => Some(req),
        Some(_) => return Err(Malformed { req: None }),
    };
    let malformed = Malformed { req };
    let Some(Value::Text(kind)) = fields.take(1) else {
        return Err(malformed);
    };
    let request = read(&kind, req, &mut fields).ok_or(Malformed { req })?;
    if fields.0.is_empty() {
        Ok(request)
    } else {
        Err(Malformed { req })
    }
}

/// The request of this type, from its fields; none when one is missing or
/// of the wrong shape.
fn read(kind: &str, req: Option<u64>, fields: &mut Fields) -> Option<Request> {
    match kind {
        "hello" if req.is_none() => {
            let versions = match fields.take(3)? {
                Value::Array(items) => items.into_iter().map(unsigned).collect::<Option<_>>()?,
                _ => return None,
            };
            match fields.take(4) {
                None | Some(Value::Text(_)) => {}
                Some(_) => return None,
            }
            Some(Request::Hello { versions })
        }
        "auth" if req.is_none() => Some(Request::Auth {
            key: MemberPublicKey {
                signing: fields.array(3)?,
                agreement: fields.array(4)?,
            },
            signature: fields.bytes(5)?,
        }),
        "push" => {
            let space = SpaceId(fields.array(3)?);
            let Value::Array(items) = fields.take(4)? else {
                return None;
            };
            if items.len() > MAX_PUSH {
                return None;
            }
            let frames = items
                .into_iter()
                .map(|item| match item {
                    Value::Bytes(frame) => Some(Frame::Change(frame)),
                    Value::Map(map) => {
                        let mut upload = Fields(map);
                        let id = upload.bytes(1)?;
                        upload.0.is_empty().then_some(Frame::Upload(id))
                    }
                    _ => None,
                })
                .collect::<Option<_>>()?;
            Some(Request::Push {
                req: req?,
                space,
                frames,
            })
        }
        "pull" => Some(Request::Pull(Cursor {
            req: req?,
            space: SpaceId(fields.array(3)?),
            epoch: Epoch(fields.array(4)?),
            after: unsigned(fields.take(5)?)?,
            limit: unsigned(fields.take(6)?)?,
        })),
        "subscribe" => Some(Request::Subscribe(Cursor {
            req: req?,
            space: SpaceId(fields.array(3)?),
            epoch: Epoch(fields.array(4)?),
            after: unsigned(fields.take(5)?)?,
            limit: 0,
        })),
        "unsubscribe" => Some(Request::Unsubscribe {
            req: req?,
            space: SpaceId(fields.array(3)?),
        }),
        "upload" => Some(Request::Upload {
            req: req?,
            space: SpaceId(fields.array(3)?),
            size: unsigned(fields.take(4)?)?,
        }),
        "keys" => Some(Request::Keys {
            req: req?,
            space: SpaceId(fields.array(3)?),
        }),
        "person-keys" => Some(Request::PersonKeys {
            req: req?,
            space: SpaceId(fields.array(3)?),
        }),
        "put-person-keys" => {
            let space = SpaceId(fields.array(3)?);
            let Value::Array(items) = fields.take(4)? else {
                return None;
            };
            let records = items.into_iter().map(read_record).collect::<Option<_>>()?;
            Some(Request::PutPersonKeys {
                req: req?,
                space,
                records,
            })
        }
        "ping" => Some(Request::Ping { req: req? }),
        _ => None,
    }
}

/// A person-key record: its five fields, and no other.
fn read_record(value: Value) -> Option<PersonKeyRecord> {
    let Value::Map(map) = value else {
        return None;
    };
    let mut fields = Fields(map);
    let record = PersonKeyRecord {
        id: PersonKeyId(fields.array(1)?),
        generation: unsigned(fields.take(2)?)?,
        writer: MemberId(fields.array(3)?),
        wrapped: fields.bytes(4)?,
        signature: fields.bytes(5)?,
    };
    fields.0.is_empty().then_some(record)
}

fn unsigned(value: Value) -> Option<u64> {
    match value {
        Value::Unsigned(n) => Some(n),
        _ => None,
    }
}

/// The fields of a request, taken one by one.
struct Fields(BTreeMap<Key, Value>);

impl Fields {
    fn take(&mut self, k: u64) -> Option<Value> {
        self.0.remove(&Key::Unsigned(k))
    }

    fn bytes(&mut self, k: u64) -> Option<Vec<u8>> {
        match self.take(k)? {
            Value::Bytes(bytes) => Some(bytes),
            _ => None,
        }
    }

    fn array<const N: usize>(&mut self, k: u64) -> Option<[u8; N]> {
        self.bytes(k)?.try_into().ok()
    }
}

/// A message the server sends: its type, its request id if any, its fields.
fn message(kind: &str, req: Option<u64>, fields: Vec<(u64, Value)>) -> Vec<u8> {
    let mut map = BTreeMap::from([(Key::Unsigned(1), Value::Text(kind.to_owned()))]);
    if let Some(req) = req {
        map.insert(Key::Unsigned(2), Value::Unsigned(req));
    }
    map.extend(fields.into_iter().map(|(k, v)| (Key::Unsigned(k), v)));
    cbor::encode(&Value::Map(map))
}

/// A map of fields numbered from 1.
fn map(fields: Vec<(u64, Value)>) -> Value {
    Value::Map(
        fields
            .into_iter()
            .map(|(k, v)| (Key::Unsigned(k), v))
            .collect(),
    )
}

fn uint(n: usize) -> Value {
    Value::Unsigned(n as u64)
}

/// `challenge {version, relay id, nonce, limits}`; the limits are the
/// largest message, the most frames per push and the largest frame.
pub fn challenge(version: u64, relay_id: &[u8], nonce: &[u8; 32], limits: [usize; 3]) -> Vec<u8> {
    let [message_size, push, frame] = limits;
    let limits = map(vec![
        (1, uint(message_size)),
        (2, uint(push)),
        (3, uint(frame)),
    ]);
    message(
        "challenge",
        None,
        vec![
            (3, Value::Unsigned(version)),
            (4, Value::Bytes(relay_id.to_vec())),
            (5, Value::Bytes(nonce.to_vec())),
            (6, limits),
        ],
    )
}

/// `welcome {member id, relay epoch, spaces}`.
pub fn welcome(member: &[u8; 32], epoch: &[u8; 16], spaces: &[[u8; 16]]) -> Vec<u8> {
    let spaces = spaces.iter().map(|s| Value::Bytes(s.to_vec())).collect();
    message(
        "welcome",
        None,
        vec![
            (3, Value::Bytes(member.to_vec())),
            (4, Value::Bytes(epoch.to_vec())),
            (5, Value::Array(spaces)),
        ],
    )
}

/// `added {space}`: the member was added to the space (RFC 0007,
/// "Subscribe").
pub fn added(space: SpaceId) -> Vec<u8> {
    message("added", None, vec![(3, Value::Bytes(space.0.to_vec()))])
}

/// `pushed {req, results}`: per frame `{1: position}` when accepted,
/// `{2: position}` when the relay already held it, `{3: code}` when refused.
pub fn pushed(req: u64, results: &[Pushed]) -> Vec<u8> {
    let results = results
        .iter()
        .map(|result| match result {
            Pushed::Accepted(position) => map(vec![(1, Value::Unsigned(*position))]),
            Pushed::Duplicate(position) => map(vec![(2, Value::Unsigned(*position))]),
            Pushed::Rejected(code) => map(vec![(3, Value::Text(code.code().to_owned()))]),
        })
        .collect();
    message("pushed", Some(req), vec![(3, Value::Array(results))])
}

/// An entry as `entries` carries it: its frame inline, or a download URL
/// for a frame too large for one message (RFC 0007, "Large changes").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    /// Its position in the space's log.
    pub position: Position,
    /// When the relay appended it.
    pub accepted_at: u64,
    /// Its frame, or where to fetch it.
    pub carried: Carried,
}

/// How an entry carries its frame.
#[derive(Clone, PartialEq, Eq)]
pub enum Carried {
    /// The frame itself.
    Frame(Vec<u8>),
    /// A short-lived URL that serves the frame, and the frame's size.
    Download {
        /// The download URL.
        url: String,
        /// The frame's size in bytes.
        size: u64,
    },
}

impl std::fmt::Debug for Carried {
    /// Never a frame or a URL's token: their lengths.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Carried::Frame(frame) => write!(f, "Frame({} bytes)", frame.len()),
            Carried::Download { size, .. } => write!(f, "Download({size} bytes)"),
        }
    }
}

/// The size of an `entries` message without entries, plus the most the
/// entries array's length can grow (9 bytes).
pub fn entries_overhead(req: Option<u64>, space: SpaceId, head: Position) -> usize {
    entries(req, space, &[], head).0.len() + 8
}

/// The size of an item's encoding in an `entries` message.
pub fn item_size(item: &Item) -> usize {
    cbor::encode(&encode_item(item)).len()
}

/// `entries {req, space, entries, head}`, without a request id for a live
/// subscription, with as many of `items` as fit into one message, and how
/// many that is. Each entry is `{1: position, 2: accepted at, 3: frame}`,
/// or `{1: position, 2: accepted at, 3: {1: url, 2: size}}` for a frame
/// that comes as a download. The first entry always fits: an entry carries
/// no frame larger than [`MAX_INLINE`].
pub fn entries(
    req: Option<u64>,
    space: SpaceId,
    items: &[Item],
    head: Position,
) -> (Vec<u8>, usize) {
    let items: Vec<Value> = items.iter().map(encode_item).collect();
    let encode = |items: Vec<Value>| {
        message(
            "entries",
            req,
            vec![
                (3, Value::Bytes(space.0.to_vec())),
                (4, Value::Array(items)),
                (5, Value::Unsigned(head)),
            ],
        )
    };
    // The empty page, plus the longest an array's length can grow (9
    // bytes), plus each entry's encoding.
    let mut size = encode(Vec::new()).len() + 8;
    let mut fit = 0;
    for item in &items {
        size += cbor::encode(item).len();
        if fit > 0 && size > MAX_MESSAGE {
            break;
        }
        fit += 1;
    }
    let bytes = encode(items.into_iter().take(fit).collect());
    (bytes, fit)
}

fn encode_item(item: &Item) -> Value {
    let carried = match &item.carried {
        Carried::Frame(frame) => Value::Bytes(frame.clone()),
        Carried::Download { url, size } => map(vec![
            (1, Value::Text(url.clone())),
            (2, Value::Unsigned(*size)),
        ]),
    };
    map(vec![
        (1, Value::Unsigned(item.position)),
        (2, Value::Unsigned(item.accepted_at)),
        (3, carried),
    ])
}

/// An entry of the log, its frame inline: `{1: position, 2: accepted at,
/// 3: frame}`.
fn entry(entry: &Entry) -> Value {
    map(vec![
        (1, Value::Unsigned(entry.position)),
        (2, Value::Unsigned(entry.accepted_at)),
        (3, Value::Bytes(entry.frame.clone())),
    ])
}

/// `keys {req, space, envelopes, chain}`, each an array of entries.
pub fn keys(req: u64, space: SpaceId, keys: &Keys) -> Vec<u8> {
    let entries = |entries: &[Entry]| Value::Array(entries.iter().map(entry).collect());
    message(
        "keys",
        Some(req),
        vec![
            (3, Value::Bytes(space.0.to_vec())),
            (4, entries(&keys.envelopes)),
            (5, entries(&keys.chain)),
        ],
    )
}

/// `person-keys {req, space, records}`.
pub fn person_keys(req: u64, space: SpaceId, records: &[PersonKeyRecord]) -> Vec<u8> {
    let records = records.iter().map(|record| {
        map(vec![
            (1, Value::Bytes(record.id.0.to_vec())),
            (2, Value::Unsigned(record.generation)),
            (3, Value::Bytes(record.writer.0.to_vec())),
            (4, Value::Bytes(record.wrapped.clone())),
            (5, Value::Bytes(record.signature.clone())),
        ])
    });
    message(
        "person-keys",
        Some(req),
        vec![
            (3, Value::Bytes(space.0.to_vec())),
            (4, Value::Array(records.collect())),
        ],
    )
}

/// `upload {req, id, url}`: the upload's id, which a push names, and the
/// one-time URL that takes its frame.
pub fn upload(req: u64, id: &[u8; 16], url: &str) -> Vec<u8> {
    message(
        "upload",
        Some(req),
        vec![
            (3, Value::Bytes(id.to_vec())),
            (4, Value::Text(url.to_owned())),
        ],
    )
}

/// `ok {req}`.
pub fn ok(req: u64) -> Vec<u8> {
    message("ok", Some(req), Vec::new())
}

/// `pong {req}`.
pub fn pong(req: u64) -> Vec<u8> {
    message("pong", Some(req), Vec::new())
}

/// `error {req, code}`: never a frame or any content.
pub fn error(req: Option<u64>, code: ErrorCode) -> Vec<u8> {
    message("error", req, vec![(3, Value::Text(code.code().to_owned()))])
}
