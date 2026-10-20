//! Unit formatters (REQ-FMT-040) and a column's display transform
//! (REQ-DATA-100).
//!
//! [`UnitFormat`] writes a number with its unit, the prefix or unit chosen
//! from the value's size: SI prefixes over any symbol (`1.5 kW`), IEC bytes
//! (`1.5 KiB`), bits a second (`12 Mb/s`), durations from nanoseconds to
//! days (`250 µs`, `1.5 h`), prices rounded to a tick size (`182.40`,
//! `101.25`, `99-16` in 32nds) and signed percentages (`+1.25%`). Digits go
//! through [`NumberFormat`], so the locale's decimal, group and minus apply
//! and nothing formats a float with `core::fmt` (REQ-FMT-030).
//!
//! [`Display`] is a column's `scale`, `offset` and `unit`: displayed value =
//! stored × scale + offset, applied to ticks, readouts and table cells and
//! never to the stored values (REQ-DATA-100).

use super::{js, step_format, Kind, Locale, NumberFormat, Sign, Specifier, PREFIXES};
use crate::data::Column;
use crate::ticks::{tick_step, ticks};

/// What a [`UnitFormat`] writes.
#[derive(Clone, Debug, PartialEq)]
pub enum Unit {
    /// An SI prefix before the symbol, 10³ apart (`1.5 kW`, `20 mV`).
    Si(String),
    /// IEC bytes, 1024 apart: `B`, `KiB`, `MiB`, `GiB`, `TiB`, `PiB`, `EiB`.
    Bytes,
    /// Bits a second, 10³ apart: `b/s`, `kb/s`, `Mb/s`, `Gb/s`, `Tb/s`,
    /// `Pb/s`.
    BitsPerSecond,
    /// A duration given in seconds, written in `ns`, `µs`, `ms`, `s`,
    /// `min`, `h` or `d`, the largest under which it is at least 1.
    Duration,
    /// A price rounded to the nearest multiple of a tick size: decimals for a
    /// tick of at most four of them (0.01, 0.25), else whole and fraction
    /// (`99-16` for 99 16/32 at a tick of 1/32).
    Price(f64),
    /// A fraction as a percentage with its sign, `+` above zero
    /// (`+1.25%`, `-0.40%`, `0.00%`).
    SignedPercent,
}

/// A number with its unit (REQ-FMT-040).
#[derive(Clone, Debug, PartialEq)]
pub struct UnitFormat {
    unit: Unit,
    digits: u32,
    locale: Locale,
}

/// Bytes' IEC units.
const IEC: [&str; 7] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB"];
/// Bit rates' units.
const BITS: [&str; 6] = ["b/s", "kb/s", "Mb/s", "Gb/s", "Tb/s", "Pb/s"];
/// Durations' units, each with its length in seconds.
const DURATIONS: [(&str, f64); 7] = [("ns", 1e-9), ("µs", 1e-6), ("ms", 1e-3), ("s", 1.0), ("min", 60.0), ("h", 3600.0), ("d", 86400.0)];

impl UnitFormat {
    /// A unit format: `digits` significant digits (at least 1) for SI,
    /// bytes, bit rates and durations, decimals for signed percentages;
    /// prices take theirs from the tick size.
    pub fn new(unit: Unit, digits: u32, locale: &Locale) -> UnitFormat {
        UnitFormat { unit, digits, locale: locale.clone() }
    }

    /// One value.
    pub fn format(&self, v: f64) -> String {
        let mut out = String::new();
        self.format_into(v, &mut out);
        out
    }

    /// The number `m` with `digits` significant digits (none after the point
    /// when its integer part has that many), zeros trimmed.
    fn significant(&self, m: f64) -> (f64, String) {
        let p = self.places(m);
        let rounded = (m * js::pow10(f64::from(p))).round() / js::pow10(f64::from(p));
        let spec = Specifier { precision: Some(p), trim: true, kind: Kind::Fixed, ..Specifier::default() };
        (rounded, NumberFormat::new(&spec, &self.locale).format(m))
    }

    /// The places after the point that leave `digits` significant digits.
    fn places(&self, m: f64) -> u32 {
        let int = if m.abs() >= 1.0 { js::floor_log10(m.abs()) as i32 + 1 } else { 1 };
        (self.digits.max(1) as i32 - int).max(0) as u32
    }

