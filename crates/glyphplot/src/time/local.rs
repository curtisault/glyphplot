//! Local time over a zone table (REQ-TZ-030): JavaScript's local `Date`
//! methods and d3-time 3.1.0's local intervals and `timeTicks`, so ticks
//! over a table equal d3-scale's `scaleTime().ticks()` run under
//! `TZ=<zone>`, across daylight-saving changes.
//!
//! - [`Tz`] is the local half of ECMAScript's `Date` (ECMA-262 §21.4.1):
//!   `LocalTime(t)` adds the table's offset at `t`; `UTC(t)` takes a local
//!   time back to an instant, the earlier of two in a repeated hour and,
//!   in a skipped one, by the offset before the change, as the standard says
//!   and V8 does (ICU's `getOffsetFromLocal` with `UCAL_TZ_LOCAL_FORMER`).
//!   The setters (`setHours`, `setDate`, `setMonth`, `setFullYear`) are
//!   `MakeDay`, `MakeDate` and `UTC` as the standard writes them.
//! - The intervals are d3-time's `timeInterval` (`src/interval.js`, its
//!   `floor`, `ceil`, `range` and `every` with `filter`), `second`,
//!   `timeMinute`, `timeHour`, `timeDay`, `timeSunday`, `timeMonth`,
//!   `timeYear` (with its own `every`) and `millisecond` (ISC licence,
//!   © Mike Bostock), step for step: each `floori` and `offseti` calls the
//!   same `Date` methods in the same order, since a setter in a repeated or
//!   skipped hour moves the time.
//! - [`ticks`] is d3-time's `timeTicks` and [`tick_pattern`] d3-scale
//!   4.0.2's multi-scale `scaleTime().tickFormat()` (`src/time.js`).
//!
//! The core reads no tzdata (REQ-TZ-010): an instant outside the table's
//! transitions takes the nearest one's offset.

// d3 tests `!(x < y)` so that NaN and invalid dates take the same branch; so does this port.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use super::{days_from_civil, multi_pattern, time_clip, utc_tick_interval, Civil, Interval, TimeLocale, Unit, DAY, HOUR, MINUTE, SECOND};
use crate::locale::ZoneTable;
use crate::ticks::MAX_TICKS;

/// The furthest a zone's offset reaches from UTC: 18 hours, in ms
/// (`zone.schema.json` holds offsets to ±1080 minutes).
const REACH: f64 = 18.0 * HOUR;

/// The most steps a filtered interval takes to find its next time: d3's
/// `filter` loops until its test passes, which a valid time always does
/// within a year of hours; past this the date is treated as invalid.
const FILTER_STEPS: usize = 10_000;

/// JavaScript's local `Date` over a zone table: local time is UTC plus the
/// table's offset at the instant (REQ-TZ-030).
#[derive(Clone, Copy, Debug)]
pub struct Tz<'a>(pub &'a ZoneTable);

/// `MakeDay(year, month, date)` (ECMA-262 §21.4.1.28): the day count of a
/// date whose month may run past 11 or below 0 and whose day past the
/// month's; None when not finite or past ±400,000 years.
fn make_day(year: f64, month: f64, date: f64) -> Option<f64> {
    if !(year.is_finite() && month.is_finite() && date.is_finite()) {
        return None;
    }
    let ym = year + (month / 12.0).floor();
    if !(ym.abs() <= 400_000.0) {
        return None;
    }
    let mn = month.rem_euclid(12.0);
    Some(days_from_civil(ym as i64, mn as u32 + 1, 1) as f64 + date - 1.0)
}

