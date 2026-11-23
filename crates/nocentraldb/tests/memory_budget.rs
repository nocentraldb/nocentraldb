//! The memory budget of one change (NOC-479; Carsten, 2026-11-23, NOC-451):
//! a change at the relay's 16 MiB limit fits in 256 MiB of core memory.
//! Natively, the guard writes one change just below the limit with the
//! bulk import measurement (`examples/bulk_import`), through the
//! `ncdb-relay` binary, and a second device of the same account receives
//! it as it catches up. Each device runs in its own process, and each
//! process's peak resident memory, SQLite's pages included, must stay
//! within the budget. The same budget in a browser tab's WebAssembly
//! memory is held by scripts/memory-budget/browser.test.mjs.
//!
//! The guard runs only for releases: the `verify` job of
//! .github/workflows/release.yml runs it, for beta and stable tags and in
//! the publish dry run, with
//! `cargo test --release --locked -p nocentraldb --test memory_budget -- --ignored`.
//! It takes about a minute, which PR CI does not spend on every change. It
//! reads `/proc`, so it runs on Linux.

#[path = "support/example.rs"]
mod example;

/// The budget: 256 MiB.
const BUDGET: u64 = 256 * 1024 * 1024;

/// The relay's largest frame (RFC 0007, "Large changes").
const LIMIT: u64 = 16 * 1024 * 1024;

/// Creates of the bulk import's records in the one change: about 390
/// bytes each, so the change lands just below the limit.
const RECORDS: &str = "42000";

/// "231.0 MB" → bytes.
fn bytes(megabytes: &str) -> u64 {
    let value: f64 = megabytes
        .strip_suffix(" MB")
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| panic!("no peak memory reported: {megabytes}"));
    (value * 1_000_000.0) as u64
}

fn mib(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

#[test]
#[ignore = "release-only memory budget guard: the verify job of release.yml runs it with --ignored"]
fn a_change_at_the_16_mib_limit_fits_in_256_mib_when_written_and_when_received() {
    let line = example::measure(&["--operations", RECORDS, "--batch", RECORDS]);
    assert_eq!(line("changes written:"), "1");
    assert_eq!(
        line("items it reads:"),
        RECORDS,
        "the second device received the change"
    );
    let largest: u64 = line("largest frame:")
        .strip_suffix(" bytes")
        .and_then(|n| n.parse().ok())
        .expect("the largest frame in bytes");
    assert!(
        (LIMIT - LIMIT / 32..=LIMIT).contains(&largest),
        "the change is not at the 16 MiB limit: {largest} bytes"
    );
    let writer = bytes(&line("first device's peak RSS:"));
    let receiver = bytes(&line("second device's peak RSS:"));
    println!(
        "a change of {:.2} MiB: writer {:.0} MiB, receiver {:.0} MiB, budget {:.0} MiB",
        mib(largest),
        mib(writer),
        mib(receiver),
        mib(BUDGET)
    );
    assert!(
        writer <= BUDGET,
        "writing a change of {:.2} MiB took {:.0} MiB, above the budget of {:.0} MiB",
        mib(largest),
        mib(writer),
        mib(BUDGET)
    );
    assert!(
        receiver <= BUDGET,
        "receiving a change of {:.2} MiB took {:.0} MiB, above the budget of {:.0} MiB",
        mib(largest),
        mib(receiver),
        mib(BUDGET)
    );
}
