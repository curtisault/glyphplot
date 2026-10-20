//! The readout (REQ-CANDLE-050, REQ-OVERLAY-020) and the accessible text's
//! numbers.
//!
//! The readout names the candle under the cursor: its time (with its
//! interval when resampled), `open`, `high`, `low` and `close` by their
//! columns' names (`open`, `high`, `low` and the column's name when one
//! column gives all four), the volume, and the signed change from the previous
//! candle's close (from its own open for the first), with the change as a
//! signed percentage. Prices are written through the close column's display
//! transform (REQ-DATA-100) at the price axis's tick precision plus two
//! digits, or at the decimals the transform's scale implies; every digit
//! comes from the core's formatters (REQ-FMT-030). The parts are packed into
//! lines no wider than the plot, two spaces apart, drawn from the plot's top
//! right.

use super::data::Prepared;
use super::resample::Level;
use crate::format::{js, Display, Kind as FormatKind, Locale, NumberFormat, Specifier, Unit, UnitFormat};
use crate::scale::Kind;
use crate::text;
use crate::time::{Zone, DAY, MINUTE, SECOND};

/// The most digits after the point a price is written with.
const MAX_DIGITS: u32 = 12;

/// The digits after the point a price takes: the tick step's, two more, or
/// the display's own.
pub(super) fn price_digits(step: f64, display: &Display) -> u32 {
    if let Some(p) = display.places() {
        return p;
    }
    let from_step = js::exponent(step).map_or(0, |e| (-e).max(0) as u32);
    (from_step + 2).min(MAX_DIGITS)
}

/// The price format at `digits` after the point, grouped.
pub(super) fn price_format(digits: u32, locale: &Locale) -> NumberFormat {
    NumberFormat::new(&Specifier { comma: true, precision: Some(digits), kind: FormatKind::Fixed, ..Specifier::default() }, locale)
}

/// A volume as written: grouped, up to six decimals, zeros trimmed.
pub(super) fn volume_text(v: f64, display: &Display, locale: &Locale) -> String {
    let spec = Specifier { comma: true, trim: true, kind: FormatKind::Fixed, ..Specifier::default() };
    display.readout(v, &spec, locale)
}

/// The strftime pattern of a candle's time: its interval's, or for the rows
/// themselves the one their spacing asks for.
pub(super) fn time_pattern(level: &Level, data: &Prepared) -> &'static str {
    if let Some(i) = level.interval {
        return i.pattern();
    }
    let xs = data.xs();
    let r = data.rows.clone();
    let spacing = if r.len() > 1 { (xs[r.end - 1] - xs[r.start]) / (r.len() - 1) as f64 } else { DAY };
    if spacing >= DAY {
        "%Y-%m-%d"
    } else if spacing >= MINUTE {
        "%Y-%m-%d %H:%M"
    } else if spacing >= SECOND {
        "%Y-%m-%d %H:%M:%S"
    } else {
        "%Y-%m-%d %H:%M:%S.%L"
    }
}

/// A candle's time as the readout and the table write it.
pub(super) fn time_text(level: &Level, k: usize, data: &Prepared, zone: &Zone, locale: &Locale) -> String {
    let t = level.stamp[k];
    if data.kind != Kind::Time {
        return NumberFormat::new(&Specifier::default(), locale).format(t);
    }
    let mut out = String::new();
    zone.strftime(time_pattern(level, data), t, &locale.time, &mut out);
    out
}

/// What the readout is written from.
pub(super) struct Input<'a> {
    /// The data.
    pub data: &'a Prepared,
    /// The level holding the candle.
    pub level: &'a Level,
    /// The candle.
    pub k: usize,
    /// The previous candle's close, NaN when there is none.
    pub prev: f64,
    /// The price axis's displayed tick step.
    pub step: f64,
    /// The zone times are written in.
    pub zone: &'a Zone,
    /// The locale.
    pub locale: &'a Locale,
}

/// The readout's parts, in order (REQ-CANDLE-050).
pub(super) fn parts(input: &Input) -> Vec<String> {
    let Input { data, level, k, prev, step, zone, locale } = *input;
    let mut out = Vec::with_capacity(7);
    let mut when = format!("{} {}", data.x_name, time_text(level, k, data, zone, locale));
    if let Some(i) = level.interval {
        when.push_str(&format!(" ({})", i.name()));
    }
    out.push(when);
    let digits = price_digits(step, &data.price);
    let f = price_format(digits, locale);
    for (j, name) in data.names.iter().enumerate() {
        // One column for all four: `open`, `high` and `low`, then its name.
        let name = if data.single && j < 3 { super::data::PRICES[j] } else { name.as_str() };
        let v = level.ohlc[j][k];
        let shown = if v.is_finite() { f.format(data.price.shown(v)) } else { String::new() };
        let mut part = format!("{name} {shown}");
        if j == 3 {
            part = data.price.with_unit(&part);
        }
        out.push(part);
    }
    if data.has_volume() && level.volume[k].is_finite() {
        out.push(data.vol.with_unit(&format!("{} {}", data.volume_name, volume_text(level.volume[k], &data.vol, locale))));
    }
    let (o, c) = (level.ohlc[0][k], level.ohlc[3][k]);
    let from = if prev.is_finite() { prev } else { o };
    if c.is_finite() && from.is_finite() {
        let change = data.price.shown(c) - data.price.shown(from);
        let signed = Specifier { sign: crate::format::Sign::Plus, comma: true, precision: Some(digits), kind: FormatKind::Fixed, ..Specifier::default() };
        let mut part = NumberFormat::new(&signed, locale).format(change);
        let base = data.price.shown(from);
        if base != 0.0 {
            part.push_str(&format!(" ({})", UnitFormat::new(Unit::SignedPercent, 2, locale).format(change / base.abs())));
        }
        out.push(part);
    }
    out
}

/// `parts` packed into lines of at most `width` cells, two spaces apart.
pub(super) fn lines(parts: &[String], width: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for p in parts {
        match out.last_mut() {
            Some(l) if text::width(l) + 2 + text::width(p) <= width => {
                l.push_str("  ");
                l.push_str(p);
            }
            _ => out.push(p.clone()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parts_pack_into_lines() {
        let parts: Vec<String> = ["t 2024-11-01", "open 1.00", "high 2.00"].map(String::from).to_vec();
        assert_eq!(lines(&parts, 80), ["t 2024-11-01  open 1.00  high 2.00"]);
        assert_eq!(lines(&parts, 24), ["t 2024-11-01  open 1.00", "high 2.00"]);
        assert_eq!(price_digits(5.0, &Display::default()), 2);
        assert_eq!(price_digits(0.5, &Display::default()), 3);
        assert_eq!(price_digits(5.0, &Display::new(Some(0.01), None, None)), 2);
    }
}
