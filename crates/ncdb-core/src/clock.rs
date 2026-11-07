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
    pub fn new(_millis: u64, _counter: u16) -> Option<Hlc> {
        todo!()
    }

    /// The milliseconds since the Unix epoch: the upper 48 bits.
    pub fn millis(self) -> u64 {
        todo!()
    }

    /// The counter: the lower 16 bits.
    pub fn counter(self) -> u16 {
        todo!()
    }
}

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
    pub fn observe(&mut self, _seen: Hlc) {
        todo!()
    }

    /// The clock for a change this member writes now. `wall` returns the
    /// wall clock in milliseconds since the Unix epoch; it is asked again
    /// while the counter would overflow, until it reaches the next
    /// millisecond.
    pub fn now(&mut self, _wall: impl FnMut() -> u64) -> Hlc {
        todo!()
    }
}
