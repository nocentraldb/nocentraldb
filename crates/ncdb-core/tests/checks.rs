//! A member's log checks a received change with the real signature and
//! decryption (NOC-48): `SealChecks` verifies with `seal::verify` and
//! decrypts with `seal::open`, so a change member A seals is folded on
//! member B, and a change with one bit flipped stops at the step that
//! fails, with that step's reason. B learns A's keys, the membership and
//! the space key from control changes (NOC-78).

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

use ncdb_core::change::{Change, Header, Payload};
use ncdb_core::checks::SealChecks;
use ncdb_core::clock::Hlc;
use ncdb_core::keys::{MemberKey, SpaceKey};
use ncdb_core::log::{Log, ParkReason, RejectReason, State};
use ncdb_core::membership::{Role, Verdict};
use ncdb_core::receive::ReceiveError;
use ncdb_core::seal;

use member::{Recorder, set_title};
use space::Space;

const A: u8 = 1;
const B: u8 = 2;

/// The space A creates; B is a reader in it.
fn space() -> Space {
    Space::new(A)
}

/// A sealed change by A with two operations, as member B receives it.
fn sealed_by_a(space: &Space, key: &SpaceKey) -> Change {
    let payload = Payload::new(Hlc(1), BTreeSet::new(), vec![set_title(1), set_title(2)]);
    let generation = space.first_generation;
    seal::seal(&space::key(A), key, space.space, 2, generation, &payload)
}

/// The control changes that make B a reader and give it the space key.
fn controls(space: &mut Space) -> Vec<Change> {
    let genesis = space.genesis.clone();
    let add = space.add(A, B, Role::Reader, &[&genesis]);
    let envelope = space.envelope(A, B, &[&add]);
    vec![genesis, add, envelope]
}

/// Member B's checks after these control changes.
fn checks_after(space: &Space, controls: &[Change]) -> SealChecks {
    let mut checks = SealChecks::new(space.space, space::key(B));
    let mut log = Log::new(space.space);
    for control in controls {
        log.receive(control.bytes(), &mut checks, &mut Recorder::default())
            .unwrap();
    }
    checks
}

/// Member B, which holds every control change.
fn member_b(space: &mut Space) -> SealChecks {
    let controls = controls(space);
    checks_after(space, &controls)
}

/// What B's log does with these frame bytes.
fn receive(space: &Space, checks: &mut SealChecks, bytes: &[u8]) -> Result<State, ReceiveError> {
    let mut log = Log::new(space.space);
    let mut fold = Recorder::default();
    log.receive(bytes, checks, &mut fold)?;
    let change = Change::decode(bytes).expect("an appended change decodes");
    let state = log
        .state(change.id())
        .cloned()
        .expect("a received change has a state");
    if !matches!(state, State::Folded { .. }) {
        assert!(fold.folded.is_empty(), "{state:?} folded operations");
    }
    Ok(state)
}

fn rejected(reason: RejectReason) -> Result<State, ReceiveError> {
    Ok(State::Rejected(reason))
}

#[test]
fn a_change_sealed_by_a_is_folded_on_b() {
    let mut s = space();
    let change = sealed_by_a(&s, &space::space_key());
    let mut log = Log::new(s.space);
    let mut fold = Recorder::default();
    log.receive(change.bytes(), &mut member_b(&mut s), &mut fold)
        .unwrap();
    assert_eq!(
        log.state(change.id()),
        Some(&State::Folded { parked: vec![] })
    );
    let ids = [change.operation_id(0), change.operation_id(1)];
    assert_eq!(fold.folded, ids.into_iter().collect());
}

#[test]
fn b_knows_suite_1_only() {
    use ncdb_core::receive::Checks;
    let b = member_b(&mut space());
    assert!(b.knows_suite(seal::SUITE));
    assert!(!b.knows_suite(2));
}

#[test]
fn an_author_whose_keys_b_does_not_know_is_rejected_at_step_3() {
    let s = space();
    let mut b = checks_after(&s, &[]);
    assert_eq!(
        receive(&s, &mut b, sealed_by_a(&s, &space::space_key()).bytes()),
        rejected(RejectReason::UnknownAuthor)
    );
}

#[test]
fn a_change_signed_by_another_key_is_rejected_at_step_3() {
    let mut s = space();
    let mallory = MemberKey::from_secrets([7; 32], [8; 32]);
    // Mallory signs A's header: the author is A, the key is not A's.
    let header = sealed_by_a(&s, &space::space_key()).header().clone();
    let forged = seal::sign(&mallory, header);
    let mut b = member_b(&mut s);
    assert_eq!(
        receive(&s, &mut b, forged.bytes()),
        rejected(RejectReason::BadSignature)
    );
}

