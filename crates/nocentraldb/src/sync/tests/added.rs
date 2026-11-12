//! A member added while it is connected (RFC 0007, "Subscribe"): the relay
//! sends `added`, and the member pulls, pushes and subscribes to the space as
//! it does for a space named in `welcome`, without connecting again.

use super::support::*;
use crate::members::{AddMember, Role};
use crate::value::Value;

#[tokio::test(flavor = "multi_thread")]
async fn a_member_added_while_connected_syncs_the_space_at_once() {
    let relay = LocalRelay::start().await;
    let admin = open(1, &relay.url()).await;
    let member = open(2, &relay.url()).await;
    synced(&member).await;

    let project = now(admin.spaces().create()).unwrap();
    let task = now(project.write(|tx| {
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
    now(project.members().add(&invitation, writer)).unwrap();
    synced(&admin).await;

    let mut joined = Err(());
    eventually("the member holds the admin's task", || {
        joined = now(member.spaces().get(project.id())).map_err(|_| ());
        joined
            .as_ref()
            .is_ok_and(|space| now(space.get("tasks", task)).unwrap().is_some())
    })
    .await;

    // The member follows the space live: the admin's next write arrives
    // without a reconnect.
    let next = now(project.write(|tx| {
        tx.create("tasks", [("title", Value::from("live"))])?;
        Ok(())
    }))
    .unwrap()
    .ids[0];
    let joined = joined.unwrap();
    eventually("the member holds the admin's next task", || {
        now(joined.get("tasks", next)).unwrap().is_some()
    })
    .await;
}
