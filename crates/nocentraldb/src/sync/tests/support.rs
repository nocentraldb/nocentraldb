//! What the sync tests share: the relay server on SQLite, run locally
//! behind a proxy the test can cut, a database of a simulated member, and
//! a raw client of the relay server's own tests, through which another
//! simulated member pushes.

use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ncdb_core::change::Change;
use ncdb_core::ids::{ChangeId as CoreChangeId, SpaceId as CoreSpaceId};
use ncdb_relay::sqlite::SqliteStore;
use ncdb_relay::store::RelayStore;
use ncdb_relay::{Config, Relay};
use ncdb_relay_lambda::harness::Gateway;
use ncdb_relay_server::Server;
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;

pub use super::relay_support::space::{self, Who, account};
pub use super::relay_support::{Client, TempDir};

use crate::db::{Db, OpenOptions};
use crate::id::SpaceId;
use crate::space::Space;
use crate::sync::{Connection, SyncStatus};

/// The lock of `tests/model/model.ts`.
pub const LOCK: &str = include_str!("../../../tests/model/model.lock.json");

/// The relay id of the test servers.
const RELAY_ID: [u8; 16] = [0x5e; 16];

/// How long a test waits for the database to sync.
const WAIT: Duration = Duration::from_secs(20);

/// A relay server on a fresh SQLite database, and a proxy in front of it
/// that the databases connect through.
pub struct LocalRelay {
    /// The server's own address, which the raw clients use.
    pub addr: SocketAddr,
    pub dir: TempDir,
    pub proxy: Proxy,
    allowed: BTreeSet<[u8; 32]>,
}

impl LocalRelay {
    /// A relay on which members of the simulated account may create
    /// spaces.
    pub async fn start() -> LocalRelay {
        LocalRelay::with(BTreeSet::from([account().verifying_key().to_bytes()])).await
    }

    /// A relay on which no member may create a space.
    pub async fn closed() -> LocalRelay {
        LocalRelay::with(BTreeSet::new()).await
    }

    /// A relay whose clock is `clock`, in milliseconds since the Unix
    /// epoch, so that a test can move it, as for RFC 0007's `too_early`.
    pub async fn start_with_clock(clock: fn() -> u64) -> LocalRelay {
        let allowed = BTreeSet::from([account().verifying_key().to_bytes()]);
        let dir = TempDir::new();
        let addr = serve(&dir, allowed.clone(), Some(clock)).await;
        let proxy = Proxy::start(addr).await;
        LocalRelay {
            addr,
            dir,
            proxy,
            allowed,
        }
    }

    pub async fn with(allowed: BTreeSet<[u8; 32]>) -> LocalRelay {
        let dir = TempDir::new();
        let addr = serve(&dir, allowed.clone(), None).await;
        let proxy = Proxy::start(addr).await;
        LocalRelay {
            addr,
            dir,
            proxy,
            allowed,
        }
    }

    /// A copy of the relay's database as it is now.
    pub fn backup(&self) -> TempDir {
        let backup = TempDir::new();
        let connection = rusqlite::Connection::open(self.dir.database()).expect("the database");
        let path = backup.database();
        let path = path.to_str().expect("a UTF-8 path");
        connection
            .execute("VACUUM INTO ?1", [path])
            .expect("a backup");
        backup
    }

    /// Restores the relay from `backup`, as RFC 0007's restore does: a new
    /// epoch, then a server on the restored database, which the proxy
    /// leads to from now on. Every append after the backup is gone.
    pub async fn restore(&mut self, backup: TempDir) {
        let store = SqliteStore::open(&backup.database()).expect("the backup opens");
        store.new_epoch().await.expect("a new epoch");
        self.addr = serve(&backup, self.allowed.clone(), None).await;
        self.proxy.lead_to(self.addr);
        self.dir = backup;
    }

    /// The URL the databases open with.
    pub fn url(&self) -> String {
        format!("ws://{}", self.proxy.addr)
    }

    fn store(&self) -> SqliteStore {
        SqliteStore::open(&self.dir.database()).expect("the relay's database opens")
    }

    /// The change ids of the relay's log of `space`, in position order.
    pub async fn log(&self, space: CoreSpaceId) -> Vec<CoreChangeId> {
        let entries = self.store().read(space, 0, 10_000).await.expect("a read");
        entries.iter().map(|entry| entry.change).collect()
    }

    /// The change `id` of `space`, as the relay holds it.
    pub async fn change(&self, space: CoreSpaceId, id: CoreChangeId) -> Change {
        let entries = self.store().read(space, 0, 10_000).await.expect("a read");
        let entry = entries.iter().find(|entry| entry.change == id);
        Change::decode(&entry.expect("the relay holds the change").frame).expect("a change")
    }

    /// The person-key records of `space` the relay stores, in person-key
    /// id order.
    pub async fn person_keys(&self, space: CoreSpaceId) -> Vec<ncdb_relay::store::PersonKeyRecord> {
        self.store().person_keys(space).await.expect("a read")
    }

    /// A raw connection of the simulated member `who` to the server.
    pub async fn client(&self, who: Who) -> Client {
        Client::member(self.addr, who).await.0
    }
}

