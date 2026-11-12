//! The example app end to end (NOC-237): two instances of `tasks`, each
//! with a database of its own, sync through a local relay server on
//! SQLite. The first creates a space and adds the second's device after
//! its fingerprint is confirmed; a task added on one is seen live on the
//! other, completed there, and seen completed on the first.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

use ncdb_relay::sqlite::SqliteStore;
use ncdb_relay::{Config, Relay};
use ncdb_relay_server::Server;

/// The relay id of the test's server.
const RELAY_ID: [u8; 16] = [0x7a; 16];

/// A directory of its own for one run of the test, removed when dropped.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> TempDir {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("tasks-e2e-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// One device: its database and key file, and the relay it syncs with.
struct Device {
    data: PathBuf,
    keys: PathBuf,
    relay: String,
}

impl Device {
    fn new(dir: &Path, name: &str, relay: &str) -> Device {
        Device {
            data: dir.join(name),
            keys: dir.join(format!("{name}.keys")),
            relay: relay.to_owned(),
        }
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_tasks"));
        command
            .arg("--data")
            .arg(&self.data)
            .arg("--key-file")
            .arg(&self.keys)
            .args(["--relay", &self.relay])
            .args(args);
        command
    }

    /// Runs `tasks` with `args`; its output, whatever its exit.
    fn try_run(&self, args: &[&str]) -> Output {
        self.command(args).output().expect("tasks runs")
    }

    /// Runs `tasks` with `args`, which must succeed; its standard output.
    fn run(&self, args: &[&str]) -> String {
        let output = self.try_run(args);
        assert!(
            output.status.success(),
            "tasks {args:?} failed:\n{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).expect("UTF-8 output")
    }

    fn spawn(&self, args: &[&str]) -> Child {
        self.command(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("tasks starts")
    }
}

/// The invitation text and the fingerprint `tasks invitation` prints.
fn invitation(device: &Device) -> (String, String) {
    let out = device.run(&["invitation"]);
    let mut lines = out.lines();
    let invitation = lines.next().expect("an invitation").to_owned();
    let fingerprint = lines
        .next()
        .and_then(|l| l.strip_prefix("fingerprint: "))
        .expect("a fingerprint")
        .to_owned();
    (invitation, fingerprint)
}

/// Waits for `child` to exit, up to `limit`; its output.
fn finish(mut child: Child, limit: Duration) -> Output {
    let start = Instant::now();
    while child.try_wait().expect("the child").is_none() {
        if start.elapsed() > limit {
            child.kill().expect("kill");
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    child.wait_with_output().expect("the output")
}

/// A relay server on a fresh SQLite database in `dir`, on which members of
/// `allowed` may create spaces; the URL the devices sync with.
async fn relay(dir: &Path, allowed: BTreeSet<[u8; 32]>) -> String {
    let store = SqliteStore::open(&dir.join("relay.sqlite")).expect("the relay's database");
    let mut config = Config::new(allowed);
    config.max_frame = ncdb_relay_server::MAX_FRAME;
    let server = Server::bind("127.0.0.1:0", Relay::new(store, config), RELAY_ID)
        .await
        .expect("the server binds");
    let addr = server.local_addr().expect("an address");
    tokio::spawn(server.run());
    format!("ws://{addr}")
}

#[tokio::test(flavor = "multi_thread")]
async fn a_task_added_on_one_device_is_seen_and_completed_on_the_other() {
    let dir = TempDir::new();
    // Both devices first run offline, to learn their invitations; the
    // relay lets the first device's account create spaces.
    let laptop = Device::new(&dir.0, "laptop", "ws://127.0.0.1:9");
    let (laptop_invitation, _) = invitation(&laptop);
    let url = relay(&dir.0, allowed_accounts(&laptop_invitation)).await;
    let laptop = Device::new(&dir.0, "laptop", &url);
    let phone = Device::new(&dir.0, "phone", &url);

    let space = laptop.run(&["create"]).trim().to_owned();
    let (invitation, fingerprint) = invitation(&phone);

    // An account new to the laptop's database needs its fingerprint.
    let refused = laptop.try_run(&["invite", &space, &invitation]);
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains(&fingerprint));
    let wrong = laptop.try_run(&["invite", &space, &invitation, "--fingerprint", "12345"]);
    assert!(!wrong.status.success());
    laptop.run(&["invite", &space, &invitation, "--fingerprint", &fingerprint]);

    phone.run(&["sync", "--space", &space]);
    assert!(phone.run(&["spaces"]).contains(&format!("{space}  writer")));

    // Live: the phone watches while the laptop adds.
    let watch = phone.spawn(&["watch", "--until", "Buy milk", "--timeout", "30"]);
    let task = laptop.run(&["add", &space, "Buy milk"]).trim().to_owned();
    let watched = finish(watch, Duration::from_secs(40));
    assert!(
        watched.status.success(),
        "{}",
        String::from_utf8_lossy(&watched.stderr)
    );
    assert!(String::from_utf8_lossy(&watched.stdout).contains("[ ] Buy milk"));

    phone.run(&["done", &space, &task]);
    laptop.run(&["sync"]);
    let deadline = Instant::now() + Duration::from_secs(30);
    let line = format!("[x] Buy milk  (task {task}, space {space})");
    loop {
        let list = laptop.run(&["list"]);
        if list.contains(&line) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the laptop never saw it done:\n{list}"
        );
        laptop.run(&["sync"]);
    }

    let status = phone.run(&["status", &space]);
    assert!(status.contains("role: writer"), "{status}");
    assert!(status.contains("initial sync complete: true"), "{status}");
}

/// The accounts the relay lets create spaces: the account `invitation`
/// names.
///
/// Still to do (handover, NOC-237): the SDK shows no account key, and
/// `ncdb-relay --allow-invitation` (NOC-239 A, #261) reads it from the
/// invitation text inside the relay binary only. Start the `ncdb-relay`
/// binary with `--allow-invitation <text>` here, as README step 3 does,
/// in place of the in-process relay. Until then this test fails here.
fn allowed_accounts(_invitation: &str) -> BTreeSet<[u8; 32]> {
    todo!("start ncdb-relay with --allow-invitation (#261), as README step 3 does")
}
