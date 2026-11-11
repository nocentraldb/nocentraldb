//! Adding a former member again (RFC 0008, "A member demoted and raised
//! again" and M5; NOC-154 point 2): the admin rotates to a fresh
//! generation first, with an envelope for every member, then adds the
//! member from that generation, and the relay accepts every change.

use crate::db::Db;
use crate::id::ObjectId;
use crate::list::{SpaceInfo, SpaceModel};
use crate::members::tests::remove;
use crate::members::{AddMember, Invitation, NoticeCode, Role};
use crate::space::Space;
use crate::sync::tests::support::*;
use crate::value::Value;

/// The admin, who creates the space.
const ADMIN: Who = 1;
/// The member the admin adds, removes and adds again.
const MEMBER: Who = 2;

fn writer() -> AddMember {
    AddMember {
        role: Role::Writer,
        fingerprint: None,
    }
}

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

/// The admin's space, shared with the member as a writer and then
/// removed from it, without a rotation; both databases synced.
async fn removed(relay: &LocalRelay) -> (Db, Db, Space, Invitation) {
    let admin = open(ADMIN, &relay.url()).await;
    let member = open(MEMBER, &relay.url()).await;
    let project = now(admin.spaces().create()).unwrap();
    let invitation = now(member.invitation());
    now(project.members().add(&invitation, writer())).unwrap();
    synced(&admin).await;
    let space = project.id().0;
    eventually("the member's subscription caught up", || {
        now(member.spaces().get(project.id())).is_ok()
            && crate::db::lock(&member.inner).sync_live(space)
    })
    .await;
    remove(&admin, &project, &invitation);
    synced(&admin).await;
    eventually("the member lists the space without a role", || {
        listed(&member, &project, None)
    })
    .await;
    (admin, member, project, invitation)
}

#[tokio::test(flavor = "multi_thread")]
async fn a_former_member_is_added_again_from_a_fresh_generation() {
    let relay = LocalRelay::start().await;
    let (admin, member, project, invitation) = removed(&relay).await;
    let before = crate::db::lock(&admin.inner).spaces[&project.id().0]
        .checks
        .resolved()
        .current_generation()
        .unwrap();

    let notice = now(project.members().add(&invitation, writer())).unwrap();

    assert_eq!(notice.code, NoticeCode::MemberAdded);
    let status = synced(&admin).await;
    assert_eq!(status.refused, 0);
    {
        let inner = crate::db::lock(&admin.inner);
        let resolved = inner.spaces[&project.id().0].checks.resolved();
        let after = resolved.current_generation().unwrap();
        assert!(resolved.descends(after, before) && after != before);
        let subject = resolved.member(invitation.key.id()).unwrap();
        assert_eq!(subject.role, Some(Role::Writer.into()));
        assert_eq!(
            subject
                .grants
                .last()
                .and_then(|grant| grant.from_generation),
            Some(after)
        );
    }

    eventually("the member lists the space as a writer again", || {
        listed(&member, &project, Some(Role::Writer))
    })
    .await;
    let from_admin = task(&project, "after the re-add");
    synced(&admin).await;
    let on_member = now(member.spaces().get(project.id())).unwrap();
    eventually("the member reads the admin's new task", || {
        now(on_member.get("tasks", from_admin)).unwrap().is_some()
    })
    .await;
    let from_member = task(&on_member, "from the member, again");
    let status = synced(&member).await;
    assert_eq!(status.refused, 0);
    eventually("the admin reads the member's new task", || {
        now(project.get("tasks", from_member)).unwrap().is_some()
    })
    .await;
}
