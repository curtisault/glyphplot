//! UTC calendar arithmetic and a strftime subset; the core reads no clock,
//! zone or tzdata (REQ-TZ-010).
//!
//! Times are milliseconds since 1970-01-01T00:00:00Z as `f64`, as in
//! JavaScript's `Date`, which also bounds them to ±8.64 × 10¹⁵ ms and
//! truncates fractions (`TimeClip`, ECMAScript §21.4.1.31).
//!
//! - Civil dates are Howard Hinnant's algorithms `days_from_civil` and
//!   `civil_from_days` ("chrono-Compatible Low-Level Date Algorithms",
//!   <https://howardhinnant.github.io/date_algorithms.html>).
//! - [`utc_ticks`], [`utc_tick_interval`] and [`Interval`] are d3-time 3.1.0's
//!   `utcTicks`, `utcTickInterval` and the UTC intervals they use
//!   (`src/ticks.js`, `src/interval.js`, `second.js`, `minute.js`,
//!   `hour.js`, `day.js` (`unixDay`), `week.js` (`utcSunday`), `month.js`,
//!   `year.js`, `millisecond.js`; ISC licence, © Mike Bostock) (REQ-TICK-020).
//! - [`utc_tick_label`] is d3-scale 4.0.2's default multi-scale tick format
//!   for `scaleUtc` (`src/time.js`, ISC), and [`strftime`] the d3-time-format
//!   4.1.0 `utcFormat` directives it needs and REQ-FMT-050 lists
//!   (`src/locale.js`, ISC).

// d3 tests `!(x < y)` so that NaN and invalid dates take the same branch; so does this port.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use crate::format::js;
use crate::ticks::{tick_step, MAX_TICKS};

/// One second in milliseconds.
pub const SECOND: f64 = 1000.0;
/// One minute in milliseconds.
pub const MINUTE: f64 = 60.0 * SECOND;
/// One hour in milliseconds.
pub const HOUR: f64 = 60.0 * MINUTE;
/// One day in milliseconds.
pub const DAY: f64 = 24.0 * HOUR;
/// One week in milliseconds.
pub const WEEK: f64 = 7.0 * DAY;
/// d3-time's nominal month, 30 days, used only to choose an interval.
pub const MONTH: f64 = 30.0 * DAY;
/// d3-time's nominal year, 365 days, used only to choose an interval.
pub const YEAR: f64 = 365.0 * DAY;

/// The largest |t| a JavaScript `Date` holds: 100,000,000 days.
const MAX_TIME: f64 = 8.64e15;

/// The zone a time scale draws its ticks and labels in. Only UTC exists
/// until offset tables arrive from the shell (REQ-TZ-020, v0.5.0); the core
/// never reads the machine's zone or tzdata, and UTC is the default
/// (REQ-TZ-010).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Zone {
    /// Coordinated Universal Time.
    #[default]
    Utc,
}

/// JavaScript's `TimeClip`: the time truncated to a whole millisecond, or
/// None outside ±8.64 × 10¹⁵ ms or when not finite.
pub fn time_clip(t: f64) -> Option<f64> {
    if !(t.abs() <= MAX_TIME) {
        return None;
    }
    Some(t.trunc() + 0.0)
}

/// Days since 1970-01-01 of a proleptic Gregorian date (month 1–12), by
/// Hinnant's `days_from_civil`.
pub fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let (m, d) = (i64::from(month), i64::from(day));
    let y = if m <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The proleptic Gregorian (year, month 1–12, day 1–31) of a day count since
/// 1970-01-01, by Hinnant's `civil_from_days`.
pub fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m as u32, d as u32)
}

/// The day of the week, 0 for Sunday, of a day count since 1970-01-01 (a
/// Thursday), by Hinnant's `weekday_from_days`.
pub fn weekday_from_days(days: i64) -> u32 {
    (days + 4).rem_euclid(7) as u32
}

