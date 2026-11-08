//! The hybrid logical clock of RFC 0002: 48 bits of milliseconds and a
//! 16-bit counter, never backwards and higher than every clock seen.
//!
//! When the counter is full, the clock carries into the next millisecond
//! instead of waiting for the wall clock, and at the largest value it
//! returns an error instead of wrapping. The carry is pending the owner's
//! decision ("What does the clock do when its counter is full and a clock
//! from the future was seen?"); pm recommends it.

#[path = "support/props.rs"]
mod props;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::clock::{Clock, ClockExhausted, Hlc};
use proptest::prelude::*;

fn hlc(millis: u64, counter: u16) -> Hlc {
    Hlc::new(millis, counter).unwrap()
}

#[test]
fn milliseconds_take_the_upper_48_bits_and_the_counter_the_lower_16() {
    let clock = hlc(1_793_000_000_000, 3);
    assert_eq!(clock.0, (1_793_000_000_000 << 16) | 3);
    assert_eq!(clock.millis(), 1_793_000_000_000);
    assert_eq!(clock.counter(), 3);
    assert_eq!(Hlc::new(Hlc::MAX_MILLIS, u16::MAX), Some(Hlc(u64::MAX)));
    assert_eq!(Hlc::new(Hlc::MAX_MILLIS + 1, 0), None);
    // Compared as a number: milliseconds first, then the counter.
    assert!(hlc(5, u16::MAX) < hlc(6, 0));
}

#[test]
fn a_wall_clock_ahead_of_everything_seen_is_used_as_is() {
    let mut clock = Clock::new();
    clock.observe(hlc(1000, 7));
    assert_eq!(clock.now(2000), Ok(hlc(2000, 0)));
    assert_eq!(clock.highest(), hlc(2000, 0));
}

#[test]
fn a_wall_clock_behind_counts_on_from_the_highest_seen() {
    let mut clock = Clock::new();
    clock.observe(hlc(5000, 7));
    assert_eq!(clock.now(1000), Ok(hlc(5000, 8)));
    assert_eq!(clock.now(5000), Ok(hlc(5000, 9)));
    // Observing an older clock lowers nothing.
    clock.observe(hlc(10, 0));
    assert_eq!(clock.highest(), hlc(5000, 9));
}

#[test]
fn a_full_counter_carries_into_the_next_millisecond_without_waiting() {
    let mut clock = Clock::new();
    clock.observe(hlc(1000, u16::MAX));
    // The wall clock is behind and stays behind; the clock does not wait.
    assert_eq!(clock.now(999), Ok(hlc(1001, 0)));
    assert_eq!(clock.now(999), Ok(hlc(1001, 1)));
    // A clock seen from far in the future with a full counter.
    let mut clock = Clock::new();
    clock.observe(hlc(Hlc::MAX_MILLIS - 1, u16::MAX));
    assert_eq!(clock.now(1000), Ok(hlc(Hlc::MAX_MILLIS, 0)));
}

#[test]
fn the_largest_clock_is_an_error_not_a_wrap() {
    let mut clock = Clock::new();
    clock.observe(Hlc(u64::MAX - 1));
    assert_eq!(clock.now(0), Ok(Hlc(u64::MAX)));
    assert_eq!(clock.now(0), Err(ClockExhausted));
    assert_eq!(clock.now(u64::MAX), Err(ClockExhausted));
    assert_eq!(clock.highest(), Hlc(u64::MAX));

    let mut clock = Clock::new();
    clock.observe(Hlc(u64::MAX));
    assert_eq!(clock.now(1_793_000_000_000), Err(ClockExhausted));
    assert_eq!(
        ClockExhausted.to_string(),
        "the clock has reached its largest value"
    );
}

#[test]
fn a_wall_clock_beyond_48_bits_reads_as_the_largest_millisecond() {
    let mut clock = Clock::new();
    assert_eq!(clock.now(u64::MAX), Ok(hlc(Hlc::MAX_MILLIS, 0)));
    assert_eq!(clock.now(u64::MAX), Ok(hlc(Hlc::MAX_MILLIS, 1)));
}

#[derive(Clone, Debug)]
enum Step {
    /// A change arrives with this clock.
    Observe(u64),
    /// The member writes while its wall clock reads this.
    Write(u64),
}

fn steps() -> impl Strategy<Value = Vec<Step>> {
    let millis = prop_oneof![0u64..2000, Just(0), Just(Hlc::MAX_MILLIS), Just(u64::MAX)];
    let counter = prop_oneof![any::<u16>(), Just(u16::MAX), Just(u16::MAX - 1)];
    let observe = prop_oneof![
        (0u64..2000, counter).prop_map(|(millis, counter)| Step::Observe(hlc(millis, counter).0)),
        (0u64..4).prop_map(|below| Step::Observe(u64::MAX - below)),
    ];
    let write = millis.prop_map(Step::Write);
    proptest::collection::vec(prop_oneof![observe, write], 1..40)
}

#[test]
fn the_clock_never_runs_backwards_and_passes_every_clock_seen() {
    props::check(1024, steps(), |steps| {
        let mut clock = Clock::new();
        let mut highest = Hlc(0);
        for step in steps {
            match step {
                Step::Observe(seen) => {
                    clock.observe(Hlc(seen));
                    highest = highest.max(Hlc(seen));
                }
                Step::Write(wall) => {
                    let wall_clock = hlc(wall.min(Hlc::MAX_MILLIS), 0);
                    match clock.now(wall) {
                        Ok(written) => {
                            prop_assert!(written > highest, "{written:?} <= {highest:?}");
                            // Exactly the RFC's rule: the larger of the wall
                            // clock and one above the highest seen.
                            prop_assert_eq!(written, wall_clock.max(Hlc(highest.0 + 1)));
                            highest = written;
                        }
                        Err(ClockExhausted) => {
                            prop_assert_eq!(highest, Hlc(u64::MAX));
                        }
                    }
                }
            }
            prop_assert_eq!(clock.highest(), highest);
        }
        Ok(())
    });
}
