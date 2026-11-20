//! RFC 0008's M-C5, "Local checks never split the state", for a key
//! envelope that holds a wrong key (NOC-382). Whether an envelope holds the
//! right key is a local check ("What the fold may read"): the control state
//! does not change, and the member that can tell reports it. A member's own
//! verdict on a sealed change must still be a function of the set of
//! changes it holds, not of the order in which they arrived (principle 2;
//! RFC 0002, "Rejected changes").
//!
//! Here Carol holds Alice's envelope with the right key of the first
//! generation and a second envelope from Alice for the same generation that
//! holds a wrong key, and a sealed change of Alice's under that
//! generation. In every order of arrival she must end with the same state
//! for the sealed change.

mod support;

#[path = "support/member.rs"]
mod member;
#[path = "support/space.rs"]
mod space;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::change::Change;
use ncdb_core::checks::SealChecks;
use ncdb_core::envelope;
use ncdb_core::keys::SpaceKey;
use ncdb_core::log::{Log, State};
use ncdb_core::membership::{Entry, Role};

use member::{Recorder, set_title};
use space::Space;

const ALICE: u8 = 1;
const CAROL: u8 = 3;

/// The state Carol's log ends in for `sealed` after receiving `order`.
fn state_after(space: &Space, order: &[&Change], sealed: &Change) -> Option<State> {
    let mut log = Log::new(space.space);
    let mut checks = SealChecks::new(space.space, space::key(CAROL));
    let mut fold = Recorder::default();
    for change in order {
        log.receive(change.bytes(), &mut checks, &mut fold)
            .expect("every change is appended");
    }
    log.state(sealed.id()).cloned()
}

#[test]
fn m_c5_a_wrong_key_envelope_leaves_a_sealed_change_in_one_state_in_any_order() {
    let mut s = Space::new(ALICE);
    let genesis = s.genesis.clone();
    let carol = s.add(ALICE, CAROL, Role::Reader, &[&genesis]);
    let right = s.envelope(ALICE, CAROL, &[&carol]);
    let wrong_key = SpaceKey::from_bytes([0xee; 32]);
    let sealed_wrong = envelope::seal_with_ephemeral(
        &wrong_key,
        s.space,
        s.first_generation,
        &space::key(CAROL).public(),
        [0x77; 32],
    )
    .expect("Carol's key takes an envelope");
    let wrong = s.write(ALICE, Entry::KeyEnvelope(sealed_wrong), &[&carol]);
    let sealed = s.sealed(ALICE, vec![set_title(1)]);

    let orders: [[&Change; 5]; 4] = [
        [&genesis, &carol, &right, &wrong, &sealed],
        [&genesis, &carol, &wrong, &right, &sealed],
        [&genesis, &carol, &sealed, &right, &wrong],
        [&genesis, &carol, &sealed, &wrong, &right],
    ];
    let states: Vec<Option<State>> = orders
        .iter()
        .map(|order| state_after(&s, order, &sealed))
        .collect();
    for (order, state) in states.iter().enumerate() {
        assert_eq!(
            state, &states[0],
            "order {order} ends in {state:?}, order 0 in {:?}",
            states[0]
        );
    }
}