/// The UTC calendar fields of an instant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Civil {
    /// The year, proleptic Gregorian; 0 is 1 BC.
    pub year: i64,
    /// The month, 1–12.
    pub month: u32,
    /// The day of the month, 1–31.
    pub day: u32,
    /// The hour, 0–23.
    pub hour: u32,
    /// The minute, 0–59.
    pub minute: u32,
    /// The second, 0–59.
    pub second: u32,
    /// The millisecond, 0–999.
    pub millisecond: u32,
    /// The day of the week, 0 for Sunday.
    pub weekday: u32,
    /// The day of the year, 1–366.
    pub yday: u32,
}

impl Civil {
    /// The UTC fields of `t` ms since the epoch, as JavaScript's
    /// `getUTC*` give them for `new Date(t)`; None for an invalid date.
    pub fn utc(t: f64) -> Option<Civil> {
        let t = time_clip(t)? as i64;
        let days = t.div_euclid(86_400_000);
        let ms = t.rem_euclid(86_400_000);
        let (year, month, day) = civil_from_days(days);
        Some(Civil {
            year,
            month,
            day,
            hour: (ms / 3_600_000) as u32,
            minute: (ms / 60_000 % 60) as u32,
            second: (ms / 1000 % 60) as u32,
            millisecond: (ms % 1000) as u32,
            weekday: weekday_from_days(days),
            yday: (days - days_from_civil(year, 1, 1) + 1) as u32,
        })
    }
}

/// The instant of a UTC date at midnight, or None outside a `Date`'s range.
fn midnight(year: i64, month: u32, day: u32) -> Option<f64> {
    if year.abs() > 300_000 {
        return None;
    }
    let t = days_from_civil(year, month, day) as f64 * DAY;
    (t.abs() <= MAX_TIME).then_some(t)
}

/// The unit of a d3-time interval.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    /// Milliseconds (`millisecond`).
    Millisecond,
    /// Seconds (`second`).
    Second,
    /// Minutes (`utcMinute`).
    Minute,
    /// Hours (`utcHour`).
    Hour,
    /// Days counted from the epoch (`unixDay`).
    Day,
    /// Weeks starting Sunday (`utcSunday`).
    Week,
    /// Months (`utcMonth`).
    Month,
    /// Years (`utcYear`).
    Year,
}

/// A d3-time UTC interval taken every `step` units, as `interval.every(step)`
/// makes it: times on the interval are those whose field is a multiple of
/// the step (seconds of the minute, minutes of the hour, hours of the day,
/// days since the epoch, months of the year, years).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Interval {
    /// The unit.
    pub unit: Unit,
    /// The step, a whole number of units, at least 1.
    pub step: f64,
}

/// d3-time's `tickIntervals`: the unit, the step and the step's nominal
/// duration, from a second to a year.
const TICK_INTERVALS: [(Unit, f64, f64); 18] = [
    (Unit::Second, 1.0, SECOND),
    (Unit::Second, 5.0, 5.0 * SECOND),
    (Unit::Second, 15.0, 15.0 * SECOND),
    (Unit::Second, 30.0, 30.0 * SECOND),
    (Unit::Minute, 1.0, MINUTE),
    (Unit::Minute, 5.0, 5.0 * MINUTE),
    (Unit::Minute, 15.0, 15.0 * MINUTE),
    (Unit::Minute, 30.0, 30.0 * MINUTE),
    (Unit::Hour, 1.0, HOUR),
    (Unit::Hour, 3.0, 3.0 * HOUR),
    (Unit::Hour, 6.0, 6.0 * HOUR),
    (Unit::Hour, 12.0, 12.0 * HOUR),
    (Unit::Day, 1.0, DAY),
    (Unit::Day, 2.0, 2.0 * DAY),
    (Unit::Week, 1.0, WEEK),
    (Unit::Month, 1.0, MONTH),
    (Unit::Month, 3.0, 3.0 * MONTH),
    (Unit::Year, 1.0, YEAR),
];

