//! Basic values meet RFC 0004's plugin contract C1 to C7, checked by the
//! convergence harness (NOC-19) with at least three copies, random orders of
//! arrival, duplicate deliveries and malformed writes. Two broken variants
//! of the type must be caught (C6).

mod support;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::basic::Read;
use ncdb_core::field::FieldType;
use support::basic::{self, Basic, Body, Choice, Float, Schema};
use support::convergence::{self, Config, Mergeable, Op, OpId, Outcome};

/// The seeds every check runs with.
const SEEDS: core::ops::Range<u64> = 0..16;

fn meets_the_contract<S: Schema>(config: Config) {
    for seed in SEEDS {
        let config = Config {
            seed,
            ..config.clone()
        };
        let result = convergence::check::<Basic<S>, _>(&config, &basic::writes::<S>());
        assert!(result.is_ok(), "{}", result.unwrap_err());
    }
}

#[test]
fn a_choice_meets_the_contract_with_options_the_readers_do_not_know() {
    meets_the_contract::<Choice>(Config::default());
}

#[test]
fn a_float_meets_the_contract_with_nans_and_both_zeros() {
    meets_the_contract::<Float>(Config::default());
}

#[test]
fn basic_values_meet_the_contract_with_more_members_copies_and_duplicates() {
    meets_the_contract::<Choice>(Config {
        members: 4,
        copies: 5,
        max_duplicates: 3,
        max_learned: 4,
        cases: 64,
        ..Config::default()
    });
}

/// Runs a broken variant and expects the harness to report `property` on
/// every seed (C6: the checks fire for this type).
fn expect_violation<S: Mergeable<Change = Body> + AsRef<Basic<Choice>>>(property: &str) {
    for seed in SEEDS {
        let config = Config {
            seed,
            ..Config::default()
        };
        let violation = convergence::check::<S, _>(&config, &basic::writes::<Choice>())
            .expect_err("the harness passed a broken variant");
        let report = violation.to_string();
        assert!(
            report.starts_with(&format!("{property} ")),
            "expected {property}, got: {report}"
        );
    }
}

/// Broken on purpose (C4): plain last writer wins, RFC 0004's first
/// rejected option. Every losing write reads as replaced by the winner,
/// although the winner's author may never have seen it, so it disappears
/// silently.
#[derive(Clone, Debug, Default)]
struct LosersDropped(Basic<Choice>);

impl AsRef<Basic<Choice>> for LosersDropped {
    fn as_ref(&self) -> &Basic<Choice> {
        &self.0
    }
}

impl Mergeable for LosersDropped {
    type Change = Body;
    type Read = Read;

    fn apply(&mut self, op: &Op<Body>) {
        self.0.apply(op);
    }

    fn state_bytes(&self) -> Vec<u8> {
        self.0.state_bytes()
    }

    fn read(&self) -> Read {
        let mut read = self.0.read();
        read.conflicts.clear();
        read
    }

    fn outcome(&self, id: OpId) -> Outcome {
        match (self.0.outcome(id), self.0.read().value) {
            (Outcome::Lost, Some(winner)) => Outcome::Replaced(basic::op_id(winner.id)),
            (outcome, _) => outcome,
        }
    }
}

#[test]
fn c6_plain_last_writer_wins_is_caught() {
    expect_violation::<LosersDropped>("C4");
}

/// Broken on purpose (C7): breaks a tie of equal stamps by the write that
/// arrived first, kept beside the state, instead of by author member id
/// and operation id.
#[derive(Clone, Debug, Default)]
struct TiesByArrival {
    field: Basic<Choice>,
    arrived: Vec<OpId>,
}

impl AsRef<Basic<Choice>> for TiesByArrival {
    fn as_ref(&self) -> &Basic<Choice> {
        &self.field
    }
}

impl Mergeable for TiesByArrival {
    type Change = Body;
    type Read = Read;

    fn apply(&mut self, op: &Op<Body>) {
        if !self.arrived.contains(&op.id) {
            self.arrived.push(op.id);
        }
        self.field.apply(op);
    }

    fn state_bytes(&self) -> Vec<u8> {
        self.field.state_bytes()
    }

    fn read(&self) -> Read {
        let mut read = self.field.read();
        let Some(winner) = read.value.clone() else {
            return read;
        };
        let rank = |id| self.arrived.iter().position(|a| *a == basic::op_id(id));
        let tied = read
            .conflicts
            .iter()
            .position(|c| c.stamp == winner.stamp && rank(c.id) < rank(winner.id));
        if let Some(i) = tied {
            let first = read.conflicts.remove(i);
            read.conflicts.push(winner);
            read.value = Some(first);
        }
        read
    }

    fn outcome(&self, id: OpId) -> Outcome {
        self.field.outcome(id)
    }
}

#[test]
fn c6_a_tie_broken_by_arrival_is_caught() {
    expect_violation::<TiesByArrival>("C7");
}

/// C3: the seeds of the golden digest.
const GOLDEN_SEEDS: [u64; 4] = [0, 1, 2, 3];

/// C3: the SHA-256 of the final state bytes of basic values for
/// `GOLDEN_SEEDS`, committed from a native run. The native and the wasm32
/// run of the test below must both give it.
const GOLDEN_DIGEST: &str = "";

fn golden_config() -> Config {
    Config {
        cases: 64,
        ..Config::default()
    }
}

#[test]
fn c3_the_same_seeds_give_the_same_state_bytes_natively_and_in_wasm32() {
    let choice = convergence::digest::<Basic<Choice>, _>(
        &golden_config(),
        &GOLDEN_SEEDS,
        &basic::writes::<Choice>(),
    );
    let float = convergence::digest::<Basic<Float>, _>(
        &golden_config(),
        &GOLDEN_SEEDS,
        &basic::writes::<Float>(),
    );
    assert_eq!(format!("{choice} {float}"), GOLDEN_DIGEST);
}

#[test]
fn state_bytes_differ_for_different_states() {
    let field = Choice::field();
    let one = convergence::Op {
        id: 0,
        author: 0,
        body: Body {
            stamp: 1,
            value: Choice::values()[0].clone(),
            replaces: Default::default(),
        },
    };
    let mut other = one.clone();
    other.body.value = Choice::values()[1].clone();
    let (mut a, mut b) = (Default::default(), Default::default());
    field.apply(&mut a, &basic::write(&one));
    field.apply(&mut b, &basic::write(&other));
    assert_ne!(field.state_bytes(&a), field.state_bytes(&b));
    assert_ne!(
        field.state_bytes(&a),
        field.state_bytes(&Default::default())
    );
}
