//! The editor binding's lease on a space a simulated admin shares with the
//! member (RFC 0013, "An editor: the binding"; NOC-461): a writer's
//! binding is writable, a reader's is not, and a reader's write of an
//! update is refused with `NotAllowed` before any change is made.

use ncdb_core::change::Change;
use ncdb_core::field_op;
use ncdb_core::ids::ObjectId as CoreObjectId;
use ncdb_core::membership::Role as CoreRole;
use ncdb_core::model_lock::ModelLock;

use super::support::{LOCK, Who, now, space};
use crate::db::{Db, OpenOptions, SpaceState};
use crate::error::ErrorCode;
use crate::id::{ObjectId, SpaceId};
use crate::rich_text::{self, RichTextUpdate};

const ADMIN: Who = 0;
const ME: Who = 1;
const TASK: CoreObjectId = CoreObjectId([0x42; 16]);

/// The test lock, with a rich-text `body` on tasks.
fn lock() -> String {
    let title = r#""title": { "rule": 1, "since": 1, "type": "ncdb/string" },"#;
    LOCK.replacen(
        title,
        &format!(r#"{title} "body": {{ "rule": 1, "since": 3, "type": "ncdb/richText" }},"#),
        1,
    )
}

/// `ME`'s database, and the space `ADMIN` shares with it as `role`, with
/// one task the admin created.
fn shared_as(role: CoreRole) -> (Db, SpaceId) {
    let lock = lock();
    let model_id = ModelLock::from_json(&lock).expect("the lock").model.id.0;
    let db = now(Db::open_as(
        OpenOptions::new(&lock).in_memory(),
        [ME; 32],
        [ME ^ 0x80; 32],
        space::account(),
    ))
    .expect("the lock opens");
    let mut space = space::Space::with_model(ADMIN, [7; 16], model_id);
    let genesis = space.genesis.clone();
    let add = space.add(ADMIN, ME, role, &[&genesis]);
    let own = space.envelope(ADMIN, ADMIN, &[&add]);
    let mine = space.envelope(ADMIN, ME, &[&own]);
    let task = space.sealed(ADMIN, vec![field_op::create(TASK, "tasks")]);
    receive(&db, &[&genesis, &add, &own, &mine, &task]);
    (db, SpaceId(space.space))
}

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

#[test]
fn a_writers_binding_is_writable() {
    let (db, space) = shared_as(CoreRole::Writer);
    let opened = rich_text::open(&mut db.lock(), space, "tasks", ObjectId(TASK), "body").unwrap();
    assert!(opened.writable);
}

#[test]
fn a_readers_binding_is_not_writable_and_its_write_is_refused() {
    let (db, space) = shared_as(CoreRole::Reader);
    let opened = rich_text::open(&mut db.lock(), space, "tasks", ObjectId(TASK), "body").unwrap();
    assert!(!opened.writable);
    let lease = opened.binding.lease;
    // A deletion of nothing: it adds no items, so only the role refuses it.
    let update = RichTextUpdate {
        update: vec![0, 0],
        client: lease.client,
        nonce: lease.nonce,
    };
    let handle = now(db.spaces().get(space)).unwrap();
    let refused = now(handle.update("tasks", ObjectId(TASK), [("body", update.value())]));
    assert_eq!(refused.unwrap_err().code(), ErrorCode::NotAllowed);
}
