//! A member the relay no longer serves a space (RFC 0007, "The errors":
//! `not_member` and `removed_member`; RFC 0009, "Sync and status"). The
//! relay serves no change to a removed member, so the member never folds
//! its own removal; the relay's answer is what tells it. From then on it
//! stops syncing the space, lists it without a role, refuses writes into
//! it with `NotMember`, reports it in the sync status, and keeps reading
//! its copy.
//!
//! The admin removes the member through the crate's internals, since
//! `members().remove` is not built.

use super::support::*;
use crate::db::Db;
use crate::error::ErrorCode;
use crate::id::ObjectId;
use crate::list::{SpaceInfo, SpaceModel};
use crate::members::tests::{regrant, remove};
use crate::members::{AddMember, Invitation, Role};
use crate::space::Space;
use crate::value::Value;

/// The admin, who creates the space.
const ADMIN: Who = 1;
/// The member the admin adds and then removes.
const MEMBER: Who = 2;

/// Both databases, synced with the relay at `relay`, and the space the
/// admin shares with the member as a writer, with one task in it.
struct Shared {
    admin: Db,
    member: Db,
    on_admin: Space,
    on_member: Space,
    invitation: Invitation,
    task: ObjectId,
}

async fn shared(relay: &LocalRelay) -> Shared {
    let admin = open(ADMIN, &relay.url()).await;
    let member = open(MEMBER, &relay.url()).await;
    let on_admin = now(admin.spaces().create()).unwrap();
    let task = now(on_admin.write(|tx| {
        tx.create("tasks", [("title", Value::from("from the admin"))])?;
        Ok(())
    }))
    .unwrap()
    .ids[0];
    let invitation = now(member.invitation());
    let writer = AddMember {
        role: Role::Writer,
        fingerprint: None,
    };
    now(on_admin.members().add(&invitation, writer)).unwrap();
    synced(&admin).await;

    // The member stays connected: the relay tells it that it was added
    // (RFC 0007, "Subscribe", decision R13).
    let mut on_member = Err(());
    eventually("the member holds the admin's task", || {
        on_member = now(member.spaces().get(on_admin.id())).map_err(|_| ());
        on_member
            .as_ref()
            .is_ok_and(|space| now(space.get("tasks", task)).unwrap().is_some())
    })
    .await;
    synced(&member).await;
    // The task may come from the member's pull, before its subscription
    // caught up; a removal the relay appends until then is `not_member`,
    // not `removed_member` (NOC-184).
    let space = on_admin.id().0;
    eventually("the member's subscription caught up", || {
        crate::db::lock(&member.inner).sync_live(space)
    })
    .await;
    Shared {
        admin,
        member,
        on_admin,
        on_member: on_member.unwrap(),
        invitation,
        task,
    }
}

/// The member's list of spaces shows `space` without a role.
fn listed_without_a_role(member: &Db, space: &Space) -> bool {
    now(member.spaces().list()).unwrap()
        == vec![SpaceInfo {
            id: space.id(),
            role: None,
            model: SpaceModel::Ours,
        }]
}

/// The member keeps reading its copy, and a write into the space, or
/// adding a member to it, is refused with `NotMember`.
fn reads_but_may_not_write(shared: &Shared) {
    let held = now(shared.member.spaces().get(shared.on_admin.id())).unwrap();
    let task = now(held.get("tasks", shared.task)).unwrap();
    assert!(task.is_some(), "the member keeps its copy");
    let refused = now(shared.on_member.write(|tx| {
        tx.create("tasks", [("title", Value::from("after the removal"))])?;
        Ok(())
    }))
    .unwrap_err();
    assert_eq!(refused.code(), ErrorCode::NotMember);
    let other = now(shared.admin.invitation());
    let writer = AddMember {
        role: Role::Writer,
        fingerprint: None,
    };
    let refused = now(shared.on_member.members().add(&other, writer)).unwrap_err();
    assert_eq!(refused.code(), ErrorCode::NotMember);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_member_removed_while_connected_sees_itself_removed() {
    let relay = LocalRelay::start().await;
    let shared = shared(&relay).await;

    remove(&shared.admin, &shared.on_admin, &shared.invitation);
    synced(&shared.admin).await;

    eventually("the member lists the space without a role", || {
        listed_without_a_role(&shared.member, &shared.on_admin)
    })
    .await;
    let status = shared.member.sync().current();
    assert_eq!(status.error.as_deref(), Some("removed_member"));
    reads_but_may_not_write(&shared);

    // The report outlasts the connection that brought it.
    relay.proxy.cut();
    relay.proxy.restore();
    let status = synced(&shared.member).await;
    assert_eq!(status.error.as_deref(), Some("removed_member"));
    assert!(listed_without_a_role(&shared.member, &shared.on_admin));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_member_removed_while_offline_sees_itself_removed_when_it_connects() {
    let relay = LocalRelay::start().await;
    let shared = shared(&relay).await;

    // Both databases go offline; the admin's removal reaches the relay
    // through a raw connection of the admin's member key.
    relay.proxy.cut();
    let removal = remove(&shared.admin, &shared.on_admin, &shared.invitation);
    let mut client = relay.client(ADMIN).await;
    push(&mut client, &[&removal]).await;
    relay.proxy.restore();

    eventually("the member lists the space without a role", || {
        listed_without_a_role(&shared.member, &shared.on_admin)
    })
    .await;
    let status = synced(&shared.member).await;
    assert_eq!(status.error.as_deref(), Some("not_member"));
    reads_but_may_not_write(&shared);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_removed_member_added_again_syncs_the_space_again() {
    let relay = LocalRelay::start().await;
    let shared = shared(&relay).await;
    remove(&shared.admin, &shared.on_admin, &shared.invitation);
    synced(&shared.admin).await;
    eventually("the member lists the space without a role", || {
        listed_without_a_role(&shared.member, &shared.on_admin)
    })
    .await;

    // The admin rotates and adds the member again (RFC 0008, M5); the
    // changes reach the relay through a raw connection of its member key.
    let [rotation, add] = regrant(&shared.admin, &shared.on_admin, &shared.invitation);
    let mut client = relay.client(ADMIN).await;
    push(&mut client, &[&rotation, &add]).await;

    eventually("the member lists the space as a writer again", || {
        now(shared.member.spaces().list()).unwrap()
            == vec![SpaceInfo {
                id: shared.on_admin.id(),
                role: Some(Role::Writer),
                model: SpaceModel::Ours,
            }]
    })
    .await;
    let status = synced(&shared.member).await;
    assert_eq!(status.error, None);
    let space = shared.on_admin.id().0;
    let log = relay.log(space).await;
    eventually("the member holds the relay's whole log", || {
        let inner = crate::db::lock(&shared.member.inner);
        log.iter()
            .all(|id| inner.view.relay_position(space, *id).is_some())
    })
    .await;
}
