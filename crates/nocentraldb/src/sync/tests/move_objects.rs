//! A move between two devices through the relay (RFC 0016, "Conflicts at
//! the move", "What the members of the old space keep"; NOC-475). Two
//! devices write one task's title at once, so the copy holds a conflict;
//! A then moves the task into a space B does not belong to. The move writes
//! the winning title, and its result names the field that held the
//! conflict. B, a member of the old space only, reads the trace: moved,
//! with the new space, and no fields.

use super::support::*;
use crate::members::{AddMember, Role};
use crate::value::Value;

const A: Who = 1;
const B: Who = 2;

#[tokio::test(flavor = "multi_thread")]
async fn a_move_resolves_a_conflict_names_it_and_the_old_members_read_the_trace() {
    let relay = LocalRelay::start().await;
    let a = open(A, &relay.url()).await;
    let b = open(B, &relay.url()).await;

    let project = now(a.spaces().create()).unwrap();
    let task = now(project.write(|tx| {
        tx.create("tasks", [("title", Value::from("first"))])?;
        Ok(())
    }))
    .unwrap()
    .ids[0];
    let invitation = now(b.invitation());
    let writer = AddMember {
        role: Role::Writer,
        fingerprint: None,
    };
    now(project.members().add(&invitation, writer)).unwrap();
    synced(&a).await;
    let mut on_b = Err(());
    eventually("B holds the task", || {
        on_b = now(b.spaces().get(project.id())).map_err(|_| ());
        on_b.as_ref()
            .is_ok_and(|space| now(space.get("tasks", task)).unwrap().is_some())
    })
    .await;
    let on_b = on_b.unwrap();

    relay.proxy.cut();
    now(project.update("tasks", task, [("title", Value::from("A's"))])).unwrap();
    now(on_b.update("tasks", task, [("title", Value::from("B's"))])).unwrap();
    relay.proxy.restore();
    synced(&a).await;
    synced(&b).await;
    let log = relay.log(project.id().0).await;
    eventually("A holds B's title", || {
        let inner = crate::db::lock(&a.inner);
        log.iter()
            .all(|id| inner.view.relay_position(project.id().0, *id).is_some())
    })
    .await;
    let before = now(project.get("tasks", task)).unwrap().unwrap();
    let winner = before.get::<&str>("title").unwrap().unwrap().to_owned();

    let private = now(a.spaces().create()).unwrap();
    let read = now(project.get("tasks", task)).unwrap().unwrap();
    let moved = now(project.move_objects(&[read], &private)).unwrap();

    assert_eq!(moved.conflicts, [(task, "title".to_owned())]);
    let there = now(private.get("tasks", task)).unwrap().unwrap();
    assert_eq!(there.get::<&str>("title").unwrap(), Some(winner.as_str()));

    synced(&a).await;
    eventually("B reads the trace", || {
        now(on_b.get("tasks", task))
            .unwrap()
            .is_some_and(|trace| trace.moved_to() == Some(private.id()))
    })
    .await;
    let trace = now(on_b.get("tasks", task)).unwrap().unwrap();
    assert_eq!(trace.get::<&str>("title").unwrap(), None);
    assert!(
        now(b.spaces().get(private.id())).is_err(),
        "B holds no copy of the new space"
    );
}