    /// `v` in the largest of `units` (each `step` times the last) under which
    /// it is at least 1, moving up when rounding reaches the next.
    fn stepped(&self, v: f64, units: &[&str], step: f64, out: &mut String) {
        let a = v.abs();
        let mut e = 0usize;
        while e + 1 < units.len() && a >= step.powi(e as i32 + 1) {
            e += 1;
        }
        let (rounded, mut text) = self.significant(v / step.powi(e as i32));
        if rounded.abs() >= step && e + 1 < units.len() {
            e += 1;
            text = self.significant(v / step.powi(e as i32)).1;
        }
        out.push_str(&text);
        out.push(' ');
        out.push_str(units[e]);
    }

    /// One value, appended to `out`; nothing (the locale's `nan`) for NaN
    /// or ±∞ (REQ-FMT-030).
    pub fn format_into(&self, v: f64, out: &mut String) {
        if !v.is_finite() {
            out.push_str(&self.locale.nan);
            return;
        }
        match &self.unit {
            Unit::Si(symbol) => {
                let spec = Specifier { precision: Some(self.digits.max(1)), trim: true, kind: Kind::Si, ..Specifier::default() };
                let s = NumberFormat::new(&spec, &self.locale).format(v);
                // d3's `s` ends in its prefix, if any; the unit goes after a space.
                let (number, prefix) = match s.char_indices().last() {
                    Some((i, c)) if PREFIXES.iter().any(|p| p.starts_with(c)) => (&s[..i], &s[i..]),
                    _ => (s.as_str(), ""),
                };
                out.push_str(number);
                out.push(' ');
                out.push_str(prefix);
                out.push_str(symbol);
            }
            Unit::Bytes => self.stepped(v, &IEC, 1024.0, out),
            Unit::BitsPerSecond => self.stepped(v, &BITS, 1000.0, out),
            Unit::Duration => {
                let a = v.abs();
                let mut i = DURATIONS.iter().rposition(|&(_, len)| a >= len).unwrap_or(0);
                if a == 0.0 {
                    i = 3;
                }
                let (rounded, mut text) = self.significant(v / DURATIONS[i].1);
                if i + 1 < DURATIONS.len() && rounded.abs() * DURATIONS[i].1 >= DURATIONS[i + 1].1 {
                    i += 1;
                    text = self.significant(v / DURATIONS[i].1).1;
                }
                out.push_str(&text);
                out.push(' ');
                out.push_str(DURATIONS[i].0);
            }
            Unit::Price(tick) => self.price(v, *tick, out),
            Unit::SignedPercent => {
                let d = self.digits.min(20);
                let spec = Specifier { precision: Some(d), kind: Kind::Percent, sign: Sign::Minus, ..Specifier::default() };
                // A plus above zero; a value that rounds to zero has no sign.
                if (v * 100.0 * js::pow10(f64::from(d))).round() > 0.0 {
                    out.push('+');
                }
                NumberFormat::new(&spec, &self.locale).format_into(v, out);
            }
        }
    }

    /// A price at the nearest multiple of `tick`.
    fn price(&self, v: f64, tick: f64, out: &mut String) {
        if !(tick > 0.0 && tick.is_finite()) {
            out.push_str(&NumberFormat::new(&Specifier { comma: true, ..Specifier::default() }, &self.locale).format(v));
            return;
        }
        let r = js::round(v / tick) * tick;
        let places = (0..=10).find(|&p| {
            let x = tick * js::pow10(f64::from(p));
            (x - x.round()).abs() < 1e-9 * x.max(1.0)
        });
        match places {
            Some(p) if p <= 4 => {
                let spec = Specifier { comma: true, precision: Some(p), kind: Kind::Fixed, ..Specifier::default() };
                out.push_str(&NumberFormat::new(&spec, &self.locale).format(r));
            }
            _ => {
                // Whole and fraction: 99-16 is 99 16/32, the numerator padded to the denominator's digits.
                let n = (1.0 / tick).round();
                let a = r.abs();
                let mut whole = a.trunc();
                let mut num = ((a - whole) * n).round();
                if num >= n {
                    whole += 1.0;
                    num = 0.0;
                }
                if r < 0.0 && (whole > 0.0 || num > 0.0) {
                    out.push_str(&self.locale.minus);
                }
                let int = Specifier { comma: true, precision: Some(0), kind: Kind::Fixed, ..Specifier::default() };
                out.push_str(&NumberFormat::new(&int, &self.locale).format(whole));
                out.push('-');
                let width = js::floor_log10(n - 1.0).max(0.0) as usize + 1;
                let digits = NumberFormat::new(&Specifier { precision: Some(0), kind: Kind::Fixed, ..Specifier::default() }, &self.locale).format(num);
                for _ in digits.chars().count()..width {
                    out.push_str(self.locale.numerals.as_ref().map_or("0", |n| n[0].as_str()));
                }
                out.push_str(&digits);
            }
        }
    }
}

