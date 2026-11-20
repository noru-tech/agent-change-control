//! UTC instants and calendar dates.
//!
//! Thin wrappers over `jiff` that pin down the wire format: input is strict RFC 3339 (any
//! offset, normalized to UTC) or a strict `YYYY-MM-DD` date, and an instant prints as RFC 3339
//! with a `Z` suffix and 0, 3, 6 or 9 fractional digits, whichever is the shortest exact form.

use jiff::{SignedDuration, civil, tz::Offset};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::fmt;
use std::str::FromStr;

/// A UTC instant. Serialized as RFC 3339 with a `Z` suffix; any offset is accepted on input.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(jiff::Timestamp);

/// A calendar date, serialized as `YYYY-MM-DD`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date(civil::Date);

/// Why a string is not a timestamp or a date.
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("not an RFC 3339 timestamp")]
    Timestamp,
    #[error("not a YYYY-MM-DD date")]
    Date,
}

const DAY: i64 = 86_400;

impl Timestamp {
    /// 1970-01-01T00:00:00Z.
    pub const UNIX_EPOCH: Self = Self(jiff::Timestamp::UNIX_EPOCH);

    /// The system clock.
    pub fn now() -> Self {
        Self(jiff::Timestamp::now())
    }

    /// The instant `secs` whole seconds after the Unix epoch, if representable.
    pub fn from_unix_seconds(secs: i64) -> Option<Self> {
        jiff::Timestamp::from_second(secs).ok().map(Self)
    }

    /// Whole seconds since the Unix epoch, rounding toward negative infinity.
    pub fn unix_seconds(self) -> i64 {
        self.0.as_second() - i64::from(self.0.subsec_nanosecond() < 0)
    }

    /// This instant with its fractional second dropped.
    pub fn trunc_seconds(self) -> Self {
        Self::from_unix_seconds(self.unix_seconds()).expect("truncation stays in range")
    }

    /// The UTC calendar date of this instant.
    pub fn date(self) -> Date {
        Date(Offset::UTC.to_datetime(self.0).date())
    }

    /// `days` whole days (of 24 hours) earlier, if representable.
    pub fn checked_sub_days(self, days: u64) -> Option<Self> {
        let secs = i64::try_from(days).ok()?.checked_mul(DAY)?;
        self.0
            .checked_sub(SignedDuration::from_secs(secs))
            .ok()
            .map(Self)
    }

    /// Whether `self` and `other` are less than `secs` seconds apart.
    pub fn within_seconds(self, other: Self, secs: i64) -> bool {
        self.0.duration_since(other.0).abs() < SignedDuration::from_secs(secs)
    }
}

impl Date {
    /// 9999-12-31, the latest date.
    pub const MAX: Self = Self(civil::Date::MAX);

    /// The date for a valid year, month and day.
    pub fn new(year: i16, month: i8, day: i8) -> Option<Self> {
        civil::Date::new(year, month, day).ok().map(Self)
    }

    /// `days` days later, if representable.
    pub fn checked_add_days(self, days: u64) -> Option<Self> {
        let days = i32::try_from(days).ok()?;
        self.0
            .checked_add(jiff::Span::new().days(days))
            .ok()
            .map(Self)
    }

    /// The first instant of this UTC day, if representable. Timestamps end at
    /// 9999-12-30T22:00:00.999999999Z, so the very last days of year 9999 have none.
    pub fn start_of_day(self) -> Option<Timestamp> {
        self.at(0, 0, 0, 0)
    }

    /// The last nanosecond of this UTC day, if representable.
    pub fn end_of_day(self) -> Option<Timestamp> {
        self.at(23, 59, 59, 999_999_999)
    }

    fn at(self, hour: i8, minute: i8, second: i8, nanos: i32) -> Option<Timestamp> {
        let dt = self.0.at(hour, minute, second, nanos);
        Offset::UTC.to_timestamp(dt).ok().map(Timestamp)
    }
}

/// `YYYY-MM-DD` with ASCII digits.
fn date_shape(b: &[u8]) -> bool {
    b.len() == 10
        && b.iter().enumerate().all(|(i, c)| match i {
            4 | 7 => *c == b'-',
            _ => c.is_ascii_digit(),
        })
}

/// RFC 3339 `date-time`: `YYYY-MM-DD`, `T` (or `t` or a space), `HH:MM:SS`, an optional
/// fraction, then `Z` (or `z`) or `±HH:MM`. Anything beyond that grammar, such as RFC 9557
/// annotations or ISO 8601 basic format, is rejected.
fn timestamp_shape(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() < 20 || !date_shape(&b[..10]) || !matches!(b[10], b'T' | b't' | b' ') {
        return false;
    }
    let digits = |r: std::ops::Range<usize>| b[r].iter().all(u8::is_ascii_digit);
    if !(digits(11..13) && b[13] == b':' && digits(14..16) && b[16] == b':' && digits(17..19)) {
        return false;
    }
    let mut i = 19;
    if b[i] == b'.' {
        let start = i + 1;
        i = start;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == start {
            return false;
        }
    }
    match &b[i..] {
        [b'Z' | b'z'] => true,
        [b'+' | b'-', h1, h2, b':', m1, m2] => [h1, h2, m1, m2].iter().all(|c| c.is_ascii_digit()),
        _ => false,
    }
}

