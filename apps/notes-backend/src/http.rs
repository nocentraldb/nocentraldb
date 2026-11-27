//! The backend's HTTP API, on plain HTTP/1.1 with one request per
//! connection: enough for an example and its tests. An app serves the same
//! routes behind its own login and TLS.
//!
//! - `POST /signup` with `{"email"}` gives `{"session"}`. It stands in for
//!   the app's own signup and login, which NoCentralDB plays no part in.
//! - `POST /admission` with the account's identity as the body gives the
//!   admission's text, `ncdb-admit-1:...`: what the app's `admission`
//!   function returns to the SDK. The backend records the identity first.
//! - `PUT /me/invitations` with a device's invitation text records it.
//! - `GET /users/<email>/invitations` gives `{"identity", "invitations"}`,
//!   or 404 while the user has no device.
//! - `POST /invites` with `{"email", "space", "role"}` records an invite
//!   and gives `{"id", "ready"}`.
//! - `GET /invites/ready` gives the caller's invites whose invited user has
//!   devices to add: `{"invites": [{"id", "email", "space", "role",
//!   "invitations"}]}`.
//! - `POST /invites/<id>/done` removes an invite once it was carried out.
//!
//! Every route but `/signup` takes `Authorization: Bearer <session>`.

use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use crate::State;
use crate::admission::{self, Grant};
use crate::directory::{Ready, Refusal};

/// The longest request head and body the backend reads.
const MAX_REQUEST: usize = 64 * 1024;

/// A request: its method, path, session and body.
struct Request {
    method: String,
    path: String,
    session: Option<String>,
    body: String,
}

/// An answer: its status, content type and body.
struct Answer(u16, &'static str, String);

impl Answer {
    fn json(status: u16, body: Value) -> Answer {
        Answer(status, "application/json", body.to_string())
    }

    fn status(status: u16) -> Answer {
        Answer(status, "text/plain", String::new())
    }

    fn refused(refusal: Refusal) -> Answer {
        match refusal {
            Refusal::OtherIdentity => Answer::status(409),
            Refusal::Malformed => Answer::status(400),
            Refusal::NotFound => Answer::status(404),
        }
    }
}

/// Serves one connection: reads its request and writes the answer.
pub(crate) fn serve(mut stream: TcpStream, state: &State) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    let answer = match read(&mut stream)? {
        Some(request) => route(&request, state),
        None => Answer::status(400),
    };
    let reason = match answer.0 {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        409 => "Conflict",
        _ => "Error",
    };
    write!(
        stream,
        "HTTP/1.1 {} {reason}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        answer.0,
        answer.1,
        answer.2.len(),
        answer.2
    )?;
    stream.flush()
}

/// Reads one request, or none when it is malformed or too long.
fn read(stream: &mut TcpStream) -> io::Result<Option<Request>> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 4096];
    let end = loop {
        if let Some(at) = buffer.windows(4).position(|w| w == b"\r\n\r\n") {
            break at;
        }
        let n = stream.read(&mut chunk)?;
        if n == 0 || buffer.len() > MAX_REQUEST {
            return Ok(None);
        }
        buffer.extend_from_slice(&chunk[..n]);
    };
    let Ok(head) = std::str::from_utf8(&buffer[..end]) else {
        return Ok(None);
    };
    let mut lines = head.split("\r\n");
    let mut first = lines.next().unwrap_or("").split(' ');
    let (method, path) = (
        first.next().unwrap_or("").to_owned(),
        first.next().unwrap_or("").to_owned(),
    );
    let (mut length, mut session) = (0usize, None);
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            return Ok(None);
        };
        let value = value.trim();
        match name.to_ascii_lowercase().as_str() {
            "content-length" => match value.parse() {
                Ok(n) if n <= MAX_REQUEST => length = n,
                _ => return Ok(None),
            },
            "authorization" => session = value.strip_prefix("Bearer ").map(str::to_owned),
            _ => {}
        }
    }
    let mut body = buffer[end + 4..].to_vec();
    while body.len() < length {
        let n = stream.read(&mut chunk)?;
        if n == 0 {
            return Ok(None);
        }
        body.extend_from_slice(&chunk[..n]);
    }
    body.truncate(length);
    Ok(String::from_utf8(body).ok().map(|body| Request {
        method,
        path,
        session,
        body,
    }))
}

