//! What a space's status reads of its log costs no pass over the log
//! (NOC-439): the refused changes of an author, the equivocations and the
//! newest model the parked data needs. The SDK reads them after every
//! write, for each space whose status an app follows, as every space
//! handle of the TypeScript SDK does; read by passes over the log, a write
//! of one object at 30,000 changes cost 2.6 times what it cost at 2,000 in
//! Chromium.
//!
//! The test compares the reads on a log of `LARGE` changes with the same
//! reads on one of `SMALL`, in the same process, so the machine's speed
//! cancels out: a pass over the log would make them about `LARGE / SMALL`
//! times as slow. Natively only: wasm32 has no clock for `Instant`.
#![cfg(not(target_arch = "wasm32"))]

#[path = "support/member.rs"]
mod member;

use std::time::{Duration, Instant};

use ncdb_core::log::RelayRefusal;

use member::*;

const SMALL: u64 = 1_000;
const LARGE: u64 = 32_000;

/// How many times slower the reads on the large log may be.
const FACTOR: u32 = 4;

/// A member holding `changes` sealed changes of `FOUNDER`, the first of
/// them refused by the relay, so that each read has something to find.
fn holding(changes: u64) -> Member {
    let mut m = Member::default();
    m.receive(&envelope(FOUNDER, 1, 0));
    for sequence in 2..changes + 2 {
        let change = m.receive(&sealed(FOUNDER, sequence, 0, vec![set_title(1)]));
        if sequence == 2 {
            m.log
                .record_relay_refusal(change.id(), RelayRefusal("membership_rule".into()));
        }
    }
    m
}

/// The fastest of several rounds of the status reads on `m`'s log.
fn status_reads(m: &Member) -> Duration {
    (0..7)
        .map(|_| {
            let start = Instant::now();
            for _ in 0..50 {
                assert_eq!(m.log.refused_by(member(FOUNDER)).count(), 1);
                assert!(m.log.equivocations().is_empty());
                assert_eq!(m.log.newest_model(&m.control, MODEL), None);
            }
            start.elapsed()
        })
        .min()
        .expect("seven rounds")
}

#[test]
fn a_status_read_costs_no_more_on_a_long_log_than_on_a_short_one() {
    let small = status_reads(&holding(SMALL));
    let large = status_reads(&holding(LARGE));

    assert!(
        large <= small * FACTOR + Duration::from_millis(1),
        "the status reads took {large:?} on {LARGE} changes and {small:?} on {SMALL}"
    );
}
