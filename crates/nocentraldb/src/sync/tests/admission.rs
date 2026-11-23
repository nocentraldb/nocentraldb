//! RFC 0015, "In the SDK": the app passes `OpenOptions::admission`, a
//! function that fetches an admission from its backend for the account's
//! identity. When the relay refuses a genesis with `not_allowed`, the SDK
//! calls it, presents the answer with `admit`, and pushes the genesis
//! again, once. A function that fails, or a text that is no admission,
//! leaves the genesis refused as before (pm, 2026-11-23, NOC-469; names
//! waiting on the owner's NOC-493).

use std::collections::BTreeSet;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use ed25519_dalek::SigningKey;
use ncdb_core::admission::Admission;
use ncdb_relay::sqlite::SqliteStore;
use ncdb_relay::{Config, Relay};
use ncdb_relay_server::Server;

use super::support::*;
use crate::db::{Db, OpenOptions};

const ME: Who = 1;
/// The relay id of the test servers (`support`).
const RELAY_ID: [u8; 16] = [0x5e; 16];

fn admission_key() -> SigningKey {
    SigningKey::from_bytes(&[0x5a; 32])
}

/// A relay server that lists no account and takes the admissions of the
/// admission key; its address.
async fn admitting(dir: &TempDir) -> SocketAddr {
    let store = SqliteStore::open(&dir.database()).expect("the relay's database opens");
    let mut config = Config::new(BTreeSet::new());
    config.admission_keys = BTreeSet::from([admission_key().verifying_key().to_bytes()]);
    let server = Server::bind("127.0.0.1:0", Relay::new(store, config), RELAY_ID)
        .await
        .expect("the server binds");
    let addr = server.local_addr().expect("an address");
    tokio::spawn(server.run());
    addr
}

/// The app's backend: an admission of `identity` for 10 spaces, for an
/// hour.
fn sign(identity: &str) -> String {
    let bytes: Vec<u8> = (0..identity.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&identity[i..i + 2], 16).unwrap())
        .collect();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let identity: [u8; 32] = bytes.try_into().expect("32 bytes");
    Admission::sign(
        &admission_key(),
        RELAY_ID.to_vec(),
        identity,
        now + 3600,
        10,
    )
    .to_string()
}

async fn open(url: &str, options: impl FnOnce(OpenOptions) -> OpenOptions) -> Db {
    let options = options(OpenOptions::new(LOCK).in_memory().relay(url));
    Db::open_as(options, [ME; 32], [ME ^ 0x80; 32], account())
        .await
        .expect("the test lock opens")
}

#[tokio::test(flavor = "multi_thread")]
async fn a_refused_genesis_is_admitted_through_the_apps_function_and_pushed_again() {
    let dir = TempDir::new();
    let url = format!("ws://{}", admitting(&dir).await);
    let calls = Arc::new(AtomicUsize::new(0));
    let asked = Arc::new(std::sync::Mutex::new(Vec::new()));
    let (counted, seen) = (calls.clone(), asked.clone());
    let db = open(&url, move |options| {
        options.admission(move |identity: String| {
            counted.fetch_add(1, Ordering::SeqCst);
            seen.lock().unwrap().push(identity.clone());
            async move { Ok::<_, Infallible>(sign(&identity)) }
        })
    })
    .await;
    db.spaces().create().await.unwrap();
    let status = synced(&db).await;
    assert_eq!(status.refused, 0);
    // One admission covers the account space and the app's space.
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(*asked.lock().unwrap(), [db.account().identity().unwrap()]);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_function_that_fails_or_answers_no_admission_leaves_the_genesis_refused() {
    for answer in [
        Err("the backend is down"),
        Ok("ncdb-admit-1:AAAA"),
        Ok("not one"),
    ] {
        let dir = TempDir::new();
        let url = format!("ws://{}", admitting(&dir).await);
        let db = open(&url, move |options| {
            options.admission(move |_identity: String| async move { answer.map(str::to_owned) })
        })
        .await;
        db.spaces().create().await.unwrap();
        eventually("the relay refused the geneses", || {
            let status = db.sync().current();
            status.unconfirmed == 0 && status.refused > 0
        })
        .await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn without_the_option_a_refused_genesis_stays_refused() {
    let dir = TempDir::new();
    let url = format!("ws://{}", admitting(&dir).await);
    let db = open(&url, |options| options).await;
    db.spaces().create().await.unwrap();
    eventually("the relay refused the geneses", || {
        let status = db.sync().current();
        status.unconfirmed == 0 && status.refused > 0
    })
    .await;
}
