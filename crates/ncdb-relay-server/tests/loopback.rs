//! RFC 0007, "Transport and encoding": a member talks to the relay over
//! WebSocket with TLS. This server serves plain `ws://` and leaves TLS to a
//! proxy in front of it, so it binds only a loopback address, where only
//! that proxy, on the same host, reaches it.

mod support;

use std::collections::BTreeSet;
use std::io::ErrorKind;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use ncdb_relay::Relay;
use ncdb_relay::sqlite::SqliteStore;
use ncdb_relay_server::Server;
use support::*;

async fn bind(addr: &str) -> std::io::Result<()> {
    let dir = TempDir::new();
    let store = SqliteStore::open(&dir.database()).expect("the database opens");
    let relay = Relay::new(store, ncdb_relay_server::config(BTreeSet::new()));
    Server::bind(addr, relay, RELAY_ID).await.map(|_| ())
}

#[tokio::test(flavor = "multi_thread")]
async fn the_server_binds_a_loopback_address() {
    bind("127.0.0.1:0").await.expect("IPv4 loopback");
    bind("localhost:0").await.expect("a name of the loopback");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_server_refuses_an_address_beyond_the_loopback() {
    for addr in ["0.0.0.0:0", "[::]:0"] {
        let refused = bind(addr).await.expect_err(addr);
        assert_eq!(refused.kind(), ErrorKind::InvalidInput, "{addr}: {refused}");
        assert!(refused.to_string().contains("loopback"), "{refused}");
    }
}

#[test]
fn the_binary_refuses_to_listen_beyond_the_loopback() {
    let dir = TempDir::new();
    let mut child = Command::new(env!("CARGO_BIN_EXE_ncdb-relay"))
        .arg("--database")
        .arg(dir.database())
        .args(["--listen", "0.0.0.0:0"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("ncdb-relay runs");
    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        if let Some(status) = child.try_wait().expect("its status") {
            break status;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("ncdb-relay still runs on 0.0.0.0");
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let output = child.wait_with_output().expect("its output");
    assert_eq!(status.code(), Some(1));
    assert!(output.stdout.is_empty(), "it does not listen");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("loopback"), "{stderr}");
}
