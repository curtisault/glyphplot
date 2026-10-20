//! Tick values: d3-array's linear ticks and d3-time's UTC ticks (REQ-TICK-010
//! to 040).
//!
//! [`ticks`], [`tick_increment`] and [`tick_step`] are d3-array 3.2.4's
//! `ticks`, `tickIncrement` and `tickStep` (`src/ticks.js`, ISC licence,
//! © Mike Bostock), line for line, with JavaScript's `Math.round`,
//! `Math.pow` and `Math.log10` where d3 calls them (`format::js`), so every
//! tick equals d3's bit for bit (REQ-TICK-010). UTC calendar ticks are
//! d3-time's `utcTicks`, in [`crate::time`] and re-exported here
//! (REQ-TICK-020). [`mark`] places a tick on an axis's cells and dots
//! (REQ-TICK-040); `crate::layout::fit_ticks` chooses how many ticks an axis
//! shows (REQ-TICK-030).
//!
//! From v0.5.0:
//! - [`log_ticks`] and [`LogTickFormat`] are d3-scale 4.0.2's
//!   `scaleLog().ticks()` and `tickFormat()` (`src/log.js`, ISC), checked
//!   against captures; a log axis labels only powers of its base and may
//!   mark the other ticks unlabelled (REQ-TICK-060, [`LogTickFormat::is_power`]).
//! - [`symlog_ticks`]: 0 and ± powers of ten (REQ-SCALE-050).
//! - [`snap_rows`]: a y domain on which every tick sits on a row, a nice
//!   step a row (REQ-TICK-050); [`Snap::resolve`] makes it the default under
//!   [`SNAP_ROWS_BELOW`] rows for the charts that ask.
//! - [`context_row`]: which ticks of a time axis spanning under two days
//!   carry a date, and the date (REQ-TICK-070).

// d3 tests `!(x > 0)` so that NaN takes the same branch; so does this port.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use crate::cells::Axis;
use crate::format::{js, step_format, Kind as SpecKind, Locale, NumberFormat, Specifier};
use crate::time::{TimeLocale, Zone, DAY};

pub use crate::time::{utc_tick_interval, utc_ticks};

/// The most ticks one call returns. d3 has no limit (it allocates whatever
/// the count asks for); a count from options could otherwise ask for
/// billions. Past it, the call returns no ticks.
pub const MAX_TICKS: usize = 1_000_000;

/// d3-array's `tickSpec`: the first and last tick index and the increment;
/// a negative increment means "divide by its negation", which keeps
/// decimal steps exact.
fn tick_spec(start: f64, stop: f64, count: f64) -> (f64, f64, f64) {
    let e10 = 50f64.sqrt();
    let e5 = 10f64.sqrt();
    let e2 = 2f64.sqrt();
    let step = (stop - start) / js::max(0.0, count);
    let power = js::floor_log10(step);
    let error = step / js::pow10(power);
    let factor = if error >= e10 {
        10.0
    } else if error >= e5 {
        5.0
    } else if error >= e2 {
        2.0
    } else {
        1.0
    };
    let (mut i1, mut i2, inc);
    if power < 0.0 {
        let k = js::pow10(-power) / factor;
        i1 = js::round(start * k);
        i2 = js::round(stop * k);
        if i1 / k < start {
            i1 += 1.0;
        }
        if i2 / k > stop {
            i2 -= 1.0;
        }
        inc = -k;
    } else {
        let k = js::pow10(power) * factor;
        i1 = js::round(start / k);
        i2 = js::round(stop / k);
        if i1 * k < start {
            i1 += 1.0;
        }
        if i2 * k > stop {
            i2 -= 1.0;
        }
        inc = k;
    }
    if i2 < i1 && (0.5..2.0).contains(&count) {
        return tick_spec(start, stop, count * 2.0);
    }
    (i1, i2, inc)
}

/// d3-array's `ticks(start, stop, count)`: about `count` round values (a
/// step of 1, 2 or 5 × 10ⁿ) from `start` to `stop` inclusive, in the
/// domain's direction; `[start]` when the ends are equal, none when the
/// count is not positive (REQ-TICK-010).
pub fn ticks(start: f64, stop: f64, count: f64) -> Vec<f64> {
    if !(count > 0.0) {
        return Vec::new();
    }
    if start == stop {
        return vec![start];
    }
    let reverse = stop < start;
    let (i1, i2, inc) = if reverse { tick_spec(stop, start, count) } else { tick_spec(start, stop, count) };
    if !(i2 >= i1) {
        return Vec::new();
    }
    let n = i2 - i1 + 1.0;
    if !(n <= MAX_TICKS as f64) {
        return Vec::new();
    }
    (0..n as usize)
        .map(|i| {
            let i = i as f64;
            let k = if reverse { i2 - i } else { i1 + i };
            if inc < 0.0 {
                k / -inc
            } else {
                k * inc
            }
        })
        .collect()
}

