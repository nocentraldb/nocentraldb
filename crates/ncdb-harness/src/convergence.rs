//! The convergence harness: principle 2, "the same set of changes yields the
//! same state on every copy, in any order of arrival", and the plugin
//! contract C1 to C7 of RFC 0004 (`docs/rfc/0004-merge-rules-basic-types.md`).

use core::cell::Cell;
use core::fmt::Debug;
use std::collections::BTreeSet;

use proptest::arbitrary::any;
use proptest::collection::vec;
use proptest::strategy::{BoxedStrategy, Strategy, ValueTree};
use proptest::test_runner::{
    Config as RunnerConfig, RngAlgorithm, TestCaseError, TestError, TestRng, TestRunner,
};
use sha2::{Digest, Sha256};

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
    /// The field type parks it: it never enters the state (RFC 0004,
    /// "Operations a member does not understand").
    Parked,
}

/// A field type's state, as RFC 0004's plugin contract defines it. The
/// harness drives it; every field type the core ships must pass the harness.
/// `Default` is the initial state of a field nobody has written.
pub trait Mergeable: Default + Clone + Debug {
    /// The body of one operation.
    type Change: Clone + Debug + 'static;
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
    /// Whether the state reads back from its own bytes as itself (RFC 0011,
    /// "The field type a plugin implements": `decode_state` must
    /// round-trip). A type whose state never leaves the core has nothing to
    /// read back.
    fn round_trips(&self) -> bool {
        true
    }
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

    /// Generates arbitrary operations, malformed ones included (unknown
    /// ids, cycles, values outside the schema), which the harness mixes in
    /// (C5). `None` mixes in nothing.
    fn malformed(&self) -> Option<BoxedStrategy<S::Change>> {
        None
    }
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
    /// Most operations of others a member learns before it makes one.
    pub max_learned: usize,
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
            max_learned: 3,
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
    /// Idempotent: applying an operation the state already holds changes
    /// nothing.
    C2,
    /// Robust: malformed operations apply without a panic and converge.
    C5,
    /// Nothing lost silently: every operation's outcome is the same on every
    /// copy, and replaced only by an operation whose author had seen it.
    C4,
    /// Tested reads: copies with equal state bytes read the same.
    C7,
    /// The state reads back from its bytes as itself (RFC 0011).
    RoundTrip,
}

