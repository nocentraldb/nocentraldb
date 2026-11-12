//! The value of the hybrid logical clock (RFC 0002, "The clock"): one
//! unsigned 64-bit integer, milliseconds since the Unix epoch in the upper
//! 48 bits and a counter in the lower 16. `ncdb-core` keeps the clock that
//! draws them and re-exports this value as its own.

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
