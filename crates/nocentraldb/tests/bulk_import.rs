//! The bulk import measurement (NOC-414, `examples/bulk_import`), run small:
//! 2,000 records, 50 creates to a change, through the `ncdb-relay` binary
//! on SQLite, and a second device of the same account that catches up, as
//! a new device and after both were offline.
//! The measurement at the size of docs/bulk-import.md, 81,000 records,
//! takes minutes and runs by hand, as that guide says; this test keeps the
//! example working.

#[path = "support/example.rs"]
mod example;

use example::measure;

#[test]
fn a_small_import_reaches_the_relay_and_a_second_device_catches_up() {
    let line = measure(&["--operations", "2000", "--batch", "50"]);
    assert_eq!(line("scenario:"), "a new device joins after the burst");
    assert_eq!(line("changes written:"), "40");
    assert_eq!(line("items it reads:"), "2000");
    // 40 changes of the import, the space's genesis and its envelope, and
    // the account space's own changes.
    let entries: u64 = line("relay log entries:").parse().expect("a number");
    assert!(entries >= 42, "{entries} relay log entries");
    assert_eq!(line("frames through an upload:"), "0");
}

/// Island mode (GENESIS.md): the second device joins first and goes
/// offline; the first writes the burst offline, then reconnects and pushes
/// it; then the second comes back and pulls it.
#[test]
fn a_burst_written_offline_reaches_a_device_that_was_offline_too() {
    let line = measure(&["--operations", "2000", "--batch", "50", "--island"]);
    assert_eq!(line("scenario:"), "both devices offline during the burst");
    assert_eq!(line("changes written:"), "40");
    assert_eq!(line("items it reads:"), "2000");
}