/// d3-array's `tickIncrement(start, stop, count)`, for `start ≤ stop`: the
/// step if it is at least 1, else the negated inverse of the step (−10 for
/// 0.1), which d3-scale's `nice` uses.
pub fn tick_increment(start: f64, stop: f64, count: f64) -> f64 {
    tick_spec(start, stop, count).2
}

/// d3-array's `tickStep(start, stop, count)`: the signed distance between
/// ticks.
pub fn tick_step(start: f64, stop: f64, count: f64) -> f64 {
    let reverse = stop < start;
    let inc = if reverse { tick_increment(stop, start, count) } else { tick_increment(start, stop, count) };
    (if reverse { -1.0 } else { 1.0 }) * (if inc < 0.0 { 1.0 / -inc } else { inc })
}

/// Where a tick at `v` is drawn on `axis`, as `(cell, dot)`: its gridline
/// at the dot nearest its position, the dot holding it, and its mark in the
/// cell holding that dot, the nearest cell (REQ-TICK-040). The dot is
/// [`Axis::dot_in`]'s, the one data at `v` is drawn in, so a gridline meets
/// the data to the dot; [`crate::cells::nearest`] on [`Axis::position`] gives
/// the same dot except at a dot's very edge, where float rounding can tip it
/// to the neighbour, which is as near. The domain's far end falls in the last
/// cell and dot. `None` for a value outside the domain or not finite, or an
/// axis of no cells.
pub fn mark(axis: &Axis, v: f64) -> Option<(usize, usize)> {
    let dot = axis.dot_in(v)?;
    Some((dot / axis.per(), dot))
}

/// A log scale's `logs` and `pows` (d3-scale's `logp` and `powp`), mirrored
/// through 0 for a domain below it, as d3's `reflect` does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LogBase {
    /// The base, above 1.
    pub base: f64,
    /// Whether the domain lies below 0.
    pub negative: bool,
}

impl LogBase {
    /// The base of a log scale over a domain starting at `start`: negative
    /// when `start < 0`, as d3 decides it.
    pub fn new(base: f64, start: f64) -> LogBase {
        LogBase { base, negative: start < 0.0 }
    }

    fn log_raw(&self, x: f64) -> f64 {
        if self.base == core::f64::consts::E {
            js::log(x)
        } else if self.base == 10.0 {
            js::log10(x)
        } else if self.base == 2.0 {
            js::log2(x)
        } else {
            js::log(x) / js::log(self.base)
        }
    }

    fn pow_raw(&self, x: f64) -> f64 {
        if self.base == 10.0 {
            // d3's pow10: `isFinite(x) ? +("1e" + x) : x < 0 ? 0 : x`, which is
            // NaN for a fraction.
            if !x.is_finite() {
                if x < 0.0 {
                    0.0
                } else {
                    x
                }
            } else if x == x.trunc() {
                js::pow10(x)
            } else {
                f64::NAN
            }
        } else if self.base == core::f64::consts::E {
            x.exp()
        } else {
            js::pow(self.base, x)
        }
    }

    /// The logarithm in the base: `logs(x)`.
    pub fn logs(&self, x: f64) -> f64 {
        if self.negative {
            -self.log_raw(-x)
        } else {
            self.log_raw(x)
        }
    }

    /// The base to the power: `pows(x)`.
    pub fn pows(&self, x: f64) -> f64 {
        if self.negative {
            -self.pow_raw(-x)
        } else {
            self.pow_raw(x)
        }
    }
}

