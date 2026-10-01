use std::fmt;
use std::str::FromStr;

use crate::error::ScheduleError;

fn parse_err(what: &'static str, input: &str, reason: impl Into<String>) -> ScheduleError {
    ScheduleError::Parse {
        what,
        input: input.into(),
        reason: reason.into(),
    }
}

/// A time of day, stored like the API does: seconds since local midnight.
///
/// Parses `7:30`, `07:30`, `07:30:15`, `7:30am`, `7:30 PM`, `12am`-style hours.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TimeOfDay(u32);

impl TimeOfDay {
    /// Seconds in a day; the exclusive upper bound of valid values.
    pub const SECONDS_PER_DAY: u32 = 86_400;

    /// Builds from seconds since midnight, `None` when ≥ 24 h.
    pub fn from_seconds(seconds: u32) -> Option<Self> {
        (seconds < Self::SECONDS_PER_DAY).then_some(Self(seconds))
    }

    /// Builds from hour/minute/second components.
    pub fn from_hms(hour: u32, minute: u32, second: u32) -> Option<Self> {
        (hour < 24 && minute < 60 && second < 60).then(|| Self(hour * 3600 + minute * 60 + second))
    }

    /// Seconds since local midnight (the API's `secondFromStartOfTheDay`).
    pub fn seconds(self) -> u32 {
        self.0
    }

    /// `(hour, minute, second)`.
    pub fn hms(self) -> (u32, u32, u32) {
        (self.0 / 3600, self.0 / 60 % 60, self.0 % 60)
    }
}

impl fmt::Display for TimeOfDay {
    /// `HH:MM`, or `HH:MM:SS` when seconds are non-zero.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (h, m, s) = self.hms();
        if s == 0 {
            write!(f, "{h:02}:{m:02}")
        } else {
            write!(f, "{h:02}:{m:02}:{s:02}")
        }
    }
}

impl FromStr for TimeOfDay {
    type Err = ScheduleError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let text = input.trim().to_ascii_lowercase();
        let (body, meridiem) = if let Some(b) = text.strip_suffix("am") {
            (b.trim_end(), Some(false))
        } else if let Some(b) = text.strip_suffix("pm") {
            (b.trim_end(), Some(true))
        } else {
            (text.as_str(), None)
        };
        let number = |s: &str, what: &str| -> Result<u32, ScheduleError> {
            if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) || s.len() > 2 {
                return Err(parse_err(
                    "time",
                    input,
                    format!("{what} must be 1–2 digits"),
                ));
            }
            Ok(s.parse().expect("digits"))
        };
        let parts: Vec<&str> = body.split(':').collect();
        if parts.len() > 3 || (parts.len() == 1 && meridiem.is_none()) {
            return Err(parse_err(
                "time",
                input,
                "expected HH:MM, HH:MM:SS or H[:MM]am/pm",
            ));
        }
        let mut hour = number(parts[0], "hour")?;
        let minute = parts.get(1).map_or(Ok(0), |p| number(p, "minute"))?;
        let second = parts.get(2).map_or(Ok(0), |p| number(p, "second"))?;
        if let Some(pm) = meridiem {
            if !(1..=12).contains(&hour) {
                return Err(parse_err("time", input, "12-hour clock hours are 1–12"));
            }
            hour = hour % 12 + if pm { 12 } else { 0 };
        }
        Self::from_hms(hour, minute, second)
            .ok_or_else(|| parse_err("time", input, "hour must be 0–23, minute and second 0–59"))
    }
}

/// The weekdays a schedule runs on, Sunday first (the API's order).
///
/// Parses comma/space separated names (`mon,wed`), ranges (`mon-fri`, wrapping
/// ones like `fri-mon`), and the keywords `daily`, `weekdays`, `weekends`, `none`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Days(pub [bool; 7]);

const SHORT: [&str; 7] = ["sun", "mon", "tue", "wed", "thu", "fri", "sat"];
const WEEKDAYS: [bool; 7] = [false, true, true, true, true, true, false];
const WEEKENDS: [bool; 7] = [true, false, false, false, false, false, true];

fn day_index(name: &str) -> Option<usize> {
    let name = name.to_ascii_lowercase();
    let idx = match name.as_str() {
        "sun" | "sunday" => 0,
        "mon" | "monday" => 1,
        "tue" | "tues" | "tuesday" => 2,
        "wed" | "weds" | "wednesday" => 3,
        "thu" | "thur" | "thurs" | "thursday" => 4,
        "fri" | "friday" => 5,
        "sat" | "saturday" => 6,
        _ => return None,
    };
    Some(idx)
}

impl Days {
    /// Every day.
    pub const DAILY: Days = Days([true; 7]);
    /// Monday to Friday.
    pub const WEEKDAYS: Days = Days(WEEKDAYS);
    /// Saturday and Sunday.
    pub const WEEKENDS: Days = Days(WEEKENDS);

    /// Whether no day is selected.
    pub fn is_empty(&self) -> bool {
        !self.0.iter().any(|d| *d)
    }
}

impl fmt::Display for Days {
    /// `daily`, `weekdays`, `weekends`, `none`, or e.g. `mon,wed,fri` — always re-parseable.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if *self == Self::DAILY {
            return f.write_str("daily");
        }
        if *self == Self::WEEKDAYS {
            return f.write_str("weekdays");
        }
        if *self == Self::WEEKENDS {
            return f.write_str("weekends");
        }
        if self.is_empty() {
            return f.write_str("none");
        }
        let names: Vec<&str> = (0..7).filter(|i| self.0[*i]).map(|i| SHORT[i]).collect();
        f.write_str(&names.join(","))
    }
}

impl FromStr for Days {
    type Err = ScheduleError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let mut days = [false; 7];
        let mut any_token = false;
        for token in input
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|t| !t.is_empty())
        {
            any_token = true;
            let lower = token.to_ascii_lowercase();
            match lower.as_str() {
                "daily" | "everyday" | "all" => days = [true; 7],
                "weekdays" => (0..7).for_each(|i| days[i] |= WEEKDAYS[i]),
                "weekends" => (0..7).for_each(|i| days[i] |= WEEKENDS[i]),
                "none" => {}
                _ => {
                    if let Some((from, to)) = lower.split_once('-') {
                        let (Some(a), Some(b)) = (day_index(from), day_index(to)) else {
                            return Err(parse_err(
                                "days",
                                input,
                                format!("unknown range {token:?}"),
                            ));
                        };
                        let mut i = a;
                        loop {
                            days[i] = true;
                            if i == b {
                                break;
                            }
                            i = (i + 1) % 7;
                        }
                    } else if let Some(i) = day_index(&lower) {
                        days[i] = true;
                    } else {
                        return Err(parse_err("days", input, format!("unknown day {token:?}")));
                    }
                }
            }
        }
        if !any_token {
            return Err(parse_err("days", input, "no days given"));
        }
        Ok(Days(days))
    }
}