impl Tz<'_> {
    /// The table's offset at the instant `t`, in ms.
    fn offset(&self, t: f64) -> f64 {
        f64::from(self.0.offset_at(t)) * MINUTE
    }

    /// `LocalTime(t)`: the instant's local time, as ms on the local clock.
    pub fn local(&self, t: f64) -> f64 {
        t + self.offset(t)
    }

    /// `UTC(l)`: the instant of the local time `l`. When `l` occurs twice
    /// (the hour repeated as clocks go back) the earlier instant; when it does
    /// not occur (the hour skipped as they go forward) `l` read with the
    /// offset before the change, so 02:30 on a spring-forward night in New
    /// York is 03:30 EDT (ECMA-262 §21.4.1.26).
    pub fn utc(&self, l: f64) -> f64 {
        let tr = &self.0.transitions;
        if !l.is_finite() || tr.is_empty() {
            return f64::NAN;
        }
        let first = tr.partition_point(|x| (x.at as f64) <= l - REACH - 1.0).saturating_sub(1);
        let mut gap = None;
        let mut before: Option<f64> = None;
        for i in first..tr.len() {
            let begin = if i == 0 { f64::NEG_INFINITY } else { tr[i].at as f64 };
            if begin > l + REACH + 1.0 {
                break;
            }
            let end = tr.get(i + 1).map_or(f64::INFINITY, |n| n.at as f64);
            let t = l - f64::from(tr[i].offset) * MINUTE;
            if t >= begin && t < end {
                return t;
            }
            // Skipped: the previous offset's reading lies past this change,
            // and this offset's reading lies before it.
            if gap.is_none() && t < begin && before.is_some_and(|b| b >= begin) {
                gap = before;
            }
            before = Some(t);
        }
        gap.unwrap_or(l - self.offset(l))
    }

    /// The local calendar fields of the instant `t` (a time value).
    pub fn fields(&self, t: f64) -> Option<Civil> {
        let l = self.local(t);
        l.is_finite().then(|| Civil::of(l as i64))
    }

    /// `date.setHours(h, m, s, ms)`.
    fn set_hours(&self, t: f64, h: f64, m: f64, s: f64, ms: f64) -> Option<f64> {
        let l = self.local(t);
        let date = (l / DAY).floor() * DAY + (h * HOUR + m * MINUTE + s * SECOND + ms);
        time_clip(self.utc(date))
    }

    /// The local date of `t` moved by `MakeDay(year, month, date)`, keeping
    /// its time of day: `setDate`, `setMonth` and `setFullYear`.
    fn set_day(&self, t: f64, year: f64, month: f64, date: f64) -> Option<f64> {
        let l = self.local(t);
        let day = make_day(year, month, date)?;
        time_clip(self.utc(day * DAY + l.rem_euclid(DAY)))
    }

    /// `date.setDate(d)`.
    fn set_date(&self, t: f64, d: f64) -> Option<f64> {
        let c = self.fields(t)?;
        self.set_day(t, c.year as f64, f64::from(c.month - 1), d)
    }

    /// `date.setMonth(m)` or `date.setMonth(m, d)`.
    fn set_month(&self, t: f64, m: f64, d: Option<f64>) -> Option<f64> {
        let c = self.fields(t)?;
        self.set_day(t, c.year as f64, m, d.unwrap_or(f64::from(c.day)))
    }

    /// `date.setFullYear(y)` on a valid date.
    fn set_full_year(&self, t: f64, y: f64) -> Option<f64> {
        let c = self.fields(t)?;
        self.set_day(t, y, f64::from(c.month - 1), f64::from(c.day))
    }

    /// The `floori` of d3-time's base local interval of `unit`.
    fn floor1(&self, unit: Unit, t: f64) -> Option<f64> {
        let c = self.fields(t)?;
        let (ms, s, m) = (f64::from(c.millisecond), f64::from(c.second), f64::from(c.minute));
        match unit {
            Unit::Millisecond => Some(t),
            Unit::Second => time_clip(t - ms),
            Unit::Minute => time_clip(t - ms - s * SECOND),
            Unit::Hour => time_clip(t - ms - s * SECOND - m * MINUTE),
            Unit::Day => self.set_hours(t, 0.0, 0.0, 0.0, 0.0),
            Unit::Week => {
                let t = self.set_date(t, f64::from(c.day) - f64::from((c.weekday + 7) % 7))?;
                self.set_hours(t, 0.0, 0.0, 0.0, 0.0)
            }
            Unit::Month => {
                let t = self.set_date(t, 1.0)?;
                self.set_hours(t, 0.0, 0.0, 0.0, 0.0)
            }
            Unit::Year => {
                let t = self.set_month(t, 0.0, Some(1.0))?;
                self.set_hours(t, 0.0, 0.0, 0.0, 0.0)
            }
        }
    }

    /// The `offseti(date, 1)` of d3-time's base local interval of `unit`.
    fn next1(&self, unit: Unit, t: f64) -> Option<f64> {
        match unit {
            Unit::Millisecond => time_clip(t + 1.0),
            Unit::Second => time_clip(t + SECOND),
            Unit::Minute => time_clip(t + MINUTE),
            Unit::Hour => time_clip(t + HOUR),
            Unit::Day => self.set_date(t, f64::from(self.fields(t)?.day) + 1.0),
            Unit::Week => self.set_date(t, f64::from(self.fields(t)?.day) + 7.0),
            Unit::Month => self.set_month(t, f64::from(self.fields(t)?.month - 1) + 1.0, None),
            Unit::Year => self.set_full_year(t, self.fields(t)?.year as f64 + 1.0),
        }
    }

    /// The field `interval.every(step)` filters on, as d3-time reads it:
    /// UTC seconds, local minutes, hours, day of the month less one and
    /// month.
    fn field(&self, unit: Unit, t: f64) -> Option<f64> {
        Some(match unit {
            Unit::Second => f64::from(Civil::utc(t)?.second),
            Unit::Minute => f64::from(self.fields(t)?.minute),
            Unit::Hour => f64::from(self.fields(t)?.hour),
            Unit::Day => f64::from(self.fields(t)?.day - 1),
            Unit::Month => f64::from(self.fields(t)?.month - 1),
            Unit::Millisecond | Unit::Week | Unit::Year => 0.0,
        })
    }

    /// Whether `t` passes `every(step)`'s filter.
    fn test(&self, iv: &Interval, t: f64) -> bool {
        self.field(iv.unit, t).is_some_and(|f| f % iv.step == 0.0)
    }

    /// `interval.floor(t)` for `interval` taken every `step`: d3's
    /// `millisecond.every` and `timeYear.every` by arithmetic, the others by
    /// `filter`, stepping back a millisecond until the floor passes the test.
    pub fn floor(&self, iv: &Interval, t: f64) -> Option<f64> {
        let t = time_clip(t)?;
        match iv.unit {
            Unit::Millisecond if iv.step > 1.0 => time_clip((t / iv.step).floor() * iv.step),
            Unit::Year => {
                let y = self.fields(t)?.year as f64;
                let t = self.set_full_year(t, (y / iv.step).floor() * iv.step)?;
                let t = self.set_month(t, 0.0, Some(1.0))?;
                self.set_hours(t, 0.0, 0.0, 0.0, 0.0)
            }
            _ if !(iv.step > 1.0) || iv.unit == Unit::Week => self.floor1(iv.unit, t),
            _ => {
                let mut t = t;
                for _ in 0..FILTER_STEPS {
                    t = self.floor1(iv.unit, t)?;
                    if self.test(iv, t) {
                        return Some(t);
                    }
                    t = time_clip(t - 1.0)?;
                }
                None
            }
        }
    }

    /// `offseti(date, 1)` for the interval taken every `step`.
    fn next(&self, iv: &Interval, t: f64) -> Option<f64> {
        match iv.unit {
            Unit::Millisecond if iv.step > 1.0 => time_clip(t + iv.step),
            Unit::Year => self.set_full_year(t, self.fields(t)?.year as f64 + iv.step),
            _ if !(iv.step > 1.0) || iv.unit == Unit::Week => self.next1(iv.unit, t),
            _ => {
                let mut t = t;
                for _ in 0..FILTER_STEPS {
                    t = self.next1(iv.unit, t)?;
                    if self.test(iv, t) {
                        return Some(t);
                    }
                }
                None
            }
        }
    }

    /// `interval.ceil(t)`: `floor(t − 1)`, one step on, floored.
    pub fn ceil(&self, iv: &Interval, t: f64) -> Option<f64> {
        let f = self.floor(iv, t - 1.0)?;
        let n = self.next(iv, f)?;
        self.floor(iv, n)
    }

    /// `interval.range(start, stop)`: every time on the interval from
    /// `start` up to, not including, `stop`; empty past [`MAX_TICKS`].
    pub fn range(&self, iv: &Interval, start: f64, stop: f64) -> Vec<f64> {
        let mut out = Vec::new();
        let Some(mut t) = self.ceil(iv, start) else { return out };
        if !(t < stop) {
            return out;
        }
        loop {
            if out.len() == MAX_TICKS {
                return Vec::new();
            }
            out.push(t);
            match self.next(iv, t).and_then(|n| self.floor(iv, n)) {
                Some(n) if t < n && n < stop => t = n,
                _ => return out,
            }
        }
    }
}

