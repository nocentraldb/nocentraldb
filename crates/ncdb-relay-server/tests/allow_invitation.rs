//! `ncdb-relay --allow-invitation <ncdb-invite-1:…>` (NOC-257; the owner's
//! NOC-239 A, Carsten, 2026-11-12): the relay reads the account key from an
//! invitation's text, RFC 0003's layout ("Invitation"), and lets that
//! account's members create spaces, as `--allow-account` does with the key
//! in hex. A developer who uses only the public SDK starts the relay with
//! their app's invitation. A text that is not an invitation stops the relay
//! at start, with a message that names the flag and repeats nothing of the
//! text.

mod support;

use std::io::{BufRead, BufReader};
use std::net::SocketAddr;
use std::process::{Child, Command, Stdio};

use ncdb_core::cbor::Value;
use ncdb_core::membership::{Certificate, MemberKind};
use support::space::{Space, account, key};
use support::*;

/// The invitation of RFC 0003's vectors: its account is not the simulated
/// account, whose members the tests sign as.
const VECTOR: &str = include_str!("../../../docs/rfc/vectors/0003/invitation.txt");

/// The invitation, with an account key statement, of RFC 0003's vectors.
const VECTOR_STATEMENT: &str =
    include_str!("../../../docs/rfc/vectors/0003/invitation-statement.txt");

/// A running `ncdb-relay`, killed on drop.
struct Running(Child);

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Starts `ncdb-relay` on the database in `dir` and a free local port, with
/// `args` after the database and the address.
fn start(dir: &TempDir, args: &[&str]) -> (Running, SocketAddr) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_ncdb-relay"))
        .arg("--database")
        .arg(dir.database())
        .args(["--listen", "127.0.0.1:0"])
        .args(args)
        .stdout(Stdio::piped())
        .spawn()
        .expect("ncdb-relay starts");
    let mut line = String::new();
    let stdout = child.stdout.take().expect("its output");
    BufReader::new(stdout).read_line(&mut line).expect("a line");
    let running = Running(child);
    let addr = line
        .trim()
        .strip_prefix("ncdb-relay listening on ws://")
        .unwrap_or_else(|| panic!("unexpected output: {line}"))
        .parse()
        .expect("an address");
    (running, addr)
}

/// Unpadded base64url (RFC 4648, section 5) of `bytes`.
fn base64url(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut text = String::new();
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, b)| n | u32::from(*b) << (16 - 8 * i));
        for i in 0..=chunk.len() {
            text.push(ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char);
        }
    }
    text
}

/// RFC 0003's invitation layout of simulated member `who`, certified by the
/// simulated account, without an account key statement.
fn layout(who: u8) -> Vec<u8> {
    let member = key(who);
    let public = member.public();
    let certificate = Certificate::sign(&account(), member.id(), MemberKind::Device);
    let mut bytes = vec![1];
    bytes.extend_from_slice(&public.signing);
    bytes.extend_from_slice(&public.agreement);
    bytes.extend_from_slice(&certificate.account_key);
    bytes.push(1);
    bytes.extend_from_slice(&certificate.signature);
    bytes.push(0);
    bytes
}

/// The invitation text of simulated member `who`.
fn invitation(who: u8) -> String {
    format!("ncdb-invite-1:{}", base64url(&layout(who)))
}

/// The bytes of a vector's text.
fn vector_bytes(text: &str) -> Vec<u8> {
    let hex = match text.trim() {
        t if t == VECTOR.trim() => include_str!("../../../docs/rfc/vectors/0003/invitation.hex"),
        _ => include_str!("../../../docs/rfc/vectors/0003/invitation-statement.hex"),
    };
    let hex: String = hex.split_whitespace().collect();
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("hex"))
        .collect()
}

/// Whether simulated member 0 may create a space on the relay at `addr`.
async fn creates_a_space(addr: SocketAddr) -> Value {
    let (mut client, _) = Client::member(addr, 0).await;
    let space = Space::new(0);
    client
        .push(1, space.space, &[&space.genesis])
        .await
        .remove(0)
}

#[tokio::test(flavor = "multi_thread")]
async fn an_account_an_invitation_names_may_create_a_space() {
    let dir = TempDir::new();
    let (_running, addr) = start(&dir, &["--allow-invitation", &invitation(0)]);
    assert_eq!(creates_a_space(addr).await, accepted(1));
}

