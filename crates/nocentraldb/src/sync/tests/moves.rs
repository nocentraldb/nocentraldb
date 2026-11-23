//! Later edits and two moves at once, between two devices through the
//! relay (RFC 0016, "Later edits", "Two moves at once"; NOC-475).
//!
//! B edits a task offline while A moves it: B's edit reaches the old space
//! after the move, so it is a later edit. `space.moves().later_edits()`
//! lists it, the same after A's database is reopened from disk, and
//! `carry_over` writes it into the new space as one write. When A and B
//! move one task to two spaces at once, `conflicts()` names the space that
//! won and the one that lost.

use super::reopen::open_at;
use super::support::*;
use crate::members::{AddMember, Role};
use crate::moves::Edit;
use crate::space::Space;
use crate::value::Value;
use crate::{Db, ErrorCode, ObjectId};

/// A, whose database lives on disk (`reopen::open_at`).
const A: Who = 1;
/// B, in memory.
const B: Who = 2;

/// A's space with a task, B added as a writer and holding the task.
async fn shared_task(a: &Db, b: &Db) -> (Space, Space, ObjectId) {
    let project = now(a.spaces().create()).unwrap();
    let task = now(project.write(|tx| {
        tx.create("tasks", [("title", Value::from("first"))])?;
        Ok(())
    }))
    .unwrap()
    .ids[0];
    let writer = AddMember {
        role: Role::Writer,
        fingerprint: None,
    };
    now(project.members().add(&now(b.invitation()), writer)).unwrap();
    synced(a).await;
    let mut on_b = Err(());
    eventually("B holds the task", || {
        on_b = now(b.spaces().get(project.id())).map_err(|_| ());
        on_b.as_ref()
            .is_ok_and(|space| now(space.get("tasks", task)).unwrap().is_some())
    })
    .await;
    (project, on_b.unwrap(), task)
}

