//! The `ncdb-relay` binary runs the server locally on a SQLite database:
//! it prints the address it listens on, keeps its relay id across restarts,
//! and lets the accounts it is given create spaces.

mod support;

use std::io::{BufRead, BufReader};
use std::net::SocketAddr;
use std::process::{Child, Command, Stdio};

use ncdb_core::cbor::Value;
use support::space::Space;
use support::*;

/// A running `ncdb-relay`, killed on drop.
struct Running(Child);

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Starts `ncdb-relay` on the database in `dir` and a free local port.
fn start(dir: &TempDir) -> (Running, SocketAddr) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_ncdb-relay"))
        .arg("--database")
        .arg(dir.database())
        .args(["--listen", "127.0.0.1:0", "--allow-account", &account_hex()])
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

async fn relay_id(addr: SocketAddr) -> Vec<u8> {
    let mut client = Client::connect(addr).await;
    let hello = message(
        "hello",
        None,
        &[(3, Value::Array(vec![Value::Unsigned(1)]))],
    );
    bytes(field(&client.ask(&hello).await, 4)).to_vec()
}

#[tokio::test(flavor = "multi_thread")]
async fn the_binary_serves_a_local_relay_and_keeps_its_id() {
    let dir = TempDir::new();
    let (running, addr) = start(&dir);
    let first = relay_id(addr).await;
    assert_eq!(first.len(), 16);
    let (mut client, welcome) = Client::member(addr, 0).await;
    let space = Space::new(0);
    assert_eq!(
        client.push(1, space.space, &[&space.genesis]).await,
        [accepted(1)]
    );
    drop(running);

    let (_running, addr) = start(&dir);
    assert_eq!(relay_id(addr).await, first);
    let (_, again) = Client::member(addr, 0).await;
    assert_eq!(again.epoch, welcome.epoch);
    assert_eq!(again.spaces, [space.space.0.to_vec()]);
}

#[test]
fn the_binary_refuses_arguments_it_does_not_know() {
    let status = Command::new(env!("CARGO_BIN_EXE_ncdb-relay"))
        .args(["--allow-account", "not hex"])
        .stderr(Stdio::null())
        .status()
        .expect("ncdb-relay runs");
    assert_eq!(status.code(), Some(2));
    let status = Command::new(env!("CARGO_BIN_EXE_ncdb-relay"))
        .arg("--frobnicate")
        .stderr(Stdio::null())
        .status()
        .expect("ncdb-relay runs");
    assert_eq!(status.code(), Some(2));
}
