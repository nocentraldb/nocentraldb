//! The oldest own change without a relay position (RFC 0006, "What the SDK
//! reports": the durability report's `oldestUnconfirmedAt`) costs no pass
//! over the log (NOC-439). The SDK reads it after every write, since the
//! TypeScript SDK follows the durability report; read by a scan of the
//! `log` table, a write of one object grew slower with every change the
//! database held, and most in a browser, where each page the scan reads
//! comes from OPFS.
//!
//! The test compares the read on a view of `LARGE` own changes with the
//! same read on one of `SMALL`, in the same process, so the machine's speed
//! cancels out: a scan would make it about `LARGE / SMALL` times as slow.
//! Natively only: wasm32 has no clock for `Instant`.
#![cfg(not(target_arch = "wasm32"))]

mod support;

#[path = "support/space.rs"]
mod space;
#[path = "support/view.rs"]
mod view_support;

use std::time::{Duration, Instant};

use ncdb_core::checks::SealChecks;
use ncdb_core::field_op;
use ncdb_core::ids::ObjectId;
use ncdb_core::view::View;

use space::Space;
use view_support::*;

/// The member whose view this is: the space's creator.
const ME: u8 = 1;

/// 2026-11-12T08:00:00Z, in milliseconds since the Unix epoch.
const MORNING: u64 = 1_794_384_000_000;

const SMALL: u64 = 1_000;
const LARGE: u64 = 16_000;

/// How many times slower the read on the large view may be.
const FACTOR: u32 = 4;

/// A view in memory holding `changes` tasks the member wrote, none of
/// them confirmed by the relay, written in batches of 500.
fn holding(changes: u64) -> View {
    let mut space = Space::new(ME);
    let mut checks = SealChecks::new(space.space, space::key(ME));
    let mut view = View::open_in_memory(&lock()).unwrap();
    view.receive(space.genesis.bytes(), &mut checks).unwrap();
    let tasks: Vec<_> = (0..changes)
        .map(|n| {
            let mut id = [0; 16];
            id[..8].copy_from_slice(&n.to_be_bytes());
            space.sealed(ME, vec![field_op::create(ObjectId(id), TASK)])
        })
        .collect();
    for (i, batch) in tasks.chunks(500).enumerate() {
        let frames: Vec<&[u8]> = batch.iter().map(|task| task.bytes()).collect();
        view.receive_all_own(&frames, &mut checks, MORNING + i as u64)
            .unwrap();
    }
    view
}

/// The fastest of several rounds of reading the oldest unconfirmed time.
fn oldest_reads(view: &View) -> Duration {
    (0..7)
        .map(|_| {
            let start = Instant::now();
            for _ in 0..50 {
                assert_eq!(view.oldest_unconfirmed_own().unwrap(), Some(MORNING));
            }
            start.elapsed()
        })
        .min()
        .expect("seven rounds")
}

#[test]
fn the_oldest_unconfirmed_time_costs_no_more_on_a_long_log_than_on_a_short_one() {
    let small = oldest_reads(&holding(SMALL));
    let large = oldest_reads(&holding(LARGE));

    assert!(
        large <= small * FACTOR + Duration::from_millis(1),
        "the reads took {large:?} on {LARGE} changes and {small:?} on {SMALL}"
    );
}
