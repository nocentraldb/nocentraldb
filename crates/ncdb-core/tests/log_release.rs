//! A log that releases its sealed changes once its store keeps their
//! frames (NOC-452): it reads them back through the fold's store when a
//! pass needs them, ends in the same states and fold as a log that keeps
//! every change in memory, in any order of arrival, through revocations
//! that un-fold changes, and reads from the store only the changes that a
//! passing control change may un-fold.

mod support;

#[path = "support/member.rs"]
mod member;
#[path = "support/space.rs"]
mod space;
#[path = "support/tips.rs"]
mod tips;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::{BTreeMap, BTreeSet};

use ncdb_core::change::{Change, Operation};
use ncdb_core::checks::SealChecks;
use ncdb_core::field_op::Origin;
use ncdb_core::ids::{ChangeId, ObjectId, SpaceId};
use ncdb_core::log::{Log, NoFrames, OperationParkReason, State};
use ncdb_core::object::ObjectOperation;
use ncdb_core::receive::Fold;
use proptest::collection::vec;
use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};

use member::Recorder;
use tips::{CAROL, of, pool, recount};

/// A fold whose store keeps every frame it was given, and records each
/// change the log reads back from it.
#[derive(Default)]
struct Stored {
    fold: Recorder,
    frames: BTreeMap<ChangeId, Change>,
    reads: Vec<ChangeId>,
}

impl Fold for Stored {
    fn model_id(&self) -> [u8; 16] {
        self.fold.model_id()
    }
    fn knows_collection(&self, collection: &str) -> bool {
        self.fold.knows_collection(collection)
    }
    fn knows_field(&self, collection: &str, field: &str) -> bool {
        self.fold.knows_field(collection, field)
    }
    fn fold(&mut self, origin: Origin, operation: &Operation) -> Result<(), OperationParkReason> {
        self.fold.fold(origin, operation)
    }
    fn fold_object(&mut self, object: ObjectId, operation: &ObjectOperation) {
        self.fold.fold_object(object, operation)
    }
    fn clear_field(&mut self, object: ObjectId, collection: &str, field: &str) {
        self.fold.clear_field(object, collection, field)
    }
    fn clear_object(&mut self, object: ObjectId) {
        self.fold.clear_object(object)
    }
    fn stored(&mut self, _space: SpaceId, id: ChangeId) -> Option<Change> {
        self.reads.push(id);
        self.frames.get(&id).cloned()
    }
}

/// Carol's copy of the pool's space, whose log keeps every change, and
/// one whose log releases each sealed change once its store keeps it.
struct Copies {
    kept: (Log, SealChecks, Recorder),
    released: (Log, SealChecks, Stored),
}

impl Copies {
    fn new() -> Copies {
        let checks = || SealChecks::new(pool().space, space::key(CAROL));
        Copies {
            kept: (Log::new(pool().space), checks(), Recorder::default()),
            released: (Log::new(pool().space), checks(), Stored::default()),
        }
    }

    fn receive(&mut self, change: &Change) {
        let (log, checks, fold) = &mut self.kept;
        log.receive(change.bytes(), checks, fold).unwrap();
        let (log, checks, store) = &mut self.released;
        store.frames.insert(change.id(), change.clone());
        log.receive(change.bytes(), checks, store).unwrap();
        for id in log.ids().collect::<Vec<_>>() {
            log.release(id);
        }
    }

    /// Both copies hold the same states, the same fold and the same tips.
    fn assert_equal(&mut self) {
        let states = |log: &Log| -> Vec<(ChangeId, Option<State>)> {
            log.ids().map(|id| (id, log.state(id).cloned())).collect()
        };
        assert_eq!(states(&self.released.0), states(&self.kept.0));
        assert_eq!(self.released.2.fold, self.kept.2);
        let (log, checks, store) = &mut self.released;
        let released = of(log.tips(checks, store));
        let (log, checks, _) = &mut self.kept;
        assert_eq!(released, of(log.tips(checks, &mut NoFrames)));
        assert_eq!(released, recount(log, checks, &mut NoFrames));
    }
}

#[test]
fn a_log_that_releases_its_sealed_changes_ends_as_one_that_keeps_them_in_any_order() {
    let changes = &pool().changes;
    let mut bytes = [0u8; 32];
    bytes[..8].copy_from_slice(&452u64.to_le_bytes());
    let config = Config {
        cases: 64,
        failure_persistence: None,
        ..Config::default()
    };
    let mut runner =
        TestRunner::new_with_rng(config, TestRng::from_seed(RngAlgorithm::ChaCha, &bytes));
    let order = vec(0..changes.len(), 0..=2 * changes.len());
    runner
        .run(&order, |order| {
            let mut copies = Copies::new();
            for i in order {
                copies.receive(&changes[i]);
                copies.assert_equal();
            }
            Ok(())
        })
        .unwrap();
}

/// Every control change passes before the revocations, then each
/// revocation un-folds changes the log released, and reads them back.
#[test]
fn revocations_un_fold_released_changes_as_kept_ones() {
    let pool = pool();
    let mut copies = Copies::new();
    for change in &pool.changes {
        if !pool.revocations.contains(change) {
            copies.receive(change);
        }
    }
    for revocation in &pool.revocations {
        copies.receive(revocation);
        copies.assert_equal();
    }
    assert!(!copies.released.2.reads.is_empty());
}

/// Alice demotes Dan after his second change, before his third arrived:
/// the demotion passes, admits every sealed change the log holds, and
/// replaces no key, so un-folding reads none of them from the store.
#[test]
fn a_control_change_that_un_folds_nothing_reads_no_sealed_change() {
    let pool = pool();
    let (demotion, d3) = (&pool.revocations[1], &pool.sealed[4]);
    let mut copies = Copies::new();
    for change in &pool.changes {
        if !pool.revocations.contains(change) && change != d3 {
            copies.receive(change);
        }
    }
    let folded: BTreeSet<ChangeId> = pool
        .sealed
        .iter()
        .filter(|change| *change != d3)
        .map(Change::id)
        .collect();
    let log = &copies.released.0;
    assert!(
        folded
            .iter()
            .all(|id| matches!(log.state(*id), Some(State::Folded { .. })))
    );
    copies.released.2.reads.clear();

    copies.receive(demotion);

    assert!(matches!(
        copies.released.0.state(demotion.id()),
        Some(State::Folded { .. })
    ));
    assert_eq!(copies.released.2.reads, Vec::<ChangeId>::new());
    copies.assert_equal();
}
