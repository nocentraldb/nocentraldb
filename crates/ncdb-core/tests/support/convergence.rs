//! The convergence harness: principle 2, "the same set of changes yields the
//! same state on every copy, in any order of arrival".

use core::fmt::Debug;

use proptest::arbitrary::any;
use proptest::collection::vec;
use proptest::strategy::{BoxedStrategy, Strategy};
use proptest::test_runner::{
    Config as RunnerConfig, RngAlgorithm, TestCaseError, TestError, TestRng, TestRunner,
};

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
    /// Most extra times one copy receives the same change.
    pub max_duplicates: usize,
    /// Number of generated cases.
    pub cases: u32,
    /// Seeds the generator: the same seed generates the same cases.
    pub seed: u64,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            members: 3,
            copies: 3,
            max_changes_per_member: 4,
            max_duplicates: 2,
            cases: 256,
            seed: 0,
        }
    }
}

/// Copies that ended in different states: the minimal counterexample the
/// harness found.
#[derive(Clone, Debug)]
pub struct Divergence {
    /// The seed of the run that found it.
    pub seed: u64,
    /// The changes the members made, in the order they were generated.
    pub changes: Vec<String>,
    /// Per copy, the order in which it received the changes, as indexes
    /// into `changes`; an index appears once per delivery, so a duplicate
    /// appears more than once.
    pub orders: Vec<Vec<usize>>,
    /// Per copy, the state it ended in.
    pub states: Vec<String>,
}

impl core::fmt::Display for Divergence {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        writeln!(
            f,
            "copies diverged with seed {}; minimal counterexample:",
            self.seed
        )?;
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

/// One generated change and when it arrives: per copy, one arrival key per
/// delivery, so at least one and more for duplicates. A copy receives the
/// deliveries in the order of their keys, ties in generated order. Keys
/// travel with their change, so shrinking a change away keeps the relative
/// order of the others; keys shrink towards generated order and duplicates
/// shrink away.
#[derive(Clone, Debug)]
struct Sent<C> {
    change: C,
    arrivals: Vec<Vec<u32>>,
}

/// Per copy, the order in which it receives `sent`, as indexes into it.
fn orders_of<C>(sent: &[Sent<C>], copies: usize) -> Vec<Vec<usize>> {
    (0..copies)
        .map(|copy| {
            let mut deliveries: Vec<(u32, usize)> = sent
                .iter()
                .enumerate()
                .flat_map(|(i, s)| s.arrivals[copy].iter().map(move |&key| (key, i)))
                .collect();
            deliveries.sort();
            deliveries.into_iter().map(|(_, i)| i).collect()
        })
        .collect()
}

/// Delivers `sent` to each copy in its order and returns the end states.
fn deliver<S: Mergeable>(sent: &[Sent<S::Change>], orders: &[Vec<usize>]) -> Vec<S> {
    orders
        .iter()
        .map(|order| {
            let mut state = S::default();
            for &i in order {
                state.apply(&sent[i].change);
            }
            state
        })
        .collect()
}

fn diverges<S: Mergeable>(sent: &[Sent<S::Change>], copies: usize) -> bool {
    let states = deliver::<S>(sent, &orders_of(sent, copies));
    states.iter().any(|state| *state != states[0])
}

/// Finishes what proptest's shrinking leaves: removes single changes and
/// single duplicate deliveries while the copies still diverge, until no one
/// of them can go. proptest's vector shrinker does not retry removing an
/// element once it has shrunk the elements, so it can stop short of this.
fn minimise<S: Mergeable>(mut sent: Vec<Sent<S::Change>>, copies: usize) -> Vec<Sent<S::Change>> {
    'smaller: loop {
        for i in 0..sent.len() {
            let mut fewer = sent.clone();
            fewer.remove(i);
            if diverges::<S>(&fewer, copies) {
                sent = fewer;
                continue 'smaller;
            }
        }
        for i in 0..sent.len() {
            for copy in 0..copies {
                let deliveries = sent[i].arrivals[copy].len();
                // Every copy keeps at least one delivery of every change.
                for key in (0..deliveries).filter(|_| deliveries > 1) {
                    let mut fewer = sent.clone();
                    fewer[i].arrivals[copy].remove(key);
                    if diverges::<S>(&fewer, copies) {
                        sent = fewer;
                        continue 'smaller;
                    }
                }
            }
        }
        return sent;
    }
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
    let copies = config.copies;
    let per_member: Vec<_> = (0..config.members)
        .map(|member| {
            let arrivals = vec(vec(any::<u32>(), 1..=1 + config.max_duplicates), copies);
            let sent =
                (change(member), arrivals).prop_map(|(change, arrivals)| Sent { change, arrivals });
            vec(sent, 0..=config.max_changes_per_member)
        })
        .collect();
    let cases = per_member.prop_map(|lists| lists.into_iter().flatten().collect::<Vec<_>>());

    // The generator is seeded here, never from the OS, so a run is the same
    // on every machine and target.
    let mut seed = [0u8; 32];
    seed[..8].copy_from_slice(&config.seed.to_le_bytes());
    let rng = TestRng::from_seed(RngAlgorithm::ChaCha, &seed);
    let runner_config = RunnerConfig {
        cases: config.cases,
        failure_persistence: None,
        ..RunnerConfig::default()
    };
    let mut runner = TestRunner::new_with_rng(runner_config, rng);
    let result = runner.run(&cases, |sent| {
        if diverges::<S>(&sent, copies) {
            Err(TestCaseError::fail("copies diverged"))
        } else {
            Ok(())
        }
    });
    match result {
        Ok(()) => Ok(()),
        Err(TestError::Fail(_, sent)) => {
            let sent = minimise::<S>(sent, copies);
            let orders = orders_of(&sent, copies);
            let states = deliver::<S>(&sent, &orders);
            Err(Divergence {
                seed: config.seed,
                changes: sent.iter().map(|s| format!("{:?}", s.change)).collect(),
                orders,
                states: states.iter().map(|s| format!("{s:?}")).collect(),
            })
        }
        Err(TestError::Abort(reason)) => panic!("the convergence harness aborted: {reason}"),
    }
}
