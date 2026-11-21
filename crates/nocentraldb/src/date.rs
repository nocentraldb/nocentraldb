//! A day and a point in time, the values of `nc.date()` and `nc.timestamp()`
//! fields (RFC 0005, "Days and points in time"; Carsten, 2026-11-21,
//! NOC-434): the SDK's own types, so that its API follows no date crate's
//! major versions. Both range over the years 1 to 9999 of the proleptic
//! Gregorian calendar, as RFC 0004 ("Days and points in time") fixes it.

use std::fmt;
use std::str::FromStr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ncdb_core::basic::{DAYS, MILLISECONDS};

use crate::error::{Error, ErrorCode};

/// Milliseconds in a day: a point in time counts every day as 86,400,000
/// milliseconds, as Unix time does.
const DAY: i64 = 86_400_000;

/// A calendar day, with no time and no time zone, such as a birthday: the
/// same day wherever it is read. Made with [`Date::new`] or parsed from
/// `"YYYY-MM-DD"`, which is also how it prints; written with `Value::from`,
/// read with `get::<Date>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date {
    /// Days since 1970-01-01, in [`DAYS`].
    days: i64,
}

/// A point in time to the millisecond, in UTC: one absolute moment, which
/// each reader shows in its own time zone. Made with
/// [`Timestamp::from_millis`] or from a `SystemTime`; prints as RFC 3339,
/// `"2026-11-21T19:08:44.636Z"`; written with `Value::from`, read with
/// `get::<Timestamp>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp {
    /// Milliseconds since 1970-01-01T00:00:00.000Z, in [`MILLISECONDS`].
    millis: i64,
}

fn not_a_day() -> Error {
    Error::new(ErrorCode::InvalidValue, crate::texts::error::NOT_ADAY)
}

fn not_a_timestamp() -> Error {
    Error::new(ErrorCode::InvalidValue, crate::texts::error::NOT_ATIMESTAMP)
}

impl Date {
    /// The day `day` of the month `month` (1 to 12) of `year` (1 to 9999).
    /// A day that does not exist, such as 2026-02-30, is `InvalidValue`.
    pub fn new(year: i32, month: u8, day: u8) -> Result<Date, Error> {
        let (y, m, d) = (i64::from(year), i64::from(month), i64::from(day));
        if !(1..=9999).contains(&y) || !(1..=12).contains(&m) || d < 1 {
            return Err(not_a_day());
        }
        let date = Date {
            days: days_from_civil(y, m, d),
        };
        // A day past the month's end lands in the next month.
        match date.civil() == (y, m, d) {
            true => Ok(date),
            false => Err(not_a_day()),
        }
    }

    /// The day `days` after 1970-01-01, when it lies in the years 1 to 9999.
    pub(crate) fn from_days(days: i64) -> Option<Date> {
        DAYS.contains(&days).then_some(Date { days })
    }

    /// Days since 1970-01-01, as a change carries the day.
    pub(crate) fn days(self) -> i64 {
        self.days
    }

    /// The year, 1 to 9999.
    pub fn year(self) -> i32 {
        self.civil().0 as i32
    }

    /// The month, 1 to 12.
    pub fn month(self) -> u8 {
        self.civil().1 as u8
    }

    /// The day of the month, 1 to 31.
    pub fn day(self) -> u8 {
        self.civil().2 as u8
    }

    fn civil(self) -> (i64, i64, i64) {
        civil_from_days(self.days)
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (y, m, d) = self.civil();
        write!(f, "{y:04}-{m:02}-{d:02}")
    }
}

impl FromStr for Date {
    type Err = Error;

    /// Parses exactly `YYYY-MM-DD`, as the day prints.
    fn from_str(text: &str) -> Result<Date, Error> {
        let bytes = text.as_bytes();
        let digits = |range: std::ops::Range<usize>| {
            bytes[range.clone()]
                .iter()
                .all(u8::is_ascii_digit)
                .then(|| text[range].parse::<u32>().ok())
                .flatten()
        };
        if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
            return Err(not_a_day());
        }
        match (digits(0..4), digits(5..7), digits(8..10)) {
            (Some(y), Some(m), Some(d)) => Date::new(y as i32, m as u8, d as u8),
            _ => Err(not_a_day()),
        }
    }
}

impl Timestamp {
    /// The point in time `millis` milliseconds after
    /// 1970-01-01T00:00:00.000Z; one outside the years 1 to 9999 is
    /// `InvalidValue`.
    pub fn from_millis(millis: i64) -> Result<Timestamp, Error> {
        match MILLISECONDS.contains(&millis) {
            true => Ok(Timestamp { millis }),
            false => Err(not_a_timestamp()),
        }
    }

    /// Milliseconds since 1970-01-01T00:00:00.000Z.
    pub fn as_millis(self) -> i64 {
        self.millis
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let day = Date {
            days: self.millis.div_euclid(DAY),
        };
        let ms = self.millis.rem_euclid(DAY);
        let (h, min, s, ms) = (ms / 3_600_000, ms / 60_000 % 60, ms / 1000 % 60, ms % 1000);
        write!(f, "{day}T{h:02}:{min:02}:{s:02}.{ms:03}Z")
    }
}

impl TryFrom<SystemTime> for Timestamp {
    type Error = Error;

    /// The millisecond `time` falls in, rounded down toward the past, so
    /// that both SDKs store the same value for the same instant.
    fn try_from(time: SystemTime) -> Result<Timestamp, Error> {
        let millis = match time.duration_since(UNIX_EPOCH) {
            Ok(after) => i128::try_from(after.as_millis()).map_err(|_| not_a_timestamp())?,
            Err(before) => {
                let nanos = before.duration().as_nanos();
                -i128::try_from(nanos.div_ceil(1_000_000)).map_err(|_| not_a_timestamp())?
            }
        };
        Timestamp::from_millis(i64::try_from(millis).map_err(|_| not_a_timestamp())?)
    }
}

impl TryFrom<Timestamp> for SystemTime {
    type Error = Error;

    /// The same instant as a `SystemTime`; `InvalidValue` on a platform
    /// whose `SystemTime` cannot hold it.
    fn try_from(point: Timestamp) -> Result<SystemTime, Error> {
        let span = Duration::from_millis(point.millis.unsigned_abs());
        match point.millis >= 0 {
            true => UNIX_EPOCH.checked_add(span),
            false => UNIX_EPOCH.checked_sub(span),
        }
        .ok_or_else(not_a_timestamp)
    }
}

/// Days since 1970-01-01 of a day of the proleptic Gregorian calendar
/// (Howard Hinnant, "chrono-Compatible Low-Level Date Algorithms").
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The year, month and day of `days` since 1970-01-01.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}
