//! A database at a path, with its keys in a key file (RFC 0009, "The Rust
//! SDK"; RFC 0003, "The member key"): reopening it finds its
//! keys, its spaces and its objects again, and the member writes on. CI
//! has no platform keystore, so the key file is the path tested here in
//! full; `src/keys/tests.rs` tests the keystore.

mod support;

use nocentraldb::{Db, ErrorCode, OpenOptions, Value};
use support::{LOCK, TempDir, now};

fn options(dir: &TempDir) -> OpenOptions {
    OpenOptions::new(LOCK)
        .path(dir.join("data"))
        .key_file(dir.join("member.key"))
}

#[test]
fn a_reopened_database_finds_its_keys_its_spaces_and_its_objects() {
    let dir = TempDir::new("reopen");
    let (space, task) = {
        let db = now(Db::open(options(&dir))).unwrap();
        let project = now(db.spaces().create()).unwrap();
        let written = now(project.write(|tx| {
            tx.create("tasks", [("title", Value::from("Ship the SDK"))])?;
            Ok(())
        }))
        .unwrap();
        (project.id(), written.ids[0])
    };

    let db = now(Db::open(options(&dir))).unwrap();
    let project = now(db.spaces().get(space)).unwrap();
    let read = now(project.get("tasks", task)).unwrap().unwrap();
    let again = now(project.write(|tx| {
        tx.update("tasks", task, [("title", Value::from("Shipped"))])?;
        Ok(())
    }));

    assert_eq!(read.get::<&str>("title").unwrap(), Some("Ship the SDK"));
    again.unwrap();
    let read = now(project.get("tasks", task)).unwrap().unwrap();
    assert_eq!(read.get::<&str>("title").unwrap(), Some("Shipped"));
}

#[test]
fn the_directory_and_the_key_file_are_created_on_first_open() {
    let dir = TempDir::new("create");

    now(Db::open(options(&dir))).unwrap();

    assert!(dir.join("data").is_dir());
    assert!(dir.join("member.key").is_file());
}

#[cfg(unix)]
#[test]
fn only_its_user_can_read_the_key_file() {
    use std::os::unix::fs::PermissionsExt;
    let dir = TempDir::new("mode");

    now(Db::open(options(&dir))).unwrap();

    let mode = std::fs::metadata(dir.join("member.key"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
}

/// A new member key would hold none of the space keys the database's
/// spaces were shared with, so the SDK does not draw one for an existing
/// database.
#[test]
fn an_existing_database_whose_key_file_is_gone_does_not_open() {
    let dir = TempDir::new("gone");
    drop(now(Db::open(options(&dir))).unwrap());
    std::fs::remove_file(dir.join("member.key")).unwrap();

    let error = now(Db::open(options(&dir))).unwrap_err();

    assert_eq!(error.code(), ErrorCode::StorageFailed);
    assert!(error.to_string().contains("key"), "{error}");
    assert!(!dir.join("member.key").exists());
}

#[test]
fn a_key_file_that_does_not_read_does_not_open() {
    let dir = TempDir::new("damaged");
    std::fs::write(dir.join("member.key"), b"not a key").unwrap();

    let error = now(Db::open(options(&dir))).unwrap_err();

    assert_eq!(error.code(), ErrorCode::StorageFailed);
}

/// Memory and a path, or memory and a key file, say two things; the SDK
/// takes neither (the safe subset of NOC-108).
#[test]
fn memory_with_a_path_or_a_key_file_is_invalid_value() {
    let dir = TempDir::new("both");
    for options in [
        OpenOptions::new(LOCK).in_memory().path(dir.join("data")),
        OpenOptions::new(LOCK)
            .in_memory()
            .key_file(dir.join("member.key")),
    ] {
        let error = now(Db::open(options)).unwrap_err();
        assert_eq!(error.code(), ErrorCode::InvalidValue);
    }
    assert!(!dir.join("data").exists());
}

#[test]
fn a_key_file_without_a_path_is_invalid_value() {
    let dir = TempDir::new("no-path");

    let error = now(Db::open(
        OpenOptions::new(LOCK).key_file(dir.join("member.key")),
    ))
    .unwrap_err();

    assert_eq!(error.code(), ErrorCode::InvalidValue);
    assert!(!dir.join("member.key").exists());
}
