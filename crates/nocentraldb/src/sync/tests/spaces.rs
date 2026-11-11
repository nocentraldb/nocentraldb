//! A space the member was added to, through `db.spaces.get` (RFC 0009,
//! "Spaces"; RFC 0006, decision 1): once its genesis and its key arrive,
//! the member writes into it like into a space it created, after the
//! heads of its log (RFC 0002, "Dependencies"). A space of another model is
//! `spaceOfAnotherModel`, and one whose key has not arrived `notFound`
//! (RFC 0009, "Errors"). A space of another model is never folded: its
//! sealed changes are parked as another model, so nothing of it reaches
//! the view (RFC 0009, "Spaces of another model"; NOC-57).

use std::collections::BTreeSet;

use ncdb_core::change::Change;
use ncdb_core::field_op;
use ncdb_core::ids::ObjectId as CoreObjectId;
use ncdb_core::log::{ParkReason as CoreParkReason, State};
use ncdb_core::membership::Role;
use ncdb_core::model_lock::ModelLock;
use ncdb_core::seal;

use super::support::*;
use crate::error::ErrorCode;
use crate::id::{ObjectId, SpaceId};
use crate::value::Value;

/// The simulated admin who creates the space.
const ADMIN: Who = 0;
/// The member whose database syncs.
const ME: Who = 1;

const TASK: CoreObjectId = CoreObjectId([0x11; 16]);

/// The `model.id` of the test lock.
fn model_id() -> [u8; 16] {
    ModelLock::from_json(LOCK)
        .expect("the test lock")
        .model
        .id
        .0
}

/// A space of `ADMIN`'s on the relay whose genesis names `model_id`, with
/// `ME` as a writer, and one task. `ME` gets the space key when `key`
/// holds. Returns the space, `ME`'s envelope or the admin's own, and the
/// task.
async fn shared(
    relay: &LocalRelay,
    model_id: [u8; 16],
    key: bool,
) -> (space::Space, Change, Change) {
    let mut space = space::Space::with_model(ADMIN, [7; 16], model_id);
    let genesis = space.genesis.clone();
    let add = space.add(ADMIN, ME, Role::Writer, &[&genesis]);
    let own = space.envelope(ADMIN, ADMIN, &[&add]);
    let mut changes = vec![genesis, add, own.clone()];
    let last = match key {
        true => {
            let envelope = space.envelope(ADMIN, ME, &[&own]);
            changes.push(envelope.clone());
            envelope
        }
        false => own,
    };
    let task = space.sealed(ADMIN, vec![field_op::create(TASK, "tasks")]);
    changes.push(task.clone());
    let mut client = relay.client(ADMIN).await;
    push(&mut client, &changes.iter().collect::<Vec<_>>()).await;
    (space, last, task)
}

/// Whether the database folded `change` of `space`, which the relay
/// confirms on the pull.
fn pulled(db: &crate::Db, space: &space::Space, change: &Change) -> bool {
    let inner = crate::db::lock(&db.inner);
    inner
        .view
        .relay_position(space.space, change.id())
        .is_some()
}

/// `ME`'s change `id` of `space` as the relay holds it, opened with the
/// space key: its author sequence and its dependencies.
async fn opened(
    relay: &LocalRelay,
    space: &space::Space,
    id: crate::id::ChangeId,
) -> (u64, BTreeSet<ncdb_core::ids::ChangeId>) {
    let change = relay.change(space.space, id.0).await;
    let payload = seal::open(&change, &space::key(ME).public(), &space::space_key())
        .expect("the change opens under the space key");
    (change.header().sequence, payload.dependencies)
}