impl Interval {
    /// d3-time's `interval.every(step)`: None unless the step, floored, is
    /// finite and positive. Steps of seconds, minutes, hours and months must
    /// divide the next unit up, as every step d3's tick table holds does.
    pub fn every(unit: Unit, step: f64) -> Option<Interval> {
        let step = step.floor();
        (step.is_finite() && step > 0.0).then_some(Interval { unit, step })
    }

    /// The interval's fixed length in ms, for units that have one.
    fn fixed(&self) -> Option<f64> {
        match self.unit {
            Unit::Millisecond => Some(self.step),
            Unit::Second => Some(self.step * SECOND),
            Unit::Minute => Some(self.step * MINUTE),
            Unit::Hour => Some(self.step * HOUR),
            Unit::Day => Some(self.step * DAY),
            Unit::Week | Unit::Month | Unit::Year => None,
        }
    }

    /// `interval.floor(t)`: the latest time on the interval at or before `t`.
    pub fn floor(&self, t: f64) -> Option<f64> {
        let t = time_clip(t)?;
        if let Some(size) = self.fixed() {
            return time_clip((t / size).floor() * size);
        }
        let c = Civil::utc(t)?;
        let days = (t / DAY).floor();
        match self.unit {
            Unit::Week => time_clip((days - f64::from(c.weekday)) * DAY),
            Unit::Month => {
                let m = i64::from(c.month - 1);
                let k = self.step as i64;
                midnight(c.year, (m - m.rem_euclid(k)) as u32 + 1, 1)
            }
            _ => {
                let y = ((c.year as f64) / self.step).floor() * self.step;
                if !(y.abs() <= 300_000.0) {
                    return None;
                }
                midnight(y as i64, 1, 1)
            }
        }
    }

    /// `interval.offset(t, 1)` from a time on the interval: the next one.
    fn next(&self, t: f64) -> Option<f64> {
        if let Some(size) = self.fixed() {
            return time_clip(t + size);
        }
        match self.unit {
            Unit::Week => time_clip(t + WEEK),
            Unit::Month => {
                let c = Civil::utc(t)?;
                let i = c.year * 12 + i64::from(c.month - 1) + self.step as i64;
                midnight(i.div_euclid(12), i.rem_euclid(12) as u32 + 1, 1)
            }
            _ => {
                let y = Civil::utc(t)?.year as f64 + self.step;
                if !(y.abs() <= 300_000.0) {
                    return None;
                }
                midnight(y as i64, 1, 1)
            }
        }
    }

    /// `interval.ceil(t)`: the earliest time on the interval at or after
    /// `t`, computed as d3 does, from `floor(t − 1)`.
    pub fn ceil(&self, t: f64) -> Option<f64> {
        self.floor(t - 1.0).and_then(|f| self.next(f)).and_then(|n| self.floor(n))
    }

    /// `interval.range(start, stop)`: every time on the interval from
    /// `start` up to, not including, `stop`. None of them past
    /// [`MAX_TICKS`]: then the range is empty.
    pub fn range(&self, start: f64, stop: f64) -> Vec<f64> {
        let mut out = Vec::new();
        let Some(mut t) = self.ceil(start) else { return out };
        if !(t < stop) {
            return out;
        }
        loop {
            if out.len() == MAX_TICKS {
                return Vec::new();
            }
            out.push(t);
            match self.next(t).and_then(|n| self.floor(n)) {
                Some(n) if t < n && n < stop => t = n,
                _ => return out,
            }
        }
    }
}

