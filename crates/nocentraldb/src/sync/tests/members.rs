//! Adding a member through the SDK (RFC 0009, "Invitations and members";
//! RFC 0003, "Sharing a space key with a new member"): a second database,
//! of an account of its own, hands out its invitation; an admin adds it
//! with the account's fingerprint; after sync, the new member writes into
//! the space through `spaces().get`.

use super::support::*;
use crate::db::{Db, OpenOptions};
use crate::members::{AddMember, Role};
use crate::value::Value;

#[tokio::test(flavor = "multi_thread")]
async fn a_member_of_another_account_added_with_its_fingerprint_writes_after_sync() {
    let relay = LocalRelay::start().await;
    let admin = open(1, &relay.url()).await;
    let joining = Db::open(OpenOptions::new(LOCK).in_memory().relay(&relay.url()))
        .await
        .expect("the test lock opens");

    let project = now(admin.spaces().create()).unwrap();
    let invitation = now(joining.invitation());
    let options = AddMember {
        role: Role::Writer,
        fingerprint: Some(invitation.fingerprint()),
    };
    now(project.members().add(&invitation, options)).unwrap();
    synced(&admin).await;

    // The new member stays connected: the relay tells it that it was
    // added (RFC 0007, "Subscribe").
    let mut joined = Err(());
    eventually("the new member holds the space", || {
        joined = now(joining.spaces().get(project.id())).map_err(|_| ());
        joined.is_ok()
    })
    .await;
    let written = now(joined.unwrap().write(|tx| {
        tx.create("tasks", [("title", Value::from("from the new member"))])?;
        Ok(())
    }))
    .unwrap();
    let status = synced(&joining).await;
    assert_eq!(status.refused, 0);

    eventually("the admin reads the new member's task", || {
        now(project.get("tasks", written.ids[0])).unwrap().is_some()
    })
    .await;
}