/// A column's display transform (REQ-DATA-100): displayed value = stored ×
/// `scale` + `offset`, with `unit` as text beside it. It applies to ticks,
/// readouts and table cells; the stored values never change.
#[derive(Clone, Debug, PartialEq)]
pub struct Display {
    /// The factor, 1 when the column gives none.
    pub scale: f64,
    /// The term added after the factor, 0 when the column gives none.
    pub offset: f64,
    /// The unit's text (`USD`), not a conversion.
    pub unit: Option<String>,
}

impl Default for Display {
    fn default() -> Display {
        Display { scale: 1.0, offset: 0.0, unit: None }
    }
}

impl Display {
    /// The transform a column declares with `scale`, `offset` and `unit`;
    /// a scale that is 0 or not finite counts as 1.
    pub fn of(column: &Column) -> Display {
        Display::new(column.scale, column.offset, column.unit.as_deref())
    }

    /// A transform from its parts, each optional.
    pub fn new(scale: Option<f64>, offset: Option<f64>, unit: Option<&str>) -> Display {
        Display {
            scale: scale.filter(|s| s.is_finite() && *s != 0.0).unwrap_or(1.0),
            offset: offset.filter(|o| o.is_finite()).unwrap_or(0.0),
            unit: unit.map(str::to_string),
        }
    }

    /// Whether it changes no number.
    pub fn is_identity(&self) -> bool {
        self.scale == 1.0 && self.offset == 0.0
    }

    /// The displayed value of a stored one.
    pub fn shown(&self, stored: f64) -> f64 {
        if self.is_identity() {
            stored
        } else {
            stored * self.scale + self.offset
        }
    }

    /// The stored value a displayed one stands for.
    pub fn stored(&self, shown: f64) -> f64 {
        if self.is_identity() {
            shown
        } else {
            (shown - self.offset) / self.scale
        }
    }

    /// A stored domain as displayed. The map is affine, so a linear axis over
    /// the displayed domain puts every value where the stored domain would:
    /// a chart may fit ticks to it and place them unchanged.
    pub fn domain(&self, [a, b]: [f64; 2]) -> [f64; 2] {
        [self.shown(a), self.shown(b)]
    }

    /// The decimals the scale implies: 2 for 0.01, none for a whole factor
    /// or the identity.
    pub fn places(&self) -> Option<u32> {
        let s = self.scale.abs();
        if self.is_identity() || s >= 1.0 {
            return None;
        }
        (1..=12).find(|&p| {
            let x = s * js::pow10(f64::from(p));
            (x - x.round()).abs() < 1e-9 * x
        })
    }

    /// The format of readouts and table cells: `spec`, with the scale's
    /// decimals as its precision when it names none (`auto` becomes `,.Nf`).
    pub fn readout_format(&self, spec: &Specifier, locale: &Locale) -> NumberFormat {
        let mut spec = spec.clone();
        if let (None, Some(p)) = (spec.precision, self.places()) {
            if spec.kind == Kind::Auto {
                spec.kind = Kind::Fixed;
                spec.comma = true;
            }
            spec.precision = Some(p);
        }
        NumberFormat::new(&spec, locale)
    }

    /// A readout or table cell of a stored value: `182.40` for stored 18240
    /// at scale 0.01 (REQ-DATA-100).
    pub fn readout(&self, stored: f64, spec: &Specifier, locale: &Locale) -> String {
        self.readout_format(spec, locale).format(self.shown(stored))
    }

    /// Ticks over a stored domain, chosen and labelled in displayed units
    /// (d3's ticks and tick format, REQ-FMT-010) and given back as stored
    /// values to place, with their labels.
    pub fn tick_labels(&self, stored: [f64; 2], count: f64, spec: &Specifier, locale: &Locale) -> Vec<(f64, String)> {
        let [a, b] = self.domain(stored);
        let f = step_format(tick_step(a, b, count), a, b, spec, locale);
        ticks(a, b, count).into_iter().map(|t| (self.stored(t), f.format(t))).collect()
    }

    /// `text` followed by the unit, a space apart, when there is one.
    pub fn with_unit(&self, text: &str) -> String {
        match &self.unit {
            Some(u) if !u.is_empty() => format!("{text} {u}"),
            _ => text.to_string(),
        }
    }
}
