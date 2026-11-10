//! A failed `accept` does not stop the server. Running out of file
//! descriptors (EMFILE) is the common case: it lasts only until
//! connections close, so the server logs the error, waits a moment and
//! accepts again, while its members stay connected.
//!
//! The test runs the `ncdb-relay` binary under a low descriptor limit
//! (`ulimit -n`, so Unix only) and opens connections that stall in the
//! handshake until the server has no descriptor left for the next one.
#![cfg(unix)]

mod support;

use std::io::{BufRead, BufReader, Read};
use std::net::SocketAddr;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use support::*;
use tokio::net::TcpStream;

/// The server's descriptor limit: room for the runtime, the database and
/// a few connections, far below the handshake limit of 64.
const DESCRIPTORS: u32 = 48;

/// More stalled connections than the server has descriptors for.
const STALLED: usize = 60;

/// The relay binary under a limit of `DESCRIPTORS`, and the address it
/// prints.
fn relay(dir: &TempDir) -> (Child, SocketAddr) {
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(format!("ulimit -n {DESCRIPTORS} && exec \"$0\" \"$@\""))
        .arg(env!("CARGO_BIN_EXE_ncdb-relay"))
        .arg("--database")
        .arg(dir.database())
        .args(["--listen", "127.0.0.1:0", "--allow-account", &account_hex()])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("ncdb-relay runs");
    let mut line = String::new();
    BufReader::new(child.stdout.take().expect("its stdout"))
        .read_line(&mut line)
        .expect("it prints its address");
    let addr = line
        .trim()
        .rsplit("ws://")
        .next()
        .and_then(|a| a.parse().ok())
        .unwrap_or_else(|| panic!("an address in {line:?}"));
    (child, addr)
}

/// Kills the server and returns what it wrote to stderr.
fn stop(mut child: Child) -> String {
    let _ = child.kill();
    let _ = child.wait();
    let mut stderr = String::new();
    if let Some(mut pipe) = child.stderr.take() {
        let _ = pipe.read_to_string(&mut stderr);
    }
    stderr
}

#[tokio::test(flavor = "multi_thread")]
async fn the_server_keeps_accepting_after_it_runs_out_of_descriptors() {
    let dir = TempDir::new();
    let (mut child, addr) = relay(&dir);
    let (mut member, _) = Client::member(addr, 0).await;

    let mut stalled = Vec::new();
    for _ in 0..STALLED {
        // A refused connection means the server stopped listening; the
        // check below says so.
        match TcpStream::connect(addr).await {
            Ok(stream) => stalled.push(stream),
            Err(_) => break,
        }
    }
    // The server tries to accept the connections it has no descriptor for.
    tokio::time::sleep(Duration::from_millis(500)).await;
    if let Some(status) = child.try_wait().expect("its status") {
        panic!("the server stopped ({status}): {}", stop(child));
    }
    let pong = member.ask(&message("ping", Some(7), &[])).await;
    assert_eq!(kind(&pong), "pong", "a connected member is still served");

    drop(stalled);
    let mut late = Client::member_eventually(addr, 1).await;
    let pong = late.ask(&message("ping", Some(8), &[])).await;
    assert_eq!(kind(&pong), "pong", "a new member is served");

    let stderr = stop(child);
    assert!(stderr.contains("accept"), "the error is logged: {stderr:?}");
}
