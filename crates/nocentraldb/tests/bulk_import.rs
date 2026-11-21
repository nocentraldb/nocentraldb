//! The bulk import measurement (NOC-414, `examples/bulk_import`), run small:
//! 2,000 records, 50 creates to a change, through the `ncdb-relay` binary
//! on SQLite, and a second device of the same account that catches up.
//! The measurement at the size of docs/bulk-import.md, 81,000 records,
//! takes minutes and runs by hand, as that guide says; this test keeps the
//! example working.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Builds `target` of `package` with the cargo that runs this test, into
/// the same target directory.
fn build(package: &str, target: &[&str]) {
    let built = Command::new(env!("CARGO"))
        .args(["build", "--locked", "-q", "-p", package])
        .args(target)
        .status()
        .expect("cargo runs");
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

#[test]
fn a_small_import_reaches_the_relay_and_a_second_device_catches_up() {
    build("ncdb-relay-server", &["--bin", "ncdb-relay"]);
    build("nocentraldb", &["--example", "bulk_import"]);
    let exe = profile()
        .join("examples")
        .join(format!("bulk_import{}", std::env::consts::EXE_SUFFIX));
    let output = Command::new(exe)
        .args(["--operations", "2000", "--batch", "50"])
        .output()
        .expect("the example runs");
    let report = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "the measurement failed:\n{report}{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let line = |key: &str| {
        report
            .lines()
            .find_map(|l| l.strip_prefix(key))
            .unwrap_or_else(|| panic!("no line {key} in:\n{report}"))
            .trim()
            .to_owned()
    };
    assert_eq!(line("changes written:"), "40");
    assert_eq!(line("items it reads:"), "2000");
    // 40 changes of the import, the space's genesis and its envelope, and
    // the account space's own changes.
    let entries: u64 = line("relay log entries:").parse().expect("a number");
    assert!(entries >= 42, "{entries} relay log entries");
    assert_eq!(line("frames through an upload:"), "0");
}