#[tokio::test(flavor = "multi_thread")]
async fn an_invitation_of_another_member_of_the_account_admits_the_account() {
    // The invitation is of member 7; member 0 of the same account creates.
    let dir = TempDir::new();
    let (_running, addr) = start(&dir, &["--allow-invitation", &invitation(7)]);
    assert_eq!(creates_a_space(addr).await, accepted(1));
}

#[tokio::test(flavor = "multi_thread")]
async fn another_account_may_not_create_a_space() {
    let dir = TempDir::new();
    let (_running, addr) = start(
        &dir,
        &[
            "--allow-invitation",
            VECTOR.trim(),
            "--allow-invitation",
            VECTOR_STATEMENT.trim(),
        ],
    );
    assert_eq!(creates_a_space(addr).await, rejected("not_allowed"));
}

#[tokio::test(flavor = "multi_thread")]
async fn invitations_and_account_keys_may_be_given_together_and_more_than_once() {
    let dir = TempDir::new();
    let (_running, addr) = start(
        &dir,
        &[
            "--allow-invitation",
            VECTOR.trim(),
            "--allow-account",
            &account_hex(),
            "--allow-invitation",
            VECTOR_STATEMENT.trim(),
        ],
    );
    assert_eq!(creates_a_space(addr).await, accepted(1));
}

/// Runs `ncdb-relay --allow-invitation <text>`, which must stop at start:
/// its exit code and what it wrote to stderr.
fn refused(text: &str) -> (Option<i32>, String) {
    let dir = TempDir::new();
    let output = Command::new(env!("CARGO_BIN_EXE_ncdb-relay"))
        .arg("--database")
        .arg(dir.database())
        .args(["--listen", "127.0.0.1:0", "--allow-invitation", text])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .expect("ncdb-relay runs");
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// The texts RFC 0003 has a reader refuse, each named.
fn malformed() -> Vec<(&'static str, String)> {
    let good = layout(0);
    let text = |bytes: &[u8]| format!("ncdb-invite-1:{}", base64url(bytes));
    let changed = |at: usize, value: u8| {
        let mut bytes = good.clone();
        bytes[at] = value;
        text(&bytes)
    };
    let mut no_number = vector_bytes(VECTOR_STATEMENT);
    no_number[195..203].copy_from_slice(&[0; 8]);
    let vector = VECTOR.trim();
    vec![
        (
            "another prefix",
            vector.replacen("ncdb-invite-1:", "ncdb-invite-2:", 1),
        ),
        ("no prefix", vector["ncdb-invite-1:".len()..].to_owned()),
        ("truncated", vector[..vector.len() - 8].to_owned()),
        ("one character short", vector[..vector.len() - 1].to_owned()),
        ("padded", format!("{vector}=")),
        ("not base64url", vector.replacen('A', "+", 1)),
        ("another version", changed(0, 2)),
        ("another kind", changed(97, 3)),
        ("a statement byte of 2", changed(162, 2)),
        ("a statement byte of 1 without a statement", changed(162, 1)),
        ("a statement of account key number 0", text(&no_number)),
        ("a byte too many", text(&[good.as_slice(), &[0]].concat())),
        ("empty", String::new()),
    ]
}

#[test]
fn a_text_that_is_not_an_invitation_stops_the_relay_at_start() {
    for (what, text) in malformed() {
        let (code, stderr) = refused(&text);
        assert_eq!(code, Some(2), "{what}: {stderr}");
        assert!(stderr.contains("--allow-invitation"), "{what}: {stderr}");
    }
}

#[test]
fn a_refused_invitation_is_not_repeated() {
    for (what, text) in malformed() {
        let (_, stderr) = refused(&text);
        let body = text.strip_prefix("ncdb-invite-1:").unwrap_or(&text);
        if body.len() >= 8 {
            assert!(!stderr.contains(body), "{what} repeats the text: {stderr}");
            assert!(
                !stderr.contains(&body[..8]),
                "{what} repeats the text: {stderr}"
            );
        }
        let account = hex(&vector_bytes(VECTOR)[65..97]);
        assert!(!stderr.contains(&account), "{what} names a key: {stderr}");
    }
}

#[test]
fn the_usage_names_the_flag() {
    let output = Command::new(env!("CARGO_BIN_EXE_ncdb-relay"))
        .arg("--frobnicate")
        .stderr(Stdio::piped())
        .output()
        .expect("ncdb-relay runs");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--allow-invitation"), "{stderr}");
}
