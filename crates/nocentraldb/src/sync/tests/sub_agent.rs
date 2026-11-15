//! A sub-agent joins its parent's space through the relay (NOC-318; RFC
//! 0003, "Sub-agents get an ad-hoc space"): the parent agent adds it by its
//! version 3 invitation, signing its certificate; the relay and the
//! sub-agent's own fold accept the add on the parent's certificate; the
//! sub-agent reads what the parent wrote and writes what the parent reads.
//! In an ad-hoc space, the sub-agent works on copies and never holds the
//! sources; the parent writes its results back by the copies' sources.

use super::support::*;
use crate::ad_hoc::AdHoc;
use crate::db::{Db, OpenOptions};
use crate::members::{AddMember, MemberKind, Role};
use crate::value::Value;

/// The parent agent.
const PARENT: Who = 1;

#[tokio::test(flavor = "multi_thread")]
async fn an_agent_adds_its_sub_agent_and_both_read_each_others_writes() {
    let relay = LocalRelay::start().await;
    let options = OpenOptions::new(LOCK)
        .in_memory()
        .relay(&relay.url())
        .agent();
    let parent = Db::open_as(options, [PARENT; 32], [PARENT ^ 0x80; 32], account())
        .await
        .unwrap();
    let child = Db::open(
        OpenOptions::new(LOCK)
            .in_memory()
            .relay(&relay.url())
            .sub_agent(),
    )
    .await
    .unwrap();

    let project = now(parent.spaces().create()).unwrap();
    let task = now(project.write(|tx| {
        tx.create("tasks", [("title", Value::from("for the sub-agent"))])?;
        Ok(())
    }))
    .unwrap()
    .ids[0];
    let invitation = now(child.invitation());
    let writer = AddMember {
        role: Role::Writer,
        fingerprint: None,
    };
    now(project.members().add(&invitation, writer)).unwrap();
    synced(&parent).await;

    let mut held = None;
    eventually("the sub-agent holds the space and the task", || {
        held = now(child.spaces().get(project.id()))
            .ok()
            .filter(|space| matches!(now(space.get("tasks", task)), Ok(Some(_))));
        held.is_some()
    })
    .await;
    let on_child = held.unwrap();
    let members = now(on_child.members().list()).unwrap();
    let me = members.iter().find(|m| m.is_self).unwrap();
    assert_eq!(me.kind, MemberKind::SubAgent);
    assert_eq!(me.role, Some(Role::Writer));

    let done = now(on_child.write(|tx| {
        tx.update(
            "tasks",
            task,
            [("title", Value::from("done by the sub-agent"))],
        )?;
        Ok(())
    }))
    .unwrap();
    let status = synced(&child).await;
    assert_eq!(status.refused, 0);
    eventually("the parent reads the sub-agent's write", || {
        now(project.get("tasks", task))
            .unwrap()
            .and_then(|t| t.get::<&str>("title").unwrap().map(str::to_owned))
            .as_deref()
            == Some("done by the sub-agent")
    })
    .await;
    assert_eq!(done.ids, Vec::new());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_sub_agent_works_on_copies_in_an_ad_hoc_space_and_the_parent_writes_back() {
    let relay = LocalRelay::start().await;
    let options = OpenOptions::new(LOCK)
        .in_memory()
        .relay(&relay.url())
        .agent();
    let parent = Db::open_as(options, [PARENT; 32], [PARENT ^ 0x80; 32], account())
        .await
        .unwrap();
    let child = Db::open(
        OpenOptions::new(LOCK)
            .in_memory()
            .relay(&relay.url())
            .sub_agent(),
    )
    .await
    .unwrap();

    let project = now(parent.spaces().create()).unwrap();
    let task = now(project.write(|tx| {
        tx.create(
            "tasks",
            [
                ("title", Value::from("a task")),
                ("status", Value::from("open")),
            ],
        )?;
        Ok(())
    }))
    .unwrap()
    .ids[0];
    let source = now(project.get("tasks", task)).unwrap().unwrap();
    let created = now(parent.spaces().create_ad_hoc(AdHoc {
        copy: vec![source],
        ..AdHoc::default()
    }))
    .unwrap();
    let ad_hoc = created.space;
    let copy = created.copies[0].1;
    let invitation = now(child.invitation());
    let writer = AddMember {
        role: Role::Writer,
        fingerprint: None,
    };
    now(ad_hoc.members().add(&invitation, writer)).unwrap();
    synced(&parent).await;

    let mut held = None;
    eventually("the sub-agent holds the ad-hoc space and the copy", || {
        held = now(child.spaces().get(ad_hoc.id()))
            .ok()
            .filter(|space| matches!(now(space.get("tasks", copy)), Ok(Some(_))));
        held.is_some()
    })
    .await;
    let on_child = held.unwrap();
    // The sub-agent holds neither the source space nor the mapping.
    assert!(now(child.spaces().get(project.id())).is_err());
    assert_eq!(now(on_child.source_of(copy)).unwrap(), None);
    assert!(now(on_child.get("tasks", task)).unwrap().is_none());

    now(on_child.update("tasks", copy, [("status", Value::from("done"))])).unwrap();
    let status = synced(&child).await;
    assert_eq!(status.refused, 0);
    eventually("the parent reads the sub-agent's result", || {
        now(ad_hoc.get("tasks", copy))
            .unwrap()
            .and_then(|t| t.get::<&str>("status").unwrap().map(str::to_owned))
            .as_deref()
            == Some("done")
    })
    .await;

    // The parent writes the result back into the source it copied.
    let (source_space, source) = now(ad_hoc.source_of(copy)).unwrap().unwrap();
    assert_eq!((source_space, source), (project.id(), task));
    now(project.update("tasks", source, [("status", Value::from("done"))])).unwrap();
    let read = now(project.get("tasks", task)).unwrap().unwrap();
    assert_eq!(read.get::<&str>("status").unwrap(), Some("done"));
}
