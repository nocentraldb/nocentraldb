//! RFC 0007's messages as bytes ("Transport and encoding"): one CBOR map
//! per binary WebSocket message, in RFC 0002's canonical profile. Key 1 is
//! the message's type, as text; key 2 the request id, absent when there is
//! none; the fields follow at keys 3, 4, ... in the order RFC 0007 lists
//! them, and the fields of a nested map at keys 1, 2, ... (the error map of
//! "The errors" is the model). A request with a key its type does not
//! define is malformed.

use std::collections::BTreeMap;

use ncdb_core::cbor::{self, Key, Value};
use ncdb_core::ids::SpaceId;
use ncdb_core::keys::MemberPublicKey;
use ncdb_relay::store::{Entry, Epoch, Position};
use ncdb_relay::{ErrorCode, Pushed};

use crate::{MAX_MESSAGE, MAX_PUSH};

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
        frames: Vec<Vec<u8>>,
    },
    /// A page of a space's log after a cursor.
    Pull(Cursor),
    /// Liveness.
    Ping {
        /// The request id.
        req: u64,
    },
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
                    Value::Bytes(frame) => Some(frame),
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
        "ping" => Some(Request::Ping { req: req? }),
        _ => None,
    }
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

/// `entries {req, space, entries, head}`, without a request id for a live
/// subscription, with as many of `entries` as fit into one message, and how
/// many that is. Each entry is `{1: position, 2: accepted at, 3: frame}`.
/// The first entry always fits: the server accepts no frame larger than
/// [`crate::MAX_FRAME`].
pub fn entries(
    req: Option<u64>,
    space: SpaceId,
    entries: &[Entry],
    head: Position,
) -> (Vec<u8>, usize) {
    let items: Vec<Value> = entries
        .iter()
        .map(|entry| {
            map(vec![
                (1, Value::Unsigned(entry.position)),
                (2, Value::Unsigned(entry.accepted_at)),
                (3, Value::Bytes(entry.frame.clone())),
            ])
        })
        .collect();
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

/// `pong {req}`.
pub fn pong(req: u64) -> Vec<u8> {
    message("pong", Some(req), Vec::new())
}

/// `error {req, code}`: never a frame or any content.
pub fn error(req: Option<u64>, code: ErrorCode) -> Vec<u8> {
    message("error", req, vec![(3, Value::Text(code.code().to_owned()))])
}