/// d3-time's `timeTicks(start, stop, count)` as JavaScript runs it under
/// `TZ=<zone>`, the zone given by its table: the interval `utcTickInterval`
/// would choose, taken in local time (REQ-TZ-030).
pub(super) fn ticks(table: &ZoneTable, start: f64, stop: f64, count: f64) -> Vec<f64> {
    let reverse = stop < start;
    let (start, stop) = if reverse { (stop, start) } else { (start, stop) };
    let mut ticks = match utc_tick_interval(start, stop, count) {
        Some(iv) => Tz(table).range(&iv, start, stop + 1.0),
        None => Vec::new(),
    };
    if reverse {
        ticks.reverse();
    }
    ticks
}

/// d3-scale's `scaleTime().nice(count)` under `TZ=<zone>`.
pub(super) fn nice(table: &ZoneTable, start: f64, stop: f64, count: f64) -> [f64; 2] {
    let (a, b) = (time_clip(start).unwrap_or(f64::NAN), time_clip(stop).unwrap_or(f64::NAN));
    let Some(iv) = utc_tick_interval(a, b, count) else { return [a, b] };
    let tz = Tz(table);
    let (lo, hi, flip) = if b < a { (b, a, true) } else { (a, b, false) };
    let lo = tz.floor(&iv, lo).unwrap_or(f64::NAN);
    let hi = tz.ceil(&iv, hi).unwrap_or(f64::NAN);
    if flip {
        [hi, lo]
    } else {
        [lo, hi]
    }
}

