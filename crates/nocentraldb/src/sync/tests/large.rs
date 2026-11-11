//! Large changes (RFC 0007, "Large changes"): a change whose frame does not
//! fit into one message goes up through an upload URL and comes down
//! through a download URL, which the local relay serves (NOC-102). The
//! other member folds it as it folds any change, live and on a pull.

use super::support::*;
use crate::members::{AddMember, Role};
use crate::value::Value;

/// A title of 300 KiB: its change does not fit into a 128 KiB message.
fn large_title() -> String {
    "x".repeat(300 * 1024)
}

async fn admin_and_member(relay: &LocalRelay) -> (crate::Db, crate::Db, crate::Space) {
    let admin = open(1, &relay.url()).await;
    let member = open(2, &relay.url()).await;
    synced(&member).await;
    let project = now(admin.spaces().create()).unwrap();
    let writer = AddMember {
        role: Role::Writer,
        fingerprint: None,
    };
    now(project.members().add(&now(member.invitation()), writer)).unwrap();
    synced(&admin).await;
    (admin, member, project)
}

/// The title of `task` as `db` holds it, once it holds the task.
fn title(db: &crate::Db, space: crate::SpaceId, task: crate::ObjectId) -> Option<String> {
    let space = now(db.spaces().get(space)).ok()?;
    let task = now(space.get("tasks", task)).ok()??;
    task.get::<&str>("title").ok()?.map(str::to_owned)
}

#[tokio::test(flavor = "multi_thread")]
async fn a_large_change_goes_up_and_reaches_a_subscribed_member() {
    let relay = LocalRelay::start().await;
    let (admin, member, project) = admin_and_member(&relay).await;
    eventually("the member holds the space", || {
        now(member.spaces().get(project.id())).is_ok()
    })
    .await;
    let big = large_title();
    let task = now(project.write(|tx| {
        tx.create("tasks", [("title", Value::from(big.as_str()))])?;
        Ok(())
    }))
    .unwrap()
    .ids[0];
    let status = synced(&admin).await;
    assert_eq!(status.refused, 0, "the relay refused the large change");
    eventually("the member holds the large task", || {
        title(&member, project.id(), task).as_deref() == Some(big.as_str())
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_member_that_joins_later_pulls_a_large_change() {
    let relay = LocalRelay::start().await;
    let admin = open(1, &relay.url()).await;
    let project = now(admin.spaces().create()).unwrap();
    let big = large_title();
    let task = now(project.write(|tx| {
        tx.create("tasks", [("title", Value::from(big.as_str()))])?;
        Ok(())
    }))
    .unwrap()
    .ids[0];
    let status = synced(&admin).await;
    assert_eq!(status.refused, 0, "the relay refused the large change");
    let member = open(2, &relay.url()).await;
    let writer = AddMember {
        role: Role::Writer,
        fingerprint: None,
    };
    now(project.members().add(&now(member.invitation()), writer)).unwrap();
    eventually("the member holds the large task", || {
        title(&member, project.id(), task).as_deref() == Some(big.as_str())
    })
    .await;
}
