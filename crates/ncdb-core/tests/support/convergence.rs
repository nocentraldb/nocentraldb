//! The convergence harness: principle 2, "the same set of changes yields the
//! same state on every copy, in any order of arrival".

use core::fmt::Debug;

use proptest::collection::vec;
use proptest::sample::Index;
use proptest::strategy::{BoxedStrategy, Strategy};
use proptest::test_runner::{Config as RunnerConfig, TestCaseError, TestError, TestRunner};

/// A state that changes merge into. The harness drives it; every field type
/// the core ships must pass the harness.
pub trait Mergeable: Default + Clone + PartialEq + Debug {
    /// One change, as a member makes it and a copy receives it.
    type Change: Clone + Debug;

    /// Merges one received change into this state.
    fn apply(&mut self, change: &Self::Change);
}

/// Who made a change: one of the simulated members.
pub type Member = u8;

/// How the harness runs.
#[derive(Clone, Debug)]
pub struct Config {
    /// Number of simulated members that make changes.
    pub members: Member,
    /// Number of copies the changes are delivered to; at least three.
    pub copies: usize,
    /// Most changes one member makes in one case.
    pub max_changes_per_member: usize,
    /// Number of generated cases.
    pub cases: u32,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            members: 3,
            copies: 3,
            max_changes_per_member: 4,
            cases: 256,
        }
    }
}

/// Copies that ended in different states: the minimal counterexample the
/// harness found.
#[derive(Clone, Debug)]
pub struct Divergence {
    /// The changes the members made, in the order they were generated.
    pub changes: Vec<String>,
    /// Per copy, the order in which it received the changes, as indexes
    /// into `changes`.
    pub orders: Vec<Vec<usize>>,
    /// Per copy, the state it ended in.
    pub states: Vec<String>,
}

impl core::fmt::Display for Divergence {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        writeln!(f, "copies diverged; minimal counterexample:")?;
        writeln!(f, "changes:")?;
        for (i, change) in self.changes.iter().enumerate() {
            writeln!(f, "  #{i} {change}")?;
        }
        for (copy, (order, state)) in self.orders.iter().zip(&self.states).enumerate() {
            write!(f, "copy {copy} received {order:?} and ended in {state}")?;
            if copy > 0 && *state != self.states[0] {
                write!(f, "  <- copy {copy} differs from copy 0")?;
            }
            writeln!(f)?;
        }
        Ok(())
    }
}

/// One generated case: what the members made and how it arrives.
#[derive(Clone, Debug)]
struct Case<C> {
    changes: Vec<C>,
    /// Per copy, the picks that order its delivery (see `delivery_order`).
    picks: Vec<Vec<Index>>,
}

/// Turns picks into a delivery order of `n` changes. Each pick takes one of
/// the changes not yet delivered; changes left when the picks run out follow
/// in generated order. A pick shrinks towards the first remaining change, so
/// orders shrink towards generated order.
fn delivery_order(n: usize, picks: &[Index]) -> Vec<usize> {
    let mut remaining: Vec<usize> = (0..n).collect();
    let mut order = Vec::with_capacity(n);
    for pick in picks {
        if remaining.is_empty() {
            break;
        }
        order.push(remaining.remove(pick.index(remaining.len())));
    }
    order.extend(remaining);
    order
}

/// Delivers `changes` to each copy in its order and returns the end states.
fn deliver<S: Mergeable>(changes: &[S::Change], orders: &[Vec<usize>]) -> Vec<S> {
    orders
        .iter()
        .map(|order| {
            let mut state = S::default();
            for &i in order {
                state.apply(&changes[i]);
            }
            state
        })
        .collect()
}

fn orders_of<C>(case: &Case<C>) -> Vec<Vec<usize>> {
    case.picks
        .iter()
        .map(|picks| delivery_order(case.changes.len(), picks))
        .collect()
}

/// Delivers generated changes to several copies in different orders and
/// checks that every copy ends in the same state. `change` generates one
/// change that `member` makes.
pub fn check<S: Mergeable>(
    config: &Config,
    change: impl Fn(Member) -> BoxedStrategy<S::Change>,
) -> Result<(), Divergence>
where
    S::Change: 'static,
{
    assert!(
        config.copies >= 3,
        "the harness needs at least three copies"
    );
    let max_changes = usize::from(config.members) * config.max_changes_per_member;
    let per_member: Vec<_> = (0..config.members)
        .map(|member| vec(change(member), 0..=config.max_changes_per_member))
        .collect();
    let changes = per_member.prop_map(|lists| lists.into_iter().flatten().collect::<Vec<_>>());
    let picks = vec(vec(any_index(), 0..=max_changes), config.copies);
    let cases = (changes, picks).prop_map(|(changes, picks)| Case { changes, picks });

    let mut runner = TestRunner::new(RunnerConfig {
        cases: config.cases,
        failure_persistence: None,
        ..RunnerConfig::default()
    });
    let result = runner.run(&cases, |case| {
        let states = deliver::<S>(&case.changes, &orders_of(&case));
        if states.iter().all(|state| *state == states[0]) {
            Ok(())
        } else {
            Err(TestCaseError::fail("copies diverged"))
        }
    });
    match result {
        Ok(()) => Ok(()),
        Err(TestError::Fail(_, case)) => {
            let orders = orders_of(&case);
            let states = deliver::<S>(&case.changes, &orders);
            Err(Divergence {
                changes: case.changes.iter().map(|c| format!("{c:?}")).collect(),
                orders,
                states: states.iter().map(|s| format!("{s:?}")).collect(),
            })
        }
        Err(TestError::Abort(reason)) => panic!("the convergence harness aborted: {reason}"),
    }
}

fn any_index() -> impl Strategy<Value = Index> {
    proptest::arbitrary::any::<Index>()
}
