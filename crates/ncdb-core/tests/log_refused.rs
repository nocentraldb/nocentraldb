//! The relay's refusals of one author's changes, in local order, read from
//! the log in memory (RFC 0009, "Sync and status": `refused` of
//! `space.status`, this member's changes the relay refused, by change id
//! and code).

#[path = "support/member.rs"]
mod member;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::log::RelayRefusal;

use member::*;

#[test]
fn an_authors_refused_changes_are_listed_in_local_order_with_their_codes() {
    let mut m = Member::default();
    m.receive(&envelope(FOUNDER, 1, 0));
    let first = m.receive(&sealed(FOUNDER, 2, 0, vec![set_title(1)]));
    let kept = m.receive(&sealed(FOUNDER, 3, 0, vec![set_title(2)]));
    let second = m.receive(&sealed(FOUNDER, 4, 0, vec![set_title(3)]));
    m.log
        .record_relay_refusal(second.id(), RelayRefusal("membership_rule".into()));
    m.log
        .record_relay_refusal(first.id(), RelayRefusal("bad_signature".into()));

    let refused: Vec<_> = m.log.refused_by(member(FOUNDER)).collect();

    assert_eq!(
        refused,
        [
            (first.id(), &RelayRefusal("bad_signature".into())),
            (second.id(), &RelayRefusal("membership_rule".into())),
        ]
    );
    assert!(m.log.relay_refusal(kept.id()).is_none());
    assert_eq!(m.log.refused_by(member(9)).count(), 0);
}
