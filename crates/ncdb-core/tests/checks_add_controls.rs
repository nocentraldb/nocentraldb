//! `SealChecks::add_controls` (NOC-438): a reopened copy hands its checks
//! every folded control change of its log at once. The checks then resolve
//! the set once (RFC 0008, "The function"), instead of once after each
//! change, which made opening a space with many persons cost the square of
//! its control changes. What the checks know afterwards is what adding the
//! same changes one by one gives: the control state, each generation's
//! space key, the model and the wrong envelopes.

mod support;

#[path = "support/member.rs"]
mod member;
#[path = "support/space.rs"]
mod space;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeSet;

use ncdb_core::change::Change;
use ncdb_core::checks::SealChecks;
use ncdb_core::envelope;
use ncdb_core::keys::SpaceKey;
use ncdb_core::membership::{self, Entry, KeyChainEntry, PreviousKey, Role};
use ncdb_core::receive::Checks;

use space::{Space, Who, id, key, space_key};

const ALICE: Who = 1;
const BOB: Who = 2;
const CAROL: Who = 3;

/// The key of the second generation; the first is [`space_key`].
fn second_key() -> SpaceKey {
    SpaceKey::from_bytes([0x42; 32])
}

/// Alice's space with Bob, a rotation to a second generation whose key
/// chain entry gives the first generation's key, Carol added under the
/// second generation with an envelope for it only, and `persons`
/// person-key entries: every control change in the order written, and
/// the two generations.
fn space(persons: u8) -> (Space, Vec<Change>, [u64; 2]) {
    let mut space = Space::new(ALICE);
    let genesis = space.genesis.clone();
    let add_bob = space.add(ALICE, BOB, Role::Writer, &[&genesis]);
    let to_bob = space.envelope(ALICE, BOB, &[&add_bob]);
    let first = space.first_generation;
    let second = membership::generation_id(space.space, id(ALICE), &[first]);
    let chain = Entry::KeyChain(KeyChainEntry {
        generation: second,
        previous: vec![PreviousKey {
            generation: first,
            key: membership::seal_previous_key(
                &second_key(),
                space.space,
                second,
                first,
                &space_key(),
            ),
        }],
        answers: BTreeSet::new(),
    });
    let rotation = space.write(ALICE, chain, &[&to_bob]);
    let add_carol = space.add_at(ALICE, CAROL, Role::Reader, second, &[&rotation]);
    let sealed = envelope::seal(&second_key(), space.space, second, &key(CAROL).public())
        .expect("a simulated member's key takes an envelope");
    let to_carol = space.write(ALICE, Entry::KeyEnvelope(sealed), &[&add_carol]);
    let mut changes = vec![genesis, add_bob, to_bob, rotation, add_carol, to_carol];
    for person in 0..persons {
        let last = changes.last().expect("the genesis at least").clone();
        changes.push(space.person_key_at(ALICE, person, person, second, &[&last]));
    }
    (space, changes, [first, second])
}

/// `key` sealed with a fixed ephemeral key, which compares keys without
/// reading their bytes.
fn fingerprint(key: &SpaceKey, generation: u64) -> envelope::KeyEnvelope {
    let space = space(0).0.space;
    envelope::seal_with_ephemeral(key, space, generation, &key_of(BOB), [9; 32])
        .expect("a simulated member's key takes an envelope")
}

fn key_of(who: Who) -> ncdb_core::keys::MemberPublicKey {
    key(who).public()
}

/// What Carol's checks know, as far as anything outside them reads it.
fn known(checks: &SealChecks, generations: [u64; 2]) -> impl PartialEq + std::fmt::Debug {
    (
        checks.resolved().clone(),
        generations.map(|g| checks.space_key(g).map(|key| fingerprint(key, g))),
        checks.model_id(),
        checks.wrong_envelopes(),
        checks.space_model_id(),
    )
}

#[test]
fn adding_the_control_changes_at_once_gives_what_adding_them_one_by_one_gives() {
    let (space, changes, generations) = space(5);
    let mut one_by_one = SealChecks::new(space.space, key(CAROL));
    for change in &changes {
        Checks::add_control(&mut one_by_one, change);
    }
    let mut at_once = SealChecks::new(space.space, key(CAROL));

    at_once.add_controls(&changes);

    assert!(
        at_once.space_key(generations[0]).is_some(),
        "the chain opens"
    );
    assert_eq!(
        known(&at_once, generations),
        known(&one_by_one, generations)
    );
}

#[test]
fn the_checks_resolve_the_control_changes_added_at_once_once_however_many_they_are() {
    for persons in [5, 50] {
        let (space, changes, _) = space(persons);
        let mut checks = SealChecks::new(space.space, key(CAROL));

        checks.add_controls(&changes);

        assert_eq!(checks.resolves(), 1, "with {persons} persons");
    }
}

#[test]
fn control_changes_the_checks_hold_already_resolve_nothing_again() {
    let (space, changes, _) = space(3);
    let mut checks = SealChecks::new(space.space, key(CAROL));
    checks.add_controls(&changes);

    checks.add_controls(&changes);
    Checks::add_control(&mut checks, &changes[2]);

    assert_eq!(checks.resolves(), 1);
}
