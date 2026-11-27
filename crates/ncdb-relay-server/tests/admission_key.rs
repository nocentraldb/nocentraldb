//! `ncdb-relay --admission-key <hex>` (Carsten, 2026-11-27, NOC-516): the
//! self-hosted relay takes the admissions that verify under one of the
//! public admission keys it is given, 64 hex digits each, as the relay on
//! AWS does with `NCDB_ADMISSION_KEYS` (RFC 0015, "Admission tokens"). An
//! account the relay does not list then creates spaces by its admission,
//! and an admission signed by a key the relay was not given admits nothing.

mod support;

use std::io::{BufRead, BufReader};
use std::net::SocketAddr;
use std::process::{Child, Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use ed25519_dalek::SigningKey;
use ncdb_core::admission::Admission;
use ncdb_core::cbor::Value;
use support::space::{Space, account};
use support::*;

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

/// The app's admission key, whose public half the relay is given.
fn app_key() -> SigningKey {
    SigningKey::from_bytes(&[0x5a; 32])
}

/// The relay's id, as its `hello` answers it.
async fn relay_id(addr: SocketAddr) -> Vec<u8> {
    let mut client = Client::connect(addr).await;
    let hello = message(
        "hello",
        None,
        &[(3, Value::Array(vec![Value::Unsigned(1)]))],
    );
    bytes(field(&client.ask(&hello).await, 4)).to_vec()
}

/// An admission by `key` for the simulated account, whose certificates
/// carry no statement, so its identity is its account key; it holds for an
/// hour and grants three spaces.
fn admission(key: &SigningKey, relay_id: &[u8]) -> Vec<u8> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let identity = account().verifying_key().to_bytes();
    Admission::sign(key, relay_id.to_vec(), identity, now + 3600, 3).encode()
}

/// Member 0 presents an admission by `key`, then pushes a genesis; the
/// answers to both.
async fn admitted_by(addr: SocketAddr, key: &SigningKey) -> (Value, Value) {
    let relay_id = relay_id(addr).await;
    let (mut client, _) = Client::member(addr, 0).await;
    let admit = message(
        "admit",
        Some(1),
        &[(3, Value::Bytes(admission(key, &relay_id)))],
    );
    let admitted = client.ask(&admit).await;
    let space = Space::new(0);
    let pushed = client.push(2, space.space, &[&space.genesis]).await.remove(0);
    (admitted, pushed)
}

#[tokio::test(flavor = "multi_thread")]
async fn an_admission_by_a_given_key_admits_a_genesis() {
    let dir = TempDir::new();
    let public = hex(&app_key().verifying_key().to_bytes());
    let (_running, addr) = start(&dir, &["--admission-key", &public]);
    let (admitted, pushed) = admitted_by(addr, &app_key()).await;
    assert_eq!(admitted, message("ok", Some(1), &[]));
    assert_eq!(pushed, accepted(1));
}

#[tokio::test(flavor = "multi_thread")]
async fn an_admission_by_an_unknown_key_admits_nothing() {
    let dir = TempDir::new();
    let public = hex(&app_key().verifying_key().to_bytes());
    let (_running, addr) = start(&dir, &["--admission-key", &public]);
    let stranger = SigningKey::from_bytes(&[0x5b; 32]);
    let (admitted, pushed) = admitted_by(addr, &stranger).await;
    assert_eq!(admitted, error(Some(1), "not_allowed"));
    assert_eq!(pushed, rejected("not_allowed"));
}

#[tokio::test(flavor = "multi_thread")]
async fn admission_keys_may_be_given_more_than_once_and_with_accounts() {
    let dir = TempDir::new();
    let other = hex(&[0xcc; 32]);
    let public = hex(&app_key().verifying_key().to_bytes());
    let listed = hex(&[0xdd; 32]);
    let (_running, addr) = start(
        &dir,
        &[
            "--admission-key",
            &other,
            "--allow-account",
            &listed,
            "--admission-key",
            &public,
        ],
    );
    let (admitted, pushed) = admitted_by(addr, &app_key()).await;
    assert_eq!(admitted, message("ok", Some(1), &[]));
    assert_eq!(pushed, accepted(1));
}

#[test]
fn a_malformed_admission_key_stops_the_relay_with_the_usage() {
    for bad in ["not hex", "5a5a", &"g".repeat(64)] {
        let dir = TempDir::new();
        let output = Command::new(env!("CARGO_BIN_EXE_ncdb-relay"))
            .arg("--database")
            .arg(dir.database())
            .args(["--admission-key", bad])
            .output()
            .expect("ncdb-relay runs");
        assert_eq!(output.status.code(), Some(2), "{bad}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("--admission-key"), "{stderr}");
    }
}
