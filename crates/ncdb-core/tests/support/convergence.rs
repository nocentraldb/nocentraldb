//! The convergence harness: principle 2, "the same set of changes yields the
//! same state on every copy, in any order of arrival", and the plugin
//! contract C1 to C7 of RFC 0004 (`docs/rfc/0004-merge-rules-basic-types.md`).

// Every test binary that declares `mod support;` compiles the whole harness
// but uses only part of it.
#![allow(dead_code)]

use core::fmt::Debug;

use proptest::arbitrary::any;
use proptest::collection::vec;
use proptest::strategy::{BoxedStrategy, Strategy};
use proptest::test_runner::{
    Config as RunnerConfig, RngAlgorithm, TestCaseError, TestError, TestRng, TestRunner,
};

/// Who made an operation: one of the simulated members.
pub type Member = u8;

/// An operation's id. The harness numbers the operations of a case in the
/// order they were made, so no two share an id.
pub type OpId = u32;

/// One operation, as a member makes it and a copy receives it: the id and
/// the author come with the change (RFC 0002), the body is the field type's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Op<C> {
    pub id: OpId,
    pub author: Member,
    pub body: C,
}

/// Where an operation went (RFC 0004, "The plugin contract"): every received
/// operation has exactly one outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// It is visible in the read value: won, or applied.
    Visible,
    /// An operation whose author had seen it replaced it.
    Replaced(OpId),
    /// It lost, and the app can read that it lost.
    Lost,
}

/// A field type's state, as RFC 0004's plugin contract defines it. The
/// harness drives it; every field type the core ships must pass the harness.
/// `Default` is the initial state of a field nobody has written.
pub trait Mergeable: Default + Clone + Debug {
    /// The body of one operation.
    type Change: Clone + Debug;
    /// What the app, views and computed fields see.
    type Read: PartialEq + Debug;

    /// Merges one received operation into this state.
    fn apply(&mut self, op: &Op<Self::Change>);
    /// The canonical encoding of the state: equal states, equal bytes. The
    /// harness compares copies by these bytes, never by `PartialEq`.
    fn state_bytes(&self) -> Vec<u8>;
    /// The read value.
    fn read(&self) -> Self::Read;
    /// Where the operation `id`, which this state received, went.
    fn outcome(&self, id: OpId) -> Outcome;
}

/// The member that makes an operation, as its generator sees it.
pub struct Author<'a, S: Mergeable> {
    /// Who makes the operation.
    pub member: Member,
    /// The id the operation will have.
    pub id: OpId,
    /// The author's own copy: every operation in `seen`, applied.
    pub copy: &'a S,
    /// The operations the author had seen, in id order.
    pub seen: &'a [Op<S::Change>],
}

/// Generates the operations of a field type. An operation may only replace
/// what its author had seen, so `make` builds it from the author's copy.
pub trait Generator<S: Mergeable> {
    /// The random input of one operation, which the harness generates and
    /// shrinks.
    type Raw: Clone + Debug + 'static;

    /// Generates the random input of one operation that `member` makes.
    fn raw(&self, member: Member) -> BoxedStrategy<Self::Raw>;
    /// Builds the operation from its random input and its author's view.
    fn make(&self, author: &Author<'_, S>, raw: &Self::Raw) -> S::Change;
}

/// A generator whose operations do not depend on what their author had
/// seen; see [`independent`].
pub struct Independent<F>(F);

/// Generates every operation of `member` with `change(member)`, whatever its
/// author had seen.
pub fn independent<C, F: Fn(Member) -> BoxedStrategy<C>>(change: F) -> Independent<F> {
    Independent(change)
}

impl<S, F> Generator<S> for Independent<F>
where
    S: Mergeable,
    S::Change: 'static,
    F: Fn(Member) -> BoxedStrategy<S::Change>,
{
    type Raw = S::Change;

    fn raw(&self, member: Member) -> BoxedStrategy<S::Change> {
        (self.0)(member)
    }

    fn make(&self, _: &Author<'_, S>, raw: &S::Change) -> S::Change {
        raw.clone()
    }
}

/// How the harness runs.
#[derive(Clone, Debug)]
pub struct Config {
    /// Number of simulated members that make operations.
    pub members: Member,
    /// Number of copies the operations are delivered to; at least three.
    pub copies: usize,
    /// Most operations one member makes in one case, on average.
    pub max_changes_per_member: usize,
    /// Most extra times one copy receives the same operation.
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

/// A property of RFC 0004's plugin contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Property {
    /// Converges in any order.
    C1,
}

impl core::fmt::Display for Property {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Property::C1 => "C1 (converges in any order)",
        })
    }
}

