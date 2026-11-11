//! RFC 0002, "Rejected changes", with the convergence harness (NOC-9, with
//! NOC-19's contract checks
//! C1, C2, C4 and C7): copies that receive the same
//! changes, control changes among them, in any order and with duplicates,
//! end with the same state for every change, the same folded operations and
//! the same control state (RFC 0006, "Why the end state depends only on the
//! set").

mod support;

#[path = "support/member.rs"]
mod member;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeMap;

use ncdb_core::ids::ChangeId;
use ncdb_core::log::{Position, State};
use proptest::prelude::*;
use support::convergence::{self, Config, Mergeable, Op, OpId, Outcome, independent};

use member::*;

/// One copy: a member's log, control state and fold. Its `Debug` output
/// shows the state of each change, so a divergence report says which
/// change ended where.
#[derive(Clone, Default)]
struct Copy(Member);

impl std::fmt::Debug for Copy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Copy")
            .field("states", &self.states())
            .field("folded", &self.0.fold.folded)
            .field("control", &self.0.control)
            .finish()
    }
}

impl Copy {
    /// The state of each change, by change id, so that the order in which
    /// the log holds them does not count.
    fn states(&self) -> BTreeMap<ChangeId, Option<State>> {
        let log = &self.0.log;
        log.after(Position::START)
            .map(|(_, change)| (change.id(), log.state(change.id()).cloned()))
            .collect()
    }
}

impl Mergeable for Copy {
    type Change = Vec<u8>;
    type Read = String;

    fn apply(&mut self, op: &Op<Vec<u8>>) {
        let m = &mut self.0;
        m.log
            .receive(&op.body, &mut m.control, &mut m.fold)
            .expect("every generated frame is appended");
    }

    /// The state of every change, the folded operations and the control
    /// state, in a canonical order: equal copies, equal bytes.
    fn state_bytes(&self) -> Vec<u8> {
        format!("{:?}", (self.states(), &self.0.fold, &self.0.control)).into_bytes()
    }

    fn read(&self) -> String {
        format!("{:?}", self.states())
    }

    /// The log keeps every change it receives, folded, parked or rejected:
    /// none is replaced or lost.
    fn outcome(&self, _: OpId) -> Outcome {
        Outcome::Visible
    }
}

/// A change `author` makes: a membership entry, a key envelope, or a sealed
/// change that may carry a bad signature, a payload that does not decrypt
/// or an operation on an unknown collection. Key generations 0 and 1, and
/// members 0 to 4, so that many changes wait for others.
fn change(author: u8) -> BoxedStrategy<Vec<u8>> {
    let sequence = 1..6u64;
    let generation = 0..2u8;
    prop_oneof![
        (sequence.clone(), 0..5u8, generation.clone())
            .prop_map(move |(s, m, g)| membership(author, s, m, g)),
        (sequence.clone(), generation.clone()).prop_map(move |(s, g)| envelope(author, s, g)),
        (sequence, generation, 0..4u8).prop_map(move |(s, g, variant)| {
            let g = u64::from(g);
            match variant {
                0 => frame(
                    sealed_header(author, s, g, payload(vec![set_title(s as u8)])),
                    BAD,
                ),
                1 => frame(sealed_header(author, s, g, GARBLED.to_vec()), GOOD),
                _ => sealed(author, s, g, vec![set_title(s as u8), set_title(author)]),
            }
        }),
    ]
    .boxed()
}

#[test]
fn d11_the_final_states_depend_only_on_the_set_of_changes() {
    let config = Config {
        members: 4,
        max_changes_per_member: 5,
        ..Config::default()
    };
    let result = convergence::check::<Copy, _>(&config, &independent(change));
    assert!(result.is_ok(), "{}", result.unwrap_err());
}
