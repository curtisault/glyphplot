//! The readout's text: the cursor's x, named by the x column, then the
//! snapped series' name and value (REQ-LINE-040, REQ-OVERLAY-020).
//!
//! A number is written in its axis's tick format, d3-scale's `,f` at the
//! precision of the drawn tick step, with as many more digits after the point
//! as tell the datum apart from its neighbours in the series, at most 17
//! significant digits and 20 after the point. A value those limits cannot
//! tell apart from a neighbour (one far below the step) takes d3-format's
//! default instead, twelve significant digits. A time x is a date and time, to the millisecond
//! when its neighbours fall in the same second or the view spans under ten
//! seconds. Every digit comes from the core's formatters (REQ-FMT-030).

use super::draw::{self, Geom};
use super::series::Prepared;
use crate::format::{js, Kind as FormatKind, Locale, NumberFormat, Specifier};
use crate::scale::Kind;
use crate::view::View;

/// The most digits after the point a readout writes, as `toFixed` allows.
const MAX_DIGITS: usize = 20;
/// The most significant digits a readout writes: past them an `f64` holds
/// no more.
const MAX_SIGNIFICANT: i32 = 17;

/// The readout's number formats, `,f` by precision, made once a locale.
#[derive(Debug, Default)]
pub(super) struct Formats {
    fixed: Vec<Option<NumberFormat>>,
}

impl Formats {
    /// Forgets every format, for a new locale.
    pub fn clear(&mut self) {
        self.fixed.clear();
    }

    /// `,f` with `p` digits after the point.
    fn fixed(&mut self, p: usize, locale: &Locale) -> &NumberFormat {
        if self.fixed.len() <= p {
            self.fixed.resize(p + 1, None);
        }
        self.fixed[p].get_or_insert_with(|| {
            let spec = Specifier { comma: true, precision: Some(p as u32), kind: FormatKind::Fixed, ..Specifier::default() };
            NumberFormat::new(&spec, locale)
        })
    }

    /// `v` in the tick format of an axis whose ticks are `step` apart, with
    /// the digits that tell it apart from `near` (REQ-FMT-030).
    pub fn number(&mut self, v: f64, step: f64, near: &[f64], locale: &Locale, out: &mut String) {
        match digits(v, step, near) {
            Some(p) => self.fixed(p, locale).format_into(v, out),
            None => out.push_str(&draw::number(v, locale)),
        }
    }
}

/// 10^p for the readout's few digits.
fn pow10(p: usize) -> f64 {
    10f64.powi(p as i32)
}

/// The digits after the point of the tick format of a step: d3-format's
/// `precisionFixed(step)`.
fn step_digits(step: f64) -> usize {
    js::exponent(step).map_or(0, |e| (-e).max(0) as usize)
}

/// The digits after the point that write `v`: the step's, then more until
/// `v` and each neighbour of another value round apart; `None` when the
/// limits come first.
fn digits(v: f64, step: f64, near: &[f64]) -> Option<usize> {
    let p0 = step_digits(step).min(MAX_DIGITS);
    let cap = match js::exponent(v) {
        Some(e) => (MAX_SIGNIFICANT - 1 - e).clamp(0, MAX_DIGITS as i32) as usize,
        None => MAX_DIGITS,
    }
    .max(p0);
    let mut p = p0;
    for &n in near {
        if !n.is_finite() || n == v {
            continue;
        }
        // From the digit of their difference, a digit more while they round
        // the same.
        let mut q = ((-(v - n).abs().log10()).floor().max(0.0) as usize).clamp(p, cap);
        let same = |q: usize| (v * pow10(q)).round() == (n * pow10(q)).round();
        while q < cap && same(q) {
            q += 1;
        }
        if same(q) {
            return None;
        }
        p = p.max(q);
    }
    Some(p)
}

/// The values before and after position `i` of `values`, NaN past the ends.
fn neighbours(values: &[f64], i: usize) -> [f64; 2] {
    let at = |j: Option<usize>| j.and_then(|j| values.get(j)).copied().unwrap_or(f64::NAN);
    [at(i.checked_sub(1)), at(i.checked_add(1))]
}