/// A property the subject broke, with the minimal counterexample the harness
/// found.
#[derive(Clone, Debug)]
pub struct Violation {
    /// The property it broke.
    pub property: Property,
    /// What exactly went wrong.
    pub detail: String,
    /// The seed of the run that found it.
    pub seed: u64,
    /// The operations the members made, in the order they were made.
    pub changes: Vec<String>,
    /// Per copy, the order in which it received the operations, as indexes
    /// into `changes`; an index appears once per delivery, so a duplicate
    /// appears more than once.
    pub orders: Vec<Vec<usize>>,
    /// Per copy, the state it ended in.
    pub states: Vec<String>,
}

impl core::fmt::Display for Violation {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        writeln!(
            f,
            "{} failed with seed {}: {}; minimal counterexample:",
            self.property, self.seed, self.detail
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

/// One generated step: a member makes one operation, which then arrives at
/// every copy. Per copy, one arrival key per delivery, so at least one and
/// more for duplicates. A copy receives the deliveries in the order of their
/// keys, ties in generated order. Keys travel with their operation, so
/// shrinking an operation away keeps the relative order of the others; keys
/// shrink towards generated order and duplicates shrink away.
#[derive(Clone, Debug)]
struct Step<R> {
    member: Member,
    raw: R,
    arrivals: Vec<Vec<u32>>,
}

/// The operations a case's steps make.
struct Made<C> {
    ops: Vec<Op<C>>,
}

/// Plays the steps: each member makes its operation from its own copy.
fn simulate<S: Mergeable, G: Generator<S>>(
    generator: &G,
    steps: &[Step<G::Raw>],
) -> Made<S::Change> {
    let mut ops: Vec<Op<S::Change>> = Vec::with_capacity(steps.len());
    for (k, step) in steps.iter().enumerate() {
        let id = k as OpId;
        let copy = S::default();
        let author = Author {
            member: step.member,
            id,
            copy: &copy,
            seen: &[],
        };
        let body = generator.make(&author, &step.raw);
        ops.push(Op {
            id,
            author: step.member,
            body,
        });
    }
    Made { ops }
}

/// Per copy, the order in which it receives the steps' operations, as
/// indexes into them.
fn orders_of<R>(steps: &[Step<R>], copies: usize) -> Vec<Vec<usize>> {
    (0..copies)
        .map(|copy| {
            let mut deliveries: Vec<(u32, usize)> = steps
                .iter()
                .enumerate()
                .flat_map(|(i, s)| s.arrivals[copy].iter().map(move |&key| (key, i)))
                .collect();
            deliveries.sort();
            deliveries.into_iter().map(|(_, i)| i).collect()
        })
        .collect()
}

/// Delivers `ops` to each copy in its order and returns the end states.
fn deliver<S: Mergeable>(ops: &[Op<S::Change>], orders: &[Vec<usize>]) -> Vec<S> {
    orders
        .iter()
        .map(|order| {
            let mut state = S::default();
            for &i in order {
                state.apply(&ops[i]);
            }
            state
        })
        .collect()
}

/// The first property the case breaks, and how.
fn violation<S: Mergeable, G: Generator<S>>(
    generator: &G,
    steps: &[Step<G::Raw>],
    copies: usize,
) -> Option<(Property, String)> {
    let made = simulate::<S, G>(generator, steps);
    let states = deliver::<S>(&made.ops, &orders_of(steps, copies));
    let bytes: Vec<Vec<u8>> = states.iter().map(S::state_bytes).collect();
    if let Some(copy) = (1..copies).find(|&copy| bytes[copy] != bytes[0]) {
        return Some((
            Property::C1,
            format!("copy {copy} ended in other state bytes than copy 0"),
        ));
    }
    None
}

/// Finishes what proptest's shrinking leaves: removes single steps and
/// single duplicate deliveries while the case still breaks `property`, until
/// no one of them can go. proptest's vector shrinker does not retry removing
/// an element once it has shrunk the elements, so it can stop short of this.
fn minimise<S: Mergeable, G: Generator<S>>(
    generator: &G,
    mut steps: Vec<Step<G::Raw>>,
    copies: usize,
    property: Property,
) -> Vec<Step<G::Raw>> {
    let breaks = |steps: &[Step<G::Raw>]| {
        violation::<S, G>(generator, steps, copies).is_some_and(|(p, _)| p == property)
    };
    'smaller: loop {
        for i in 0..steps.len() {
            let mut fewer = steps.clone();
            fewer.remove(i);
            if breaks(&fewer) {
                steps = fewer;
                continue 'smaller;
            }
        }
        for i in 0..steps.len() {
            for copy in 0..copies {
                let deliveries = steps[i].arrivals[copy].len();
                // Every copy keeps at least one delivery of every operation.
                for key in (0..deliveries).filter(|_| deliveries > 1) {
                    let mut fewer = steps.clone();
                    fewer[i].arrivals[copy].remove(key);
                    if breaks(&fewer) {
                        steps = fewer;
                        continue 'smaller;
                    }
                }
            }
        }
        return steps;
    }
}

/// The cases a run generates.
fn cases<S: Mergeable, G: Generator<S>>(
    config: &Config,
    generator: &G,
) -> BoxedStrategy<Vec<Step<G::Raw>>> {
    let members = config.members;
    let copies = config.copies;
    let max_duplicates = config.max_duplicates;
    let raws: Vec<BoxedStrategy<G::Raw>> = (0..members).map(|m| generator.raw(m)).collect();
    let step = (0..members)
        .prop_flat_map(move |member| {
            let arrivals = vec(vec(any::<u32>(), 1..=1 + max_duplicates), copies);
            (raws[member as usize].clone(), arrivals).prop_map(move |(raw, arrivals)| Step {
                member,
                raw,
                arrivals,
            })
        })
        .boxed();
    vec(
        step,
        0..=config.members as usize * config.max_changes_per_member,
    )
    .boxed()
}

/// A runner seeded from `seed`, never from the OS, so a run is the same on
/// every machine and target.
fn runner(cases: u32, seed: u64) -> TestRunner {
    let mut bytes = [0u8; 32];
    bytes[..8].copy_from_slice(&seed.to_le_bytes());
    let rng = TestRng::from_seed(RngAlgorithm::ChaCha, &bytes);
    let config = RunnerConfig {
        cases,
        failure_persistence: None,
        ..RunnerConfig::default()
    };
    TestRunner::new_with_rng(config, rng)
}

/// Makes operations from several simulated members, delivers them to
/// several copies in different orders, and checks the plugin contract of
/// RFC 0004 on every case.
pub fn check<S: Mergeable, G: Generator<S>>(
    config: &Config,
    generator: &G,
) -> Result<(), Violation> {
    assert!(
        config.copies >= 3,
        "the harness needs at least three copies"
    );
    let copies = config.copies;
    let mut runner = runner(config.cases, config.seed);
    let result = runner.run(&cases::<S, G>(config, generator), |steps| {
        match violation::<S, G>(generator, &steps, copies) {
            Some((property, _)) => Err(TestCaseError::fail(property.to_string())),
            None => Ok(()),
        }
    });
    match result {
        Ok(()) => Ok(()),
        Err(TestError::Fail(_, steps)) => {
            let (property, _) =
                violation::<S, G>(generator, &steps, copies).expect("a failing case fails again");
            let steps = minimise::<S, G>(generator, steps, copies, property);
            let (property, detail) =
                violation::<S, G>(generator, &steps, copies).expect("a minimised case still fails");
            let made = simulate::<S, G>(generator, &steps);
            let orders = orders_of(&steps, copies);
            let states = deliver::<S>(&made.ops, &orders);
            Err(Violation {
                property,
                detail,
                seed: config.seed,
                changes: made.ops.iter().map(|op| format!("{op:?}")).collect(),
                orders,
                states: states.iter().map(|s| format!("{s:?}")).collect(),
            })
        }
        Err(TestError::Abort(reason)) => panic!("the convergence harness aborted: {reason}"),
    }
}