/// A relay server on the database in `dir`, or with NCDB_TEST_RELAY=lambda
/// the relay's Lambda handler behind its local API Gateway; its address.
async fn serve(
    dir: &TempDir,
    allowed: BTreeSet<[u8; 32]>,
    clock: Option<fn() -> u64>,
) -> SocketAddr {
    let store = SqliteStore::open(&dir.database()).expect("the relay's database opens");
    let mut config = Config::new(allowed);
    if let Some(clock) = clock {
        config.clock = clock;
    }
    config.max_frame = ncdb_relay_server::MAX_FRAME;
    if std::env::var("NCDB_TEST_RELAY").as_deref() == Ok("lambda") {
        let gateway = Gateway::bind("127.0.0.1:0", Relay::new(store, config), RELAY_ID)
            .await
            .expect("the gateway binds");
        let addr = gateway.local_addr().expect("an address");
        tokio::spawn(gateway.run());
        return addr;
    }
    let server = Server::bind("127.0.0.1:0", Relay::new(store, config), RELAY_ID)
        .await
        .expect("the server binds");
    let addr = server.local_addr().expect("an address");
    tokio::spawn(server.run());
    addr
}

/// A TCP proxy in front of the server. Cut, it drops every connection
/// through it and refuses new ones, as a lost network would.
pub struct Proxy {
    pub addr: SocketAddr,
    target: Arc<Mutex<SocketAddr>>,
    up: Arc<AtomicBool>,
    links: Arc<Mutex<Vec<JoinHandle<()>>>>,
}

impl Proxy {
    async fn start(target: SocketAddr) -> Proxy {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("a port");
        let addr = listener.local_addr().expect("an address");
        let target = Arc::new(Mutex::new(target));
        let up = Arc::new(AtomicBool::new(true));
        let links = Arc::new(Mutex::new(Vec::new()));
        let (accept_target, accept_up, accept_links) = (target.clone(), up.clone(), links.clone());
        tokio::spawn(async move {
            loop {
                let Ok((mut inbound, _)) = listener.accept().await else {
                    return;
                };
                if !accept_up.load(Ordering::SeqCst) {
                    continue;
                }
                let target = *accept_target.lock().unwrap();
                let link = tokio::spawn(async move {
                    if let Ok(mut outbound) = TcpStream::connect(target).await {
                        let _ = tokio::io::copy_bidirectional(&mut inbound, &mut outbound).await;
                    }
                });
                accept_links.lock().unwrap().push(link);
            }
        });
        Proxy {
            addr,
            target,
            up,
            links,
        }
    }

    /// Drops every connection and refuses new ones.
    pub fn cut(&self) {
        self.up.store(false, Ordering::SeqCst);
        for link in self.links.lock().unwrap().drain(..) {
            link.abort();
        }
    }

    /// Leads new connections to the server at `target`.
    fn lead_to(&self, target: SocketAddr) {
        *self.target.lock().unwrap() = target;
    }

    /// Lets connections through again.
    pub fn restore(&self) {
        self.up.store(true, Ordering::SeqCst);
    }
}

/// The database of the simulated member `who`, of the simulated account,
/// syncing with the relay at `url`.
pub async fn open(who: Who, url: &str) -> Db {
    open_with(who, url, LOCK).await
}

/// [`open`] with the lock `lock`.
pub async fn open_with(who: Who, url: &str, lock: &str) -> Db {
    let options = OpenOptions::new(lock).in_memory().relay(url);
    Db::open_as(options, [who; 32], [who ^ 0x80; 32], account())
        .await
        .expect("the test lock opens")
}

/// The database of the simulated member `who` as a device that joins the
/// account, as a pairing one does (RFC 0010, "Adding a device or an agent
/// to an account"): it creates no account space of its own.
pub async fn open_joining(who: Who, url: &str) -> Db {
    let options = OpenOptions::new(LOCK)
        .in_memory()
        .relay(url)
        .joins_account();
    Db::open_as(options, [who; 32], [who ^ 0x80; 32], account())
        .await
        .expect("the test lock opens")
}

/// A handle on a space the database holds.
pub fn handle(db: &Db, space: CoreSpaceId) -> Space {
    Space::handle(db.inner.clone(), SpaceId(space))
}

/// Waits until `done` holds.
pub async fn eventually(what: &str, mut done: impl FnMut() -> bool) {
    let start = Instant::now();
    while !done() {
        assert!(start.elapsed() < WAIT, "{what}: not in time");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// Waits until the database is connected and the relay confirmed every
/// change of this member.
pub async fn synced(db: &Db) -> SyncStatus {
    let mut status = db.sync().current();
    eventually("synced", || {
        status = db.sync().current();
        status.connection == Connection::Connected && status.unconfirmed == 0
    })
    .await;
    status
}

/// Pushes `changes` to their space as `client`, and checks the relay
/// accepted each.
pub async fn push(client: &mut Client, changes: &[&Change]) {
    let space = changes[0].header().space;
    let results = client.push(1, space, changes).await;
    for result in results {
        assert!(
            super::relay_support::get(&result, 1).is_some(),
            "not appended: {}",
            outcome(&result)
        );
    }
}

/// A push result as RFC 0007 gives it: key 1 is the position of an
/// append, key 2 the position of a duplicate, key 3 the code of a
/// refusal.
fn outcome(result: &ncdb_core::cbor::Value) -> String {
    use super::relay_support::get;
    if let Some(position) = get(result, 2) {
        return format!("a duplicate, at {position:?}");
    }
    match get(result, 3) {
        Some(ncdb_core::cbor::Value::Text(code)) => format!("refused with {code}"),
        _ => format!("{result:?}"),
    }
}

/// The output of an SDK future, which is ready at its first poll: every
/// call of the SDK finishes on the device.
pub fn now<F: std::future::Future>(future: F) -> F::Output {
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    match std::pin::pin!(future).poll(&mut cx) {
        std::task::Poll::Ready(output) => output,
        std::task::Poll::Pending => panic!("an SDK call waited"),
    }
}