/// d3-scale 4.0.2's `scaleLog().base(base).domain([start, stop]).ticks(count)`:
/// for an integer base and fewer decades than the count, every integer
/// multiple of each power of the base inside the domain (falling back to
/// linear ticks when that gives fewer than half the count); otherwise powers
/// of the base at a nice step of exponents.
pub fn log_ticks(start: f64, stop: f64, count: f64, base: f64) -> Vec<f64> {
    let lb = LogBase::new(base, start);
    let reverse = stop < start;
    let (u, v) = if reverse { (stop, start) } else { (start, stop) };
    let (i, j) = (lb.logs(u), lb.logs(v));
    let n = count;
    let mut z = Vec::new();
    // JavaScript's `!(base % 1)`: an integer base, or NaN.
    let integral = base % 1.0 == 0.0 || base.is_nan();
    if integral && j - i < n {
        let (i0, j0) = (i.floor(), j.ceil());
        if !(j0 - i0 <= MAX_TICKS as f64) || !(base <= 1e6) {
            return Vec::new();
        }
        // k from 1 up to below the base; down from base − 1 below 0.
        let mut ks: Vec<f64> = (1..).map(f64::from).take_while(|&k| k < base).collect();
        if !(u > 0.0) {
            ks.reverse();
        }
        let mut i = i0;
        'outer: while i <= j0 {
            for &k in &ks {
                let t = if (u > 0.0 && i < 0.0) || (!(u > 0.0) && i > 0.0) { k / lb.pows(-i) } else { k * lb.pows(i) };
                if t < u {
                    continue;
                }
                if t > v {
                    break;
                }
                if z.len() == MAX_TICKS {
                    z.clear();
                    break 'outer;
                }
                z.push(t);
            }
            i += 1.0;
        }
        if (z.len() as f64) * 2.0 < n {
            z = ticks(u, v, n);
        }
    } else {
        z = ticks(i, j, js::min(j - i, n)).into_iter().map(|e| lb.pows(e)).collect();
    }
    if reverse {
        z.reverse();
    }
    z
}

/// d3-scale 4.0.2's `scaleLog().tickFormat(count, specifier)` over a domain:
/// the specifier (`s` for base 10, else `,`; trimmed when it names no
/// precision and the base is an integer) for ticks whose leading digit is at
/// most `max(1, base × count / ticks().length)`, an empty label for the
/// rest.
#[derive(Clone, Debug, PartialEq)]
pub struct LogTickFormat {
    format: NumberFormat,
    base: LogBase,
    /// The largest leading multiple labelled.
    k: f64,
}

impl LogTickFormat {
    /// The tick format of a log scale of `base` over `domain` at `count`
    /// ticks, from `spec` (None for d3's default).
    pub fn new(domain: [f64; 2], base: f64, count: f64, spec: Option<&Specifier>, locale: &Locale) -> LogTickFormat {
        let mut spec = spec.cloned().unwrap_or(if base == 10.0 {
            Specifier { kind: SpecKind::Si, ..Specifier::default() }
        } else {
            Specifier { comma: true, kind: SpecKind::Auto, ..Specifier::default() }
        });
        if base % 1.0 == 0.0 && spec.precision.is_none() {
            spec.trim = true;
        }
        let format = NumberFormat::new(&spec, locale);
        let k = if count == f64::INFINITY { f64::INFINITY } else { js::max(1.0, base * count / log_ticks(domain[0], domain[1], 10.0, base).len() as f64) };
        LogTickFormat { format, base: LogBase::new(base, domain[0]), k }
    }

    /// d3's `i = d / pows(round(logs(d)))`, times the base when below one:
    /// the tick's leading multiple.
    fn multiple(&self, d: f64) -> f64 {
        let lb = &self.base;
        let mut i = d / lb.pows(js::round(lb.logs(d)));
        if i * lb.base < lb.base - 0.5 {
            i *= lb.base;
        }
        i
    }

    /// Whether `d` is a power of the base (its leading multiple is 1): the
    /// only ticks a log axis labels (REQ-TICK-060).
    pub fn is_power(&self, d: f64) -> bool {
        (self.multiple(d) - 1.0).abs() < 1e-9
    }

    /// d3's label of `d`: empty past the largest labelled multiple.
    pub fn format(&self, d: f64) -> String {
        if self.multiple(d) <= self.k {
            self.format.format(d)
        } else {
            String::new()
        }
    }

    /// The label of `d` with the format alone, whatever its multiple.
    pub fn format_always(&self, d: f64, out: &mut String) {
        self.format.format_into(d, out);
    }
}

