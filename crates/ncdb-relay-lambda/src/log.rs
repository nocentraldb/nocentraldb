//! The one log line of each invocation, as JSON, which Lambda writes to
//! CloudWatch Logs. It holds only ids, sizes, codes and times (RFC 0007,
//! "Services"): the route, the connection id, the member id, the message's
//! type, the bytes in and out, the error and refusal codes, and when and
//! how long. Its fields are typed so: no field can hold a frame, a key or a
//! signature.

use std::fmt::{self, Write as _};

use ncdb_core::ids::MemberId;
use ncdb_relay::ErrorCode;

use crate::event::Route;

/// What one invocation did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    /// The route.
    pub route: Route,
    /// API Gateway's id of the connection.
    pub connection: String,
    /// The member the connection acts as, once it does.
    pub member: Option<MemberId>,
    /// The type of the message the member sent, such as `push`, when it
    /// decoded.
    pub kind: Option<&'static str>,
    /// The size of the message in bytes.
    pub bytes_in: usize,
    /// The bytes posted to connections, all of them.
    pub bytes_out: usize,
    /// The messages posted to connections.
    pub posts: usize,
    /// The codes of the errors sent and of the frames a push refused, in
    /// the order they arose.
    pub codes: Vec<ErrorCode>,
    /// When API Gateway received the event, in milliseconds since the Unix
    /// epoch.
    pub at: u64,
    /// How long the invocation took, in milliseconds.
    pub ms: u64,
}

impl Line {
    /// A line for `route` on `connection`, before anything happened.
    pub fn new(route: Route, connection: &str, at: u64) -> Line {
        Line {
            route,
            connection: connection.to_owned(),
            member: None,
            kind: None,
            bytes_in: 0,
            bytes_out: 0,
            posts: 0,
            codes: Vec::new(),
            at,
            ms: 0,
        }
    }
}

/// API Gateway's connection ids are base64-like; anything else in one is
/// replaced, so the line stays valid JSON whatever the id holds.
fn safe(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "+/=_-".contains(c) {
                c
            } else {
                '?'
            }
        })
        .collect()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut out, b| {
        let _ = write!(out, "{b:02x}");
        out
    })
}

impl fmt::Display for Line {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{{\"route\":\"{}\",\"connection\":\"{}\"",
            self.route.name(),
            safe(&self.connection)
        )?;
        if let Some(member) = self.member {
            write!(f, ",\"member\":\"{}\"", hex(&member.0))?;
        }
        if let Some(kind) = self.kind {
            write!(f, ",\"type\":\"{kind}\"")?;
        }
        let codes: Vec<String> = self
            .codes
            .iter()
            .map(|code| format!("\"{}\"", code.code()))
            .collect();
        write!(
            f,
            ",\"in\":{},\"out\":{},\"posts\":{},\"codes\":[{}],\"at\":{},\"ms\":{}}}",
            self.bytes_in,
            self.bytes_out,
            self.posts,
            codes.join(","),
            self.at,
            self.ms
        )
    }
}
