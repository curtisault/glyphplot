//! Hand-written scales and formatting, to measure what they weigh in wasm and
//! to check them against d3. Ports of d3-array `ticks`/`tickIncrement`,
//! d3-scale `linear.nice`, d3-format's `s` with `precisionPrefix`, and
//! d3-time's UTC `utcTicks` with d3-scale's multi-scale `utcScale` labels.
//! Throwaway: written for discovery, not for glyphplot's core.

/// 10^e from a table of correctly rounded literals: no libm `pow` in the module.
pub fn pow10(e: i32) -> f64 {
    const P: [f64; 23] = [1e0, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6, 1e7, 1e8, 1e9, 1e10, 1e11, 1e12, 1e13, 1e14, 1e15, 1e16, 1e17, 1e18, 1e19, 1e20, 1e21, 1e22];
    const N: [f64; 25] = [1e0, 1e-1, 1e-2, 1e-3, 1e-4, 1e-5, 1e-6, 1e-7, 1e-8, 1e-9, 1e-10, 1e-11, 1e-12, 1e-13, 1e-14, 1e-15, 1e-16, 1e-17, 1e-18, 1e-19, 1e-20, 1e-21, 1e-22, 1e-23, 1e-24];
    if (0..23).contains(&e) {
        P[e as usize]
    } else if (-24..0).contains(&e) {
        N[(-e) as usize]
    } else {
        10f64.powi(e)
    }
}

/// JavaScript's `Math.round`: halves go up, not away from zero.
fn js_round(x: f64) -> f64 {
    (x + 0.5).floor()
}