/// A symlog scale's ticks (REQ-SCALE-050): 0 when the domain holds it, and
/// ± powers of ten inside the domain, from the constant's power outwards (a
/// side reaching no further than that gets its largest power). When they
/// number more than `count`, the largest exponent is kept with every 2nd,
/// 3rd, 5th, 10th, … below it on both sides, the least step that fits.
pub fn symlog_ticks(start: f64, stop: f64, count: f64, constant: f64) -> Vec<f64> {
    if !(count >= 1.0) || !(start.is_finite() && stop.is_finite()) {
        return Vec::new();
    }
    let reverse = stop < start;
    let (lo, hi) = if reverse { (stop, start) } else { (start, stop) };
    let floor_c = if constant > 0.0 { js::log10(constant).ceil() } else { 0.0 };
    // The exponents on one side reaching out to `far` from `near` (≥ 0).
    let side = |near: f64, far: f64| -> Vec<i32> {
        if !(far > 0.0) {
            return Vec::new();
        }
        let top = js::floor_log10(far);
        let bottom = if near > 0.0 { js::log10(near).ceil() } else { floor_c.min(top) };
        if !(top - bottom <= 700.0) {
            return Vec::new();
        }
        (bottom as i32..=top as i32).filter(|&e| (near..=far).contains(&js::pow10(f64::from(e)))).collect()
    };
    let pos = side(lo.max(0.0), hi);
    let neg = side((-hi).max(0.0), -lo);
    let zero = lo <= 0.0 && hi >= 0.0;
    // Thinning keeps the largest exponent of either side and every s-th below it.
    let top = pos.iter().chain(&neg).copied().max().unwrap_or(0);
    let total = |s: i32| {
        let keep = |e: &&i32| (top - **e) % s == 0;
        usize::from(zero) + pos.iter().filter(keep).count() + neg.iter().filter(keep).count()
    };
    let mut step = 1;
    for s in [1, 2, 3, 5, 10, 20, 30, 50, 100, 200, 300, 500, 1000] {
        step = s;
        if total(s) as f64 <= count {
            break;
        }
    }
    let mut out: Vec<f64> = neg.iter().rev().filter(|e| (top - **e) % step == 0).map(|&e| -js::pow10(f64::from(e))).collect();
    if zero {
        out.push(0.0);
    }
    out.extend(pos.iter().filter(|e| (top - **e) % step == 0).map(|&e| js::pow10(f64::from(e))));
    if reverse {
        out.reverse();
    }
    out
}

/// A band axis's ticks: category indices from 0 to `n − 1` a step of 1, 2,
/// 5, 10, … apart, the least that gives at most `count`.
pub fn band_ticks(n: usize, count: f64) -> Vec<f64> {
    if n == 0 || !(count >= 1.0) {
        return Vec::new();
    }
    let mut step = 1usize;
    let mut m = 1usize;
    while (n.div_ceil(step)) as f64 > count {
        step = [1, 2, 5][m % 3] * 10usize.pow((m / 3) as u32);
        m += 1;
    }
    (0..n).step_by(step).map(|i| i as f64).collect()
}

/// The `snap` option of an axis (`options.schema.json`, `$defs/axis`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Snap {
    /// Ticks at the nearest cell (REQ-TICK-040).
    #[default]
    None,
    /// The domain chosen so every tick lands on a row (REQ-TICK-050).
    Rows,
    /// Ticks on whole cells.
    Cells,
}

/// Below this many rows `snap: "rows"` is the default y snapping of the
/// charts that ask for it, bars and candlesticks (REQ-TICK-050).
pub const SNAP_ROWS_BELOW: usize = 20;

impl Snap {
    /// The option's value by name: `none`, `rows` or `cells`.
    pub fn from_name(name: &str) -> Option<Snap> {
        match name {
            "none" => Some(Snap::None),
            "rows" => Some(Snap::Rows),
            "cells" => Some(Snap::Cells),
            _ => None,
        }
    }

    /// The snapping of a y axis `rows` high: the option when given, else
    /// [`Snap::Rows`] under [`SNAP_ROWS_BELOW`] rows for a chart whose
    /// default it is (`rows_by_default`: bar and candlestick, REQ-TICK-050),
    /// else [`Snap::None`].
    pub fn resolve(option: Option<Snap>, rows: usize, rows_by_default: bool) -> Snap {
        option.unwrap_or(if rows_by_default && rows < SNAP_ROWS_BELOW { Snap::Rows } else { Snap::None })
    }
}

/// A y domain on which every row's centre holds a multiple of a nice step
/// (REQ-TICK-050): `step` a row, the top row's centre `top × step`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RowSnap {
    /// The domain to draw the axis over, `[low, high]`: half a step beyond
    /// the bottom and top rows' centres.
    pub domain: [f64; 2],
    /// The value a row: 1, 2 or 5 × 10ⁿ.
    pub step: f64,
    /// The rows.
    pub rows: usize,
    /// The multiple of the step at the top row's centre.
    pub top: i64,
    /// The step's leading digit, 1, 2 or 5.
    digit: i64,
    /// The step's power of ten.
    power: i32,
}

