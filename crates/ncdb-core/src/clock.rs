//! The hybrid logical clock (RFC 0002, "The clock").
//!
//! One unsigned 64-bit integer: the upper 48 bits hold milliseconds since
//! the Unix epoch, the lower 16 bits a counter. A member writes with the
//! larger of its wall clock and one more than the highest clock it has seen
//! in the space, so its clock never runs backwards and is higher than every
//! clock it has seen. When the counter would overflow, it waits for its wall
//! clock to reach the next millisecond.

/// A hybrid logical clock value, compared as a number.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Hlc(pub u64);

impl Hlc {
    /// The largest number of milliseconds the clock holds: 48 bits.
    pub const MAX_MILLIS: u64 = (1 << 48) - 1;
    /// The largest counter: 16 bits.
    pub const MAX_COUNTER: u16 = u16::MAX;

    /// The clock of these milliseconds and counter, or `None` when the
    /// milliseconds do not fit 48 bits.
    pub fn new(millis: u64, counter: u16) -> Option<Hlc> {
        (millis <= Hlc::MAX_MILLIS).then(|| Hlc(millis << 16 | u64::from(counter)))
    }

    /// The milliseconds since the Unix epoch: the upper 48 bits.
    pub fn millis(self) -> u64 {
        self.0 >> 16
    }

    /// The counter: the lower 16 bits.
    pub fn counter(self) -> u16 {
        u16::try_from(self.0 & 0xffff).expect("16 bits")
    }
}

/// The highest clock seen is the largest a clock can hold, so no later
/// clock exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClockExhausted;

impl std::fmt::Display for ClockExhausted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the clock has reached its largest value")
    }
}

impl std::error::Error for ClockExhausted {}

/// A member's clock in one space: the highest clock it has seen.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Clock {
    highest: Hlc,
}

impl Clock {
    /// A clock that has seen nothing yet.
    pub fn new() -> Clock {
        Clock::default()
    }

    /// The highest clock seen or written so far.
    pub fn highest(&self) -> Hlc {
        self.highest
    }

    /// Raises the highest clock seen to that of a received change.
    pub fn observe(&mut self, seen: Hlc) {
        self.highest = self.highest.max(seen);
    }

    /// The clock for a change this member writes now: the larger of the
    /// wall clock (`wall`, in milliseconds since the Unix epoch) and one
    /// more than the highest clock seen.
    pub fn now(&mut self, _wall: u64) -> Result<Hlc, ClockExhausted> {
        todo!()
    }
}
