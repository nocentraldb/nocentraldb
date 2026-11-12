//! `db.spaces().subscribe()` (RFC 0009, "Spaces"; Carsten, 2026-11-12,
//! NOC-241): the space list as a stream, the current list at once, then a
//! new list whenever a space is added, left or removed, or the member's
//! role in it changes, as when another device shares a space with this
//! member through the relay.

use std::time::Duration;

use futures_core::Stream;

use super::support::*;
use crate::db::{Db, lock};
use crate::list::SpaceInfo;
use crate::members::{AddMember, MemberId, Role};

const ADMIN: Who = 1;
const MEMBER: Who = 2;

/// The next list `stream` yields, within a few seconds.
async fn next(stream: &mut (impl Stream<Item = Vec<SpaceInfo>> + Unpin)) -> Vec<SpaceInfo> {
    let item = std::future::poll_fn(|cx| std::pin::Pin::new(&mut *stream).poll_next(cx));
    tokio::time::timeout(Duration::from_secs(20), item)
        .await
        .expect("a list in time")
        .expect("the stream does not end")
}

/// The lists `stream` yields until one satisfies `done`.
async fn until(
    stream: &mut (impl Stream<Item = Vec<SpaceInfo>> + Unpin),
    done: impl Fn(&[SpaceInfo]) -> bool,
) -> Vec<SpaceInfo> {
    loop {
        let list = next(stream).await;
        if done(&list) {
            return list;
        }
    }
}

fn member_id(db: &Db) -> MemberId {
    MemberId(lock(&db.inner).member.key().id())
}

#[tokio::test]
async fn the_stream_yields_the_list_at_once() {
    let db = crate::Db::open(crate::OpenOptions::new(LOCK).in_memory())
        .await
        .unwrap();
    let space = db.spaces().create().await.unwrap();
    let mut lists = db.spaces().subscribe();
    let first = next(&mut lists).await;
    assert_eq!(first, db.spaces().list().await.unwrap());
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].id, space.id());
}

#[tokio::test]
async fn a_space_created_here_yields_a_new_list() {
    let db = crate::Db::open(crate::OpenOptions::new(LOCK).in_memory())
        .await
        .unwrap();
    let mut lists = db.spaces().subscribe();
    assert!(next(&mut lists).await.is_empty());
    let space = db.spaces().create().await.unwrap();
    let list = next(&mut lists).await;
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, space.id());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_space_shared_from_another_device_arrives_and_changes_follow() {
    let relay = LocalRelay::start().await;
    let admin = open(ADMIN, &relay.url()).await;
    let member = open(MEMBER, &relay.url()).await;
    let mut lists = member.spaces().subscribe();
    assert!(next(&mut lists).await.is_empty());

    // Shared from the admin's device, through the relay.
    let project = admin.spaces().create().await.unwrap();
    let options = AddMember {
        role: Role::Writer,
        fingerprint: None,
    };
    project
        .members()
        .add(&member.invitation().await, options)
        .await
        .unwrap();
    let shared = until(&mut lists, |list| {
        list.iter()
            .any(|s| s.id == project.id() && s.role == Some(Role::Writer))
    })
    .await;
    assert_eq!(shared.len(), 1);

    // A role change.
    project
        .members()
        .set_role(member_id(&member), Role::Reader)
        .await
        .unwrap();
    until(&mut lists, |list| {
        list.iter()
            .any(|s| s.id == project.id() && s.role == Some(Role::Reader))
    })
    .await;

    // A removal.
    project.members().remove(member_id(&member)).await.unwrap();
    until(&mut lists, |list| {
        list.iter()
            .any(|s| s.id == project.id() && s.role.is_none())
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn leaving_a_space_yields_a_new_list() {
    let relay = LocalRelay::start().await;
    let admin = open(ADMIN, &relay.url()).await;
    let member = open(MEMBER, &relay.url()).await;
    let project = admin.spaces().create().await.unwrap();
    let options = AddMember {
        role: Role::Writer,
        fingerprint: None,
    };
    project
        .members()
        .add(&member.invitation().await, options)
        .await
        .unwrap();
    let mut lists = member.spaces().subscribe();
    until(&mut lists, |list| {
        list.iter()
            .any(|s| s.id == project.id() && s.role == Some(Role::Writer))
    })
    .await;

    let mine = member.spaces().get(project.id()).await.unwrap();
    mine.leave().await.unwrap();
    until(&mut lists, |list| {
        list.iter()
            .all(|s| s.id != project.id() || s.role.is_none())
    })
    .await;
}
