//! RFC 0008's M-C8, "An accepted shred stays accepted", when a holder
//! does not yet hold the whole causal past of a shred (NOC-382). "A shred
//! is final when accepted" says that once a holder accepts a shred, no
//! change that can still arrive rejects it, and that every holder reaches
//! that verdict as soon as it holds the shred and its causal past.
//!
//! Dan, a writer, shreds person 1, signed off by Carol's account, the
//! space's second account. Bob had demoted Carol to a reader before, and
//! Dan's shred depends on that demotion through Alice's acknowledgement,
//! which saw it: in the shred's whole causal past, Carol's account holds no
//! admin, so the sign-off is not valid. A holder that has the
//! acknowledgement and the shred but not yet Bob's demotion has a gap in
//! the shred's causal past. It must not accept the shred and then reject
//! it once the demotion arrives: accepting destroys the person key, which
//! cannot be undone.

mod support;

#[path = "support/space.rs"]
mod space;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::change::Change;
use ncdb_core::membership::{self, Entry, ErasureDecision, Role, Verdict, resolve};

use space::Space;

const ALICE: u8 = 1;
const BOB: u8 = 2;
const CAROL: u8 = 3;
const DAN: u8 = 4;

#[test]
fn m_c8_a_shred_accepted_over_a_gap_in_its_causal_past_is_not_rejected_when_the_gap_fills() {
    let mut s = Space::with_nonce(ALICE, [9; 16]);
    let genesis = s.genesis.clone();
    let bob = s.add(ALICE, BOB, Role::Admin, &[&genesis]);
    s.certify_by(CAROL, space::other_account(1));
    let carol = s.add(ALICE, CAROL, Role::Admin, &[&bob]);
    let dan = s.add(ALICE, DAN, Role::Writer, &[&carol]);
    let person = s.person_key(ALICE, 1, 1, &[&dan]);
    let demote_carol = s.set_role(BOB, CAROL, Role::Reader, s.sequence(CAROL), &[&person]);
    let acknowledgement = s.other(ALICE, membership::ACKNOWLEDGEMENT, &[&demote_carol]);
    let shred = s.shred(
        DAN,
        1,
        &space::other_account(1),
        &[&acknowledgement, &person],
    );
    let base = [genesis, bob, carol, dan, person];

    let with_gap = resolve(s.space, base.iter().chain([&acknowledgement, &shred]));
    let whole = resolve(
        s.space,
        base.iter().chain([&acknowledgement, &shred, &demote_carol]),
    );
    let before = with_gap.verdict(shred.id());
    let after = whole.verdict(shred.id());
    assert!(
        before != Some(&Verdict::Accepted) || after == Some(&Verdict::Accepted),
        "the shred was {before:?} while its causal past had a gap, and {after:?} once the gap filled"
    );
}

/// The same for the erasure decision that finalizes a provisional shred,
/// which destroys the person key as an accepted shred does. Erin, an admin
/// of the space's only account, finalizes Bob's provisional shred. Alice
/// had demoted Erin to a reader before, and the finalize depends on that
/// demotion through Alice's add of Frank, which saw it: in the finalize's
/// whole causal past, Erin is no admin. A holder that has the add and the
/// finalize but not yet the demotion must not accept the finalize.
#[test]
fn m_c8_a_finalize_accepted_over_a_gap_in_its_causal_past_is_not_rejected_when_the_gap_fills() {
    const ERIN: u8 = 5;
    const FRANK: u8 = 6;
    let mut s = Space::new(ALICE);
    let genesis = s.genesis.clone();
    let bob = s.add(ALICE, BOB, Role::Writer, &[&genesis]);
    let erin = s.add(ALICE, ERIN, Role::Admin, &[&bob]);
    let person = s.person_key(BOB, 1, 0xc1, &[&erin]);
    let shred = s.shred(BOB, 1, &space::account(), &[&person]);
    let demote_erin = s.set_role(ALICE, ERIN, Role::Reader, s.sequence(ERIN), &[&shred]);
    let frank = s.add(ALICE, FRANK, Role::Reader, &[&demote_erin, &shred]);
    let finalize = decide(&mut s, ERIN, &shred, &[&frank]);
    let base = [genesis, bob, erin, person, shred];

    let with_gap = resolve(s.space, base.iter().chain([&frank, &finalize]));
    let whole = resolve(
        s.space,
        base.iter().chain([&frank, &finalize, &demote_erin]),
    );
    let before = with_gap.verdict(finalize.id());
    let after = whole.verdict(finalize.id());
    assert!(
        before != Some(&Verdict::Accepted) || after == Some(&Verdict::Accepted),
        "the finalize was {before:?} while its causal past had a gap, and {after:?} once the gap filled"
    );
    let destroyed = |state: &membership::Resolved| {
        state
            .person_key(space::person(1))
            .is_some_and(|key| key.destroyed())
    };
    assert!(!destroyed(&with_gap) || destroyed(&whole));
}

fn decide(s: &mut Space, author: u8, shred: &Change, deps: &[&Change]) -> Change {
    let entry = Entry::ErasureDecision(ErasureDecision {
        shred: shred.id(),
        finalize: true,
    });
    s.write(author, entry, deps)
}
