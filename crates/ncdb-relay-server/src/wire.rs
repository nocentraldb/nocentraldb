//! RFC 0007's messages as bytes ("Transport and encoding"): one CBOR map
//! per binary WebSocket message, in RFC 0002's canonical profile. Key 1 is
//! the message's type, as text; key 2 the request id, absent when there is
//! none; the fields follow at keys 3, 4, ... in the order RFC 0007 lists
//! them, and the fields of a nested map at keys 1, 2, ... (the error map of
//! "The errors" is the model). A request with a key its type does not
//! define is malformed.

use std::collections::BTreeMap;

use ncdb_core::cbor::{self, Key, Value};
use ncdb_core::keys::MemberPublicKey;
use ncdb_relay::ErrorCode;

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
    /// Liveness.
    Ping {
        /// The request id.
        req: u64,
    },
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

/// `pong {req}`.
pub fn pong(req: u64) -> Vec<u8> {
    message("pong", Some(req), Vec::new())
}

/// `error {req, code}`: never a frame or any content.
pub fn error(req: Option<u64>, code: ErrorCode) -> Vec<u8> {
    message("error", req, vec![(3, Value::Text(code.code().to_owned()))])
}
