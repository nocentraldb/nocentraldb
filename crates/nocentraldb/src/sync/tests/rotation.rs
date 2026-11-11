//! A member added after one or more key rotations holds an envelope only
//! for the current generation. It reaches the older generations through
//! the key chain entries (RFC 0003, "Key-chain subkey"; RFC 0008, "Key
//! chain entry"): `spaces().get` gives it a handle, and it reads the
//! changes sealed before its join, back to the first generation, whose key
//! opens the genesis's sealed `model.id` (RFC 0008, "What it names").
//!
//! The SDK does not rotate keys yet, so the admin's rotations are written
//! through the core here, with a real key per generation.

use std::collections::BTreeSet;

use ncdb_core::change::{Change, Payload};
use ncdb_core::clock::Hlc;
use ncdb_core::envelope;
use ncdb_core::field_op;
use ncdb_core::ids::ObjectId as CoreObjectId;
use ncdb_core::keys::SpaceKey;
use ncdb_core::membership::{self, Entry, KeyChainEntry, PreviousKey, Role};
use ncdb_core::model_lock::ModelLock;
use ncdb_core::seal;

use super::support::*;
use crate::id::{ObjectId, SpaceId};
use crate::list::SpaceModel;
use crate::value::Value;

/// The simulated admin who creates the space and rotates its key.
const ADMIN: Who = 0;
/// The member whose database syncs, added after the rotations.
const ME: Who = 1;

/// The `model.id` of the test lock.
pub(super) fn model_id() -> [u8; 16] {
    ModelLock::from_json(LOCK)
        .expect("the test lock")
        .model
        .id
        .0
}

/// The task the admin creates under the generation at `index`.
fn task(index: usize) -> CoreObjectId {
    CoreObjectId([0x11 + byte(index); 16])
}

/// A generation's index as a byte; the tests rotate a few times only.
fn byte(index: usize) -> u8 {
    u8::try_from(index).expect("a few generations")
}

/// The admin's space on the relay after its rotations.
struct Rotated {
    space: space::Space,
    /// The generations, the first one first.
    generations: Vec<u64>,
    /// The envelope to `ME`, the admin's last change.
    last: Change,
}

/// The key of the generation at `index`: the first is the simulated
/// spaces' own key, which seals the genesis's `model.id`.
fn key(index: usize) -> SpaceKey {
    match index {
        0 => space::space_key(),
        n => SpaceKey::from_bytes([0x40 + byte(n); 32]),
    }
}

impl Rotated {
    /// Pushes a space with `rotations` rotations after the first
    /// generation, each with a key of its own. Under every generation the
    /// admin creates one task. Then it adds `ME` as a writer, from the
    /// current generation, and wraps only that generation's key to it.
    async fn push(relay: &LocalRelay, rotations: usize) -> Rotated {
        let mut space = space::Space::with_model(ADMIN, [9; 16], model_id());
        let genesis = space.genesis.clone();
        let own = space.envelope(ADMIN, ADMIN, &[&genesis]);
        let mut generations = vec![space.first_generation];
        let mut changes = vec![genesis, own.clone()];
        let first = space.first_generation;
        changes.push(sealed(&mut space, 0, first));
        let mut last = own;
        for index in 1..=rotations {
            let previous = generations[index - 1];
            let generation = membership::generation_id(space.space, space::id(ADMIN), &[previous]);
            let chain = KeyChainEntry {
                generation,
                previous: vec![PreviousKey {
                    generation: previous,
                    key: membership::seal_previous_key(
                        &key(index),
                        space.space,
                        generation,
                        previous,
                        &key(index - 1),
                    ),
                }],
                answers: BTreeSet::new(),
            };
            let rotation = space.write(ADMIN, Entry::KeyChain(chain), &[&last]);
            let own = wrap(&mut space, ADMIN, index, generation, &rotation);
            changes.extend([rotation, own.clone()]);
            changes.push(sealed(&mut space, index, generation));
            generations.push(generation);
            last = own;
        }
        let current = *generations.last().expect("the first generation");
        let add = space.add_at(ADMIN, ME, Role::Writer, current, &[&last]);
        let to_me = wrap(&mut space, ME, rotations, current, &add);
        changes.extend([add, to_me.clone()]);
        let mut client = relay.client(ADMIN).await;
        push(&mut client, &changes.iter().collect::<Vec<_>>()).await;
        Rotated {
            space,
            generations,
            last: to_me,
        }
    }
}

/// `ADMIN` wraps the key of the generation at `index` to `recipient`.
fn wrap(
    space: &mut space::Space,
    recipient: Who,
    index: usize,
    generation: u64,
    after: &Change,
) -> Change {
    let sealed = envelope::seal(
        &key(index),
        space.space,
        generation,
        &space::key(recipient).public(),
    )
    .expect("a simulated member's key takes an envelope");
    space.write(ADMIN, Entry::KeyEnvelope(sealed), &[after])
}

/// The admin's task of the generation at `index`, sealed under its key.
fn sealed(space: &mut space::Space, index: usize, generation: u64) -> Change {
    space.skip(ADMIN, 1);
    let sequence = space.sequence(ADMIN);
    let operations = vec![field_op::create(task(index), "tasks")];
    let payload = Payload::new(Hlc(sequence << 16), BTreeSet::new(), operations);
    seal::seal(
        &space::key(ADMIN),
        &key(index),
        space.space,
        sequence,
        generation,
        &payload,
    )
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

/// `ME` opens the space after `rotations` rotations and reads every task
/// the admin sealed, one under each generation.
async fn joins_after(rotations: usize) {
    let relay = LocalRelay::start().await;
    let rotated = Rotated::push(&relay, rotations).await;
    assert_eq!(rotated.generations.len(), rotations + 1);
    let db = open(ME, &relay.url()).await;
    eventually("the envelope to me", || {
        pulled(&db, &rotated.space, &rotated.last)
    })
    .await;

    let project = now(db.spaces().get(SpaceId(rotated.space.space))).unwrap();

    for index in 0..=rotations {
        let read = now(project.get("tasks", ObjectId(task(index)))).unwrap();
        assert!(read.is_some(), "the task of generation {index}");
    }
    let listed = now(db.spaces().list()).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].model, SpaceModel::Ours);
    let written = now(project.write(|tx| {
        tx.create("tasks", [("title", Value::from("mine"))])?;
        Ok(())
    }))
    .unwrap();
    assert_eq!(synced(&db).await.refused, 0);
    assert!(
        relay
            .log(rotated.space.space)
            .await
            .contains(&written.change.0)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_member_added_after_one_rotation_opens_the_space_and_reads_its_history() {
    joins_after(1).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_member_added_after_two_rotations_walks_the_key_chain_back_to_the_first_generation() {
    joins_after(2).await;
}