impl RowSnap {
    /// `k` halves of the step, written so a decimal step stays exact: `k ×
    /// digit / (2 × 10⁻ⁿ)` for a power below 0.
    fn halves(&self, k: i64) -> f64 {
        let num = (k * self.digit) as f64;
        if self.power < 0 {
            num / (2.0 * js::pow10(f64::from(-self.power)))
        } else {
            num * js::pow10(f64::from(self.power)) / 2.0
        }
    }

    /// The value at the centre of row `r`, counted from the top.
    pub fn row_value(&self, r: usize) -> f64 {
        self.halves(2 * (self.top - r as i64))
    }

    /// The rows from the top whose values are ticks at about `count`: every
    /// `q`th multiple of the step with `q × step` itself nice (1, 2 or 5 ×
    /// 10ⁿ), the least `q` giving at most `count` ticks and at least one;
    /// when no `q` does, the fewest ticks above none.
    pub fn tick_rows(&self, count: f64) -> Vec<usize> {
        let mut best: Vec<usize> = Vec::new();
        for q in (0..19u32).flat_map(|p| [1i64, 2, 5].map(|f| f * 10i64.pow(p))) {
            let mut m = i128::from(q) * i128::from(self.digit);
            while m % 10 == 0 {
                m /= 10;
            }
            if !matches!(m, 1 | 2 | 5) {
                continue;
            }
            let rows: Vec<usize> = (0..self.rows).filter(|&r| (self.top - r as i64).rem_euclid(q) == 0).collect();
            if rows.is_empty() {
                continue;
            }
            if rows.len() as f64 <= count.max(1.0) {
                return rows;
            }
            best = rows;
        }
        best
    }

    /// The tick values at about `count`, increasing.
    pub fn ticks(&self, count: f64) -> Vec<f64> {
        self.tick_rows(count).into_iter().rev().map(|r| self.row_value(r)).collect()
    }

    /// The format of these ticks at about `count`: d3-scale's tick format
    /// with the precision of the ticks' step (REQ-FMT-010).
    pub fn tick_format(&self, count: f64, spec: &Specifier, locale: &Locale) -> NumberFormat {
        let rows = self.tick_rows(count);
        let step = match rows.as_slice() {
            [a, b, ..] => self.row_value(*b) - self.row_value(*a),
            _ => self.step,
        };
        step_format(step.abs(), self.domain[0], self.domain[1], spec, locale)
    }
}

/// The y domain for `rows` rows over data from `lo` to `hi` on which every
/// tick lands on a row exactly with a nice step a row (REQ-TICK-050): the
/// least step of 1, 2 or 5 × 10ⁿ whose rows' centres, multiples of it, take
/// in the data. None for no rows or a domain that is not finite.
pub fn snap_rows(lo: f64, hi: f64, rows: usize) -> Option<RowSnap> {
    let (lo, hi) = (lo.min(hi), lo.max(hi));
    if rows == 0 || !(lo.is_finite() && hi.is_finite()) {
        return None;
    }
    let span = hi - lo;
    let rough = if span > 0.0 {
        span / rows as f64
    } else if lo != 0.0 {
        lo.abs() / 10.0
    } else {
        1.0
    };
    let mut power = js::floor_log10(rough) as i32;
    let mut d = 0usize;
    for _ in 0..12 {
        let digit = [1i64, 2, 5][d];
        let mut snap = RowSnap { domain: [0.0, 0.0], step: 0.0, rows, top: 0, digit, power };
        snap.step = snap.halves(2);
        if snap.step > 0.0 && snap.step.is_finite() {
            // The lowest row's multiple: the largest k whose row's bottom edge, (k − ½) steps, is at or below lo.
            let bottom = (lo / snap.step + 0.5).floor();
            if bottom.abs() < 1e15 {
                let bottom = bottom as i64;
                let top = bottom + rows as i64 - 1;
                snap.top = top;
                let (low, high) = (snap.halves(2 * bottom - 1), snap.halves(2 * top + 1));
                if low <= lo && high >= hi {
                    snap.domain = [low, high];
                    return Some(snap);
                }
            }
        }
        d += 1;
        if d == 3 {
            d = 0;
            power += 1;
        }
    }
    None
}

/// A date in a time axis's context row: the tick it is drawn under and its
/// text (REQ-TICK-070).
#[derive(Clone, Debug, PartialEq)]
pub struct ContextLabel {
    /// The tick's value, epoch ms.
    pub value: f64,
    /// The date, `%b %-d` (`Oct 18`).
    pub text: String,
}

