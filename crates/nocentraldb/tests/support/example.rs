//! Runs the bulk import measurement (`examples/bulk_import`) from a test
//! and reads its report: bulk_import.rs runs it small, memory_budget.rs at
//! the relay's 16 MiB limit. The example and the relay binary are built
//! with the cargo that runs the test, in the test's own profile, so a test
//! run with `--release` measures a release build.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;

/// `--release` when this test runs in a release build, as cargo's flag.
fn profile_flag() -> &'static [&'static str] {
    if cfg!(debug_assertions) {
        &[]
    } else {
        &["--release"]
    }
}

/// Builds `target` of `package` with the cargo that runs this test, into
/// the same target directory and profile.
fn build(package: &str, target: &[&str]) {
    let built = Command::new(env!("CARGO"))
        .args(["build", "--locked", "-q", "-p", package])
        .args(profile_flag())
        .args(target)
        .status()
        .expect("cargo runs");
    assert!(built.success(), "cargo build of {target:?} failed");
}

/// target/<profile>, where this test runs from.
fn profile() -> PathBuf {
    // target/<profile>/deps/<test>-<hash> → target/<profile>
    let exe = std::env::current_exe().expect("the test's path");
    exe.parent()
        .and_then(Path::parent)
        .expect("target/<profile>")
        .to_owned()
}

/// Runs the example with `args`, which must succeed; its report, as a
/// function from a line's key to its value.
pub fn measure(args: &[&str]) -> impl Fn(&str) -> String {
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
