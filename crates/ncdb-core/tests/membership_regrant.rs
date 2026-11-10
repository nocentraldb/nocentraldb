//! Re-grants with a fresh generation (RFC 0008, "Membership entry" and
//! step 1 of "The function", NOC-71): an add of a former member, or a set
//! role that gives the right to write to a member without it, is valid
//! when written only when its from generation is one whose key chain entry
//! its author wrote, with no key envelope to the subject for it in its
//! causal past.

#[path = "support/space.rs"]
mod space;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::change::Change;
use ncdb_core::membership::{Rejection, Role, Verdict, resolve};

use space::{Space, id};

const ALICE: u8 = 1;
const BOB: u8 = 2;
const CAROL: u8 = 3;

fn not_valid() -> Option<&'static Verdict> {
    Some(&Verdict::Rejected(Rejection::NotValidWhenWritten))
}

/// Alice adds Bob as a writer and Carol as an admin, then removes Bob.
struct Removed {
    s: Space,
    changes: Vec<Change>,
}

impl Removed {
    fn new() -> Removed {
        let mut s = Space::new(ALICE);
        let g = s.genesis.clone();
        let bob = s.add(ALICE, BOB, Role::Writer, &[&g]);
        let carol = s.add(ALICE, CAROL, Role::Admin, &[&bob]);
        let cutoff = s.sequence(BOB);
        let remove = s.remove(ALICE, BOB, cutoff, &[&carol]);
        Removed {
            s,
            changes: vec![g, bob, carol, remove],
        }
    }

    fn last(&self) -> Change {
        self.changes.last().unwrap().clone()
    }

    fn first(&self) -> u64 {
        self.s.first_generation
    }
}

#[test]
fn a_former_member_is_added_again_under_a_fresh_generation() {
    let mut r = Removed::new();
    let remove = r.last();
    let (rotation, two) = r.s.rotate(ALICE, &[r.first()], &[&remove]);
    let again = r.s.add_at(ALICE, BOB, Role::Writer, two, &[&rotation]);
    let envelope = r.s.envelope_at(ALICE, BOB, two, &[&again]);
    let state = resolve(
        r.s.space,
        r.changes.iter().chain([&rotation, &again, &envelope]),
    );
    assert_eq!(state.verdict(again.id()), Some(&Verdict::Accepted));
    assert_eq!(state.verdict(envelope.id()), Some(&Verdict::Accepted));
    assert_eq!(state.role(id(BOB)), Some(Role::Writer));
}

#[test]
fn a_former_member_added_again_under_an_old_generation_is_not_valid() {
    let mut r = Removed::new();
    let remove = r.last();
    let first = r.first();
    let again = r.s.add_at(ALICE, BOB, Role::Writer, first, &[&remove]);
    let state = resolve(r.s.space, r.changes.iter().chain([&again]));
    assert_eq!(state.verdict(again.id()), not_valid());
    assert_eq!(state.role(id(BOB)), None);
}

#[test]
fn a_generation_another_admin_rotated_to_is_not_fresh() {
    let mut r = Removed::new();
    let remove = r.last();
    let (by_carol, two) = r.s.rotate(CAROL, &[r.first()], &[&remove]);
    let again = r.s.add_at(ALICE, BOB, Role::Writer, two, &[&by_carol]);
    let state = resolve(r.s.space, r.changes.iter().chain([&by_carol, &again]));
    assert_eq!(state.verdict(by_carol.id()), Some(&Verdict::Accepted));
    assert_eq!(state.verdict(again.id()), not_valid());
}

#[test]
fn a_generation_with_an_envelope_to_the_subject_in_the_past_is_not_fresh() {
    let mut r = Removed::new();
    let remove = r.last();
    let (rotation, two) = r.s.rotate(ALICE, &[r.first()], &[&remove]);
    // Carol wraps generation 2 to Bob, who is removed, and Alice sees it.
    let leaked = r.s.envelope_at(CAROL, BOB, two, &[&rotation]);
    let again = r.s.add_at(ALICE, BOB, Role::Writer, two, &[&leaked]);
    let state = resolve(
        r.s.space,
        r.changes.iter().chain([&rotation, &leaked, &again]),
    );
    assert_eq!(state.verdict(leaked.id()), not_valid(), "Bob is removed");
    assert_eq!(state.verdict(again.id()), not_valid());
}

#[test]
fn a_concurrent_envelope_by_another_admin_leaves_the_generation_fresh() {
    let mut r = Removed::new();
    let remove = r.last();
    let (rotation, two) = r.s.rotate(ALICE, &[r.first()], &[&remove]);
    let again = r.s.add_at(ALICE, BOB, Role::Writer, two, &[&rotation]);
    // Carol wraps generation 2 to Bob without having seen the re-add.
    let concurrent = r.s.envelope_at(CAROL, BOB, two, &[&rotation]);
    let state = resolve(
        r.s.space,
        r.changes.iter().chain([&rotation, &again, &concurrent]),
    );
    assert_eq!(state.verdict(again.id()), Some(&Verdict::Accepted));
    assert_eq!(state.verdict(concurrent.id()), not_valid());
    assert_eq!(state.role(id(BOB)), Some(Role::Writer));
}

#[test]
fn a_reader_raised_to_writer_needs_a_fresh_generation() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let first = s.first_generation;
    let bob = s.add(ALICE, BOB, Role::Writer, &[&g]);
    let cutoff = s.sequence(BOB);
    let demote = s.set_role(ALICE, BOB, Role::Reader, cutoff, &[&bob]);
    let stale = s.set_role_at(ALICE, BOB, Role::Writer, first, cutoff, &[&demote]);
    let (rotation, two) = s.rotate(ALICE, &[first], &[&demote]);
    let raise = s.set_role_at(ALICE, BOB, Role::Writer, two, cutoff, &[&rotation]);
    let state = resolve(s.space, [&g, &bob, &demote, &stale, &rotation, &raise]);
    assert_eq!(state.verdict(stale.id()), not_valid());
    assert_eq!(state.verdict(raise.id()), Some(&Verdict::Accepted));
    assert_eq!(state.role(id(BOB)), Some(Role::Writer));
}

#[test]
fn a_member_added_as_reader_is_raised_to_writer_under_a_fresh_generation() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let first = s.first_generation;
    let bob = s.add(ALICE, BOB, Role::Reader, &[&g]);
    let stale = s.set_role_at(ALICE, BOB, Role::Writer, first, 0, &[&bob]);
    let (rotation, two) = s.rotate(ALICE, &[first], &[&bob]);
    let raise = s.set_role_at(ALICE, BOB, Role::Admin, two, 0, &[&rotation]);
    let state = resolve(s.space, [&g, &bob, &stale, &rotation, &raise]);
    assert_eq!(state.verdict(stale.id()), not_valid());
    assert_eq!(state.verdict(raise.id()), Some(&Verdict::Accepted));
    assert_eq!(state.role(id(BOB)), Some(Role::Admin));
}

#[test]
fn a_writer_raised_to_admin_is_no_re_grant() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let first = s.first_generation;
    let bob = s.add(ALICE, BOB, Role::Writer, &[&g]);
    let raise = s.set_role_at(ALICE, BOB, Role::Admin, first, 0, &[&bob]);
    let state = resolve(s.space, [&g, &bob, &raise]);
    assert_eq!(state.verdict(raise.id()), Some(&Verdict::Accepted));
}