/// The pattern of d3-scale's `scaleTime().tickFormat()` for `t` under
/// `TZ=<zone>`, on the locale's hour cycle (REQ-FMT-050).
pub(super) fn tick_pattern(table: &ZoneTable, t: f64, locale: &TimeLocale) -> Option<&'static str> {
    let t = time_clip(t)?;
    let tz = Tz(table);
    let after = |unit| tz.floor1(unit, t).is_some_and(|f| f < t);
    Some(multi_pattern(after, locale.hour12))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::locale::Transition;

    fn new_york() -> ZoneTable {
        // 2026: EST until 2026-03-08 07:00Z, EDT until 2026-11-01 06:00Z.
        let at = |y, m, d, h: f64| (days_from_civil(y, m, d) as f64 * DAY + h * HOUR) as i64;
        ZoneTable {
            name: "America/New_York".into(),
            transitions: vec![
                Transition { at: at(2025, 11, 2, 6.0), offset: -300 },
                Transition { at: at(2026, 3, 8, 7.0), offset: -240 },
                Transition { at: at(2026, 11, 1, 6.0), offset: -300 },
            ],
            covering: [at(2026, 1, 1, 0.0), at(2027, 1, 1, 0.0)],
        }
    }

    /// REQ-TZ-030: a skipped local time reads with the offset before the
    /// change, a repeated one gives the earlier instant, as `Date` does.
    #[test]
    fn local_times_resolve_as_javascript_does() {
        let ny = new_york();
        let tz = Tz(&ny);
        // 2026-03-08 02:30 local is skipped: 07:30Z, 03:30 EDT.
        let skipped = days_from_civil(2026, 3, 8) as f64 * DAY + 2.5 * HOUR;
        assert_eq!(tz.utc(skipped), skipped + 5.0 * HOUR);
        // 2026-11-01 01:30 local happens twice: the EDT one, 05:30Z.
        let twice = days_from_civil(2026, 11, 1) as f64 * DAY + 1.5 * HOUR;
        assert_eq!(tz.utc(twice), twice + 4.0 * HOUR);
        // Midnight on the day clocks go forward is 05:00Z; a day later 04:00Z.
        let day = Interval { unit: Unit::Day, step: 1.0 };
        let t = days_from_civil(2026, 3, 8) as f64 * DAY + 12.0 * HOUR;
        assert_eq!(tz.floor(&day, t), Some(days_from_civil(2026, 3, 8) as f64 * DAY + 5.0 * HOUR));
        assert_eq!(tz.ceil(&day, t), Some(days_from_civil(2026, 3, 9) as f64 * DAY + 4.0 * HOUR));
    }
}
