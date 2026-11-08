//! Sets and links meet RFC 0004's plugin contract C1 to C7, checked by the
//! convergence harness (NOC-19) with at least three copies, random orders
//! of arrival, duplicate deliveries, concurrent additions and removals of
//! the same element, and malformed operations. Links (many targets) run as
//! a set of object ids and a link (one target) as a basic value of one, each
//! through its own field type. Two broken variants of sets must be caught
//! (C6).

mod support;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeMap;

use ncdb_core::cbor;
use ncdb_core::set::Read;
use support::basic::{self, Basic, Target};
use support::convergence::{self, Config, Mergeable, Op, OpId, Outcome};
use support::set::{self, Body, Floats, SetOf, Tags, Targets};

/// The seeds every check runs with.
const SEEDS: core::ops::Range<u64> = 0..16;

fn meets_the_contract<S: Mergeable<Change = Body> + AsRef<SetOf<T>>, T: set::Schema>(
    config: Config,
) {
    for seed in SEEDS {
        let config = Config {
            seed,
            ..config.clone()
        };
        let result = convergence::check::<S, _>(&config, &set::edits::<T>());
        assert!(result.is_ok(), "{}", result.unwrap_err());
    }
}

#[test]
fn tags_meet_the_contract_with_options_the_readers_do_not_know() {
    meets_the_contract::<SetOf<Tags>, Tags>(Config::default());
}

#[test]
fn floats_meet_the_contract_with_nans_and_both_zeros() {
    meets_the_contract::<SetOf<Floats>, Floats>(Config::default());
}

#[test]
fn links_meet_the_contract() {
    meets_the_contract::<SetOf<Targets>, Targets>(Config::default());
}

#[test]
fn sets_meet_the_contract_with_more_members_copies_and_duplicates() {
    meets_the_contract::<SetOf<Tags>, Tags>(Config {
        members: 4,
        copies: 5,
        max_duplicates: 3,
        max_learned: 4,
        cases: 64,
        ..Config::default()
    });
}

#[test]
fn a_link_meets_the_contract() {
    for seed in SEEDS {
        let config = Config {
            seed,
            ..Config::default()
        };
        let result = convergence::check::<Basic<Target>, _>(&config, &basic::writes::<Target>());
        assert!(result.is_ok(), "{}", result.unwrap_err());
    }
}

/// Runs a broken variant and expects the harness to report `property` on
/// every seed (C6: the checks fire for this type).
fn expect_violation<S: Mergeable<Change = Body> + AsRef<SetOf<Tags>>>(property: &str) {
    for seed in SEEDS {
        let config = Config {
            seed,
            ..Config::default()
        };
        let violation = convergence::check::<S, _>(&config, &set::edits::<Tags>())
            .expect_err("the harness passed a broken variant");
        let report = violation.to_string();
        assert!(
            report.starts_with(&format!("{property} ")),
            "expected {property}, got: {report}"
        );
    }
}

/// The operations a broken variant received, kept beside the state.
type Received = BTreeMap<OpId, Body>;

fn key(body: &Body) -> Vec<u8> {
    cbor::encode(&body.element)
}

/// Broken on purpose (C4): remove wins, RFC 0004's rejected option. Any
/// removal of an element takes it out, and every addition of it reads as
/// replaced by that removal, although its author may never have seen the
/// addition, so the addition disappears silently.
#[derive(Clone, Debug, Default)]
struct RemoveWins {
    set: SetOf<Tags>,
    received: Received,
}

impl AsRef<SetOf<Tags>> for RemoveWins {
    fn as_ref(&self) -> &SetOf<Tags> {
        &self.set
    }
}

impl RemoveWins {
    /// The removal of `element` with the lowest id.
    fn removal(&self, element: &[u8]) -> Option<OpId> {
        self.received
            .iter()
            .find(|(_, b)| b.remove && key(b) == element)
            .map(|(id, _)| *id)
    }
}

impl Mergeable for RemoveWins {
    type Change = Body;
    type Read = Read;