/// d3-time's `utcTickInterval(start, stop, count)`: the interval whose
/// nominal duration is nearest, by ratio, to the span over the count;
/// years by d3-array's tick step beyond a year, milliseconds below a second.
pub fn utc_tick_interval(start: f64, stop: f64, count: f64) -> Option<Interval> {
    let target = (stop - start).abs() / count;
    // d3-array's bisector(…).right: NaN goes past the end.
    let i = if target.is_nan() { TICK_INTERVALS.len() } else { TICK_INTERVALS.partition_point(|iv| iv.2 <= target) };
    if i == TICK_INTERVALS.len() {
        return Interval::every(Unit::Year, tick_step(start / YEAR, stop / YEAR, count));
    }
    if i == 0 {
        return Interval::every(Unit::Millisecond, js::max(tick_step(start, stop, count), 1.0));
    }
    let j = if target / TICK_INTERVALS[i - 1].2 < TICK_INTERVALS[i].2 / target { i - 1 } else { i };
    Interval::every(TICK_INTERVALS[j].0, TICK_INTERVALS[j].1)
}

/// d3-time's `utcTicks(start, stop, count)`: about `count` calendar-aligned
/// UTC times from `start` to `stop` inclusive, in the domain's direction
/// (REQ-TICK-020).
pub fn utc_ticks(start: f64, stop: f64, count: f64) -> Vec<f64> {
    let reverse = stop < start;
    let (start, stop) = if reverse { (stop, start) } else { (start, stop) };
    let mut ticks = match utc_tick_interval(start, stop, count) {
        Some(interval) => interval.range(start, stop + 1.0),
        None => Vec::new(),
    };
    if reverse {
        ticks.reverse();
    }
    ticks
}

/// d3-scale's `scaleUtc().nice(count)`: the domain widened outwards to the
/// tick interval for `count` ticks; unchanged when there is none.
pub fn utc_nice(start: f64, stop: f64, count: f64) -> [f64; 2] {
    let (a, b) = (time_clip(start).unwrap_or(f64::NAN), time_clip(stop).unwrap_or(f64::NAN));
    let Some(interval) = utc_tick_interval(a, b, count) else { return [a, b] };
    let (lo, hi, flip) = if b < a { (b, a, true) } else { (a, b, false) };
    let lo = interval.floor(lo).unwrap_or(f64::NAN);
    let hi = interval.ceil(hi).unwrap_or(f64::NAN);
    if flip {
        [hi, lo]
    } else {
        [lo, hi]
    }
}

/// The names a date label uses: d3-time-format's locale definition, en-US by
/// default. A locale pack replaces it from v0.5.0 (REQ-FMT-050).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimeLocale {
    /// `%p`: before and after noon.
    pub periods: [String; 2],
    /// `%A`: weekdays from Sunday.
    pub days: [String; 7],
    /// `%a`: short weekdays from Sunday.
    pub days_short: [String; 7],
    /// `%B`: months from January.
    pub months: [String; 12],
    /// `%b`: short months from January.
    pub months_short: [String; 12],
}

impl Default for TimeLocale {
    fn default() -> TimeLocale {
        TimeLocale {
            periods: ["AM", "PM"].map(String::from),
            days: ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"].map(String::from),
            days_short: ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"].map(String::from),
            months: ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"]
                .map(String::from),
            months_short: ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"].map(String::from),
        }
    }
}