/// The readout of the view's cursor, `None` without one: `<x name> <x>`,
/// then `  <series> <y>` when it snapped to a datum (REQ-LINE-040). A cursor
/// naming a datum (series and row) writes that datum's x and y from the
/// data, not the view's copies, so a view read back from JSON whose numbers
/// came back an ulp off still writes the same readout (contract rule 16).
pub(super) fn text(data: &Prepared, view: &View, geom: &Geom, locale: &Locale, formats: &mut Formats) -> Option<String> {
    let c = view.cursor.as_ref()?;
    let at = c.series.as_deref().and_then(|name| data.find(name)).and_then(|s| Some((s, data.pos(s, usize::try_from(c.index?).ok()?)?)));
    let x = at.map(|(s, i)| data.xs(s)[i]).filter(|x| x.is_finite()).or(c.x)?;
    let y = match at {
        Some((s, i)) => Some(data.ys(s)[i]).filter(|y| y.is_finite()).or(c.y),
        None => c.y,
    };
    let near_x = at.map_or([f64::NAN; 2], |(s, i)| neighbours(data.xs(s), i));
    let mut out = String::with_capacity(48);
    out.push_str(&data.x_name);
    out.push(' ');
    match data.kind {
        Kind::Linear => formats.number(x, geom.x_step, &near_x, locale, &mut out),
        Kind::Time => {
            let second = |v: f64| (v / 1000.0).floor();
            let close = near_x.iter().any(|&n| n.is_finite() && n != x && second(n) == second(x));
            let span = if close { 0.0 } else { geom.x.hi() - geom.x.lo() };
            out.push_str(&draw::x_label(x, Kind::Time, span, locale));
        }
    }
    if let Some(series) = &c.series {
        out.push_str("  ");
        out.push_str(series);
        if let Some(y) = y {
            let near_y = at.map_or([f64::NAN; 2], |(s, i)| neighbours(data.ys(s), i));
            out.push(' ');
            formats.number(y, geom.y_step, &near_y, locale, &mut out);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(v: f64, step: f64, near: &[f64]) -> String {
        let mut out = String::new();
        Formats::default().number(v, step, near, &Locale::default(), &mut out);
        out
    }

    /// REQ-FMT-030, REQ-LINE-040: the axis's format, grouped, at its step's
    /// digits, with more only where a neighbour needs them.
    #[test]
    fn numbers_take_the_axis_format_and_the_digits_to_tell_neighbours_apart() {
        assert_eq!(write(5001.0, 1000.0, &[5000.0, 5002.0]), "5,001");
        assert_eq!(write(0.998026728428, 0.2, &[]), "1.0");
        assert_eq!(write(0.998026728428, 0.2, &[0.9977, 0.9983]), "0.9980");
        assert_eq!(write(42.0, 1.0, &[42.0, 42.0]), "42", "equal neighbours ask for nothing");
        assert_eq!(write(-0.04, 0.2, &[]), "0.0", "no -0");
        assert_eq!(write(1.2e-16, 0.2, &[0.0063, -0.0063]), "0.00", "a crossing's rounding error reads as zero");
        // tiny-range's data, 10⁻¹⁵ apart near 1: read back, the text is nearer
        // its datum than either neighbour.
        let (v, near) = (1.000_000_000_000_4, [1.000_000_000_000_399, 1.000_000_000_000_401]);
        let back: f64 = write(v, 1e-13, &near).parse().unwrap();
        assert!(near.iter().all(|n| (back - v).abs() < (back - n).abs()), "{back}");
        assert_eq!(write(4e-163, 1e299, &[0.0]), "4e-163", "far below the step: twelve significant digits");
        assert_eq!(write(1e300, 1e299, &[9e299]), "1e+300");
        assert_eq!(write(f64::NAN, 1.0, &[]), "");
    }
}
