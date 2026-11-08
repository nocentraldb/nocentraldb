//! A member's log checks a received change with the real signature and
//! decryption (NOC-48): `SealChecks` verifies with `seal::verify` and
//! decrypts with `seal::open`, so a change member A seals is folded on
//! member B, and a change with one bit flipped stops at the step that
//! fails, with that step's reason.

#[path = "support/member.rs"]
mod member;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeSet;

use ncdb_core::change::{Change, Header, Kind, Payload};
use ncdb_core::checks::{Membership, SealChecks};
use ncdb_core::clock::Hlc;
use ncdb_core::ids::MemberId;
use ncdb_core::keys::{MemberKey, SpaceKey};
use ncdb_core::log::{Log, ParkReason, RejectReason, State};
use ncdb_core::receive::ReceiveError;
use ncdb_core::seal;

use member::{Recorder, SPACE, set_title};

/// The key generation every test change is sealed under.
const GENERATION: u64 = 3;

/// A membership that names its members at every key generation and
/// records the control changes it was given.
#[derive(Debug, Default)]
struct Members {
    members: BTreeSet<MemberId>,
    controls: usize,
}

impl Membership for Members {
    fn is_member(&self, change: &Change) -> bool {
        self.members.contains(&change.header().author)
    }

    fn add_control(&mut self, _change: &Change) {
        self.controls += 1;
    }
}

fn member_a() -> MemberKey {
    MemberKey::from_secrets([1; 32], [2; 32])
}

fn space_key() -> SpaceKey {
    SpaceKey::from_bytes([0x5a; 32])
}

/// A sealed change by A with two operations, as member B receives it.
fn sealed_by_a() -> Change {
    let payload = Payload::new(Hlc(1), BTreeSet::new(), vec![set_title(1), set_title(2)]);
    seal::seal(&member_a(), &space_key(), SPACE, 1, GENERATION, &payload)
}

/// Member B: it knows A's public keys and the space key of [`GENERATION`],
/// and A is a member.
fn member_b() -> SealChecks<Members> {
    let a = member_a();
    let mut checks = SealChecks::new(Members {
        members: [a.id()].into(),
        controls: 0,
    });
    checks.add_member(a.public());
    checks.add_space_key(GENERATION, space_key());
    checks
}

/// What B's log does with these frame bytes.
fn receive(checks: &mut SealChecks<Members>, bytes: &[u8]) -> Result<State, ReceiveError> {
    let mut log = Log::new(SPACE);
    let mut fold = Recorder::default();
    log.receive(bytes, checks, &mut fold)?;
    let change = Change::decode(bytes).expect("an appended change decodes");
    let state = log.state(change.id()).cloned().expect("a received change has a state");
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
    let change = sealed_by_a();
    let mut log = Log::new(SPACE);
    let mut fold = Recorder::default();
    log.receive(change.bytes(), &mut member_b(), &mut fold)
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
    let b = member_b();
    assert!(b.knows_suite(seal::SUITE));
    assert!(!b.knows_suite(2));
}

#[test]
fn an_author_whose_keys_b_does_not_know_is_rejected_at_step_3() {
    let mut b = SealChecks::new(Members {
        members: [member_a().id()].into(),
        controls: 0,
    });
    b.add_space_key(GENERATION, space_key());
    assert_eq!(
        receive(&mut b, sealed_by_a().bytes()),
        rejected(RejectReason::UnknownAuthor)
    );
}

#[test]
fn a_change_signed_by_another_key_is_rejected_at_step_3() {
    let mallory = MemberKey::from_secrets([7; 32], [8; 32]);
    // Mallory signs A's header: the author is A, the key is not A's.
    let forged = seal::sign(&mallory, sealed_by_a().header().clone());
    assert_eq!(
        receive(&mut member_b(), forged.bytes()),
        rejected(RejectReason::BadSignature)
    );
}

#[test]
fn an_author_who_is_not_a_member_is_rejected_at_step_4() {
    let mut b = member_b();
    let mut outsider = SealChecks::new(Members::default());
    outsider.add_member(member_a().public());
    outsider.add_space_key(GENERATION, space_key());
    assert_eq!(
        receive(&mut outsider, sealed_by_a().bytes()),
        rejected(RejectReason::NotMember)
    );
    assert!(matches!(
        receive(&mut b, sealed_by_a().bytes()),
        Ok(State::Folded { .. })
    ));
}

#[test]
fn a_control_change_that_passes_goes_to_the_membership() {
    let a = member_a();
    let header = Header::new(
        SPACE,
        a.id(),
        1,
        GENERATION,
        Kind::CONTROL,
        vec![0xa0],
        seal::SUITE,
    );
    let control = seal::sign(&a, header);
    let mut b = member_b();
    assert!(matches!(
        receive(&mut b, control.bytes()),
        Ok(State::Folded { .. })
    ));
    assert_eq!(b.membership().controls, 1);
}

#[test]
fn a_change_without_the_space_key_of_its_generation_is_rejected_at_step_5_as_no_key() {
    let mut b = SealChecks::new(Members {
        members: [member_a().id()].into(),
        controls: 0,
    });
    b.add_member(member_a().public());
    b.add_space_key(GENERATION + 1, space_key());
    assert_eq!(
        receive(&mut b, sealed_by_a().bytes()),
        rejected(RejectReason::NoKey)
    );
}

#[test]
fn a_change_under_another_space_key_is_rejected_at_step_5_as_undecryptable() {
    let mut b = SealChecks::new(Members {
        members: [member_a().id()].into(),
        controls: 0,
    });
    b.add_member(member_a().public());
    b.add_space_key(GENERATION, SpaceKey::from_bytes([0xa5; 32]));
    assert_eq!(
        receive(&mut b, sealed_by_a().bytes()),
        rejected(RejectReason::Undecryptable)
    );
}

/// What B must do with a tampered frame: stop at the first step the
/// tampered change fails, as RFC 0002 Amendment 1 orders them.
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
    let change = sealed_by_a();
    let frame = change.bytes();
    let mut seen = BTreeSet::new();
    // One bit per byte, a different bit in each of eight neighbours, so
    // every byte and every bit position is covered.
    for byte in 0..frame.len() {
        let bit = byte % 8;
        let mut tampered = frame.to_vec();
        tampered[byte] ^= 1 << bit;
        let outcome = receive(&mut member_b(), &tampered);
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
