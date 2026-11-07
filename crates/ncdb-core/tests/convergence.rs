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
