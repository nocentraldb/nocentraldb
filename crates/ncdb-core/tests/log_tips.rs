//! The tips of a log (NOC-138): the heads, the next author sequence of
//! each author and the highest clock, which a member's next change takes
//! from the log of its space (RFC 0002, "The change" and "Dependencies").
//! The log keeps them as it appends changes and sets their states, so
//! reading them opens no change it folded itself, and they equal a full
//! recount of the log after every change, in any order, through
//! revocations that un-fold changes and fold them again.

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

use ncdb_core::log::{Log, RejectReason, State};
use proptest::collection::vec;
use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};

use member::Recorder;
use tips::{Counting, of, pool, recount};

/// Carol's copy of the pool's space.
struct Copy {
    log: Log,
    checks: Counting,
    fold: Recorder,
}

impl Copy {
    fn new() -> Copy {
        Copy {
            log: Log::new(pool().space),
            checks: Counting::new(),
            fold: Recorder::default(),
        }
    }

    fn receive(&mut self, frame: &[u8]) {
        self.log
            .receive(frame, &mut self.checks, &mut self.fold)
            .expect("every pooled frame is appended");
    }

    /// The tips equal a full recount, and reading them opened no change.
    fn assert_tips(&mut self) {
        self.checks.take_opened();
        let tips = of(self.log.tips(&self.checks));
        assert_eq!(self.checks.take_opened(), 0, "the tips opened a change");
        assert_eq!(tips, recount(&self.log, &self.checks));
    }
}

#[test]
fn the_tips_equal_a_full_recount_after_every_change_in_any_order() {
    let changes = &pool().changes;
    let mut bytes = [0u8; 32];
    bytes[..8].copy_from_slice(&138u64.to_le_bytes());
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
            let mut copy = Copy::new();
            for i in order {
                copy.receive(changes[i].bytes());
                copy.assert_tips();
            }
            Ok(())
        })
        .unwrap();
}

/// Every change before the revocations, then each revocation: the tips
/// follow each un-fold and each fold again.
#[test]
fn the_tips_follow_revocations_that_un_fold_and_fold_again() {
    let pool = pool();
    let mut copy = Copy::new();
    for change in &pool.changes {
        if !pool.revocations.contains(change) {
            copy.receive(change.bytes());
        }
    }
    copy.assert_tips();
    let before = copy.log.tips(&copy.checks).heads().clone();

    let mut un_folded = false;
    for revocation in &pool.revocations {
        copy.receive(revocation.bytes());
        copy.assert_tips();
        un_folded |= pool.sealed.iter().any(|change| {
            copy.log.state(change.id()) == Some(&State::Rejected(RejectReason::NotMember))
        });
    }

    assert!(un_folded, "a revocation un-folded a change");
    assert_ne!(copy.log.tips(&copy.checks).heads(), &before);
}

#[test]
fn an_empty_log_has_no_heads_and_starts_each_author_at_sequence_one() {
    let mut copy = Copy::new();

    let tips = copy.log.tips(&copy.checks);

    assert!(tips.heads().is_empty());
    assert!(tips.control_heads().is_empty());
    assert_eq!(tips.next_sequence(space::id(tips::ALICE)), 1);
    assert_eq!(tips.seen(), ncdb_core::clock::Hlc::default());
}
