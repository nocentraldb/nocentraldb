//! The hybrid logical clock of RFC 0002: 48 bits of milliseconds and a
//! 16-bit counter, never backwards, higher than every clock seen, and
//! waiting for the wall clock when the counter would overflow.

#[path = "support/props.rs"]
mod props;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::cell::Cell;

use ncdb_core::clock::{Clock, Hlc};
use proptest::prelude::*;

#[test]
fn milliseconds_take_the_upper_48_bits_and_the_counter_the_lower_16() {
    let hlc = Hlc::new(1_793_000_000_000, 3).unwrap();
    assert_eq!(hlc.0, (1_793_000_000_000 << 16) | 3);
    assert_eq!(hlc.millis(), 1_793_000_000_000);
    assert_eq!(hlc.counter(), 3);
    assert_eq!(Hlc::new(Hlc::MAX_MILLIS, u16::MAX), Some(Hlc(u64::MAX)));
    assert_eq!(Hlc::new(Hlc::MAX_MILLIS + 1, 0), None);
    // Compared as a number: milliseconds first, then the counter.
    assert!(Hlc::new(5, u16::MAX).unwrap() < Hlc::new(6, 0).unwrap());
}

#[test]
fn a_wall_clock_ahead_of_everything_seen_is_used_as_is() {
    let mut clock = Clock::new();
    clock.observe(Hlc::new(1000, 7).unwrap());
    assert_eq!(clock.now(|| 2000), Hlc::new(2000, 0).unwrap());
    assert_eq!(clock.highest(), Hlc::new(2000, 0).unwrap());
}

#[test]
fn a_wall_clock_behind_counts_on_from_the_highest_seen() {
    let mut clock = Clock::new();
    clock.observe(Hlc::new(5000, 7).unwrap());
    assert_eq!(clock.now(|| 1000), Hlc::new(5000, 8).unwrap());
    assert_eq!(clock.now(|| 5000), Hlc::new(5000, 9).unwrap());
    // Observing an older clock lowers nothing.
    clock.observe(Hlc::new(10, 0).unwrap());
    assert_eq!(clock.highest(), Hlc::new(5000, 9).unwrap());
}

#[test]
fn a_full_counter_waits_for_the_next_millisecond() {
    let mut clock = Clock::new();
    clock.observe(Hlc::new(1000, u16::MAX).unwrap());
    let readings = [999, 1000, 1000, 1001];
    let calls = Cell::new(0);
    let hlc = clock.now(|| {
        let reading = readings[calls.get()];
        calls.set(calls.get() + 1);
        reading
    });
    assert_eq!(hlc, Hlc::new(1001, 0).unwrap());
    assert_eq!(calls.get(), 4, "it returned before the wall clock moved on");
}

#[derive(Clone, Debug)]
enum Step {
    /// A change arrives with this clock.
    Observe(u64),
    /// The member writes; the wall clock reads these values in turn, and
    /// then moves on past every clock seen.
    Write(Vec<u64>),
}

fn steps() -> impl Strategy<Value = Vec<Step>> {
    let millis = prop_oneof![0u64..2000, Just(0), Just(Hlc::MAX_MILLIS - 1)];
    let counter = prop_oneof![any::<u16>(), Just(u16::MAX), Just(u16::MAX - 1)];
    let observe = (millis.clone(), counter)
        .prop_map(|(millis, counter)| Step::Observe(Hlc::new(millis, counter).unwrap().0));
    let write = proptest::collection::vec(millis, 1..4).prop_map(Step::Write);
    proptest::collection::vec(prop_oneof![observe, write], 1..40)
}

#[test]
fn the_clock_never_runs_backwards_and_passes_every_clock_seen() {
    props::check(512, steps(), |steps| {
        let mut clock = Clock::new();
        let mut highest = Hlc(0);
        let mut last_written: Option<Hlc> = None;
        for step in steps {
            match step {
                Step::Observe(seen) => {
                    clock.observe(Hlc(seen));
                    highest = highest.max(Hlc(seen));
                }
                Step::Write(readings) => {
                    // After the given readings, the wall clock jumps past
                    // the highest clock, so a full counter can stop waiting.
                    let after = highest.millis() + 1;
                    let mut readings = readings.into_iter();
                    let mut last_reading = 0;
                    let hlc = clock.now(|| {
                        last_reading = readings.next().unwrap_or(after);
                        last_reading
                    });
                    prop_assert!(hlc > highest, "{hlc:?} is not above {highest:?}");
                    if let Some(last) = last_written {
                        prop_assert!(hlc > last, "{hlc:?} ran back from {last:?}");
                    }
                    prop_assert!(hlc >= Hlc::new(last_reading, 0).unwrap());
                    // Never more than the larger of the wall clock and one
                    // above the highest seen: the clock does not run ahead.
                    let wall = Hlc::new(last_reading, 0).unwrap();
                    prop_assert!(hlc == wall || hlc.0 == highest.0 + 1);
                    prop_assert_eq!(clock.highest(), hlc);
                    highest = hlc;
                    last_written = Some(hlc);
                }
            }
            prop_assert_eq!(clock.highest(), highest);
        }
        Ok(())
    });
}
