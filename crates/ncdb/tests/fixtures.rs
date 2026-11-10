//! The lock fixtures other tests read are what `ncdb model build` writes
//! from their schema (NOC-77): `ncdb model check` passes on each, so a
//! fixture edited by hand, or a schema changed without a build, fails here.

use std::path::Path;
use std::process::Command;

/// Runs `ncdb model check` on a schema and its lock, paths relative to the
/// repository's root.
fn check(model: &str, lock: &str) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Command::new(env!("CARGO_BIN_EXE_ncdb"))
        .args(["model", "check", "--model", model, "--lock", lock])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn the_lock_of_the_view_tests_is_what_the_build_writes_from_its_schema() {
    check(
        "crates/ncdb-core/tests/view/model.ts",
        "crates/ncdb-core/tests/view/model.lock.json",
    );
}

#[test]
fn the_lock_of_the_rust_sdk_tests_is_what_the_build_writes_from_its_schema() {
    check(
        "crates/nocentraldb/tests/model/model.ts",
        "crates/nocentraldb/tests/model/model.lock.json",
    );
}