#[test]
fn an_author_who_is_not_a_member_is_rejected_at_step_4() {
    let mut s = space();
    // B holds A's genesis, but a change by B itself: B is only a reader.
    let mut b = member_b(&mut s);
    let by_b = s.sealed(B, vec![set_title(1)]);
    assert_eq!(
        receive(&s, &mut b, by_b.bytes()),
        rejected(RejectReason::NotMember)
    );
}

#[test]
fn a_control_change_that_passes_goes_to_the_membership() {
    let mut s = space();
    let controls = controls(&mut s);
    let b = checks_after(&s, &controls);
    for control in &controls {
        assert_eq!(b.resolved().verdict(control.id()), Some(&Verdict::Accepted));
    }
    assert_eq!(b.resolved().role(space::id(B)), Some(Role::Reader));
}

#[test]
fn a_change_without_the_space_key_of_its_generation_is_rejected_at_step_5_as_no_key() {
    let mut s = space();
    let mut controls = controls(&mut s);
    controls.pop();
    let mut b = checks_after(&s, &controls);
    assert_eq!(
        receive(&s, &mut b, sealed_by_a(&s, &space::space_key()).bytes()),
        rejected(RejectReason::NoKey)
    );
}

#[test]
fn a_change_under_another_space_key_is_rejected_at_step_5_as_undecryptable() {
    let mut s = space();
    let mut b = member_b(&mut s);
    let other = SpaceKey::from_bytes([0xa5; 32]);
    assert_eq!(
        receive(&s, &mut b, sealed_by_a(&s, &other).bytes()),
        rejected(RejectReason::Undecryptable)
    );
}

/// What B must do with a tampered frame: stop at the first step the
/// tampered change fails, as RFC 0002 orders them ("What a member does with a change it
/// receives").
fn expected(original: &Header, tampered: &[u8]) -> Result<State, ReceiveError> {
    let change = match Change::decode(tampered) {
        Ok(change) => change,
        Err(error) => return Err(ReceiveError::Decode(error)),
    };
    let header = change.header();
    if header.space != original.space {
        return Err(ReceiveError::WrongSpace);
    }
    let parked = |reason| Ok(State::Parked(reason));
    if header.suite != seal::SUITE {
        return parked(ParkReason::UnknownSuite);
    }
    if !header.kind.is_known() {
        return parked(ParkReason::UnknownKind);
    }
    if header.format != Header::FORMAT {
        return parked(ParkReason::UnknownHeaderFormat);
    }
    if header.extensions().must_understand() {
        return parked(ParkReason::UnknownHeaderKey);
    }
    if header.author != original.author {
        return rejected(RejectReason::UnknownAuthor);
    }
    // Everything else the signature covers, the ciphertext included.
    rejected(RejectReason::BadSignature)
}

#[test]
fn a_flipped_bit_anywhere_in_the_frame_stops_at_the_step_that_fails() {
    let mut s = space();
    let change = sealed_by_a(&s, &space::space_key());
    // No tampered frame passes, so none changes B's checks.
    let mut b = member_b(&mut s);
    let frame = change.bytes();
    let mut seen = BTreeSet::new();
    // One bit per byte, a different bit in each of eight neighbours, so
    // every byte and every bit position is covered.
    for byte in 0..frame.len() {
        let bit = byte % 8;
        let mut tampered = frame.to_vec();
        tampered[byte] ^= 1 << bit;
        let outcome = receive(&s, &mut b, &tampered);
        assert_eq!(
            outcome,
            expected(change.header(), &tampered),
            "bit {bit} of byte {byte} flipped"
        );
        assert!(
            !matches!(outcome, Ok(State::Folded { .. })),
            "bit {bit} of byte {byte} flipped and the change was folded"
        );
        seen.insert(kind(&outcome));
    }
    // The flips reach the decoder, step 2, and both reasons of step 3.
    for kind in ["not decoded", "parked", "unknown author", "bad signature"] {
        assert!(seen.contains(kind), "no flip ended {kind}: {seen:?}");
    }
}

/// The kind of outcome, to show that the flips reach each step.
fn kind(outcome: &Result<State, ReceiveError>) -> &'static str {
    match outcome {
        Err(ReceiveError::Decode(_)) => "not decoded",
        Err(ReceiveError::WrongSpace) => "wrong space",
        Ok(State::Parked(_)) => "parked",
        Ok(State::Rejected(RejectReason::UnknownAuthor)) => "unknown author",
        Ok(State::Rejected(RejectReason::BadSignature)) => "bad signature",
        _ => "other",
    }
}
