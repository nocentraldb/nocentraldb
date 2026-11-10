//! The hybrid logical clock (RFC 0002, "The clock").
//!
//! One unsigned 64-bit integer: the upper 48 bits hold milliseconds since
//! the Unix epoch, the lower 16 bits a counter. A member writes with the
//! larger of its wall clock and one more than the highest clock it has seen
//! in the space, so its clock never runs backwards and is higher than every
//! clock it has seen. When the counter is full, it carries into the next
//! millisecond with counter 0, whatever the wall clock says, so a member
//! never waits (RFC 0002, "The clock", decided by the owner in NOC-25).
//!
//! A received clock more than a minute ahead of the local wall clock raises
//! a warning, [`ClockAhead`] (RFC 0002, "What the clock does not do"). The
//! change is still accepted: rejecting it on some members and not on others
//! would break convergence.

use crate::ids::MemberId;

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

/// How far a received clock may be ahead of the local wall clock before the
/// core warns: one minute, in milliseconds.
pub const AHEAD_LIMIT_MILLIS: u64 = 60_000;

/// A received clock is more than [`AHEAD_LIMIT_MILLIS`] ahead of the local
/// wall clock. It names only the author and the difference, never content,
/// keys or payload bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClockAhead {
    /// The member whose change carried the clock.
    pub author: MemberId,
    /// How many milliseconds the received clock is ahead of the local wall
    /// clock.
    pub ahead_millis: u64,
}

impl std::fmt::Display for ClockAhead {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("a change by member ")?;
        crate::redact::hex(f, &self.author.0)?;
        write!(
            f,
            " has a clock {} ms ahead of the local wall clock",
            self.ahead_millis
        )
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
    pub fn observe(&mut self, seen: Hlc) {
        self.highest = self.highest.max(seen);
    }

    /// Receives the clock of a change by `author`, while the local wall
    /// clock reads `wall` milliseconds since the Unix epoch: raises the
    /// highest clock seen exactly as [`Clock::observe`] does, warned or not.
    /// Returns a warning when the clock's milliseconds are more than a
    /// minute ahead of the wall clock; the counter does not count.
    pub fn receive(&mut self, seen: Hlc, author: MemberId, wall: u64) -> Option<ClockAhead> {
        self.observe(seen);
        let ahead_millis = seen.millis().saturating_sub(wall);
        (ahead_millis > AHEAD_LIMIT_MILLIS).then_some(ClockAhead {
            author,
            ahead_millis,
        })
    }

    /// The clock for a change this member writes now: the larger of the
    /// wall clock (`wall`, in milliseconds since the Unix epoch) and one
    /// more than the highest clock seen.
    ///
    /// When the counter is full, one more carries into the next
    /// millisecond: the clock never waits for the wall clock, which may lag
    /// a clock seen from the future for as long as that member's clock was
    /// ahead. A wall clock beyond 48 bits of milliseconds (the year 10889)
    /// reads as the largest millisecond. When the highest clock seen is
    /// already the largest a clock holds, there is no later one, and the
    /// clock returns [`ClockExhausted`] rather than wrap around.
    pub fn now(&mut self, wall: u64) -> Result<Hlc, ClockExhausted> {
        let wall = Hlc::new(wall.min(Hlc::MAX_MILLIS), 0).expect("at most 48 bits");
        let next = self.highest.0.checked_add(1).ok_or(ClockExhausted)?;
        let next = wall.max(Hlc(next));
        self.highest = next;
        Ok(next)
    }
}
