//! Pushing again a change the relay refused (RFC 0008, "How the relay
//! applies the rules"; RFC 0002, "A
//! change the relay refuses after the member folded it"). The relay
//! refuses `ME`'s write because of a revocation, and `ME`'s own fold
//! rejects it too. A concurrent removal of the revoking admin, by a senior
//! admin, makes that revocation ineffective, so `ME`'s fold newly accepts
//! the write: `ME` pushes it to the relay once more, and the relay accepts
//! it. A write the relay refuses again is not pushed until `ME`'s fold
//! newly accepts it again, so nothing loops.
//!
//! The admins are simulated members whose changes a raw client pushes.

use ncdb_core::change::Change;
use ncdb_core::log::State;
use ncdb_core::membership::Role;

use super::reopen::open_at;
use super::rotation::model_id;
use super::support::*;
use crate::id::{ChangeId, SpaceId};
use crate::value::Value;

/// The senior admin, who creates the space.
const X: Who = 0;
/// The member whose database syncs, a writer.
const ME: Who = 1;
/// A second admin, added by `X`.
const Y: Who = 3;

/// The space on the relay, with `Y` an admin and `ME` a writer.
struct Scene {
    space: space::Space,
    /// The envelope to `ME`, the last change every admin saw.
    last: Change,
    client: Client,
}

impl Scene {
    async fn push(relay: &LocalRelay) -> Scene {
        let mut space = space::Space::with_model(X, [7; 16], model_id());
        let genesis = space.genesis.clone();
        let own = space.envelope(X, X, &[&genesis]);
        let add_y = space.add(X, Y, Role::Admin, &[&own]);
        let add_me = space.add(X, ME, Role::Writer, &[&add_y]);
        let last = space.envelope(X, ME, &[&add_me]);
        let mut client = relay.client(X).await;
        push(&mut client, &[&genesis, &own, &add_y, &add_me, &last]).await;
        Scene {
            space,
            last,
            client,
        }
    }

    /// `Y` lowers `ME` to a reader, holding none of `ME`'s changes.
    async fn y_demotes_me(&mut self) {
        let last = self.last.clone();
        let demote = self.space.set_role(Y, ME, Role::Reader, 0, &[&last]);
        push(&mut self.client, &[&demote]).await;
    }

    /// `X`, not having seen `Y`'s demotion of `ME`, removes `Y` below it:
    /// the demotion no longer counts, on the relay and on every member.
    fn x_removes_y(&mut self) -> Change {
        let last = self.last.clone();
        self.space.remove(X, Y, 0, &[&last])
    }

    /// `X` lowers `ME` to a reader after removing `Y`, holding none of
    /// `ME`'s changes.
    fn x_demotes_me(&mut self, after: &Change) -> Change {
        self.space.set_role(X, ME, Role::Reader, 0, &[after])
    }
}

/// `ME`'s write into the scene's space while the relay is cut off, once
/// `ME` holds its envelope.
async fn my_write(relay: &LocalRelay, db: &crate::Db, scene: &Scene) -> ChangeId {
    let id = scene.space.space;
    eventually("the envelope to me", || {
        crate::db::lock(&db.inner)
            .view
            .relay_position(id, scene.last.id())
            .is_some()
    })
    .await;
    synced(db).await;
    relay.proxy.cut();
    let project = now(db.spaces().get(SpaceId(id))).unwrap();
    let written = now(project.write(|tx| {
        tx.create("tasks", [("title", Value::from("written while offline"))])?;
        Ok(())
    }))
    .unwrap();
    written.change
}

/// The state of `ME`'s write in its fold.
fn state(db: &crate::Db, scene: &Scene, written: ChangeId) -> Option<State> {
    crate::db::lock(&db.inner)
        .view
        .state(scene.space.space, written.0)
}

fn rejected(db: &crate::Db, scene: &Scene, written: ChangeId) -> bool {
    matches!(state(db, scene, written), Some(State::Rejected(_)))
}

/// Whether the relay's log holds `ME`'s write.
async fn on_relay(relay: &LocalRelay, scene: &Scene, written: ChangeId) -> bool {
    relay.log(scene.space.space).await.contains(&written.0)
}

/// Up to `ME`'s write being refused by the relay and rejected by `ME`'s
/// own fold: `ME` writes while offline, `Y` demotes it meanwhile, and `ME`
/// reconnects, pulls the demotion and pushes the write.
async fn refused_and_rejected(relay: &LocalRelay, db: &crate::Db, scene: &mut Scene) -> ChangeId {
    let written = my_write(relay, db, scene).await;
    scene.y_demotes_me().await;
    relay.proxy.restore();
    eventually("refused", || db.sync().current().refused == 1).await;
    assert!(rejected(db, scene, written), "my fold accepts the write");
    assert!(!on_relay(relay, scene, written).await);
    written
}

#[tokio::test(flavor = "multi_thread")]
async fn a_refused_write_the_fold_newly_accepts_is_pushed_again() {
    let relay = LocalRelay::start().await;
    let mut scene = Scene::push(&relay).await;
    let db = open(ME, &relay.url()).await;
    let written = refused_and_rejected(&relay, &db, &mut scene).await;

    let removal = scene.x_removes_y();
    push(&mut scene.client, &[&removal]).await;

    eventually("pushed again", || now(on_relay(&relay, &scene, written))).await;
    let status = synced(&db).await;
    assert_eq!(status.refused, 0, "accepted on the second push");
    let folded = state(&db, &scene, written);
    assert!(matches!(folded, Some(State::Folded { .. })), "{folded:?}");
    assert!(db.sync().is_confirmed(written));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_write_refused_again_is_not_pushed_until_the_fold_accepts_it_again() {
    let relay = LocalRelay::start().await;
    let mut scene = Scene::push(&relay).await;
    let db = open(ME, &relay.url()).await;
    let written = refused_and_rejected(&relay, &db, &mut scene).await;
    relay.proxy.cut();
    eventually("offline", || {
        db.sync().current().connection == crate::sync::Connection::Offline
    })
    .await;

    // Folded again by X's removal of Y, rejected again by X's own
    // demotion: pushed once more, refused once more.
    let removal = scene.x_removes_y();
    let demote = scene.x_demotes_me(&removal);
    push(&mut scene.client, &[&removal, &demote]).await;
    relay.proxy.restore();

    let mut status = db.sync().current();
    eventually("refused again", || {
        status = db.sync().current();
        status.connection == crate::sync::Connection::Connected
            && status.unconfirmed == 0
            && status.refused == 1
    })
    .await;
    assert!(rejected(&db, &scene, written));
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    assert_eq!(db.sync().current(), status, "pushed again without cause");
    assert!(!on_relay(&relay, &scene, written).await);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_reopened_database_pushes_again_what_its_fold_newly_accepts() {
    let relay = LocalRelay::start().await;
    let mut scene = Scene::push(&relay).await;
    let dir = TempDir::new();
    let db = open_at(&dir, &relay.url()).await;
    let written = refused_and_rejected(&relay, &db, &mut scene).await;
    drop(db);

    let removal = scene.x_removes_y();
    push(&mut scene.client, &[&removal]).await;
    let db = open_at(&dir, &relay.url()).await;

    eventually("pushed again", || now(on_relay(&relay, &scene, written))).await;
    assert_eq!(synced(&db).await.refused, 0);
}