/// d3-array `tickSpec`: (i1, i2, inc); a negative `inc` means divide by `-inc`.
fn tick_spec(start: f64, stop: f64, count: f64) -> (f64, f64, f64) {
    let e10 = 50f64.sqrt();
    let e5 = 10f64.sqrt();
    let e2 = 2f64.sqrt();
    let step = (stop - start) / count.max(0.0);
    let power = step.log10().floor();
    let error = step / pow10(power as i32);
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
        let k = pow10(-power as i32) / factor;
        i1 = js_round(start * k);
        i2 = js_round(stop * k);
        if i1 / k < start {
            i1 += 1.0;
        }
        if i2 / k > stop {
            i2 -= 1.0;
        }
        inc = -k;
    } else {
        let k = pow10(power as i32) * factor;
        i1 = js_round(start / k);
        i2 = js_round(stop / k);
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

/// d3-array `ticks(start, stop, count)`.
pub fn ticks(start: f64, stop: f64, count: f64) -> Vec<f64> {
    if !(count > 0.0) {
        return vec![];
    }
    if start == stop {
        return vec![start];
    }
    let reverse = stop < start;
    let (i1, i2, inc) = if reverse { tick_spec(stop, start, count) } else { tick_spec(start, stop, count) };
    if !(i2 >= i1) {
        return vec![];
    }
    let n = (i2 - i1 + 1.0) as usize;
    (0..n)
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

/// d3-array `tickStep`: the signed step between ticks.
pub fn tick_step(start: f64, stop: f64, count: f64) -> f64 {
    let reverse = stop < start;
    let inc = if reverse { tick_spec(stop, start, count).2 } else { tick_spec(start, stop, count).2 };
    (if reverse { -1.0 } else { 1.0 }) * (if inc < 0.0 { 1.0 / -inc } else { inc })
}

/// d3-scale `linear().nice(count)` on a two-number domain.
/// Like d3, a domain that has not settled after ten rounds is returned unchanged
/// (this happens at a count of 1 on a domain that straddles zero).
pub fn nice(mut start: f64, mut stop: f64, count: f64) -> (f64, f64) {
    let (start0, stop0) = (start, stop);
    let mut settled = false;
    let flip = stop < start;
    if flip {
        core::mem::swap(&mut start, &mut stop);
    }
    let mut prestep = f64::NAN;
    for _ in 0..10 {
        let step = tick_spec(start, stop, count).2;
        if step == prestep {
            settled = true;
            break;
        } else if step > 0.0 {
            start = (start / step).floor() * step;
            stop = (stop / step).ceil() * step;
        } else if step < 0.0 {
            start = (start * step).ceil() / step;
            stop = (stop * step).floor() / step;
        } else {
            break;
        }
        prestep = step;
    }
    if !settled {
        (start0, stop0)
    } else if flip {
        (stop, start)
    } else {
        (start, stop)
    }
}

/// The decimal exponent of |x| as d3-format's `exponent` gives it.
fn exponent(x: f64) -> i32 {
    let x = x.abs();
    if x == 0.0 || !x.is_finite() {
        return 0;
    }
    let mut e = x.log10().floor() as i32;
    // log10 can be off by one near powers of ten.
    if pow10(e) > x {
        e -= 1;
    } else if pow10(e + 1) <= x {
        e += 1;
    }
    e
}

const PREFIXES: [&str; 17] = ["y", "z", "a", "f", "p", "n", "µ", "m", "", "k", "M", "G", "T", "P", "E", "Z", "Y"];

/// `x` with `p` digits after the point, rounded, using U+2212 for minus as d3 does.
pub fn fixed(x: f64, p: u32, out: &mut String) {
    let scaled = js_round(x.abs() * pow10(p as i32));
    let neg = x < 0.0 && scaled != 0.0;
    let digits = itoa(scaled as u64);
    let pad = (p as usize + 1).saturating_sub(digits.len());
    if neg {
        out.push('\u{2212}');
    }
    let point = pad + digits.len() - p as usize;
    for (i, c) in core::iter::repeat_n('0', pad).chain(digits.chars()).enumerate() {
        if i == point {
            out.push('.');
        }
        out.push(c);
    }
}

fn itoa(mut n: u64) -> String {
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    loop {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    String::from_utf8(buf[i..].to_vec()).unwrap_or_default()
}

/// d3-scale `tickFormat(start, stop, count, "s")`: one SI prefix for every
/// tick, chosen from the larger end of the domain, and the digits the step needs.
pub struct SiFormat {
    k: f64,
    p: u32,
    suffix: &'static str,
}

impl SiFormat {
    /// The format for ticks of `[start, stop]` at about `count` ticks.
    pub fn new(start: f64, stop: f64, count: f64) -> Self {
        let step = tick_step(start, stop, count);
        let value = start.abs().max(stop.abs());
        let e3 = (exponent(value).div_euclid(3)).clamp(-8, 8);
        let p = (e3 * 3 - exponent(step.abs())).max(0) as u32;
        SiFormat { k: pow10(-e3 * 3), p, suffix: PREFIXES[(8 + e3) as usize] }
    }

    /// One label.
    pub fn format(&self, x: f64, out: &mut String) {
        fixed(self.k * x, self.p, out);
        out.push_str(self.suffix);
    }
}

// ---- UTC calendar ticks -------------------------------------------------

const SEC: f64 = 1000.0;
const MIN: f64 = 60.0 * SEC;
const HOUR: f64 = 60.0 * MIN;
const DAY: f64 = 24.0 * HOUR;
const WEEK: f64 = 7.0 * DAY;
const MONTH: f64 = 30.0 * DAY;
const YEAR: f64 = 365.0 * DAY;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Unit {
    Milli,
    Second,
    Minute,
    Hour,
    Day,
    Week,
    Month,
    Year,
}

const INTERVALS: [(Unit, f64, f64); 18] = [
    (Unit::Second, 1.0, SEC),
    (Unit::Second, 5.0, 5.0 * SEC),
    (Unit::Second, 15.0, 15.0 * SEC),
    (Unit::Second, 30.0, 30.0 * SEC),
    (Unit::Minute, 1.0, MIN),
    (Unit::Minute, 5.0, 5.0 * MIN),
    (Unit::Minute, 15.0, 15.0 * MIN),
    (Unit::Minute, 30.0, 30.0 * MIN),
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

/// Days since 1970-01-01 from a civil date (Howard Hinnant).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Civil (year, month 1–12, day) from days since 1970-01-01.
fn civil(days: i64) -> (i64, i64, i64) {
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

/// The first instant at or after `t` on the interval, and the next one after a tick.
fn ceil_and_next(unit: Unit, step: f64, t: f64) -> (f64, Box<dyn Fn(f64) -> f64>) {
    match unit {
        Unit::Milli | Unit::Second | Unit::Minute | Unit::Hour | Unit::Day => {
            let size = match unit {
                Unit::Milli => 1.0,
                Unit::Second => SEC,
                Unit::Minute => MIN,
                Unit::Hour => HOUR,
                _ => DAY,
            } * step;
            ((t / size).ceil() * size, Box::new(move |x| x + size))
        }
        Unit::Week => {
            // Sundays at 00:00 UTC; 1970-01-04 was a Sunday.
            let sunday0 = 3.0 * DAY;
            ((((t - sunday0) / WEEK).ceil()) * WEEK + sunday0, Box::new(|x| x + WEEK))
        }
        Unit::Month | Unit::Year => {
            let months = if unit == Unit::Year { 12 * step as i64 } else { step as i64 };
            let (y, m, _) = civil((t / DAY).floor() as i64);
            let mut idx = y * 12 + (m - 1);
            idx -= idx.rem_euclid(months);
            let at = |idx: i64| days_from_civil(idx.div_euclid(12), idx.rem_euclid(12) + 1, 1) as f64 * DAY;
            while at(idx) < t {
                idx += months;
            }
            let first = at(idx);
            (
                first,
                Box::new(move |x: f64| {
                    let (y, m, _) = civil((x / DAY).floor() as i64);
                    let i = y * 12 + (m - 1) + months;
                    days_from_civil(i.div_euclid(12), i.rem_euclid(12) + 1, 1) as f64 * DAY
                }),
            )
        }
    }
}

/// d3-time `utcTicks(start, stop, count)`, times in ms since the epoch.
pub fn utc_ticks(start: f64, stop: f64, count: f64) -> Vec<f64> {
    let (start, stop, reverse) = if stop < start { (stop, start, true) } else { (start, stop, false) };
    let target = (stop - start).abs() / count;
    let i = INTERVALS.partition_point(|iv| iv.2 <= target);
    let (unit, step) = if i == INTERVALS.len() {
        (Unit::Year, tick_step(start / YEAR, stop / YEAR, count).max(1.0).floor())
    } else if i == 0 {
        (Unit::Milli, tick_step(start, stop, count).max(1.0).floor())
    } else {
        let j = if target / INTERVALS[i - 1].2 < INTERVALS[i].2 / target { i - 1 } else { i };
        (INTERVALS[j].0, INTERVALS[j].1)
    };
    let (mut t, next) = ceil_and_next(unit, step, start);
    let mut out = Vec::new();
    while t < stop + 1.0 && out.len() < 10_000 {
        out.push(t);
        t = next(t);
    }
    if reverse {
        out.reverse();
    }
    out
}

const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
const MONTHS_SHORT: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const DAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

fn two(n: i64, out: &mut String) {
    out.push((b'0' + (n / 10 % 10) as u8) as char);
    out.push((b'0' + (n % 10) as u8) as char);
}

/// d3-scale `utcScale().tickFormat()` for one tick: the coarsest field it is not on a boundary of.
pub fn utc_label(t: f64, out: &mut String) {
    let whole = t.floor() as i64;
    let ms = whole.rem_euclid(1000);
    let secs = whole.div_euclid(1000);
    let days = secs.div_euclid(86_400);
    let rest = secs.rem_euclid(86_400);
    let (h, mi, s) = (rest / 3600, rest / 60 % 60, rest % 60);
    let (y, m, d) = civil(days);
    let weekday = (days + 4).rem_euclid(7);
    if ms != 0 {
        out.push('.');
        out.push((b'0' + (ms / 100) as u8) as char);
        two(ms % 100, out);
    } else if s != 0 {
        out.push(':');
        two(s, out);
    } else if mi != 0 {
        two((h + 11) % 12 + 1, out);
        out.push(':');
        two(mi, out);
    } else if h != 0 {
        two((h + 11) % 12 + 1, out);
        out.push_str(if h < 12 { " AM" } else { " PM" });
    } else if d != 1 {
        if weekday != 0 {
            out.push_str(DAYS[weekday as usize]);
            out.push(' ');
        } else {
            out.push_str(MONTHS_SHORT[(m - 1) as usize]);
            out.push(' ');
        }
        two(d, out);
    } else if m != 1 {
        out.push_str(MONTHS[(m - 1) as usize]);
    } else {
        out.push_str(&itoa(y as u64));
    }
}

// ---- Fitting ticks to a row of cells -------------------------------------

/// A tick placed on an axis: its value, its cell, its label and the label's first cell.
#[derive(Clone, Debug, PartialEq)]
pub struct Placed {
    /// The domain value.
    pub value: f64,
    /// The cell the tick mark falls in, 0 to len − 1.
    pub cell: i64,
    /// The label text; every char one cell wide in this sketch.
    pub label: String,
    /// The label's leftmost cell.
    pub left: i64,
}

/// The cell of `v` on an axis of `len` cells showing `[a, b]`: cell centres
/// span the domain, so `a` is cell 0 and `b` cell len − 1.
pub fn cell_of(v: f64, a: f64, b: f64, len: i64) -> i64 {
    js_round((v - a) / (b - a) * (len - 1) as f64) as i64
}

/// Ticks and labels for a horizontal axis `len` cells wide, at least `gap`
/// blank cells between labels. Starts at one tick per four cells and lowers
/// the count until the labels fit; labels at the ends are pushed inside
/// (flush). With `time`, `a` and `b` are ms since the epoch in UTC.
pub fn fit_x(a: f64, b: f64, len: i64, gap: i64, time: bool) -> Vec<Placed> {
    let mut count = (len / 4).max(1);
    loop {
        let values = if time { utc_ticks(a, b, count as f64) } else { ticks(a, b, count as f64) };
        let si = SiFormat::new(a, b, count as f64);
        let mut placed: Vec<Placed> = Vec::with_capacity(values.len());
        let mut ok = true;
        for v in values {
            let mut label = String::new();
            if time {
                utc_label(v, &mut label);
            } else {
                si.format(v, &mut label);
            }
            let w = label.chars().count() as i64;
            let cell = cell_of(v, a, b, len);
            let left = (cell - (w - 1) / 2).clamp(0, (len - w).max(0));
            if w > len || placed.last().is_some_and(|p| left < p.left + p.label.chars().count() as i64 + gap) {
                ok = false;
                break;
            }
            placed.push(Placed { value: v, cell, label, left });
        }
        if ok || count == 1 {
            if !ok {
                // One tick still does not fit: keep the first label that fits alone.
                placed.truncate(1);
                placed.retain(|p| p.label.chars().count() as i64 <= len);
            }
            return placed;
        }
        count -= 1;
    }
}
