//! Leaving a space through the relay (RFC 0009, "Spaces"; RFC 0008, "Roles
//! and rights"): the relay accepts the member's removal of itself, the
//! admin sees it leave and keeps working, and the relay sends the member
//! nothing new.

use crate::db::Db;
use crate::id::ObjectId;
use crate::list::{SpaceInfo, SpaceModel};
use crate::members::{AddMember, NoticeCode, Role};
use crate::space::Space;
use crate::sync::tests::support::*;
use crate::value::Value;

/// The admin, who creates the space.
const ADMIN: Who = 1;
/// The member that joins and leaves.
const MEMBER: Who = 2;

/// The member's list of spaces shows `space` with `role`.
fn listed(member: &Db, space: &Space, role: Option<Role>) -> bool {
    now(member.spaces().list()).unwrap()
        == vec![SpaceInfo {
            id: space.id(),
            role,
            model: SpaceModel::Ours,
        }]
}

/// A task with `title`, written into `space`.
fn task(space: &Space, title: &str) -> ObjectId {
    now(space.write(|tx| {
        tx.create("tasks", [("title", Value::from(title))])?;
        Ok(())
    }))
    .unwrap()
    .ids[0]
}

#[tokio::test(flavor = "multi_thread")]
async fn a_member_that_left_receives_nothing_new_and_the_admin_keeps_working() {
    let relay = LocalRelay::start().await;
    let admin = open(ADMIN, &relay.url()).await;
    let member = open(MEMBER, &relay.url()).await;
    let project = now(admin.spaces().create()).unwrap();
    let invitation = now(member.invitation());
    let writer = AddMember {
        role: Role::Writer,
        fingerprint: None,
    };
    now(project.members().add(&invitation, writer)).unwrap();
    synced(&admin).await;
    eventually("the member lists the space as a writer", || {
        listed(&member, &project, Some(Role::Writer))
    })
    .await;
    let on_member = now(member.spaces().get(project.id())).unwrap();

    let notice = now(on_member.leave()).unwrap();

    assert_eq!(notice.code, NoticeCode::SpaceLeft);
    let status = synced(&member).await;
    assert_eq!(status.refused, 0);
    assert!(listed(&member, &project, None));
    eventually("the admin lists only itself", || {
        now(project.members().list()).unwrap().len() == 1
    })
    .await;
    let after = task(&project, "after the leave");
    let status = synced(&admin).await;
    assert_eq!(status.refused, 0);
    assert!(now(project.get("tasks", after)).unwrap().is_some());
    assert!(now(on_member.get("tasks", after)).unwrap().is_none());
}
