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

/// The build of `target` of `package`, with the cargo that runs this test,
/// into the same target directory and profile, without the variables cargo
/// sets for this test's package: a build script that watches one of them
/// would run again, and its crate would be rebuilt under the same file name
/// (NOC-450).
pub fn cargo_build(package: &str, target: &[&str]) -> Command {
    let mut cargo = Command::new(env!("CARGO"));
    cargo
        .args(["build", "--locked", "-q", "-p", package])
        .args(profile_flag())
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