#[tokio::test(flavor = "multi_thread")]
async fn a_member_writes_through_get_into_a_space_it_was_added_to() {
    let relay = LocalRelay::start().await;
    let (space, envelope, task) = shared(&relay, model_id(), true).await;
    let db = open(ME, &relay.url()).await;
    eventually("the task", || pulled(&db, &space, &task)).await;

    let project = now(db.spaces().get(SpaceId(space.space))).unwrap();
    let first = now(project.write(|tx| {
        tx.create("tasks", [("title", Value::from("mine"))])?;
        Ok(())
    }))
    .unwrap();
    let second = now(project.write(|tx| {
        tx.update(
            "tasks",
            first.ids[0],
            [("title", Value::from("still mine"))],
        )?;
        Ok(())
    }))
    .unwrap();

    let status = synced(&db).await;
    assert_eq!(status.refused, 0);
    let log = relay.log(space.space).await;
    assert!(log.contains(&first.change.0) && log.contains(&second.change.0));
    let (sequence, deps) = opened(&relay, &space, first.change).await;
    assert_eq!(sequence, 1);
    assert_eq!(deps, BTreeSet::from([envelope.id(), task.id()]));
    let (sequence, deps) = opened(&relay, &space, second.change).await;
    assert_eq!(sequence, 2);
    assert_eq!(deps, BTreeSet::from([first.change.0]));
    let mine = now(project.get("tasks", first.ids[0])).unwrap().unwrap();
    assert_eq!(mine.get::<&str>("title").unwrap(), Some("still mine"));
    assert!(now(project.get("tasks", ObjectId(TASK))).unwrap().is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_space_of_another_model_is_refused_by_get_with_both_model_ids() {
    let relay = LocalRelay::start().await;
    let (space, _, task) = shared(&relay, [1; 16], true).await;
    let db = open(ME, &relay.url()).await;
    eventually("the task", || pulled(&db, &space, &task)).await;

    let error = now(db.spaces().get(SpaceId(space.space))).unwrap_err();

    assert_eq!(error.code(), ErrorCode::SpaceOfAnotherModel);
    let message = error.to_string();
    assert!(message.contains(&"01".repeat(16)), "{message}");
    assert!(
        message.contains("f959ef26820d478a108e9261abf83a69"),
        "{message}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_space_whose_key_has_not_arrived_is_not_found() {
    let relay = LocalRelay::start().await;
    let (space, own, _) = shared(&relay, model_id(), false).await;
    let db = open(ME, &relay.url()).await;
    eventually("the admin's envelope", || pulled(&db, &space, &own)).await;

    let error = now(db.spaces().get(SpaceId(space.space))).unwrap_err();

    assert_eq!(error.code(), ErrorCode::NotFound);
}

/// The state of `change` of `space` in the database's log.
fn state(db: &crate::Db, space: &space::Space, change: &Change) -> Option<State> {
    crate::db::lock(&db.inner)
        .view
        .state(space.space, change.id())
}

/// Whether the database's view holds the task `TASK` of `space`.
fn holds_task(db: &crate::Db, space: &space::Space) -> bool {
    let inner = crate::db::lock(&db.inner);
    inner
        .view
        .get(space.space, "tasks", TASK)
        .expect("the view reads")
        .is_some()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_space_of_another_model_is_not_folded() {
    let relay = LocalRelay::start().await;
    let (space, envelope, task) = shared(&relay, [1; 16], true).await;
    let db = open(ME, &relay.url()).await;
    eventually("the task", || pulled(&db, &space, &task)).await;

    // The membership folds, so the copy can name the space's model; the
    // content does not.
    assert!(matches!(
        state(&db, &space, &envelope),
        Some(State::Folded { .. })
    ));
    assert_eq!(
        state(&db, &space, &task),
        Some(State::Parked(CoreParkReason::AnotherModel))
    );
    assert!(!holds_task(&db, &space));
    let counts = crate::db::lock(&db.inner).view.state_counts(space.space);
    assert_eq!(counts.parked.get(&CoreParkReason::AnotherModel), Some(&1));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_space_of_the_locks_model_is_folded() {
    let relay = LocalRelay::start().await;
    let (space, _, task) = shared(&relay, model_id(), true).await;
    let db = open(ME, &relay.url()).await;
    eventually("the task", || pulled(&db, &space, &task)).await;

    assert!(matches!(
        state(&db, &space, &task),
        Some(State::Folded { .. })
    ));
    assert!(holds_task(&db, &space));
}