impl FromStr for Timestamp {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        if !timestamp_shape(s) {
            return Err(ParseError::Timestamp);
        }
        s.parse().map(Self).map_err(|_| ParseError::Timestamp)
    }
}

impl FromStr for Date {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        if !date_shape(s.as_bytes()) {
            return Err(ParseError::Date);
        }
        s.parse().map(Self).map_err(|_| ParseError::Date)
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let dt = Offset::UTC.to_datetime(self.0);
        write!(
            f,
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
            dt.year(),
            dt.month(),
            dt.day(),
            dt.hour(),
            dt.minute(),
            dt.second()
        )?;
        match dt.subsec_nanosecond() {
            0 => {}
            n if n % 1_000_000 == 0 => write!(f, ".{:03}", n / 1_000_000)?,
            n if n % 1_000 == 0 => write!(f, ".{:06}", n / 1_000)?,
            n => write!(f, ".{n:09}")?,
        }
        f.write_str("Z")
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:04}-{:02}-{:02}",
            self.0.year(),
            self.0.month(),
            self.0.day()
        )
    }
}

impl fmt::Debug for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl fmt::Debug for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

macro_rules! serde_via_str {
    ($t:ty, $expecting:literal) => {
        impl Serialize for $t {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.collect_str(self)
            }
        }

        impl<'de> Deserialize<'de> for $t {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                struct V;
                impl de::Visitor<'_> for V {
                    type Value = $t;
                    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                        f.write_str($expecting)
                    }
                    fn visit_str<E: de::Error>(self, v: &str) -> Result<$t, E> {
                        v.parse().map_err(E::custom)
                    }
                }
                d.deserialize_str(V)
            }
        }
    };
}

serde_via_str!(Timestamp, "an RFC 3339 timestamp");
serde_via_str!(Date, "a YYYY-MM-DD date");

#[cfg(test)]
mod tests {
    use super::*;

    fn ts(s: &str) -> Timestamp {
        s.parse().unwrap()
    }

    #[test]
    fn timestamps_accept_rfc3339_only() {
        for good in [
            "2026-08-01T12:00:00Z",
            "2026-08-01t12:00:00z",
            "2026-08-01 12:00:00Z",
            "2026-08-01T14:00:00+02:00",
            "2026-08-01T12:00:00.5-00:00",
        ] {
            assert!(good.parse::<Timestamp>().is_ok(), "{good}");
        }
        for bad in [
            "",
            "2026-08-01",
            "2026-08-01T12:00:00",
            "2026-08-01T12:00Z",
            "2026-08-01T12:00:00Z[UTC]",
            "20260801T120000Z",
            "2026-08-01T12:00:00.Z",
            "2026-08-01T12:00:00+0200",
            "2026-02-30T12:00:00Z",
            "2026-08-01T25:00:00Z",
        ] {
            assert!(bad.parse::<Timestamp>().is_err(), "{bad}");
        }
    }

    #[test]
    fn timestamps_print_like_autosi_with_z() {
        for (input, shown) in [
            ("2026-08-01T14:00:00+02:00", "2026-08-01T12:00:00Z"),
            ("2026-08-01T12:00:00.5Z", "2026-08-01T12:00:00.500Z"),
            ("2026-08-01T12:00:00.1234Z", "2026-08-01T12:00:00.123400Z"),
            (
                "2026-08-01T12:00:00.1234567Z",
                "2026-08-01T12:00:00.123456700Z",
            ),
            ("1969-12-31T23:59:59.5Z", "1969-12-31T23:59:59.500Z"),
        ] {
            assert_eq!(ts(input).to_string(), shown);
            assert_eq!(
                serde_json::to_string(&ts(input)).unwrap(),
                format!("\"{shown}\"")
            );
        }
    }

    #[test]
    fn dates_accept_yyyy_mm_dd_only() {
        assert_eq!(
            "2026-09-30".parse::<Date>().unwrap(),
            Date::new(2026, 9, 30).unwrap()
        );
        for bad in [
            "",
            "2026-9-30",
            " 2026-09-30",
            "2026-02-30",
            "2026-09-30T00:00:00",
        ] {
            assert!(bad.parse::<Date>().is_err(), "{bad}");
        }
        assert!(serde_json::from_str::<Date>("\"2026-09-30T00:00:00\"").is_err());
    }

    #[test]
    fn arithmetic_and_truncation() {
        let t = ts("2026-08-31T12:00:00.75Z");
        assert_eq!(t.trunc_seconds().to_string(), "2026-08-31T12:00:00Z");
        assert_eq!(ts("1969-12-31T23:59:59.5Z").unix_seconds(), -1);
        assert_eq!(
            t.checked_sub_days(30).unwrap().to_string(),
            "2026-08-01T12:00:00.750Z"
        );
        assert_eq!(t.date().to_string(), "2026-08-31");
        assert_eq!(
            t.date().checked_add_days(1).unwrap().to_string(),
            "2026-09-01"
        );
        assert!(Date::new(9999, 12, 31).unwrap().end_of_day().is_none());
        assert_eq!(
            t.date().end_of_day().unwrap().to_string(),
            "2026-08-31T23:59:59.999999999Z"
        );
        assert!(t.within_seconds(ts("2026-08-31T12:00:01.5Z"), 1));
        assert!(!t.within_seconds(ts("2026-08-31T12:00:01.75Z"), 1));
    }
}