/// Waits until `db` holds every change the relay holds of `space`.
async fn holds_all(relay: &LocalRelay, db: &Db, space: &Space) {
    let log = relay.log(space.id().0).await;
    eventually("the copy holds the relay's log", || {
        let inner = crate::db::lock(&db.inner);
        log.iter()
            .all(|id| inner.view.relay_position(space.id().0, *id).is_some())
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn an_offline_edit_is_a_later_edit_after_a_reopen_too_and_carries_over() {
    let relay = LocalRelay::start().await;
    let dir = TempDir::new();
    let a = open_at(&dir, &relay.url()).await;
    let b = open(B, &relay.url()).await;
    let (project, on_b, task) = shared_task(&a, &b).await;
    let private = now(a.spaces().create()).unwrap();
    synced(&a).await;

    relay.proxy.cut();
    let read = now(project.get("tasks", task)).unwrap().unwrap();
    now(project.move_objects(&[read], &private)).unwrap();
    now(on_b.update("tasks", task, [("title", Value::from("offline"))])).unwrap();
    relay.proxy.restore();
    synced(&a).await;
    synced(&b).await;
    holds_all(&relay, &a, &project).await;

    let later = now(project.moves().later_edits()).unwrap();
    let [edits] = &later[..] else {
        panic!("one moved object with later edits: {later:?}");
    };
    assert_eq!((edits.id, edits.moved_to), (task, private.id()));
    let [edit] = &edits.edits[..] else {
        panic!("one later edit: {edits:?}");
    };
    assert_eq!(edit.object, task);
    assert_eq!(
        edit.edit,
        Edit::Set {
            field: "title".into(),
            value: Value::from("offline")
        }
    );

    // The same after a reopen: later edits are computed from the log.
    let space = project.id();
    drop((project, private, a));
    let a = open_at(&dir, &relay.url()).await;
    let project = now(a.spaces().get(space)).unwrap();
    assert_eq!(now(project.moves().later_edits()).unwrap(), later);

    now(project.moves().carry_over(&edits.edits)).unwrap();
    let private = now(a.spaces().get(edits.moved_to)).unwrap();
    let there = now(private.get("tasks", task)).unwrap().unwrap();
    assert_eq!(there.get::<&str>("title").unwrap(), Some("offline"));
}

#[tokio::test(flavor = "multi_thread")]
async fn carry_over_refuses_an_edit_that_is_no_later_edit() {
    let relay = LocalRelay::start().await;
    let a = open(A, &relay.url()).await;
    let b = open(B, &relay.url()).await;
    let (project, _, task) = shared_task(&a, &b).await;
    let private = now(a.spaces().create()).unwrap();
    let read = now(project.get("tasks", task)).unwrap().unwrap();
    now(project.move_objects(&[read], &private)).unwrap();

    let forged = crate::moves::LaterEdit {
        object: task,
        collection: "tasks".into(),
        change: crate::ChangeId(ncdb_core::ids::ChangeId([7; 32])),
        position: 0,
        edit: Edit::Set {
            field: "title".into(),
            value: Value::from("forged"),
        },
    };
    let refused = now(project.moves().carry_over(&[forged])).unwrap_err();
    assert_eq!(refused.code(), ErrorCode::InvalidValue);
    let there = now(private.get("tasks", task)).unwrap().unwrap();
    assert_eq!(there.get::<&str>("title").unwrap(), Some("first"));
}

#[tokio::test(flavor = "multi_thread")]
async fn two_moves_at_once_name_the_space_that_won_and_the_one_that_lost() {
    let relay = LocalRelay::start().await;
    let a = open(A, &relay.url()).await;
    let b = open(B, &relay.url()).await;
    let (project, on_b, task) = shared_task(&a, &b).await;
    let to_a = now(a.spaces().create()).unwrap();
    let to_b = now(b.spaces().create()).unwrap();
    synced(&a).await;
    synced(&b).await;

    relay.proxy.cut();
    let read = now(project.get("tasks", task)).unwrap().unwrap();
    now(project.move_objects(&[read], &to_a)).unwrap();
    let read = now(on_b.get("tasks", task)).unwrap().unwrap();
    now(on_b.move_objects(&[read], &to_b)).unwrap();
    relay.proxy.restore();
    synced(&a).await;
    synced(&b).await;
    holds_all(&relay, &a, &project).await;
    holds_all(&relay, &b, &project).await;

    let on_a = now(project.moves().conflicts()).unwrap();
    assert_eq!(
        on_a,
        now(on_b.moves().conflicts()).unwrap(),
        "every copy agrees"
    );
    let [conflict] = &on_a[..] else {
        panic!("one object moved twice at once: {on_a:?}");
    };
    assert_eq!(conflict.id, task);
    let both = [to_a.id(), to_b.id()];
    assert!(both.contains(&conflict.moved_to));
    assert_eq!(conflict.lost.len(), 1);
    assert!(both.contains(&conflict.lost[0]) && conflict.lost[0] != conflict.moved_to);
    let trace = now(project.get("tasks", task)).unwrap().unwrap();
    assert_eq!(trace.moved_to(), Some(conflict.moved_to));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_later_tag_and_a_later_delete_carry_over_as_one_write() {
    let relay = LocalRelay::start().await;
    let a = open(A, &relay.url()).await;
    let b = open(B, &relay.url()).await;
    let (project, on_b, task) = shared_task(&a, &b).await;
    let private = now(a.spaces().create()).unwrap();
    synced(&a).await;

    relay.proxy.cut();
    let read = now(project.get("tasks", task)).unwrap().unwrap();
    now(project.move_objects(&[read], &private)).unwrap();
    now(on_b.update("tasks", task, [("tags", Value::add(["late"]))])).unwrap();
    now(on_b.delete("tasks", task)).unwrap();
    relay.proxy.restore();
    synced(&a).await;
    synced(&b).await;
    holds_all(&relay, &a, &project).await;

    let later = now(project.moves().later_edits()).unwrap();
    let edits: Vec<&Edit> = later[0].edits.iter().map(|e| &e.edit).collect();
    assert_eq!(
        edits,
        [
            &Edit::Add {
                field: "tags".into(),
                elements: vec![Value::from("late")]
            },
            &Edit::Delete
        ]
    );
    now(project.moves().carry_over(&later[0].edits)).unwrap();
    let there = now(private.get("tasks", task)).unwrap().unwrap();
    assert!(there.deleted());
    assert_eq!(
        there.get::<&[Value]>("tags").unwrap().unwrap(),
        [Value::from("late")]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn later_edits_of_objects_moved_to_two_spaces_carry_over_one_space_at_a_time() {
    let relay = LocalRelay::start().await;
    let a = open(A, &relay.url()).await;
    let b = open(B, &relay.url()).await;
    let (project, on_b, first) = shared_task(&a, &b).await;
    let second = now(project.write(|tx| {
        tx.create("tasks", [("title", Value::from("second"))])?;
        Ok(())
    }))
    .unwrap()
    .ids[0];
    let (one, two) = (
        now(a.spaces().create()).unwrap(),
        now(a.spaces().create()).unwrap(),
    );
    synced(&a).await;
    eventually("B holds the second task", || {
        now(on_b.get("tasks", second)).unwrap().is_some()
    })
    .await;

    relay.proxy.cut();
    let read = now(project.get("tasks", first)).unwrap().unwrap();
    now(project.move_objects(&[read], &one)).unwrap();
    let read = now(project.get("tasks", second)).unwrap().unwrap();
    now(project.move_objects(&[read], &two)).unwrap();
    now(on_b.write(|tx| {
        tx.update("tasks", first, [("title", Value::from("b1"))])?;
        tx.update("tasks", second, [("title", Value::from("b2"))])
    }))
    .unwrap();
    relay.proxy.restore();
    synced(&a).await;
    synced(&b).await;
    holds_all(&relay, &a, &project).await;

    let later = now(project.moves().later_edits()).unwrap();
    assert_eq!(later.len(), 2);
    let all: Vec<_> = later.iter().flat_map(|l| l.edits.clone()).collect();
    let refused = now(project.moves().carry_over(&all)).unwrap_err();
    assert_eq!(refused.code(), ErrorCode::InvalidValue);
    for edits in &later {
        now(project.moves().carry_over(&edits.edits)).unwrap();
    }
    let one_read = now(one.get("tasks", first)).unwrap().unwrap();
    assert_eq!(one_read.get::<&str>("title").unwrap(), Some("b1"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_later_personal_value_is_read_as_other_and_not_carried_over() {
    // `tasks.title` is personal to the task's project; the moved task
    // holds no title, so it moves, and B's title written while it moved
    // is a sealed value this SDK does not carry over (NOC-477).
    let lock = LOCK.replacen(
        r#""title": { "rule": 1, "since": 1, "type": "ncdb/string" }"#,
        r#""title": { "personal": { "person": "project" }, "rule": 1, "since": 1, "type": "ncdb/string" }"#,
        1,
    );
    let relay = LocalRelay::start().await;
    let a = open_with(A, &relay.url(), &lock).await;
    let b = open_with(B, &relay.url(), &lock).await;
    let space = now(a.spaces().create()).unwrap();
    let written = now(space.write(|tx| {
        let project = tx.create("projects", [("name", Value::from("p"))])?;
        tx.create("tasks", [("project", Value::from(project))])?;
        Ok(())
    }))
    .unwrap();
    let (project, task) = (written.ids[0], written.ids[1]);
    let writer = AddMember {
        role: Role::Writer,
        fingerprint: None,
    };
    now(space.members().add(&now(b.invitation()), writer)).unwrap();
    let private = now(a.spaces().create()).unwrap();
    synced(&a).await;
    let mut on_b = Err(());
    eventually("B holds the task", || {
        on_b = now(b.spaces().get(space.id())).map_err(|_| ());
        on_b.as_ref()
            .is_ok_and(|s| now(s.get("tasks", task)).unwrap().is_some())
    })
    .await;
    let on_b = on_b.unwrap();

    relay.proxy.cut();
    let read = [
        now(space.get("projects", project)).unwrap().unwrap(),
        now(space.get("tasks", task)).unwrap().unwrap(),
    ];
    // The project's name is not personal; the task's title is unset.
    now(space.move_objects(&read, &private)).unwrap();
    let wrote = now(on_b.update("tasks", task, [("title", Value::from("secret"))]));
    relay.proxy.restore();
    wrote.expect("B holds the project's person key once it synced");
    synced(&a).await;
    synced(&b).await;
    holds_all(&relay, &a, &space).await;

    let later = now(space.moves().later_edits()).unwrap();
    let edit = &later[0].edits[0];
    assert_eq!(edit.edit, Edit::Other);
    let refused = now(space.moves().carry_over(std::slice::from_ref(edit))).unwrap_err();
    assert_eq!(refused.code(), ErrorCode::NotAllowed);
}