/// The answer to `request`.
fn route(request: &Request, state: &State) -> Answer {
    let segments: Vec<&str> = request.path.trim_matches('/').split('/').collect();
    if (request.method.as_str(), segments.as_slice()) == ("POST", ["signup"].as_slice()) {
        return sign_up(&request.body, state);
    }
    let user = request
        .session
        .as_ref()
        .and_then(|s| state.sessions.lock().unwrap().get(s).cloned());
    let Some(user) = user else {
        return Answer::status(401);
    };
    let mut directory = state.directory.lock().unwrap();
    let done =
        |result: Result<(), Refusal>| result.map_or_else(Answer::refused, |()| Answer::status(204));
    match (request.method.as_str(), segments.as_slice()) {
        ("POST", ["admission"]) => {
            let hex = request.body.trim();
            let Some(identity) = admission::identity(hex) else {
                return Answer::status(400);
            };
            if let Err(refusal) = directory.admit(&user, hex) {
                return Answer::refused(refusal);
            }
            Answer(200, "text/plain", admit(state, identity))
        }
        ("PUT", ["me", "invitations"]) => {
            done(directory.add_invitation(&user, request.body.trim()))
        }
        ("GET", ["users", email, "invitations"]) => match directory.lookup(email) {
            Ok((identity, invitations)) => Answer::json(
                200,
                json!({ "identity": identity, "invitations": invitations }),
            ),
            Err(refusal) => Answer::refused(refusal),
        },
        ("POST", ["invites"]) => {
            let Ok(asked) = serde_json::from_str::<Value>(&request.body) else {
                return Answer::status(400);
            };
            let field = |name: &str| asked[name].as_str().unwrap_or("").to_owned();
            match directory.invite(&user, &field("email"), &field("space"), &field("role")) {
                Ok((id, ready)) => {
                    Answer::json(200, json!({ "id": id.to_string(), "ready": ready }))
                }
                Err(refusal) => Answer::refused(refusal),
            }
        }
        ("GET", ["invites", "ready"]) => {
            let invites: Vec<Value> = directory.ready(&user).into_iter().map(ready).collect();
            Answer::json(200, json!({ "invites": invites }))
        }
        ("POST", ["invites", id, "done"]) => match id.parse() {
            Ok(id) => done(directory.done(&user, id)),
            Err(_) => Answer::status(404),
        },
        _ => Answer::status(404),
    }
}

/// Signs up the user the body names and opens a session for them.
fn sign_up(body: &str, state: &State) -> Answer {
    let Ok(asked) = serde_json::from_str::<Value>(body) else {
        return Answer::status(400);
    };
    let email = asked["email"].as_str().unwrap_or("");
    if let Err(refusal) = state.directory.lock().unwrap().sign_up(email) {
        return Answer::refused(refusal);
    }
    let mut token = [0u8; 32];
    if getrandom::fill(&mut token).is_err() {
        return Answer::status(500);
    }
    let session: String = token.iter().map(|b| format!("{b:02x}")).collect();
    state
        .sessions
        .lock()
        .unwrap()
        .insert(session.clone(), email.to_owned());
    Answer::json(200, json!({ "session": session }))
}

/// The admission's text for `identity`, lapsing after the configured
/// lifetime.
fn admit(state: &State, identity: [u8; 32]) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let grant = Grant {
        relay_id: state.config.relay_id,
        identity,
        until: (now + state.config.lifetime).as_secs(),
        spaces: state.config.spaces,
    };
    admission::sign(&state.config.admission_key, &grant)
}

/// An invite that is ready, as JSON.
fn ready(ready: Ready) -> Value {
    json!({
        "id": ready.invite.id.to_string(),
        "email": ready.invite.email,
        "space": ready.invite.space,
        "role": ready.invite.role,
        "invitations": ready.invitations,
    })
}