impl core::fmt::Display for Property {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Property::C1 => "C1 (converges in any order)",
            Property::C2 => "C2 (idempotent)",
            Property::C4 => "C4 (nothing lost silently)",
            Property::C5 => "C5 (robust)",
            Property::C7 => "C7 (tested reads)",
            Property::RoundTrip => "the round trip of the state's bytes (RFC 0011)",
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

/// One generated step: a member learns some earlier operations, then makes
/// one operation from its copy, which then arrives at every copy. Each entry
/// of `learns` picks one earlier operation, by its index modulo the number
/// of earlier operations; it shrinks away, towards concurrent operations.
/// Per copy, one arrival key per delivery, so at least one and
/// more for duplicates. A copy receives the deliveries in the order of their
/// keys, ties in generated order. Keys travel with their operation, so
/// shrinking an operation away keeps the relative order of the others; keys
/// shrink towards generated order and duplicates shrink away.
#[derive(Clone, Debug)]
struct Step<R> {
    member: Member,
    learns: Vec<u32>,
    raw: R,
    arrivals: Vec<Vec<u32>>,
}

/// What a step makes its operation from.
#[derive(Clone, Debug)]
enum Body<R, C> {
    /// The generator's random input, built into an operation by its author.
    Made(R),
    /// An arbitrary operation, possibly malformed, taken as it is (C5).
    Malformed(C),
}

/// One step of a case for the field type `S` and its generator `G`.
type StepOf<S, G> = Step<Body<<G as Generator<S>>::Raw, <S as Mergeable>::Change>>;

/// The steps of a case.
type Steps<S, G> = [StepOf<S, G>];

/// The operations a case's steps make.
struct Made<C> {
    ops: Vec<Op<C>>,
    /// Per operation, the ids of the operations its author had seen.
    seen: Vec<BTreeSet<OpId>>,
    /// Per operation, whether it is an arbitrary one (C5), whose claims
    /// about what its author had seen the harness cannot check.
    malformed: Vec<bool>,
}

/// Plays the steps. Every member keeps its own copy, which holds its own
/// operations and the ones it learned; it makes each operation from that
/// copy, so an operation can only replace what its author had seen.
fn simulate<S: Mergeable, G: Generator<S>>(generator: &G, steps: &Steps<S, G>) -> Made<S::Change> {
    let mut ops: Vec<Op<S::Change>> = Vec::with_capacity(steps.len());
    let mut seen_by_op = Vec::with_capacity(steps.len());
    let mut malformed = Vec::with_capacity(steps.len());
    let members = steps
        .iter()
        .map(|s| s.member as usize + 1)
        .max()
        .unwrap_or(0);
    let mut copies: Vec<(S, BTreeSet<OpId>)> = vec![(S::default(), BTreeSet::new()); members];
    for (k, step) in steps.iter().enumerate() {
        let id = k as OpId;
        let (copy, seen) = &mut copies[step.member as usize];
        for &learn in step.learns.iter().filter(|_| k > 0) {
            let earlier = &ops[learn as usize % k];
            if seen.insert(earlier.id) {
                copy.apply(earlier);
            }
        }
        let seen_ops: Vec<Op<S::Change>> = seen.iter().map(|&s| ops[s as usize].clone()).collect();
        let author = Author {
            member: step.member,
            id,
            copy,
            seen: &seen_ops,
        };
        let op = Op {
            id,
            author: step.member,
            body: match &step.raw {
                Body::Made(raw) => generator.make(&author, raw),
                Body::Malformed(body) => body.clone(),
            },
        };
        malformed.push(matches!(step.raw, Body::Malformed(_)));
        seen_by_op.push(seen.clone());
        copy.apply(&op);
        seen.insert(id);
        ops.push(op);
    }
    Made {
        ops,
        seen: seen_by_op,
        malformed,
    }
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
    steps: &Steps<S, G>,
    copies: usize,
) -> Option<(Property, String)> {
    let made = simulate::<S, G>(generator, steps);
    let states = deliver::<S>(&made.ops, &orders_of(steps, copies));
    if let Some(copy) = (0..copies).find(|&copy| !states[copy].round_trips()) {
        return Some((
            Property::RoundTrip,
            format!("copy {copy}'s state does not read back from its bytes as itself"),
        ));
    }
    let bytes: Vec<Vec<u8>> = states.iter().map(S::state_bytes).collect();
    if let Some(copy) = (1..copies).find(|&copy| bytes[copy] != bytes[0]) {
        return Some((
            Property::C1,
            format!("copy {copy} ended in other state bytes than copy 0"),
        ));
    }
    let mut twice = S::default();
    for op in &made.ops {
        twice.apply(op);
    }
    let once = twice.state_bytes();
    for op in &made.ops {
        twice.apply(op);
    }
    if twice.state_bytes() != once {
        return Some((
            Property::C2,
            "applying every operation a second time changed the state bytes".to_string(),
        ));
    }
    let reads: Vec<S::Read> = states.iter().map(S::read).collect();
    if let Some(copy) = (1..copies).find(|&copy| reads[copy] != reads[0]) {
        return Some((
            Property::C7,
            format!(
                "copy {copy} reads {:?}, copy 0 reads {:?}, with equal state bytes",
                reads[copy], reads[0]
            ),
        ));
    }
    for op in &made.ops {
        let outcomes: Vec<Outcome> = states.iter().map(|s| s.outcome(op.id)).collect();
        if let Some(copy) = (1..copies).find(|&copy| outcomes[copy] != outcomes[0]) {
            return Some((
                Property::C4,
                format!(
                    "copy {copy} gives #{} the outcome {:?}, copy 0 gives {:?}",
                    op.id, outcomes[copy], outcomes[0]
                ),
            ));
        }
        if let Outcome::Replaced(by) = outcomes[0] {
            let by_malformed = made.malformed.get(by as usize) == Some(&true);
            let had_seen = made
                .seen
                .get(by as usize)
                .is_some_and(|seen| seen.contains(&op.id));
            if !by_malformed && !had_seen {
                return Some((
                    Property::C4,
                    format!(
                        "#{} is replaced by #{by}, whose author had not seen it",
                        op.id
                    ),
                ));
            }
        }
    }
    None
}

/// Finishes what proptest's shrinking leaves: removes single steps, single
/// duplicate deliveries and duplicates that every copy received while the case still breaks `property`, until
/// no one of them can go. proptest's vector shrinker does not retry removing
/// an element once it has shrunk the elements, so it can stop short of this.
fn minimise<S: Mergeable, G: Generator<S>>(
    generator: &G,
    mut steps: Vec<StepOf<S, G>>,
    copies: usize,
    property: Property,
) -> Vec<StepOf<S, G>> {
    let breaks = |steps: &Steps<S, G>| {
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
            // A duplicate every copy received can only go from all of them
            // at once: removed from one copy alone, the copies differ.
            let mut fewer = steps.clone();
            for arrivals in fewer[i].arrivals.iter_mut().filter(|a| a.len() > 1) {
                arrivals.pop();
            }
            if fewer[i].arrivals != steps[i].arrivals && breaks(&fewer) {
                steps = fewer;
                continue 'smaller;
            }
        }
        return steps;
    }
}

