//! The bulk import measurement (NOC-414, `examples/bulk_import`), run small:
//! 2,000 records, 50 creates to a change, through the `ncdb-relay` binary
//! on SQLite, and a second device of the same account that catches up, as
//! a new device and after both were offline.
//! The measurement at the size of docs/bulk-import.md, 81,000 records,
//! takes minutes and runs by hand, as that guide says; this test keeps the
//! example working.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The build of `target` of `package`, with the cargo that runs this test,
/// into the same target directory, without the variables cargo sets for
/// this test's package: a build script that watches one of them would run
/// again, and its crate would be rebuilt under the same file name (NOC-450).
fn cargo_build(package: &str, target: &[&str]) -> Command {
    let mut cargo = Command::new(env!("CARGO"));
    cargo
        .args(["build", "--locked", "-q", "-p", package])
        .args(target);
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

/// Builds `target` of `package` (see `cargo_build`).
fn build(package: &str, target: &[&str]) {
    let built = cargo_build(package, target).status().expect("cargo runs");
    assert!(built.success(), "cargo build of {target:?} failed");
}

/// target/<profile>, where this test runs from.
fn profile() -> PathBuf {
    // target/<profile>/deps/bulk_import-<hash> → target/<profile>
    let exe = std::env::current_exe().expect("the test's path");
    exe.parent()
        .and_then(Path::parent)
        .expect("target/<profile>")
        .to_owned()
}

/// Runs the example with `args`, which must succeed; its report, as a
/// function from a line's key to its value.
fn measure(args: &[&str]) -> impl Fn(&str) -> String {
    build("ncdb-relay-server", &["--bin", "ncdb-relay"]);
    build("nocentraldb", &["--example", "bulk_import"]);
    let exe = profile()
        .join("examples")
        .join(format!("bulk_import{}", std::env::consts::EXE_SUFFIX));
    let output = Command::new(exe)
        .args(args)
        .output()
        .expect("the example runs");
    let report = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(
        output.status.success(),
        "the measurement failed:\n{report}{}",
        String::from_utf8_lossy(&output.stderr)
    );
    move |key: &str| {
        report
            .lines()
            .find_map(|l| l.strip_prefix(key))
            .unwrap_or_else(|| panic!("no line {key} in:\n{report}"))
            .trim()
            .to_owned()
    }
}

#[test]
fn a_small_import_reaches_the_relay_and_a_second_device_catches_up() {
    let line = measure(&["--operations", "2000", "--batch", "50"]);
    assert_eq!(line("scenario:"), "a new device joins after the burst");
    assert_eq!(line("changes written:"), "40");
    assert_eq!(line("items it reads:"), "2000");
    // 40 changes of the import, the space's genesis and its envelope, and
    // the account space's own changes.
    let entries: u64 = line("relay log entries:").parse().expect("a number");
    assert!(entries >= 42, "{entries} relay log entries");
    assert_eq!(line("frames through an upload:"), "0");
}

/// Island mode (GENESIS.md): the second device joins first and goes
/// offline; the first writes the burst offline, then reconnects and pushes
/// it; then the second comes back and pulls it.
#[test]
fn a_burst_written_offline_reaches_a_device_that_was_offline_too() {
    let line = measure(&["--operations", "2000", "--batch", "50", "--island"]);
    assert_eq!(line("scenario:"), "both devices offline during the burst");
    assert_eq!(line("changes written:"), "40");
    assert_eq!(line("items it reads:"), "2000");
}

/// The build above runs inside `cargo test`, which hands this test the
/// variables it sets for a package: CARGO_MANIFEST_DIR, CARGO_PKG_NAME and
/// the rest. Build scripts watch some of them (ring's rerun-if-env-changed),
/// so a nested cargo that saw them would build ring and rustls again under
/// the same file names. Cargo deletes a library before it rebuilds it, and
/// another cargo's doctests that link against it at that moment fail with
/// "extern location for rustls does not exist" (NOC-450). So the example's
/// build must find every dependency from a registry that the cargo running
/// this test already built.
#[test]
fn the_examples_build_reuses_every_registry_dependency_the_test_run_built() {
    let output = cargo_build("nocentraldb", &["--example", "bulk_import"])
        .arg("--message-format=json")
        .output()
        .expect("cargo runs");
    assert!(output.status.success(), "cargo build of the example failed");
    let rebuilt: Vec<&str> = std::str::from_utf8(&output.stdout)
        .expect("cargo's messages are UTF-8")
        .lines()
        .filter(|message| message.contains(r#""reason":"compiler-artifact""#))
        .filter(|message| message.contains(r#""package_id":"registry+"#))
        .filter(|message| message.contains(r#""fresh":false"#))
        .filter_map(|message| message.split('#').nth(1)?.split('"').next())
        .collect();
    assert!(
        rebuilt.is_empty(),
        "the example's build rebuilt {rebuilt:?}"
    );
}
