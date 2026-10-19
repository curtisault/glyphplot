//! Tick values: d3-array's linear ticks and d3-time's UTC ticks (REQ-TICK-010
//! to 040).
//!
//! [`ticks`], [`tick_increment`] and [`tick_step`] are d3-array 3.2.4's
//! `ticks`, `tickIncrement` and `tickStep` (`src/ticks.js`, ISC licence,
//! © Mike Bostock), line for line, with JavaScript's `Math.round`,
//! `Math.pow` and `Math.log10` where d3 calls them (`format::js`), so every
//! tick equals d3's bit for bit (REQ-TICK-010). UTC calendar ticks are
//! d3-time's `utcTicks`, in [`crate::time`] and re-exported here
//! (REQ-TICK-020).

// d3 tests `!(x > 0)` so that NaN takes the same branch; so does this port.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use crate::format::js;

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

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

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
    }
}
