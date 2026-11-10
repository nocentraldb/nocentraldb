//! What a write takes from the log (NOC-138; RFC 0002, "The change" and
//! "Dependencies") equals a full recount of the log, which reads and opens
//! every change of the space: after the member's own writes, after
//! changes pulled from another member, after a revocation un-folds pulled
//! changes, after the database is reopened, and after it is reopened with
//! a changed lock, which folds the log again (NOC-148).

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use ncdb_core::change::Kind;
use ncdb_core::checks::SealChecks;
use ncdb_core::clock::Hlc;
use ncdb_core::ids::{ChangeId, MemberId, SpaceId};
use ncdb_core::log::{Position, RejectReason, State};
use ncdb_core::membership::ControlBody;
use ncdb_core::receive::Checks;
use ncdb_core::view::View;

use crate::db::{Db, OpenOptions};
use crate::members::tests::{copy, now, remove};
use crate::members::{AddMember, Role};
use crate::space::Space;
use crate::value::Value;

const LOCK: &str = include_str!("../tests/model/model.lock.json");

/// [`LOCK`] one version on, with a text `note` on projects: an additive
/// change, which folds the log again when it opens the database.
fn with_note() -> String {
    let name = r#""name": { "rule": 1, "since": 1, "type": "ncdb/string" }"#;
    let note = r#""note": { "rule": 1, "since": 2, "type": "ncdb/string" }"#;
    assert!(LOCK.contains(name));
    LOCK.replacen(name, &format!("{name}, {note}"), 1).replacen(
        r#""version": 1"#,
        r#""version": 2"#,
        1,
    )
}

/// A directory of its own, removed when dropped.
struct Dir(PathBuf);

impl Dir {
    fn new() -> Dir {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let name = format!("ncdb-next-{}-{n}", std::process::id());
        let dir = Dir(std::env::temp_dir().join(name));
        let _ = std::fs::remove_dir_all(&dir.0);
        dir
    }

    /// The database in this directory, opened with `lock`.
    fn open(&self, lock: &str) -> Db {
        let options = OpenOptions::new(lock)
            .path(self.0.join("data"))
            .key_file(self.0.join("member.key"));
        now(Db::open(options)).expect("the database opens")
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// What a full recount of the log of `space` gives the member `me`'s next
/// change: the generation, the next sequence, the heads, the highest
/// clock, and the heads among the control changes. It reads and opens
/// every change, as `crate::next` did before NOC-138.
#[derive(Debug, PartialEq, Eq)]
struct Expected {
    generation: Option<u64>,
    sequence: u64,
    heads: BTreeSet<ChangeId>,
    seen: Hlc,
    control_heads: BTreeSet<ChangeId>,
}

fn recount(view: &View, space: SpaceId, checks: &SealChecks, me: MemberId) -> Expected {
    let mut expected = Expected {
        generation: checks.resolved().current_generation(),
        sequence: 1,
        heads: BTreeSet::new(),
        seen: Hlc::default(),
        control_heads: BTreeSet::new(),
    };
    let log = view.log(space).expect("the space's log");
    let mut covered = BTreeSet::new();
    let mut controls_covered = BTreeSet::new();
    for (_, change) in log.after(Position::START) {
        let header = change.header();
        if header.author == me {
            expected.sequence = expected.sequence.max(header.sequence + 1);
        }
        if !matches!(log.state(change.id()), Some(State::Folded { .. })) {
            continue;
        }
        let dependencies = match header.kind {
            Kind::CONTROL => {
                let dependencies = ControlBody::decode(&header.payload)
                    .map(|body| body.dependencies)
                    .unwrap_or_default();
                controls_covered.extend(dependencies.iter().copied());
                expected.control_heads.insert(change.id());
                dependencies
            }
            _ => match checks.open(change) {
                Ok(payload) => {
                    expected.seen = expected.seen.max(payload.clock);
                    payload.dependencies
                }
                Err(_) => continue,
            },
        };
        covered.extend(dependencies);
        expected.heads.insert(change.id());
    }
    expected.heads.retain(|id| !covered.contains(id));
    expected
        .control_heads
        .retain(|id| !controls_covered.contains(id));
    expected
}

/// What `db`'s next change in `space` takes, which must equal a full
/// recount of its log; returns its heads.
fn assert_next(db: &Db, space: &Space) -> BTreeSet<ChangeId> {
    let mut inner = db.lock();
    let me = inner.member.key().id();
    let crate::db::Inner { view, spaces, .. } = &mut *inner;
    let state = &spaces[&space.id().0];
    let expected = recount(view, space.id().0, &state.checks, me);
    let next = crate::next::next(view, space.id().0, &state.checks, me);
    let control_heads = crate::members::control_heads(view, space.id().0);
    let actual = Expected {
        generation: next.generation,
        sequence: next.sequence,
        heads: next.heads,
        seen: next.seen,
        control_heads,
    };
    assert_eq!(actual, expected);
    actual.heads
}

/// `n` tasks written into `space`, one change each.
fn write(space: &Space, n: usize) {
    for i in 0..n {
        now(space.write(|tx| {
            tx.create("tasks", [("title", Value::from(format!("task {i}")))])?;
            Ok(())
        }))
        .expect("the write");
    }
}

/// Whether `db` holds a change of `space` rejected because its author is
/// no member.
fn un_folded(db: &Db, space: &Space) -> bool {
    let inner = db.lock();
    let log = inner.view.log(space.id().0).expect("the space's log");
    log.after(Position::START).any(|(_, change)| {
        log.state(change.id()) == Some(&State::Rejected(RejectReason::NotMember))
    })
}

#[test]
fn a_write_takes_what_a_full_recount_gives_through_writes_pulls_un_folds_and_reopens() {
    let dir = Dir::new();
    let admin = dir.open(LOCK);
    let project = now(admin.spaces().create()).unwrap();
    assert_next(&admin, &project);
    write(&project, 3);
    assert_next(&admin, &project);

    let member = now(Db::open(OpenOptions::new(LOCK).in_memory())).unwrap();
    let invitation = now(member.invitation());
    let add = AddMember {
        role: Role::Writer,
        fingerprint: Some(invitation.fingerprint()),
    };
    now(project.members().add(&invitation, add)).unwrap();
    assert_next(&admin, &project);
    copy(&admin, &member, &project);
    let on_member = now(member.spaces().get(project.id())).unwrap();
    assert_next(&member, &on_member);
    write(&on_member, 2);
    assert_next(&member, &on_member);

    // Both write at once, then each pulls the other's changes.
    write(&project, 1);
    copy(&member, &admin, &project);
    copy(&admin, &member, &project);
    assert_eq!(assert_next(&admin, &project).len(), 2, "a fork");
    assert_eq!(assert_next(&member, &on_member).len(), 2, "a fork");
    write(&on_member, 1);
    copy(&member, &admin, &project);
    assert_next(&admin, &project);

    // The removal un-folds the member's changes on the admin's copy.
    remove(&admin, &project, &invitation);
    assert!(un_folded(&admin, &project));
    assert_next(&admin, &project);
    write(&project, 1);
    assert_next(&admin, &project);
    drop(admin);

    let admin = dir.open(LOCK);
    let project = now(admin.spaces().get(project.id())).unwrap();
    assert_next(&admin, &project);
    write(&project, 1);
    assert_next(&admin, &project);
    drop(admin);

    let admin = dir.open(&with_note());
    let project = now(admin.spaces().get(project.id())).unwrap();
    assert_next(&admin, &project);
    write(&project, 1);
    assert_next(&admin, &project);
}
