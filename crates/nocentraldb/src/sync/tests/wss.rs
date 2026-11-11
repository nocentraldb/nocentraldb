//! Syncing over TLS (NOC-121): `wss://` to the relay and `https://` for its
//! upload and download URLs, verified against the platform's trust store.
//! A TLS front end in the test, in front of the local relay server, holds a
//! certificate for `localhost` issued by the test CA in `tls/`, which the
//! SDK's tests trust besides the platform's store (`sync::tls`); the second
//! certificate there is trusted by nobody. The keys are test fixtures,
//! never used anywhere else.

use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use ncdb_relay::sqlite::SqliteStore;
use ncdb_relay::{Config, Relay};
use ncdb_relay_server::Server;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::TlsAcceptor;

use super::support::*;
use crate::members::{AddMember, Role};
use crate::sync::Connection;
use crate::value::Value;

const LEAF: &[u8] = include_bytes!("tls/leaf.der");
const LEAF_KEY: &[u8] = include_bytes!("tls/leaf.key.der");
const OTHER: &[u8] = include_bytes!("tls/other.der");
const OTHER_KEY: &[u8] = include_bytes!("tls/other.key.der");

/// A TLS front end on a port of its own, with `cert` and `key`, that leads
/// every connection to the server at `target` once it is known.
struct Front {
    port: u16,
    listener: TcpListener,
    acceptor: TlsAcceptor,
}

impl Front {
    async fn bind(cert: &[u8], key: &[u8]) -> Front {
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let config = rustls::ServerConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .expect("TLS versions")
            .with_no_client_auth()
            .with_single_cert(
                vec![CertificateDer::from(cert.to_vec())],
                PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key.to_vec())),
            )
            .expect("the test certificate");
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("a port");
        Front {
            port: listener.local_addr().expect("an address").port(),
            listener,
            acceptor: TlsAcceptor::from(Arc::new(config)),
        }
    }

    fn lead_to(self, target: SocketAddr) {
        tokio::spawn(async move {
            loop {
                let Ok((inbound, _)) = self.listener.accept().await else {
                    return;
                };
                let acceptor = self.acceptor.clone();
                tokio::spawn(async move {
                    let Ok(mut tls) = acceptor.accept(inbound).await else {
                        return;
                    };
                    if let Ok(mut outbound) = TcpStream::connect(target).await {
                        let _ = tokio::io::copy_bidirectional(&mut tls, &mut outbound).await;
                    }
                });
            }
        });
    }
}

/// A relay server behind a TLS front end with `cert`, which hands out its
/// upload and download URLs as `https://` through the front end; the
/// `wss://` URL the databases open with.
async fn relay_over_tls(dir: &TempDir, cert: &[u8], key: &[u8]) -> String {
    let front = Front::bind(cert, key).await;
    let port = front.port;
    let store = SqliteStore::open(&dir.database()).expect("the relay's database opens");
    let mut config = Config::new(BTreeSet::from([account().verifying_key().to_bytes()]));
    config.max_frame = ncdb_relay_server::MAX_FRAME;
    let server = Server::bind("127.0.0.1:0", Relay::new(store, config), [0x7e; 16])
        .await
        .expect("the server binds")
        .with_base_url(&format!("https://localhost:{port}"));
    let addr = server.local_addr().expect("an address");
    tokio::spawn(server.run());
    front.lead_to(addr);
    format!("wss://localhost:{port}")
}

#[tokio::test(flavor = "multi_thread")]
async fn a_database_syncs_with_a_relay_over_wss() {
    let dir = TempDir::new();
    let url = relay_over_tls(&dir, LEAF, LEAF_KEY).await;
    let db = open(1, &url).await;
    let project = now(db.spaces().create()).unwrap();
    now(project.write(|tx| {
        tx.create("tasks", [("title", Value::from("over TLS"))])?;
        Ok(())
    }))
    .unwrap();
    let status = synced(&db).await;
    assert_eq!(status.refused, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_relay_whose_certificate_nobody_trusts_is_never_connected() {
    let dir = TempDir::new();
    let url = relay_over_tls(&dir, OTHER, OTHER_KEY).await;
    let db = open(1, &url).await;
    tokio::time::sleep(Duration::from_secs(2)).await;
    assert_ne!(db.sync().current().connection, Connection::Connected);
}

#[tokio::test(flavor = "multi_thread")]
async fn large_changes_go_through_https_urls() {
    let dir = TempDir::new();
    let url = relay_over_tls(&dir, LEAF, LEAF_KEY).await;
    let admin = open(1, &url).await;
    let member = open(2, &url).await;
    synced(&member).await;
    let project = now(admin.spaces().create()).unwrap();
    let writer = AddMember {
        role: Role::Writer,
        fingerprint: None,
    };
    now(project.members().add(&now(member.invitation()), writer)).unwrap();
    synced(&admin).await;
    let big = "y".repeat(300 * 1024);
    let task = now(project.write(|tx| {
        tx.create("tasks", [("title", Value::from(big.as_str()))])?;
        Ok(())
    }))
    .unwrap()
    .ids[0];
    let status = synced(&admin).await;
    assert_eq!(status.refused, 0, "the relay refused the large change");
    eventually("the member holds the large task", || {
        let Ok(space) = now(member.spaces().get(project.id())) else {
            return false;
        };
        let Ok(Some(task)) = now(space.get("tasks", task)) else {
            return false;
        };
        task.get::<&str>("title").ok().flatten() == Some(big.as_str())
    })
    .await;
}