/// d3-time-format's `pad(value, fill, width)`: a sign, then the digits
/// padded on the left to `width` with `fill` (none when empty).
fn pad(value: i64, fill: Option<char>, width: usize, out: &mut String) {
    if value < 0 {
        out.push('-');
    }
    let mut digits = [0u8; 20];
    let mut n = value.unsigned_abs();
    let mut i = digits.len();
    loop {
        i -= 1;
        digits[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    let len = digits.len() - i;
    if let Some(fill) = fill {
        out.extend(core::iter::repeat_n(fill, width.saturating_sub(len)));
    }
    out.extend(digits[i..].iter().map(|&b| char::from(b)));
}

/// d3-time-format's `utcFormat(pattern)` applied to `t`, for the directives
/// `%a %A %b %B %d %e %H %I %j %L %m %M %p %S %Y %Z %%`, each with an
/// optional pad modifier `-` (none), `_` (spaces) or `0` (zeros). Another
/// letter after `%` is written as itself, as d3 does. An invalid date writes
/// nothing.
pub fn strftime(pattern: &str, t: f64, locale: &TimeLocale, out: &mut String) {
    let Some(c) = Civil::utc(t) else { return };
    let mut chars = pattern.chars();
    while let Some(ch) = chars.next() {
        if ch != '%' {
            out.push(ch);
            continue;
        }
        let Some(mut d) = chars.next() else { break };
        let modifier = match d {
            '-' => Some(None),
            '_' => Some(Some(' ')),
            '0' => Some(Some('0')),
            _ => None,
        };
        if modifier.is_some() {
            match chars.next() {
                Some(next) => d = next,
                None => break,
            }
        }
        let fill = modifier.unwrap_or(if d == 'e' { Some(' ') } else { Some('0') });
        match d {
            'a' => out.push_str(&locale.days_short[c.weekday as usize]),
            'A' => out.push_str(&locale.days[c.weekday as usize]),
            'b' => out.push_str(&locale.months_short[c.month as usize - 1]),
            'B' => out.push_str(&locale.months[c.month as usize - 1]),
            'd' | 'e' => pad(i64::from(c.day), fill, 2, out),
            'H' => pad(i64::from(c.hour), fill, 2, out),
            'I' => pad(i64::from(if c.hour % 12 == 0 { 12 } else { c.hour % 12 }), fill, 2, out),
            'j' => pad(i64::from(c.yday), fill, 3, out),
            'L' => pad(i64::from(c.millisecond), fill, 3, out),
            'm' => pad(i64::from(c.month), fill, 2, out),
            'M' => pad(i64::from(c.minute), fill, 2, out),
            'p' => out.push_str(&locale.periods[usize::from(c.hour >= 12)]),
            'S' => pad(i64::from(c.second), fill, 2, out),
            'Y' => pad(c.year % 10_000, fill, 4, out),
            'Z' => out.push_str("+0000"),
            other => out.push(other),
        }
    }
}

/// d3-scale's default `scaleUtc().tickFormat()` for one tick: the finest
/// field the time is not on a boundary of decides the pattern, `.%L`,
/// `:%S`, `%I:%M`, `%I %p`, `%a %d` (not a Sunday), `%b %d`, `%B` or `%Y`
/// (REQ-FMT-010). An invalid date writes nothing.
pub fn utc_tick_label(t: f64, locale: &TimeLocale, out: &mut String) {
    let Some(t) = time_clip(t) else { return };
    // d3 asks `interval(date) < date`; near the ends of a Date's range the
    // floor itself can be out of range, an invalid date that compares false.
    let after = |unit| Interval { unit, step: 1.0 }.floor(t).is_some_and(|f| f < t);
    let pattern = if after(Unit::Second) {
        ".%L"
    } else if after(Unit::Minute) {
        ":%S"
    } else if after(Unit::Hour) {
        "%I:%M"
    } else if after(Unit::Day) {
        "%I %p"
    } else if after(Unit::Month) {
        if after(Unit::Week) {
            "%a %d"
        } else {
            "%b %d"
        }
    } else if after(Unit::Year) {
        "%B"
    } else {
        "%Y"
    };
    strftime(pattern, t, locale, out);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn label(t: f64) -> String {
        let mut s = String::new();
        utc_tick_label(t, &TimeLocale::default(), &mut s);
        s
    }

    #[test]
    fn civil_round_trips() {
        for days in (-800_000..800_000).step_by(997) {
            let (y, m, d) = civil_from_days(days);
            assert_eq!(days_from_civil(y, m, d), days);
        }
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_782), (2024, 2, 29));
        assert_eq!(weekday_from_days(0), 4);
        assert_eq!(days_from_civil(2000, 3, 1), 11_017);
    }

    /// REQ-TZ-010: a time scale's zone is UTC unless told otherwise, and an
    /// instant's fields and labels are UTC's, whatever the machine's zone.
    #[test]
    fn default_zone_is_utc() {
        assert_eq!(Zone::default(), Zone::Utc);
        assert_eq!(crate::scale::Scale::time([0.0, DAY], [0.0, 10.0]).zone, Zone::Utc);
        let c = Civil::utc(1_792_328_400_000.0).unwrap();
        assert_eq!((c.year, c.month, c.day, c.hour, c.minute), (2026, 10, 18, 13, 0));
        assert_eq!(label(0.0), "1970");
        assert_eq!(label(HOUR), "01 AM");
        assert_eq!(label(13.0 * HOUR), "01 PM");
    }

    #[test]
    fn labels_take_the_finest_field() {
        let t0 = 1_792_281_600_000.0; // 2026-10-18, a Sunday
        assert_eq!(label(t0), "Oct 18");
        assert_eq!(label(t0 + DAY), "Mon 19");
        assert_eq!(label(t0 + 250.0), ".250");
        assert_eq!(label(t0 + 7.0 * SECOND), ":07");
        assert_eq!(label(t0 + 90.0 * MINUTE), "01:30");
        assert_eq!(label(1_790_812_800_000.0), "October");
        assert_eq!(label(f64::NAN), "");
        let mut s = String::new();
        strftime("%Y-%m-%d %H:%M:%S.%L %j %e %-d %_m %Z %%", 1_792_328_421_005.0, &TimeLocale::default(), &mut s);
        assert_eq!(s, "2026-10-18 13:00:21.005 291 18 18 10 +0000 %");
    }

    #[test]
    fn ticks_align_to_the_calendar() {
        let t0 = 1_792_328_421_250.0;
        assert_eq!(utc_ticks(t0, t0 + 3.0 * HOUR, 3.0), vec![1_792_332_000_000.0, 1_792_335_600_000.0, 1_792_339_200_000.0]);
        let months = utc_ticks(t0, t0 + 120.0 * DAY, 4.0);
        assert!(months.iter().all(|&t| Civil::utc(t).unwrap().day == 1));
        let back = utc_ticks(t0 + 120.0 * DAY, t0, 4.0);
        assert_eq!(back.iter().rev().copied().collect::<Vec<_>>(), months);
        assert!(utc_ticks(t0, t0 + DAY, 0.0).is_empty());
        assert_eq!(utc_ticks(t0, t0, 10.0), vec![t0.trunc()]);
    }

    proptest::proptest! {
        /// REQ-TICK-030 (tick order), REQ-PROP-010: UTC ticks over whole
        /// milliseconds lie in the domain, run strictly in its direction and
        /// number about the count (d3's intervals can give up to about
        /// three times it, a month of days at a count of 10 among them).
        #[test]
        fn time_ticks_are_ordered_inside_and_about_count(
            start in -2_208_988_800_000i64..4_102_444_800_000,
            span in 1i64..3_000_000_000_000,
            count in 1u32..=50,
            reverse in proptest::prelude::any::<bool>(),
        ) {
            let (lo, hi) = (start as f64, (start + span) as f64);
            let (a, b) = if reverse { (hi, lo) } else { (lo, hi) };
            let t = utc_ticks(a, b, f64::from(count));
            // d3-time gives no tick for one tick over a long span: the year step it
            // picks can be wider than the span (start -1650917612532, span
            // 2229931945150, count 1 gives [] in d3-time 3.1.0). From two ticks
            // the step is at most about 0.71 of the span, so one always lands.
            proptest::prop_assert!(!t.is_empty() || count == 1, "no ticks for {count}");
            proptest::prop_assert!(t.iter().all(|&v| v >= lo && v <= hi), "{t:?}");
            proptest::prop_assert!(t.windows(2).all(|w| if reverse { w[0] > w[1] } else { w[0] < w[1] }), "{t:?}");
            proptest::prop_assert!(t.len() as f64 <= 3.0 * f64::from(count) + 2.0, "{} ticks for {count}", t.len());
        }
    }
}