    fn apply(&mut self, op: &Op<Body>) {
        self.received
            .entry(op.id)
            .or_insert_with(|| op.body.clone());
        self.set.apply(op);
    }

    fn state_bytes(&self) -> Vec<u8> {
        self.set.state_bytes()
    }

    fn read(&self) -> Read {
        let mut read = self.set.read();
        read.elements
            .retain(|e| self.removal(&cbor::encode(&e.value)).is_none());
        read
    }

    fn outcome(&self, id: OpId) -> Outcome {
        let body = &self.received[&id];
        match (self.set.outcome(id), self.removal(&key(body))) {
            (Outcome::Visible, Some(by)) if !body.remove => Outcome::Replaced(by),
            (outcome, _) => outcome,
        }
    }
}

#[test]
fn c6_remove_wins_is_caught() {
    expect_violation::<RemoveWins>("C4");
}

/// Broken on purpose (C7): an element is in the set when the last
/// operation on it to arrive was an addition, kept beside the state, so the
/// read depends on the order of arrival.
#[derive(Clone, Debug, Default)]
struct LastArrivalWins {
    set: SetOf<Tags>,
    arrived: Vec<Op<Body>>,
}

impl AsRef<SetOf<Tags>> for LastArrivalWins {
    fn as_ref(&self) -> &SetOf<Tags> {
        &self.set
    }
}

impl Mergeable for LastArrivalWins {
    type Change = Body;
    type Read = Read;

    fn apply(&mut self, op: &Op<Body>) {
        if !self.arrived.iter().any(|a| a.id == op.id) {
            self.arrived.push(op.clone());
        }
        self.set.apply(op);
    }

    fn state_bytes(&self) -> Vec<u8> {
        self.set.state_bytes()
    }

    fn read(&self) -> Read {
        let mut read = self.set.read();
        read.elements.retain(|e| {
            let element = cbor::encode(&e.value);
            let last = self.arrived.iter().rev().find(|a| key(&a.body) == element);
            last.is_none_or(|a| !a.body.remove)
        });
        read
    }

    fn outcome(&self, id: OpId) -> Outcome {
        self.set.outcome(id)
    }
}

#[test]
fn c6_the_last_arrival_winning_is_caught() {
    expect_violation::<LastArrivalWins>("C7");
}

/// C3: the seeds of the golden digest.
const GOLDEN_SEEDS: [u64; 4] = [0, 1, 2, 3];

/// C3: the SHA-256 of the final state bytes of tags, floats, links and a
/// link for `GOLDEN_SEEDS`, committed from a native run. The native and the
/// wasm32 run of the test below must both give it.
const GOLDEN_DIGEST: &str = "a90fa24d47aa35d5b1b59c967649d2227676a4749f2b3309025ccbd484c10a56 7caa7c596f12f801058c934963f918c7d73e204c9f66f5a3f127dda8efb444a4 deb98a436a7b368b0037ffdbcc28a34a9ea3114c8c8c8891e50949336fa32bae 175d044bda0999d8173bc7c27399b504b1a7999af3a4eb9526ab42802649b06e";

fn golden_config() -> Config {
    Config {
        cases: 64,
        ..Config::default()
    }
}

#[test]
fn c3_the_same_seeds_give_the_same_state_bytes_natively_and_in_wasm32() {
    let config = golden_config();
    let digests = [
        convergence::digest::<SetOf<Tags>, _>(&config, &GOLDEN_SEEDS, &set::edits::<Tags>()),
        convergence::digest::<SetOf<Floats>, _>(&config, &GOLDEN_SEEDS, &set::edits::<Floats>()),
        convergence::digest::<SetOf<Targets>, _>(&config, &GOLDEN_SEEDS, &set::edits::<Targets>()),
        convergence::digest::<Basic<Target>, _>(&config, &GOLDEN_SEEDS, &basic::writes::<Target>()),
    ];
    assert_eq!(digests.join(" "), GOLDEN_DIGEST);
}
