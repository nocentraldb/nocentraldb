//! Equivocations (RFC 0002, "Author sequence"): two different changes with
//! the same author and sequence in one space. Both are kept, and the log
//! lists every change of an equivocation, in local order, so the SDK
//! reports them (RFC 0009, "Sync and status": `equivocations`).

#[path = "support/member.rs"]
mod member;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use member::*;

/// A member that holds the key of generation 0.
fn with_key() -> Member {
    let mut m = Member::default();
    m.receive(&envelope(FOUNDER, 1, 0));
    m
}

#[test]
fn a_log_without_two_changes_under_one_sequence_has_no_equivocation() {
    let mut m = with_key();
    let first = sealed(FOUNDER, 2, 0, vec![set_title(1)]);
    m.receive(&first);
    m.receive(&first);
    m.receive(&sealed(FOUNDER, 3, 0, vec![set_title(2)]));

    assert!(m.log.equivocations().is_empty());
}

#[test]
fn two_changes_of_one_author_under_one_sequence_are_listed_in_local_order() {
    let mut m = with_key();
    let first = m.receive(&sealed(FOUNDER, 2, 0, vec![set_title(1)]));
    let other = m.receive(&sealed(FOUNDER, 3, 0, vec![set_title(3)]));
    let second = m.receive(&sealed(FOUNDER, 2, 0, vec![set_title(2)]));
    let third = m.receive(&sealed(FOUNDER, 2, 0, vec![set_title(4)]));

    assert_eq!(m.log.equivocations(), [first.id(), second.id(), third.id()]);
    assert!(!m.log.equivocations().contains(&other.id()));
}
