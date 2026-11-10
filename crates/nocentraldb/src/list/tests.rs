//! `spaces().list()` on spaces a simulated admin shares with the member:
//! the admin's control changes are folded into the member's copy as a
//! pull folds them, without a relay, since the relay serves no change to a
//! member it removed (RFC 0007, "The protocol").

#[path = "../../../ncdb-core/tests/support/space.rs"]
mod space;

use ncdb_core::change::Change;
use ncdb_core::membership::Role as CoreRole;
use ncdb_core::model_lock::ModelLock;

use super::{SpaceInfo, SpaceModel};
use crate::db::{Db, OpenOptions, SpaceState};
use crate::id::SpaceId;
use crate::members::Role;
use space::Who;

const LOCK: &str = include_str!("../../tests/model/model.lock.json");

/// The simulated admin who creates the space.
const ADMIN: Who = 0;
/// The member whose database lists its spaces.
const ME: Who = 1;

/// The output of an SDK future, which is ready at its first poll.
fn now<F: std::future::Future>(future: F) -> F::Output {
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    match std::pin::pin!(future).poll(&mut cx) {
        std::task::Poll::Ready(output) => output,
        std::task::Poll::Pending => panic!("an SDK call waited"),
    }
}

/// The `model.id` of the test lock.
fn model_id() -> [u8; 16] {
    ModelLock::from_json(LOCK)
        .expect("the test lock")
        .model
        .id
        .0
}

/// The database of `ME`, of the simulated account, without a relay.
fn database() -> Db {
    let options = OpenOptions::new(LOCK).in_memory();
    now(Db::open_as(options, [ME; 32], [ME ^ 0x80; 32], space::account()))
        .expect("the test lock opens")
}

/// Folds `changes` into the database's copy, as a pull folds them.
fn receive(db: &Db, changes: &[&Change]) {
    let mut inner = db.lock();
    let inner = &mut *inner;
    for change in changes {
        let space = change.header().space;
        let state = inner
            .spaces
            .entry(space)
            .or_insert_with(|| SpaceState::new(space, inner.member.key()));
        inner
            .view
            .receive(change.bytes(), &mut state.checks)
            .expect("the change folds");
    }
}

/// A space of `ADMIN`'s whose genesis names `model_id`, with `ME` added as
/// `role`, and `ME`'s key envelope when `key` holds. Returns the space and
/// its changes, in order.
fn shared(model_id: [u8; 16], role: CoreRole, key: bool) -> (space::Space, Vec<Change>) {
    let mut space = space::Space::with_model(ADMIN, [7; 16], model_id);
    let genesis = space.genesis.clone();
    let add = space.add(ADMIN, ME, role, &[&genesis]);
    let own = space.envelope(ADMIN, ADMIN, &[&add]);
    let mut changes = vec![genesis, add, own];
    if key {
        let last = changes.last().expect("the admin's envelope").clone();
        changes.push(space.envelope(ADMIN, ME, &[&last]));
    }
    (space, changes)
}

/// The changes, as references.
fn all(changes: &[Change]) -> Vec<&Change> {
    changes.iter().collect()
}

#[test]
fn a_space_the_member_was_added_to_is_listed_with_its_role() {
    let db = database();
    let (space, changes) = shared(model_id(), CoreRole::Writer, true);
    receive(&db, &all(&changes));

    let listed = now(db.spaces().list()).unwrap();

    assert_eq!(
        listed,
        vec![SpaceInfo {
            id: SpaceId(space.space),
            role: Some(Role::Writer),
            model: SpaceModel::Ours,
        }]
    );
}

#[test]
fn a_space_the_member_was_removed_from_is_listed_without_a_role() {
    let db = database();
    let (mut space, changes) = shared(model_id(), CoreRole::Reader, true);
    let last = changes.last().expect("the member's envelope");
    let removed = space.remove(ADMIN, ME, 0, &[last]);
    receive(&db, &all(&changes));
    receive(&db, &[&removed]);

    let listed = now(db.spaces().list()).unwrap();

    assert_eq!(
        listed,
        vec![SpaceInfo {
            id: SpaceId(space.space),
            role: None,
            model: SpaceModel::Ours,
        }]
    );
}
