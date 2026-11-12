//! Reopening a database with a changed lock (RFC 0009, "What `nc.open`
//! does", steps 3 and 4; RFC 0005, "Two checks: at build and on the
//! device", D11): an additive successor of the lock the database was last
//! opened with opens it and folds what the old model parked; a lock that
//! is not, or that has another `model.id`, is `ModelRefused`, names each
//! refused change, and changes nothing.

mod support;

use nocentraldb::{Db, ErrorCode, ObjectId, OpenOptions, SpaceId, Value};
use support::{LOCK, TempDir, now};

/// The test model's `model.id`.
const MODEL_ID: &str = "f959ef26820d478a108e9261abf83a69";

fn options(dir: &TempDir, lock: &str) -> OpenOptions {
    OpenOptions::new(lock)
        .path(dir.join("data"))
        .key_file(dir.join("member.key"))
}

/// [`LOCK`] with `old` replaced by `new` once, at version 2.
fn changed(old: &str, new: &str) -> String {
    assert!(LOCK.contains(old));
    LOCK.replacen(old, new, 1)
        .replacen(r#""version": 1"#, r#""version": 2"#, 1)
}

/// [`LOCK`] one version on, with a text `note` on projects: additive.
fn with_note() -> String {
    let name = r#""name": { "rule": 1, "since": 1, "type": "ncdb/string" }"#;
    changed(
        name,
        &format!(r#"{name}, "note": {{ "rule": 1, "since": 2, "type": "ncdb/string" }}"#),
    )
}

/// [`LOCK`] one version on, without the tasks' `estimate`: not additive.
fn without_estimate() -> String {
    changed(
        r#""estimate": { "rule": 1, "since": 1, "type": "ncdb/float" },"#,
        "",
    )
}

/// [`LOCK`] with another `model.id` and nothing else changed.
fn other_model() -> String {
    LOCK.replacen(MODEL_ID, "00000000000000000000000000000099", 1)
}

/// A database in `dir`, opened with `lock`, holding one project named
/// "Atlas" with `fields` besides; closed again.
fn written(dir: &TempDir, lock: &str, fields: Vec<(&str, Value)>) -> (SpaceId, ObjectId) {
    let db = now(Db::open(options(dir, lock))).unwrap();
    let space = now(db.spaces().create()).unwrap();
    let mut fields = fields;
    fields.push(("name", Value::from("Atlas")));
    let written = now(space.write(|tx| {
        tx.create("projects", fields.clone())?;
        Ok(())
    }))
    .unwrap();
    (space.id(), written.ids[0])
}

#[test]
fn an_additive_change_opens_the_database_and_its_new_field_is_written() {
    let dir = TempDir::new("lock-additive");
    let (space, project) = written(&dir, LOCK, Vec::new());

    let db = now(Db::open(options(&dir, &with_note()))).unwrap();

    let atlas = now(db.spaces().get(space)).unwrap();
    now(atlas.write(|tx| {
        tx.update(
            "projects",
            project,
            [("note", Value::from("Launch in May"))],
        )?;
        Ok(())
    }))
    .unwrap();
    let read = now(atlas.get("projects", project)).unwrap().unwrap();
    assert_eq!(read.get::<&str>("name").unwrap(), Some("Atlas"));
    assert_eq!(read.get::<&str>("note").unwrap(), Some("Launch in May"));
}

/// An app rolled back to the older lock opens the database as an old
/// client: the note it does not know stays in the database, and is read
/// again once the newer lock opens it (RFC 0005, "Two checks").
#[test]
fn an_older_lock_parks_what_it_does_not_know_and_the_newer_one_folds_it_again() {
    let dir = TempDir::new("lock-rollback");
    let note = ("note", Value::from("Launch in May"));
    let (space, project) = written(&dir, &with_note(), vec![note]);

    {
        let db = now(Db::open(options(&dir, LOCK))).unwrap();
        // The older lock rebuilds the view in the background (NOC-156).
        support::rebuilt(&db);
        let atlas = now(db.spaces().get(space)).unwrap();
        let read = now(atlas.get("projects", project)).unwrap().unwrap();
        assert_eq!(read.get::<&str>("name").unwrap(), Some("Atlas"));
        let unknown = read.get::<&str>("note").unwrap_err();
        assert_eq!(unknown.code(), ErrorCode::NotFound);
    }

    let db = now(Db::open(options(&dir, &with_note()))).unwrap();
    let atlas = now(db.spaces().get(space)).unwrap();
    let read = now(atlas.get("projects", project)).unwrap().unwrap();
    assert_eq!(read.get::<&str>("note").unwrap(), Some("Launch in May"));
}

#[test]
fn a_change_that_is_not_additive_is_model_refused_and_names_it() {
    let dir = TempDir::new("lock-refused");
    let (space, project) = written(&dir, LOCK, Vec::new());

    let error = now(Db::open(options(&dir, &without_estimate()))).unwrap_err();

    assert_eq!(error.code(), ErrorCode::ModelRefused);
    assert!(error.to_string().contains("tasks.estimate"), "{error}");
    let db = now(Db::open(options(&dir, LOCK))).unwrap();
    let atlas = now(db.spaces().get(space)).unwrap();
    assert!(now(atlas.get("projects", project)).unwrap().is_some());
}

/// A lock of another lineage is refused even though its collections are
/// the same, so no space of the database answers `SpaceOfAnotherModel`
/// (NOC-57; RFC 0009, "Errors").
#[test]
fn a_lock_of_another_model_is_model_refused_and_names_both_ids() {
    let dir = TempDir::new("lock-other-model");
    let (space, _) = written(&dir, LOCK, Vec::new());

    let error = now(Db::open(options(&dir, &other_model()))).unwrap_err();

    assert_eq!(error.code(), ErrorCode::ModelRefused);
    let message = error.to_string();
    assert!(message.contains(MODEL_ID), "{message}");
    assert!(
        message.contains("00000000000000000000000000000099"),
        "{message}"
    );
    let db = now(Db::open(options(&dir, LOCK))).unwrap();
    now(db.spaces().get(space)).unwrap();
}
