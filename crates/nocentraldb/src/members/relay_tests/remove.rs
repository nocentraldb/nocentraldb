//! Removing a member (RFC 0009, "Invitations and members"; RFC 0003,
//! "Removing a member and rotating the space key"): the admin's removal,
//! its key chain entry and the envelopes reach the relay, which accepts
//! them; the removed member sees itself removed and cannot read what the
//! admin writes after the rotation.

use crate::db::Db;
use crate::id::ObjectId;
use crate::list::{SpaceInfo, SpaceModel};
use crate::members::{AddMember, MemberId, NoticeCode, Role};
use crate::space::Space;
use crate::sync::tests::support::*;
use crate::value::Value;

/// The admin, who creates the space.
const ADMIN: Who = 1;
/// The member the admin adds and removes.
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
async fn a_removed_member_cannot_read_what_is_written_after_its_removal() {
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
    let before = task(&project, "before the removal");
    synced(&admin).await;
    let on_member = now(member.spaces().get(project.id())).unwrap();
    eventually("the member reads the task before its removal", || {
        now(on_member.get("tasks", before)).unwrap().is_some()
    })
    .await;

    let notice = now(project.members().remove(MemberId(invitation.key.id()))).unwrap();

    assert_eq!(notice.code, NoticeCode::MemberRemoved);
    let status = synced(&admin).await;
    assert_eq!(status.refused, 0);
    eventually("the member lists the space without a role", || {
        listed(&member, &project, None)
    })
    .await;
    let after = task(&project, "after the removal");
    let status = synced(&admin).await;
    assert_eq!(status.refused, 0);
    assert!(now(on_member.get("tasks", before)).unwrap().is_some());
    assert!(now(on_member.get("tasks", after)).unwrap().is_none());
}