/// A time axis's context row (REQ-TICK-070): when the domain spans under
/// two days, the first tick and each tick on a later local day than the
/// tick before it carry that day's date, so every day boundary the axis
/// crosses is dated; spans of two days or more have no context row. Ticks
/// are taken in increasing order.
pub fn context_row(ticks: &[f64], domain: [f64; 2], zone: &Zone, locale: &TimeLocale) -> Vec<ContextLabel> {
    let mut out = Vec::new();
    if !((domain[1] - domain[0]).abs() < 2.0 * DAY) {
        return out;
    }
    let mut last = None;
    for &t in ticks {
        let Some(c) = zone.civil(t) else { continue };
        let day = (c.year, c.month, c.day);
        if last != Some(day) {
            let mut text = String::new();
            zone.strftime("%b %-d", t, locale, &mut text);
            out.push(ContextLabel { value: t, text });
            last = Some(day);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// REQ-TICK-040: a mark takes the cell whose middle is nearest its
    /// position and a gridline the dot whose middle is nearest, both ends of
    /// the domain inside the axis, on x and y.
    #[test]
    fn marks_take_the_nearest_cell_and_dot() {
        let x = Axis::x(0.0, 100.0, 40, 2);
        assert_eq!(mark(&x, 37.0), Some((14, 29)), "37 is at 14.8 cells, 29.6 dots");
        assert_eq!(mark(&x, 0.0), Some((0, 0)));
        assert_eq!(mark(&x, 100.0), Some((39, 79)), "the far end is in the last cell");
        assert_eq!(mark(&x, 100.5), None);
        assert_eq!(mark(&x, f64::NAN), None);
        let y = Axis::y(0.0, 10.0, 5, 4);
        assert_eq!(mark(&y, 10.0), Some((0, 0)), "the top of a y axis is row 0");
        assert_eq!(mark(&y, 0.0), Some((4, 19)));
        assert_eq!(mark(&y, 5.0), Some((2, 10)));
        assert_eq!(mark(&Axis::x(0.0, 1.0, 0, 2), 0.5), None);
        for axis in [x, y, Axis::x(-3.7, 12.9, 77, 2), Axis::y(0.1, 0.3, 13, 4)] {
            let per = axis.per() as f64;
            for i in 0..=1000 {
                let v = (axis.lo() + (axis.hi() - axis.lo()) * f64::from(i) / 1000.0).min(axis.hi());
                let (cell, dot) = mark(&axis, v).expect("a value of the domain has a mark");
                let p = axis.position(v);
                // Within float rounding of a dot's edge, either neighbour is
                // the nearest.
                let slack = 1e-9 * axis.cells() as f64;
                if p < axis.cells() as f64 {
                    assert!((cell as f64 + 0.5 - p).abs() <= 0.5 + slack, "{v} at {p} took cell {cell}");
                    assert!(((dot as f64 + 0.5) / per - p).abs() <= (0.5 + slack) / per, "{v} at {p} took dot {dot}");
                }
                assert_eq!(cell, dot / axis.per(), "the cell and the dot agree");
                let (_, near) = crate::cells::nearest(p, axis.per()).unwrap();
                assert!((near - dot as i64).abs() <= 1 || p >= axis.cells() as f64, "{v}: nearest gives dot {near}, the data dot {dot}");
            }
        }
    }

    #[test]
    fn ticks_match_d3_examples() {
        assert_eq!(ticks(0.0, 1.0, 10.0), vec![0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0]);
        assert_eq!(ticks(1.0, 0.0, 5.0), vec![1.0, 0.8, 0.6, 0.4, 0.2, 0.0]);
        assert_eq!(ticks(-37.5, 112.25, 5.0), vec![-20.0, 0.0, 20.0, 40.0, 60.0, 80.0, 100.0]);
        assert_eq!(ticks(3.0, 3.0, 10.0), vec![3.0]);
        assert!(ticks(0.0, 1.0, 0.0).is_empty());
        assert!(ticks(0.0, 1.0, f64::NAN).is_empty());
        assert_eq!(tick_step(0.0, 1.0, 10.0), 0.1);
        assert_eq!(tick_step(1.0, 0.0, 10.0), -0.1);
        assert_eq!(tick_increment(0.0, 1.0, 10.0), -10.0);
        assert_eq!(tick_increment(0.0, 100.0, 10.0), 10.0);
    }

    proptest! {
        /// REQ-TICK-030 (tick order) and REQ-PROP-010 (tick order and count):
        /// over any finite domain and a count from 1 to 50, ticks lie in the
        /// domain, run strictly in its direction, step by 1, 2 or 5 × 10ⁿ, and
        /// number at most about 2.5 × the count plus one.
        #[test]
        fn ticks_are_ordered_inside_and_about_count(
            mag in -15i32..15,
            a in -1.0f64..1.0,
            span in 1e-6f64..4.0,
            reverse in any::<bool>(),
            count in 1u32..=50,
        ) {
            let m = 10f64.powi(mag);
            let (lo, hi) = (a * m, a * m + span * m);
            let (start, stop) = if reverse { (hi, lo) } else { (lo, hi) };
            let t = ticks(start, stop, f64::from(count));
            for &v in &t {
                prop_assert!(v >= lo && v <= hi, "{v} outside [{lo}, {hi}]");
            }
            for w in t.windows(2) {
                prop_assert!(if reverse { w[0] > w[1] } else { w[0] < w[1] }, "{t:?}");
            }
            prop_assert!(t.len() as f64 <= 2.5 * f64::from(count) + 1.0, "{} ticks for {count}", t.len());
            prop_assert!(!t.is_empty(), "no ticks for [{start}, {stop}] at {count}");
            if t.len() >= 2 {
                let step = (t[1] - t[0]).abs();
                let unit = js::pow10(js::floor_log10(step));
                let r = step / unit;
                prop_assert!([1.0, 2.0, 5.0, 10.0].iter().any(|k| (r - k).abs() < 1e-6), "step {step}");
            }
        }

        /// REQ-TICK-050: with `snap: "rows"`, the domain takes in the data,
        /// the step a row is 1, 2 or 5 × 10ⁿ, and every tick sits exactly on
        /// a row's centre, its mark in that row, at every count.
        #[test]
        fn snap_rows_puts_every_tick_on_a_row(
            mag in -8i32..10,
            a in -1.0f64..1.0,
            span in 0.0f64..5.0,
            rows in 1usize..60,
            count in 1u32..30,
        ) {
            let m = 10f64.powi(mag);
            let (lo, hi) = (a * m, a * m + span * m);
            let snap = snap_rows(lo, hi, rows).expect("a finite domain snaps");
            let [d0, d1] = snap.domain;
            prop_assert!(d0 <= lo && d1 >= hi, "{:?} misses [{lo}, {hi}]", snap.domain);
            let unit = js::pow10(js::floor_log10(snap.step));
            prop_assert!([1.0, 2.0, 5.0].iter().any(|k| (snap.step / unit - k).abs() < 1e-9), "step {}", snap.step);
            let axis = Axis::y(d0, d1, rows, 4);
            for r in 0..rows {
                let v = snap.row_value(r);
                prop_assert!((axis.position(v) - (r as f64 + 0.5)).abs() < 1e-6, "row {r}: {v} at {}", axis.position(v));
                prop_assert_eq!(mark(&axis, v).map(|(cell, _)| cell), Some(r));
            }
            let t = snap.ticks(f64::from(count));
            // At most the count, unless no nice multiple of the step gives so few: then at most 5.
            prop_assert!(!t.is_empty() && (t.len() as f64 <= f64::from(count) || t.len() <= 5), "{} ticks for {count}", t.len());
            prop_assert!(t.windows(2).all(|w| w[0] < w[1]));
            if t.len() >= 2 {
                let q = (t[1] - t[0]) / snap.step;
                prop_assert!((q - q.round()).abs() < 1e-6, "ticks {} apart on a step of {}", t[1] - t[0], snap.step);
            }
        }
    }

    /// REQ-TICK-050: the default is rows under 20 rows for the charts that
    /// ask (bar, candlestick), none otherwise; an option wins.
    #[test]
    fn snap_rows_is_the_default_under_twenty_rows() {
        assert_eq!(Snap::resolve(None, 19, true), Snap::Rows);
        assert_eq!(Snap::resolve(None, 20, true), Snap::None);
        assert_eq!(Snap::resolve(None, 10, false), Snap::None);
        assert_eq!(Snap::resolve(Some(Snap::None), 10, true), Snap::None);
        assert_eq!(Snap::from_name("rows"), Some(Snap::Rows));
        assert_eq!(Snap::from_name("all"), None);
        // 0 to 87 over 10 rows: a step of 10 a row, 0 on the bottom row's centre.
        let s = snap_rows(0.0, 87.0, 10).unwrap();
        assert_eq!((s.step, s.domain), (10.0, [-5.0, 95.0]));
        assert_eq!(s.ticks(5.0), [0.0, 20.0, 40.0, 60.0, 80.0]);
        assert_eq!(s.ticks(10.0), [0.0, 10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0, 90.0]);
        let f = s.tick_format(5.0, &Specifier::default(), &Locale::default());
        assert_eq!(f.format(20.0), "20");
        let s = snap_rows(0.11, 0.17, 4).unwrap();
        assert_eq!(s.step, 0.02);
        assert_eq!((0..4).map(|r| s.row_value(r)).collect::<Vec<_>>(), [0.18, 0.16, 0.14, 0.12]);
    }

    /// REQ-TICK-060: a log axis labels only powers of its base, and its
    /// other ticks are the integer multiples between them, marked without
    /// labels.
    #[test]
    fn log_axes_label_only_powers() {
        use crate::scale::{AxisFormat, Scale};
        let l = Locale::default();
        for (domain, base) in [([1.0, 1e6], 10.0), ([0.3, 4000.0], 10.0), ([1.0, 1024.0], 2.0), ([-1e5, -2.0], 10.0), ([1e-9, 1e9], 10.0), ([1.0, 6561.0], 3.0)]
        {
            let s = Scale::log(domain, [0.0, 80.0], base);
            let lb = LogBase::new(base, domain[0]);
            for count in [2.0, 5.0, 10.0, 20.0] {
                let labelled = s.tick_labels(count, &AxisFormat::default(), &l);
                assert!(!labelled.is_empty(), "{domain:?} {base} {count}");
                for (v, text) in &labelled {
                    let e = lb.logs(*v).round();
                    assert_eq!(lb.pows(e), *v, "{v} is not a power of {base}");
                    assert!(!text.is_empty());
                }
                let up = LogBase::new(base, 1.0);
                for v in s.minor_ticks(count) {
                    let k = v.abs() / up.pows(up.logs(v.abs()).floor());
                    assert!((k - k.round()).abs() < 1e-9 && k.round() > 1.0 && k.round() < base, "{v} is not a multiple between powers");
                }
            }
        }
        let s = Scale::log([1.0, 1e3], [0.0, 80.0], 10.0);
        let labels: Vec<String> = s.tick_labels(10.0, &AxisFormat::default(), &l).into_iter().map(|(_, t)| t).collect();
        assert_eq!(labels, ["1", "10", "100", "1k"]);
        assert_eq!(
            s.minor_ticks(10.0),
            [2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0, 90.0, 200.0, 300.0, 400.0, 500.0, 600.0, 700.0, 800.0, 900.0]
        );
        // d3's own log tick format labels some multiples; the axis does not.
        let d3 = LogTickFormat::new([1.0, 1e3], 10.0, 10.0, None, &l);
        assert_eq!(d3.format(2.0), "2");
        assert!(!d3.is_power(2.0) && d3.is_power(100.0));
    }

    /// REQ-TICK-070: under two days, the first tick and each tick on a new
    /// day carry the date, in the zone; two days or more, none.
    #[test]
    fn context_row_marks_day_boundaries() {
        use crate::locale::{Transition, ZoneTable};
        use crate::time::{days_from_civil, HOUR};
        let l = TimeLocale::default();
        let t0 = days_from_civil(2026, 10, 18) as f64 * DAY; // Sunday 00:00 UTC
        let domain = [t0 + 18.0 * HOUR, t0 + 30.0 * HOUR];
        let ticks = Zone::Utc.ticks(domain[0], domain[1], 6.0);
        let row = context_row(&ticks, domain, &Zone::Utc, &l);
        let got: Vec<(f64, &str)> = row.iter().map(|c| (c.value, c.text.as_str())).collect();
        assert_eq!(got, [(t0 + 18.0 * HOUR, "Oct 18"), (t0 + 24.0 * HOUR, "Oct 19")]);
        assert!(context_row(&ticks, [t0, t0 + 2.0 * DAY], &Zone::Utc, &l).is_empty());
        assert_eq!(context_row(&ticks, [t0, t0 + 2.0 * DAY - 1.0], &Zone::Utc, &l).len(), 2);
        // In New York (UTC−4 in October) the day turns at 04:00 UTC.
        let ny =
            Zone::table(ZoneTable { name: "America/New_York".into(), transitions: vec![Transition { at: 0, offset: -240 }], covering: [0, 2_000_000_000_000] });
        let ticks = ny.ticks(domain[0], domain[1], 6.0);
        let row = context_row(&ticks, domain, &ny, &l);
        assert_eq!(row.len(), 2);
        assert_eq!(row[1].value, t0 + 28.0 * HOUR);
        assert_eq!((row[0].text.as_str(), row[1].text.as_str()), ("Oct 18", "Oct 19"));
    }
}
