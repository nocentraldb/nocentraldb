//! Large changes (RFC 0007, "Large changes"): a change whose frame does not
//! fit into one message goes up through an upload URL and comes down
//! through a download URL, which the local relay serves (NOC-102). The
//! other member folds it as it folds any change, live and on a pull.

use super::support::*;
use crate::members::{AddMember, Role};
use crate::value::Value;

/// A title of 300 KiB: its change does not fit into a 23 KiB message.
fn large_title() -> String {
    "x".repeat(300 * 1024)
}

/// The largest message, as base64 in one text frame, stays under API
/// Gateway's 32 KB frame however AWS counts a KB; a change of 40 KiB, which
/// fitted into a message before NOC-338, now goes through an upload.
#[test]
fn a_message_fits_one_32_kb_frame_and_a_40_kib_change_goes_up_through_an_upload() {
    use crate::sync::wire::{MAX_MESSAGE, MAX_TEXT, fits_a_push, to_text};
    assert_eq!(MAX_MESSAGE, 23 * 1024);
    assert_eq!(to_text(&vec![0xff; MAX_MESSAGE]).len(), MAX_TEXT);
    assert_eq!(MAX_TEXT, 31_404);
    const { assert!(MAX_TEXT <= 32_000) };
    assert!(!fits_a_push(&vec![0; 40 * 1024]));
    assert!(fits_a_push(&vec![0; 20 * 1024]));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_change_of_40_kib_goes_up_and_reaches_a_subscribed_member() {
    let relay = LocalRelay::start().await;
    let (admin, member, project) = admin_and_member(&relay).await;
    eventually("the member holds the space", || {
        now(member.spaces().get(project.id())).is_ok()
    })
    .await;
    let middle = "x".repeat(40 * 1024);
    let task = now(project.create("tasks", [("title", Value::from(middle.as_str()))]))
        .unwrap()
        .ids[0];
    let status = synced(&admin).await;
    assert_eq!(status.refused, 0, "the relay refused the change");
    eventually("the member holds the 40 KiB task", || {
        title(&member, project.id(), task).as_deref() == Some(middle.as_str())
    })
    .await;
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
    synced(&member).await;
    let writer = AddMember {
        role: Role::Writer,
        fingerprint: None,
    };
    now(project.members().add(&now(member.invitation()), writer)).unwrap();
    synced(&admin).await;
    eventually("the member holds the large task", || {
        title(&member, project.id(), task).as_deref() == Some(big.as_str())
    })
    .await;
}
