//! The convergence harness proves its own detector: a broken subject fails
//! it, a correct one passes.

mod support;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::cell::RefCell;
use std::collections::BTreeSet;

use proptest::prelude::*;
use support::convergence::{self, Config, Member, Mergeable};

/// A grow-only set: union is commutative, associative and idempotent.
#[derive(Clone, Debug, Default, PartialEq)]
struct GrowOnlySet(BTreeSet<u8>);

impl Mergeable for GrowOnlySet {
    type Change = u8;

    fn apply(&mut self, change: &u8) {
        self.0.insert(*change);
    }
}

/// Broken on purpose: the first write to arrive wins, so copies that see the
/// writes in different orders keep different values.
#[derive(Clone, Debug, Default, PartialEq)]
struct FirstArrivalWins(Option<(Member, u8)>);

impl Mergeable for FirstArrivalWins {
    type Change = (Member, u8);

    fn apply(&mut self, change: &(Member, u8)) {
        if self.0.is_none() {
            self.0 = Some(*change);
        }
    }
}

fn write(member: Member) -> BoxedStrategy<(Member, u8)> {
    any::<u8>().prop_map(move |value| (member, value)).boxed()
}

#[test]
fn a_grow_only_set_converges() {
    let result = convergence::check::<GrowOnlySet>(&Config::default(), |_| any::<u8>().boxed());
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn first_arrival_wins_diverges() {
    let result = convergence::check::<FirstArrivalWins>(&Config::default(), write);
    assert!(result.is_err(), "the harness missed a divergent subject");
}

#[test]
fn a_divergence_shrinks_and_reports_the_orders_that_diverged() {
    let divergence = convergence::check::<FirstArrivalWins>(&Config::default(), write)
        .expect_err("the harness missed a divergent subject");

    // Two writes in two orders are the smallest case that diverges.
    assert_eq!(divergence.changes.len(), 2, "{divergence:?}");
    assert_eq!(divergence.orders.len(), 3, "{divergence:?}");
    for order in &divergence.orders {
        assert_eq!(order.len(), 2, "{divergence:?}");
    }

    let report = divergence.to_string();
    for (i, change) in divergence.changes.iter().enumerate() {
        assert!(report.contains(&format!("#{i} {change}")), "{report}");
    }
    for (copy, (order, state)) in divergence.orders.iter().zip(&divergence.states).enumerate() {
        assert!(
            report.contains(&format!(
                "copy {copy} received {order:?} and ended in {state}"
            )),
            "{report}"
        );
    }
    let diverged = (1..divergence.orders.len())
        .find(|&copy| divergence.states[copy] != divergence.states[0])
        .expect("a copy that diverged from copy 0");
    assert_ne!(divergence.orders[diverged], divergence.orders[0]);
    assert!(
        report.contains(&format!("copy {diverged} differs from copy 0")),
        "{report}"
    );
}

thread_local! {
    /// Every change a `Recording` copy applied, on this test's thread.
    static APPLIED: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// A grow-only set that records every change it applies.
#[derive(Clone, Debug, Default, PartialEq)]
struct Recording(GrowOnlySet);

impl Mergeable for Recording {
    type Change = u8;

    fn apply(&mut self, change: &u8) {
        APPLIED.with(|applied| applied.borrow_mut().push(*change));
        self.0.apply(change);
    }
}

fn changes_applied_with(seed: u64) -> Vec<u8> {
    APPLIED.with(|applied| applied.borrow_mut().clear());
    let config = Config {
        seed,
        cases: 8,
        ..Config::default()
    };
    convergence::check::<Recording>(&config, |_| any::<u8>().boxed()).expect("a grow-only set");
    APPLIED.with(|applied| applied.take())
}

#[test]
fn the_seed_decides_the_generated_cases() {
    assert_eq!(changes_applied_with(7), changes_applied_with(7));
    assert_ne!(changes_applied_with(7), changes_applied_with(8));
}

#[test]
fn the_report_names_the_seed() {
    let config = Config {
        seed: 42,
        ..Config::default()
    };
    let divergence = convergence::check::<FirstArrivalWins>(&config, write)
        .expect_err("the harness missed a divergent subject");
    assert_eq!(divergence.seed, 42);
    assert!(divergence.to_string().contains("seed 42"), "{divergence}");
}

/// Broken on purpose: counts every delivery, so a copy that receives a change
/// twice counts it twice. In any order without duplicates it converges.
#[derive(Clone, Debug, Default, PartialEq)]
struct DeliveryCounter(u32);

impl Mergeable for DeliveryCounter {
    type Change = ();

    fn apply(&mut self, _: &()) {
        self.0 += 1;
    }
}

#[test]
fn duplicate_deliveries_break_a_subject_that_is_not_idempotent() {
    let divergence =
        convergence::check::<DeliveryCounter>(&Config::default(), |_| Just(()).boxed())
            .expect_err("the harness never delivered a change twice");

    // One change, received twice by some copy, is the smallest case.
    assert_eq!(divergence.changes.len(), 1, "{divergence:?}");
    assert!(
        divergence.orders.iter().any(|order| order == &[0, 0]),
        "{divergence:?}"
    );
}

/// Seeds the minimality tests run with; shrinking must not depend on luck.
const SEEDS: core::ops::Range<u64> = 0..64;

fn deliveries(divergence: &convergence::Divergence) -> usize {
    divergence.orders.iter().map(Vec::len).sum()
}

#[test]
fn every_seed_shrinks_to_two_writes_delivered_once() {
    for seed in SEEDS {
        let config = Config {
            seed,
            ..Config::default()
        };
        let divergence = convergence::check::<FirstArrivalWins>(&config, write)
            .expect_err("the harness missed a divergent subject");
        assert_eq!(divergence.changes.len(), 2, "{divergence}");
        assert_eq!(deliveries(&divergence), 2 * config.copies, "{divergence}");
    }
}

#[test]
fn every_seed_shrinks_to_one_change_delivered_twice_once() {
    for seed in SEEDS {
        let config = Config {
            seed,
            ..Config::default()
        };
        let divergence = convergence::check::<DeliveryCounter>(&config, |_| Just(()).boxed())
            .expect_err("the harness never delivered a change twice");
        assert_eq!(divergence.changes.len(), 1, "{divergence}");
        assert_eq!(deliveries(&divergence), config.copies + 1, "{divergence}");
    }
}