/// The cases a run generates.
fn cases<S: Mergeable, G: Generator<S>>(
    config: &Config,
    generator: &G,
) -> BoxedStrategy<Vec<StepOf<S, G>>> {
    let members = config.members;
    let copies = config.copies;
    let max_duplicates = config.max_duplicates;
    let max_learned = config.max_learned;
    let malformed = generator.malformed();
    let bodies: Vec<BoxedStrategy<Body<G::Raw, S::Change>>> = (0..members)
        .map(|m| {
            let made = generator.raw(m).prop_map(Body::Made);
            match &malformed {
                Some(malformed) => proptest::prop_oneof![
                    3 => made,
                    1 => malformed.clone().prop_map(Body::Malformed),
                ]
                .boxed(),
                None => made.boxed(),
            }
        })
        .collect();
    let step = (0..members)
        .prop_flat_map(move |member| {
            let learns = vec(any::<u32>(), 0..=max_learned);
            let arrivals = vec(vec(any::<u32>(), 1..=1 + max_duplicates), copies);
            (learns, bodies[member as usize].clone(), arrivals).prop_map(
                move |(learns, raw, arrivals)| Step {
                    member,
                    learns,
                    raw,
                    arrivals,
                },
            )
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
    // Shrinking keeps to the property the first failing case broke, so that
    // a case that breaks one property does not shrink into another.
    let broken: Cell<Option<Property>> = Cell::new(None);
    let result = runner.run(&cases::<S, G>(config, generator), |steps| {
        match violation::<S, G>(generator, &steps, copies) {
            Some((property, _)) if broken.get().is_none_or(|p| p == property) => {
                broken.set(Some(property));
                Err(TestCaseError::fail(property.to_string()))
            }
            _ => Ok(()),
        }
    });
    match result {
        Ok(()) => Ok(()),
        Err(TestError::Fail(_, steps)) => {
            let property = broken.get().expect("a failing case broke a property");
            let steps = minimise::<S, G>(generator, steps, copies, property);
            let (mut property, mut detail) =
                violation::<S, G>(generator, &steps, copies).expect("a minimised case still fails");
            let made = simulate::<S, G>(generator, &steps);
            let orders = orders_of(&steps, copies);
            // Minimising removed every duplicate delivery it could; one that
            // is left is needed to diverge, so the subject is not idempotent.
            let duplicated = orders.iter().any(|order| {
                let mut seen = BTreeSet::new();
                !order.iter().all(|i| seen.insert(i))
            });
            if property == Property::C1 && duplicated {
                property = Property::C2;
                detail.push_str(", and only with an operation delivered twice");
            } else if property == Property::C1 && made.malformed.contains(&true) {
                // Likewise, a malformed operation that is left is needed.
                property = Property::C5;
                detail.push_str(", and only with a malformed operation");
            }
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

/// The SHA-256, in lowercase hex, of the state bytes every copy ends in, for
/// `config.cases` cases of each seed in `seeds`, in order. The cases are the
/// ones `check` generates. A field type commits the digest for a fixed seed
/// list, and its native and wasm32 tests compare with it (C3: deterministic
/// everywhere).
pub fn digest<S: Mergeable, G: Generator<S>>(
    config: &Config,
    seeds: &[u64],
    generator: &G,
) -> String {
    let strategy = cases::<S, G>(config, generator);
    let mut hasher = Sha256::new();
    for &seed in seeds {
        let mut runner = runner(config.cases, seed);
        for _ in 0..config.cases {
            let steps = strategy
                .new_tree(&mut runner)
                .expect("the harness generates every case")
                .current();
            let made = simulate::<S, G>(generator, &steps);
            for state in deliver::<S>(&made.ops, &orders_of(&steps, config.copies)) {
                let bytes = state.state_bytes();
                hasher.update((bytes.len() as u64).to_le_bytes());
                hasher.update(&bytes);
            }
        }
    }
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
