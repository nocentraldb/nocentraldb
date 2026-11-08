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
use support::convergence::{
    self, Config, Member, Mergeable, Op, OpId, Outcome, Property, independent,
};

/// A grow-only set: union is commutative, associative and idempotent.
#[derive(Clone, Debug, Default)]
struct GrowOnlySet(BTreeSet<u8>);

impl Mergeable for GrowOnlySet {
    type Change = u8;
    type Read = Vec<u8>;

    fn apply(&mut self, op: &Op<u8>) {
        self.0.insert(op.body);
    }

    fn state_bytes(&self) -> Vec<u8> {
        self.0.iter().copied().collect()
    }

    fn read(&self) -> Vec<u8> {
        self.state_bytes()
    }

    fn outcome(&self, _: OpId) -> Outcome {
        Outcome::Visible
    }
}

/// Broken on purpose: the first write to arrive wins, so copies that see the
/// writes in different orders keep different values.
#[derive(Clone, Debug, Default)]
struct FirstArrivalWins(Option<(Member, u8)>);

impl Mergeable for FirstArrivalWins {
    type Change = (Member, u8);
    type Read = Option<(Member, u8)>;

    fn apply(&mut self, op: &Op<(Member, u8)>) {
        if self.0.is_none() {
            self.0 = Some(op.body);
        }
    }

    fn state_bytes(&self) -> Vec<u8> {
        self.0.map_or(vec![], |(member, value)| vec![member, value])
    }

    fn read(&self) -> Option<(Member, u8)> {
        self.0
    }

    fn outcome(&self, _: OpId) -> Outcome {
        Outcome::Visible
    }
}

fn write(member: Member) -> BoxedStrategy<(Member, u8)> {
    any::<u8>().prop_map(move |value| (member, value)).boxed()
}

#[test]
fn a_grow_only_set_converges() {
    let result = convergence::check::<GrowOnlySet, _>(
        &Config::default(),
        &independent(|_| any::<u8>().boxed()),
    );
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn first_arrival_wins_diverges() {
    let result = convergence::check::<FirstArrivalWins, _>(&Config::default(), &independent(write));
    assert!(result.is_err(), "the harness missed a divergent subject");
}

#[test]
fn a_divergence_shrinks_and_reports_the_orders_that_diverged() {
    let divergence =
        convergence::check::<FirstArrivalWins, _>(&Config::default(), &independent(write))
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
#[derive(Clone, Debug, Default)]
struct Recording(GrowOnlySet);

impl Mergeable for Recording {
    type Change = u8;
    type Read = Vec<u8>;

    fn apply(&mut self, op: &Op<u8>) {
        APPLIED.with(|applied| applied.borrow_mut().push(op.body));
        self.0.apply(op);
    }

    fn state_bytes(&self) -> Vec<u8> {
        self.0.state_bytes()
    }

    fn read(&self) -> Vec<u8> {
        self.0.read()
    }

    fn outcome(&self, id: OpId) -> Outcome {
        self.0.outcome(id)
    }
}

fn changes_applied_with(seed: u64) -> Vec<u8> {
    APPLIED.with(|applied| applied.borrow_mut().clear());
    let config = Config {
        seed,
        cases: 8,
        ..Config::default()
    };
    convergence::check::<Recording, _>(&config, &independent(|_| any::<u8>().boxed()))
        .expect("a grow-only set");
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
    let divergence = convergence::check::<FirstArrivalWins, _>(&config, &independent(write))
        .expect_err("the harness missed a divergent subject");
    assert_eq!(divergence.seed, 42);
    assert!(divergence.to_string().contains("seed 42"), "{divergence}");
}

/// Broken on purpose: counts every delivery, so a copy that receives a change
/// twice counts it twice. In any order without duplicates it converges.
#[derive(Clone, Debug, Default)]
struct DeliveryCounter(u32);

impl Mergeable for DeliveryCounter {
    type Change = ();
    type Read = u32;

    fn apply(&mut self, _: &Op<()>) {
        self.0 += 1;
    }

    fn state_bytes(&self) -> Vec<u8> {
        self.0.to_le_bytes().to_vec()
    }

    fn read(&self) -> u32 {
        self.0
    }

    fn outcome(&self, _: OpId) -> Outcome {
        Outcome::Visible
    }
}

#[test]
fn duplicate_deliveries_break_a_subject_that_is_not_idempotent() {
    let divergence = convergence::check::<DeliveryCounter, _>(
        &Config::default(),
        &independent(|_| Just(()).boxed()),
    )
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

fn deliveries(divergence: &convergence::Violation) -> usize {
    divergence.orders.iter().map(Vec::len).sum()
}

#[test]
fn every_seed_shrinks_to_two_writes_delivered_once() {
    for seed in SEEDS {
        let config = Config {
            seed,
            ..Config::default()
        };
        let divergence = convergence::check::<FirstArrivalWins, _>(&config, &independent(write))
            .expect_err("the harness missed a divergent subject");
        assert_eq!(divergence.changes.len(), 2, "{divergence}");
        assert_eq!(deliveries(&divergence), 2 * config.copies, "{divergence}");
    }
}

#[test]
fn every_seed_shrinks_to_one_change_applied_twice() {
    for seed in SEEDS {
        let config = Config {
            seed,
            ..Config::default()
        };
        let violation =
            convergence::check::<DeliveryCounter, _>(&config, &independent(|_| Just(()).boxed()))
                .expect_err("the harness never applied a change twice");
        assert_eq!(violation.property, Property::C2, "{violation}");
        assert_eq!(violation.changes.len(), 1, "{violation}");
        // Either one copy received it twice, or the copies converged and
        // applying the whole set twice to one copy showed it.
        assert!(
            [config.copies, config.copies + 1].contains(&deliveries(&violation)),
            "{violation}"
        );
    }
}

/// Broken on purpose: keeps every write in order of arrival, but its
/// `PartialEq` compares nothing, so copies that hold the writes in different
/// orders look equal. Its canonical state bytes show the difference.
#[derive(Clone, Debug, Default)]
struct ArrivalOrder(Vec<u8>);

impl PartialEq for ArrivalOrder {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Mergeable for ArrivalOrder {
    type Change = u8;
    type Read = ();

    fn apply(&mut self, op: &Op<u8>) {
        if !self.0.contains(&op.body) {
            self.0.push(op.body);
        }
    }

    fn state_bytes(&self) -> Vec<u8> {
        self.0.clone()
    }

    fn read(&self) {}

    fn outcome(&self, _: OpId) -> Outcome {
        Outcome::Visible
    }
}

#[test]
fn copies_are_compared_by_their_canonical_state_bytes_not_by_partial_eq() {
    let result = convergence::check::<ArrivalOrder, _>(
        &Config::default(),
        &independent(|_| any::<u8>().boxed()),
    );
    assert!(
        result.is_err(),
        "the harness took a lying PartialEq for convergence"
    );
}
