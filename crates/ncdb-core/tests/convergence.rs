//! The convergence harness proves its own detector: a broken subject fails
//! it, a correct one passes.

mod support;

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
