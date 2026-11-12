//! The view's own report, `db.view()` (RFC 0009, "Sync and status";
//! RFC 0006, "What a rebuild costs"; the names the owner decided on
//! NOC-227): whether a rebuild of the view runs, and how far it is, in
//! frames of the log. `nc.open` resolves before a rebuild ends; the app
//! reads the old view meanwhile, and the SDK runs the rebuild's pages in
//! the background until it swaps the new view in.

mod support;

use nocentraldb::{Db, ErrorCode, OpenOptions, Value};
use support::{LOCK, TempDir, now, ready, rebuilt};

fn options(dir: &TempDir, lock: &str) -> OpenOptions {
    OpenOptions::new(lock)
        .path(dir.join("data"))
        .key_file(dir.join("member.key"))
}

/// [`LOCK`] one version on, with a text `note` on projects.
fn with_note() -> String {
    let name = r#""name": { "rule": 1, "since": 1, "type": "ncdb/string" }"#;
    LOCK.replacen(
        name,
        &format!(r#"{name}, "note": {{ "rule": 1, "since": 2, "type": "ncdb/string" }}"#),
        1,
    )
    .replacen(r#""version": 1"#, r#""version": 2"#, 1)
}

#[test]
fn a_database_whose_view_fits_its_lock_runs_no_rebuild() {
    let db = support::open();

    let status = db.view().current();

    assert_eq!(status.rebuild, None);
}

#[test]
fn a_rollback_rebuilds_the_view_in_the_background_and_reports_how_far_it_is() {
    let dir = TempDir::new("view-rebuild");
    let (space, projects) = {
        let db = now(Db::open(options(&dir, &with_note()))).unwrap();
        let space = now(db.spaces().create()).unwrap();
        let mut projects = Vec::new();
        for n in 0..40 {
            let written = now(space.write(|tx| {
                tx.create(
                    "projects",
                    [
                        ("name", Value::from(format!("p{n}"))),
                        ("note", Value::from("kept")),
                    ],
                )?;
                Ok(())
            }))
            .unwrap();
            projects.push(written.ids[0]);
        }
        (space.id(), projects)
    };

    let db = now(Db::open(options(&dir, LOCK))).unwrap();
    let mut follow = db.view().subscribe();
    let first = ready(&mut follow).expect("the stream yields at once");
    let seen = rebuilt(&db);

    for status in std::iter::once(&first).chain(&seen) {
        if let Some(progress) = status.rebuild {
            assert!(progress.folded <= progress.total, "{status:?}");
            assert!(progress.total >= 40, "{status:?}");
        }
    }
    assert_eq!(seen.last().unwrap().rebuild, None);
    // The rebuilt view folds by the older model: the note is parked.
    let atlas = now(db.spaces().get(space)).unwrap();
    let read = now(atlas.get("projects", projects[0])).unwrap().unwrap();
    assert_eq!(read.get::<&str>("name").unwrap(), Some("p0"));
    let unknown = read.get::<&str>("note").unwrap_err();
    assert_eq!(unknown.code(), ErrorCode::NotFound);
}
