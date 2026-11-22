//! The example app end to end (NOC-237): two instances of `tasks`, each
//! with a database of its own, sync through a local relay server on
//! SQLite, the `ncdb-relay` binary started as README step 3 starts it,
//! with the first device's invitation (`--allow-invitation`, NOC-239 A). The first creates a space and adds the second's device after
//! its fingerprint is confirmed; a task added on one is seen live on the
//! other, completed there, and seen completed on the first.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

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

/// A running `ncdb-relay`, stopped when dropped.
struct Relay(Child);

impl Drop for Relay {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// The build of the `ncdb-relay` binary, with the cargo that runs this
/// test, into the same target directory, without the variables cargo sets
/// for this test's package (NOC-450).
fn relay_build() -> Command {
    let mut cargo = Command::new(env!("CARGO"));
    cargo.args([
        "build",
        "--locked",
        "-q",
        "-p",
        "ncdb-relay-server",
        "--bin",
        "ncdb-relay",
    ]);
    for (name, _) in std::env::vars_os() {
        if name.to_str().is_some_and(set_for_the_package) {
            cargo.env_remove(name);
        }
    }
    cargo
}

/// Whether cargo sets the variable `name` for the package whose test runs
/// (the Cargo Book, "Environment variables Cargo sets for crates"). CARGO
/// itself, CARGO_HOME, CARGO_TARGET_DIR and cargo's own configuration stay.
fn set_for_the_package(name: &str) -> bool {
    ["CARGO_PKG_", "CARGO_MANIFEST_", "CARGO_BIN_"]
        .iter()
        .any(|prefix| name.starts_with(prefix))
        || [
            "CARGO_CRATE_NAME",
            "CARGO_PRIMARY_PACKAGE",
            "CARGO_TARGET_TMPDIR",
            "OUT_DIR",
        ]
        .contains(&name)
}

/// The `ncdb-relay` binary, built by the same cargo into the same target
/// directory as this test: a test sees only its own package's binaries,
/// and the app depends on no relay crate.
fn relay_binary() -> PathBuf {
    let built = relay_build().status().expect("cargo runs");
    assert!(built.success(), "cargo build of ncdb-relay failed");
    // target/<profile>/deps/two_devices-<hash> → target/<profile>/ncdb-relay
    let exe = std::env::current_exe().expect("the test's path");
    let profile = exe
        .parent()
        .and_then(Path::parent)
        .expect("target/<profile>");
    profile.join(format!("ncdb-relay{}", std::env::consts::EXE_SUFFIX))
}

/// Starts `ncdb-relay` on the SQLite database in `dir`, on `listen`,
/// letting the account `invitation` names create spaces, as README step 3
/// does; the relay, and the URL the devices sync with.
fn relay(dir: &Path, invitation: &str, listen: &str) -> (Relay, String) {
    let mut child = Command::new(relay_binary())
        .arg("--database")
        .arg(dir.join("relay.sqlite"))
        .args(["--listen", listen, "--allow-invitation", invitation])
        .stdout(Stdio::piped())
        .spawn()
        .expect("ncdb-relay starts");
    let stdout = child.stdout.take().expect("its output");
    let relay = Relay(child);
    let mut line = String::new();
    BufReader::new(stdout).read_line(&mut line).expect("a line");
    let addr = line
        .trim()
        .strip_prefix("ncdb-relay listening on ")
        .unwrap_or_else(|| panic!("unexpected output: {line}"))
        .to_owned();
    (relay, addr)
}

#[test]
fn a_task_added_on_one_device_is_seen_and_completed_on_the_other() {
    let dir = TempDir::new();
    // Both devices first run offline, to learn their invitations; the
    // relay lets the first device's account create spaces.
    let laptop = Device::new(&dir.0, "laptop", "ws://127.0.0.1:9");
    let (laptop_invitation, _) = invitation(&laptop);
    let (running, url) = relay(&dir.0, &laptop_invitation, "127.0.0.1:0");
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

    // Offline: with the relay stopped, each device writes to its own copy,
    // says so, and succeeds (README step 6).
    drop(running);
    for (device, title) in [
        (&laptop, "Offline on the laptop"),
        (&phone, "Offline on the phone"),
    ] {
        let output = device.try_run(&["add", &space, title]);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "add offline failed: {stderr}");
        assert!(
            stderr.contains("saved on this device; the relay does not hold it yet"),
            "{stderr}"
        );
        assert!(device.run(&["list"]).contains(&format!("[ ] {title}")));
    }

    // The relay back on the same database and address: sync brings both
    // devices to the same list.
    let listen = url.strip_prefix("ws://").expect("a ws:// URL");
    let (_running, _) = relay(&dir.0, &laptop_invitation, listen);
    laptop.run(&["sync"]);
    phone.run(&["sync"]);
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let (on_laptop, on_phone) = (laptop.run(&["list"]), phone.run(&["list"]));
        let both = |list: &str| {
            list.contains("Offline on the laptop") && list.contains("Offline on the phone")
        };
        if both(&on_laptop) && both(&on_phone) {
            assert_eq!(on_laptop, on_phone);
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the devices never converged:\n{on_laptop}\n{on_phone}"
        );
        laptop.run(&["sync"]);
        phone.run(&["sync"]);
    }

    let status = phone.run(&["status", &space]);
    assert!(status.contains("role: writer"), "{status}");
    assert!(status.contains("initial sync complete: true"), "{status}");
}

/// `cargo test` hands this test the variables it sets for the package:
/// CARGO_MANIFEST_DIR, CARGO_PKG_NAME and the rest. A build script that
/// watches one of them (ring's does) would run again in the relay's build
/// and its crate would be rebuilt under the file name every other cargo
/// uses, deleted under another cargo's running doctest (NOC-450). So the
/// relay's build runs without them.
#[test]
fn the_relay_build_runs_without_the_variables_cargo_set_for_this_test() {
    let build = relay_build();
    let removed: Vec<_> = build
        .get_envs()
        .filter(|(_, value)| value.is_none())
        .map(|(name, _)| name.to_owned())
        .collect();
    for name in ["CARGO_MANIFEST_DIR", "CARGO_PKG_NAME", "CARGO_PKG_VERSION"] {
        assert!(std::env::var_os(name).is_some(), "cargo test sets {name}");
        assert!(
            removed.iter().any(|removed| removed == name),
            "the relay's build keeps {name}"
        );
    }
}
