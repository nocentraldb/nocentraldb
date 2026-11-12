//! A push whose answer never arrives, though the relay appended it (RFC
//! 0007, "Push": a push is idempotent; NOC-276). When the change ended the
//! member's membership, as its leave does, the relay answers the next
//! connection with `not_member`, and the member can no longer pull. It
//! still pushes the space's unconfirmed changes once: the relay confirms
//! those it holds as duplicates and refuses the rest, so no change waits
//! for a confirmation that can never come.

use ncdb_core::change::Change;

use super::support::*;
use crate::Db;
use crate::members::{AddMember, Role};
use crate::space::Space;

const ADMIN: Who = 1;
const LEAVER: Who = 3;

/// The unconfirmed changes of `space` in `db`.
fn unconfirmed(db: &Db, space: &Space) -> Vec<Change> {
    let inner = crate::db::lock(&db.inner);
    inner.view.unconfirmed(space.id().0).expect("the view")
}

/// A space of the admin that the leaver joined, both synced.
async fn joined_space(relay: &LocalRelay, admin: &Db, leaver: &Db) -> Space {
    synced(leaver).await;
    let project = now(admin.spaces().create()).unwrap();
    let options = AddMember {
        role: Role::Writer,
        fingerprint: None,
    };
    now(project.members().add(&now(leaver.invitation()), options)).unwrap();
    synced(admin).await;
    let mut handle = Err(());
    eventually("the leaver joined the space", || {
        handle = now(leaver.spaces().get(project.id())).map_err(|_| ());
        handle.is_ok()
    })
    .await;
    let _ = relay;
    handle.unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_leave_the_relay_holds_is_confirmed_though_its_answer_was_lost() {
    let relay = LocalRelay::start().await;
    let admin = open(ADMIN, &relay.url()).await;
    let leaver = open(LEAVER, &relay.url()).await;
    let on_leaver = joined_space(&relay, &admin, &leaver).await;
    synced(&leaver).await;

    // The leave reaches the relay, but its answer never reaches the
    // leaver's database: it was paused when it left, and the leave went
    // up on another connection of the same member.
    leaver.sync().pause().await;
    now(on_leaver.leave()).unwrap();
    let leave = unconfirmed(&leaver, &on_leaver);
    assert_eq!(leave.len(), 1);
    let mut other = relay.client(LEAVER).await;
    push(&mut other, &[&leave[0]]).await;

    leaver.sync().resume().await;
    eventually("the relay told the leaver it is no member", || {
        leaver.sync().current().error.as_deref() == Some("not_member")
    })
    .await;
    eventually("the leave is confirmed", || {
        leaver.sync().current().unconfirmed == 0
    })
    .await;
    let status = leaver.sync().current();
    assert_eq!(status.refused, 0, "the relay holds the leave: no refusal");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_change_written_after_the_relay_holds_the_leave_is_refused_not_left_waiting() {
    let relay = LocalRelay::start().await;
    let admin = open(ADMIN, &relay.url()).await;
    let leaver = open(LEAVER, &relay.url()).await;
    let on_leaver = joined_space(&relay, &admin, &leaver).await;
    synced(&leaver).await;

    // The same leave, appended through another connection, while this
    // database is paused and still writes in the space.
    leaver.sync().pause().await;
    let mut other = relay.client(LEAVER).await;
    let before = unconfirmed(&leaver, &on_leaver).len();
    assert_eq!(before, 0);
    now(on_leaver.write(|tx| {
        tx.create("tasks", [("title", crate::value::Value::from("late"))])?;
        Ok(())
    }))
    .unwrap();
    now(on_leaver.leave()).unwrap();
    let pending = unconfirmed(&leaver, &on_leaver);
    assert_eq!(pending.len(), 2);
    // Only the leave reaches the relay; the write before it does not.
    push(&mut other, &[&pending[1]]).await;

    leaver.sync().resume().await;
    eventually("nothing waits for a confirmation", || {
        leaver.sync().current().unconfirmed == 0
    })
    .await;
    let status = leaver.sync().current();
    assert_eq!(
        status.refused, 1,
        "the write the relay never held is refused"
    );
    assert_eq!(status.error.as_deref(), Some("not_member"));
}
