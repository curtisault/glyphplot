//! Number formatting: a d3-format subset, written by hand (REQ-FMT-010 to 030,
//! REQ-I18N-030).
//!
//! A specifier is `[sign][$][,][.precision][~][type]` with the types `f`,
//! `s`, `%`, `e` and `d`, or the whole word `auto` (REQ-FMT-020). Formatting
//! is d3-format 3.1.2's `locale.format` and `locale.formatPrefix`
//! (`src/locale.js`, `formatTypes.js`, `formatDecimal.js`,
//! `formatPrefixAuto.js`, `formatTrim.js`, `formatGroup.js`,
//! `precisionFixed.js`, `precisionPrefix.js`, `precisionRound.js`; ISC
//! licence, © Mike Bostock), and tick labels are d3-scale 4.0.2's
//! `tickFormat` (`src/tickFormat.js`, ISC): one format for the axis,
//! precision from the tick step, and for `s` one SI prefix from the larger end
//! of the domain (REQ-FMT-010).
//!
//! Digits come from [`js`], which writes JavaScript's `toFixed`,
//! `toExponential`, `toPrecision` and `toString` from the exact value of the
//! `f64`; nothing here formats a float with `core::fmt` (REQ-FMT-030).
//!
//! [`UnitFormat`] writes SI units, IEC bytes, bit rates, durations, prices to
//! a tick size and signed percentages (REQ-FMT-040), and [`Display`] is a
//! column's `scale`, `offset` and `unit`, applied to ticks, readouts and the
//! table and never to stored values (REQ-DATA-100). Date labels are
//! `crate::time`'s strftime subset (REQ-FMT-050), on the locale pack's hour
//! cycle.
//!
//! Where glyphplot differs from d3, on purpose:
//! - NaN and ±∞ format as the locale's `nan`, empty by default, with no sign,
//!   currency or suffix; d3 writes `NaN` and `Infinity` (REQ-FMT-030).
//! - A negative number that rounds to zero loses its sign under every sign
//!   option; d3 keeps it under `+`, writing `-0` (REQ-FMT-030).
//! - The default locale's minus is `-`, not d3's U+2212 (REQ-I18N-030).

mod big;
mod fdlibm;
pub mod js;
mod units;

pub use units::{Display, Unit, UnitFormat};

use crate::ticks::tick_step;
use crate::time::TimeLocale;

/// The SI prefixes from 10⁻²⁴ (y) to 10²⁴ (Y), as d3-format writes them
/// (µ is U+00B5).
pub const PREFIXES: [&str; 17] = ["y", "z", "a", "f", "p", "n", "µ", "m", "", "k", "M", "G", "T", "P", "E", "Z", "Y"];

/// The symbols and names labels are written with. The default is plain:
/// `.` for the decimal, `,` between groups of three, ASCII digits and `-`
/// (REQ-I18N-030). A locale pack (`spec/schemas/locale.schema.json`)
/// replaces it from v0.3.0.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Locale {
    /// The decimal separator.
    pub decimal: String,
    /// The group separator; empty for none.
    pub group: String,
    /// Group sizes from the decimal point leftward, the last repeating.
    pub grouping: Vec<usize>,
    /// The currency's prefix and suffix, for `$`.
    pub currency: [String; 2],
    /// The ten digits, 0 to 9; None for ASCII.
    pub numerals: Option<[String; 10]>,
    /// The suffix of the `%` type.
    pub percent: String,
    /// The minus sign.
    pub minus: String,
    /// What NaN and ±∞ format as (REQ-FMT-030).
    pub nan: String,
    /// Month, weekday and period names for dates.
    pub time: TimeLocale,
}

impl Default for Locale {
    fn default() -> Locale {
        Locale {
            decimal: ".".into(),
            group: ",".into(),
            grouping: vec![3],
            currency: ["$".into(), String::new()],
            numerals: None,
            percent: "%".into(),
            minus: "-".into(),
            nan: String::new(),
            time: TimeLocale::default(),
        }
    }
}

/// A separator as drawn: U+2009, U+202F and U+00A0, which fonts draw at
/// different widths, become one plain space cell (REQ-FMT-060).
fn plain_spaces(s: &str) -> String {
    s.chars().map(|c| if matches!(c, '\u{2009}' | '\u{202F}' | '\u{00A0}') { ' ' } else { c }).collect()
}

/// `N` strings from a list, or `None` when it holds another number of them.
fn exactly<const N: usize>(v: &[String]) -> Option<[String; N]> {
    v.to_vec().try_into().ok()
}

impl Locale {
    /// The locale a pack (`spec/schemas/locale.schema.json`) gives
    /// (REQ-I18N-020): its separators, with U+2009, U+202F and U+00A0 drawn
    /// as one plain space cell; its minus, `-` in the default pack and U+2212
    /// when the pack says so (REQ-FMT-060); its digits (ASCII ones as none);
    /// the suffix of its percent pattern; and its month, weekday and period
    /// names, and its hour cycle for time labels (REQ-FMT-050). A list of
    /// the wrong length keeps the default's.
    pub fn from_pack(pack: &crate::locale::LocalePack) -> Locale {
        let base = Locale::default();
        let ascii = pack.numerals.iter().enumerate().all(|(d, n)| n.len() == 1 && n.as_bytes()[0] == b'0' + d as u8);
        let numerals = if ascii { None } else { exactly::<10>(&pack.numerals) };
        let percent = plain_spaces(&pack.percent.replacen('#', "", 1));
        let time = TimeLocale {
            hour12: pack.hour_cycle == crate::locale::HourCycle::H12,
            periods: pack.periods.clone(),
            days: exactly(&pack.days).unwrap_or(base.time.days.clone()),
            days_short: exactly(&pack.days_short).unwrap_or(base.time.days_short.clone()),
            months: exactly(&pack.months).unwrap_or(base.time.months.clone()),
            months_short: exactly(&pack.months_short).unwrap_or(base.time.months_short.clone()),
        };
        Locale {
            decimal: plain_spaces(&pack.decimal),
            group: plain_spaces(&pack.group),
            grouping: pack.grouping.iter().map(|&g| g.max(1) as usize).collect(),
            currency: [plain_spaces(&pack.currency[0]), plain_spaces(&pack.currency[1])],
            numerals,
            percent,
            minus: pack.minus.clone(),
            nan: base.nan,
            time,
        }
    }
}

/// A specifier that is not in the subset, with the option it came from
/// (REQ-FMT-020).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormatError {
    /// The option that held it, as the caller names it (`axes[0].format`).
    pub option: String,
    /// The specifier as given.
    pub specifier: String,
    /// What is wrong with it.
    pub reason: &'static str,
    /// Whether it was a date pattern (REQ-FMT-050) rather than a number
    /// specifier (REQ-FMT-020).
    pub date: bool,
}

impl core::fmt::Display for FormatError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.option)?;
        f.write_str(": \"")?;
        f.write_str(&self.specifier)?;
        f.write_str(if self.date { "\" is not a date format: " } else { "\" is not a number format: " })?;
        f.write_str(self.reason)
    }
}

impl std::error::Error for FormatError {}

/// The sign option.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sign {
    /// `-`: a minus for negatives only (the default).
    #[default]
    Minus,
    /// `+`: a plus for positives and zero, a minus for negatives.
    Plus,
    /// `(`: negatives in parentheses.
    Parens,
    /// ` `: a space for positives and zero, a minus for negatives.
    Space,
}

/// The type of a specifier.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Kind {
    /// `f`: fixed point.
    Fixed,
    /// `s`: an SI prefix, precision in significant digits.
    Si,
    /// `%`: times 100, fixed point, with a percent sign.
    Percent,
    /// `e`: exponent notation.
    Exponent,
    /// `d`: an integer, rounded.
    Integer,
    /// `auto`: on an axis, `,f` with precision from the tick step; alone,
    /// d3-format's default, 12 significant digits with zeros trimmed.
    #[default]
    Auto,
}

/// A parsed specifier: `[sign][$][,][.precision][~][type]` or `auto`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Specifier {
    /// The sign option.
    pub sign: Sign,
    /// `$`: the locale's currency around the number.
    pub currency: bool,
    /// `,`: group separators.
    pub comma: bool,
    /// `.n`: digits after the point (`f`, `%`, `e`) or significant digits
    /// (`s`); None to take it from the tick step, or d3's default 6.
    pub precision: Option<u32>,
    /// `~`: trailing zeros trimmed.
    pub trim: bool,
    /// The type.
    pub kind: Kind,
}

impl Specifier {
    /// Parses a specifier from the option named `option`, refusing anything
    /// outside the subset with an error that names the option (REQ-FMT-020).
    pub fn parse(option: &str, specifier: &str) -> Result<Specifier, FormatError> {
        let error = |reason| Err(FormatError { option: option.into(), specifier: specifier.into(), reason, date: false });
        if specifier == "auto" {
            return Ok(Specifier::default());
        }
        let b = specifier.as_bytes();
        let mut i = 0;
        let mut spec = Specifier::default();
        let sign = match b.first() {
            Some(b'-') => Some(Sign::Minus),
            Some(b'+') => Some(Sign::Plus),
            Some(b'(') => Some(Sign::Parens),
            Some(b' ') => Some(Sign::Space),
            _ => None,
        };
        if let Some(sign) = sign {
            spec.sign = sign;
            i += 1;
        }
        if b.get(i) == Some(&b'$') {
            spec.currency = true;
            i += 1;
        }
        if b.get(i) == Some(&b',') {
            spec.comma = true;
            i += 1;
        }
        if b.get(i) == Some(&b'.') {
            i += 1;
            let start = i;
            let mut p = 0u32;
            while let Some(&c) = b.get(i).filter(|c| c.is_ascii_digit()) {
                p = p.saturating_mul(10).saturating_add(u32::from(c - b'0'));
                i += 1;
            }
            if i == start {
                return error("a `.` must be followed by the precision's digits");
            }
            spec.precision = Some(p);
        }
        if b.get(i) == Some(&b'~') {
            spec.trim = true;
            i += 1;
        }
        spec.kind = match b.get(i) {
            Some(b'f') => Kind::Fixed,
            Some(b's') => Kind::Si,
            Some(b'%') => Kind::Percent,
            Some(b'e') => Kind::Exponent,
            Some(b'd') => Kind::Integer,
            None => return error("it has no type; the types are f, s, %, e and d, or the whole word auto"),
            Some(c) if c.is_ascii_alphabetic() => return error("its type is not one of f, s, %, e and d"),
            Some(_) => return error("the subset is [sign][$][,][.precision][~][type]; fill, alignment, width, zero padding and # are not in it"),
        };
        if i + 1 != b.len() {
            return error("the subset is [sign][$][,][.precision][~][type], ending at the type");
        }
        Ok(spec)
    }
}

/// d3-format's types this module writes; `General` is d3's `g`, which only
/// `auto` reaches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ty {
    Fixed,
    Si,
    Percent,
    Exponent,
    Integer,
    General,
}

/// A number format ready to write labels: d3-format's `format(specifier)`,
/// or `formatPrefix` when it carries an SI scale.
#[derive(Clone, Debug, PartialEq)]
pub struct NumberFormat {
    sign: Sign,
    currency: bool,
    comma: bool,
    trim: bool,
    ty: Ty,
    precision: usize,
    /// `formatPrefix`'s factor and SI suffix.
    scale: Option<(f64, &'static str)>,
    locale: Locale,
}

impl NumberFormat {
    /// The format of a specifier on its own, as d3-format's `format`; `auto`
    /// is d3's default, 12 significant digits with zeros trimmed.
    pub fn new(spec: &Specifier, locale: &Locale) -> NumberFormat {
        let (ty, precision, trim) = match spec.kind {
            Kind::Fixed => (Ty::Fixed, spec.precision, spec.trim),
            Kind::Si => (Ty::Si, spec.precision, spec.trim),
            Kind::Percent => (Ty::Percent, spec.precision, spec.trim),
            Kind::Exponent => (Ty::Exponent, spec.precision, spec.trim),
            Kind::Integer => (Ty::Integer, spec.precision, spec.trim),
            Kind::Auto => (Ty::General, Some(spec.precision.unwrap_or(12)), true),
        };
        let precision = match precision {
            None => 6,
            Some(p) if matches!(ty, Ty::Si | Ty::General) => p.clamp(1, 21) as usize,
            Some(p) => p.min(20) as usize,
        };
        NumberFormat { sign: spec.sign, currency: spec.currency, comma: spec.comma, trim, ty, precision, scale: None, locale: locale.clone() }
    }

    /// One label.
    pub fn format(&self, x: f64) -> String {
        let mut out = String::new();
        self.format_into(x, &mut out);
        out
    }

    /// One label, appended to `out`.
    pub fn format_into(&self, x: f64, out: &mut String) {
        let value = match self.scale {
            Some((k, _)) => k * x,
            None => x,
        };
        let a = value.abs();
        // ±∞, NaN, and a percentage past the largest double read as `nan`.
        if !value.is_finite() || (self.ty == Ty::Percent && !(a * 100.0).is_finite()) {
            out.push_str(&self.locale.nan);
            return;
        }
        let mut negative = value < 0.0 || (value == 0.0 && value.is_sign_negative());
        let mut digits = String::new();
        let mut si = None;
        match self.ty {
            Ty::Fixed => js::to_fixed(a, self.precision, &mut digits),
            Ty::Percent => js::to_fixed(a * 100.0, self.precision, &mut digits),
            Ty::Exponent => js::to_exponential(a, Some(self.precision), &mut digits),
            Ty::General => js::to_precision(a, self.precision, &mut digits),
            Ty::Integer => {
                let r = js::round(a);
                if r >= 1e21 {
                    js::to_integer_string(r, &mut digits);
                } else {
                    js::to_string(r, &mut digits);
                }
            }
            Ty::Si => si = prefix_auto(a, self.precision, &mut digits),
        }
        if self.trim {
            digits = trim(&digits);
        }
        if negative && reads_zero(&digits) {
            negative = false;
        }
        let l = &self.locale;
        let mut s = String::new();
        if negative {
            s.push_str(if self.sign == Sign::Parens { "(" } else { l.minus.as_str() });
        } else if self.sign == Sign::Plus {
            s.push('+');
        } else if self.sign == Sign::Space {
            s.push(' ');
        }
        if self.currency {
            s.push_str(&l.currency[0]);
        }
        // The integer part, grouped, then the rest with the locale's decimal.
        let split = digits.bytes().position(|c| !c.is_ascii_digit()).unwrap_or(digits.len());
        let (int, rest) = digits.split_at(split);
        if self.comma {
            group(int, &l.grouping, &l.group, &mut s);
        } else {
            s.push_str(int);
        }
        if let Some(fraction) = rest.strip_prefix('.') {
            s.push_str(&l.decimal);
            s.push_str(fraction);
        } else {
            s.push_str(rest);
        }
        if let Some(e) = si {
            s.push_str(PREFIXES[(8 + e / 3) as usize]);
        }
        if self.currency {
            s.push_str(&l.currency[1]);
        } else if self.ty == Ty::Percent {
            s.push_str(&l.percent);
        }
        if let Some((_, suffix)) = self.scale {
            s.push_str(suffix);
        }
        if negative && self.sign == Sign::Parens {
            s.push(')');
        }
        match &l.numerals {
            None => out.push_str(&s),
            Some(numerals) => {
                for c in s.chars() {
                    match c.to_digit(10) {
                        Some(d) => out.push_str(&numerals[d as usize]),
                        None => out.push(c),
                    }
                }
            }
        }
    }
}

/// d3-format's `formatPrefixAuto(x, p)` for finite x ≥ 0: p significant
/// digits with the point moved to an SI prefix's exponent, which it returns
/// (None for 0, which gets no prefix).
fn prefix_auto(x: f64, p: usize, out: &mut String) -> Option<i32> {
    if x == 0.0 {
        js::to_precision(x, p, out);
        return None;
    }
    let (coefficient, exponent) = js::decimal_parts(x, Some(p));
    let e = exponent.div_euclid(3).clamp(-8, 8) * 3;
    let i = exponent - e + 1;
    let n = coefficient.len() as i32;
    let push = |digits: &[u8], out: &mut String| out.extend(digits.iter().map(|&d| char::from(b'0' + d)));
    if i == n {
        push(&coefficient, out);
    } else if i > n {
        push(&coefficient, out);
        out.extend(core::iter::repeat_n('0', (i - n) as usize));
    } else if i > 0 {
        push(&coefficient[..i as usize], out);
        out.push('.');
        push(&coefficient[i as usize..], out);
    } else {
        // Below 1y: d3 asks for p + i − 1 digits, the shortest when that is 0.
        out.push_str("0.");
        out.extend(core::iter::repeat_n('0', (-i) as usize));
        let q = (p as i32 + i - 1).max(0) as usize;
        push(&js::decimal_parts(x, Some(q)).0, out);
    }
    Some(e)
}

/// d3-format's `formatTrim`: zeros after the point dropped, and the point
/// with them when nothing follows (`1.20k` → `1.2k`, `1.00e+3` → `1e+3`).
fn trim(s: &str) -> String {
    let b = s.as_bytes();
    let (mut i0, mut i1) = (-1i64, 0usize);
    for (i, &c) in b.iter().enumerate().skip(1) {
        match c {
            b'.' => {
                i0 = i as i64;
                i1 = i;
            }
            b'0' => {
                if i0 == 0 {
                    i0 = i as i64;
                }
                i1 = i;
            }
            b'1'..=b'9' => {
                if i0 > 0 {
                    i0 = 0;
                }
            }
            _ => break,
        }
    }
    if i0 > 0 {
        let mut t = String::with_capacity(s.len());
        t.push_str(&s[..i0 as usize]);
        t.push_str(&s[i1 + 1..]);
        t
    } else {
        s.to_string()
    }
}

/// Whether digits as written read as zero (`0.00`, `0e+0`).
fn reads_zero(s: &str) -> bool {
    s.bytes().take_while(|&c| c != b'e').all(|c| c == b'0' || c == b'.')
}

/// d3-format's `formatGroup`: the separator between groups of the integer
/// part's digits, sizes from the right, the last repeating.
fn group(int: &str, grouping: &[usize], separator: &str, out: &mut String) {
    if grouping.is_empty() || grouping[0] == 0 {
        out.push_str(int);
        return;
    }
    let mut parts = Vec::new();
    let (mut i, mut j) = (int.len(), 0);
    let mut g = grouping[0];
    while i > 0 && g > 0 {
        let start = i.saturating_sub(g);
        parts.push(&int[start..i]);
        i = start;
        j = (j + 1) % grouping.len();
        g = grouping[j];
    }
    for (k, part) in parts.iter().rev().enumerate() {
        if k > 0 {
            out.push_str(separator);
        }
        out.push_str(part);
    }
}

/// d3-format's `precisionFixed(step)`.
fn precision_fixed(step: f64) -> Option<i32> {
    js::exponent(step).map(|e| (-e).max(0))
}

/// d3-format's `precisionPrefix(step, value)`.
fn precision_prefix(step: f64, value: f64) -> Option<i32> {
    let ev = js::exponent(value)?;
    let es = js::exponent(step)?;
    Some((ev.div_euclid(3).clamp(-8, 8) * 3 - es).max(0))
}

/// d3-format's `precisionRound(step, max)`.
fn precision_round(step: f64, max: f64) -> Option<i32> {
    let step = step.abs();
    let max = max.abs() - step;
    Some((js::exponent(max)? - js::exponent(step)?).max(0) + 1)
}

/// The format of the ticks of `[start, stop]` at about `count` ticks, as
/// d3-scale's `tickFormat(start, stop, count, specifier)`: `auto` is `,f`;
/// a missing precision comes from the tick step; `s` takes one SI prefix
/// from the larger end of the domain (REQ-FMT-010).
pub fn tick_format(start: f64, stop: f64, count: f64, spec: &Specifier, locale: &Locale) -> NumberFormat {
    step_format(tick_step(start, stop, count), start, stop, spec, locale)
}

/// The format of ticks `step` apart over `[start, stop]`, as
/// [`tick_format`] gives it for that step: for ticks placed some other way
/// than d3's, such as one a row (REQ-TICK-050).
pub fn step_format(step: f64, start: f64, stop: f64, spec: &Specifier, locale: &Locale) -> NumberFormat {
    let mut spec = spec.clone();
    if spec.kind == Kind::Auto {
        spec.kind = Kind::Fixed;
        spec.comma = true;
    }
    let set = |p: i32| Some(p.max(0) as u32);
    match spec.kind {
        Kind::Si => {
            let value = js::max(start.abs(), stop.abs());
            if spec.precision.is_none() {
                if let Some(p) = precision_prefix(step, value) {
                    spec.precision = set(p);
                }
            }
            spec.kind = Kind::Fixed;
            let mut f = NumberFormat::new(&spec, locale);
            f.scale = Some(match js::exponent(value) {
                Some(e) => {
                    let e = e.div_euclid(3).clamp(-8, 8) * 3;
                    (js::pow10(f64::from(-e)), PREFIXES[(8 + e / 3) as usize])
                }
                None => (f64::NAN, ""),
            });
            return f;
        }
        Kind::Exponent => {
            if spec.precision.is_none() {
                if let Some(p) = precision_round(step, js::max(start.abs(), stop.abs())) {
                    spec.precision = set(p - 1);
                }
            }
        }
        Kind::Fixed | Kind::Percent => {
            if spec.precision.is_none() {
                if let Some(p) = precision_fixed(step) {
                    spec.precision = set(p - if spec.kind == Kind::Percent { 2 } else { 0 });
                }
            }
        }
        Kind::Integer | Kind::Auto => {}
    }
    NumberFormat::new(&spec, locale)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(spec: &str, x: f64) -> String {
        NumberFormat::new(&Specifier::parse("format", spec).unwrap(), &Locale::default()).format(x)
    }

    /// REQ-FMT-020: anything outside `[sign][$][,][.precision][~][type]`
    /// with types f, s, %, e, d, or `auto`, is refused, naming the option.
    #[test]
    fn rejects_unknown_specifiers() {
        for bad in ["", "g", ".2", ".2r", "x", ">10.2f", "010f", "#x", "*^10f", ",.f", ".2ff", "auto ", "Auto", "~", "$$f", "12f", "c", "n"] {
            let e = Specifier::parse("axes[0].format", bad).unwrap_err();
            assert_eq!(e.option, "axes[0].format");
            assert!(e.to_string().starts_with("axes[0].format: \""), "{e}");
        }
        for good in ["auto", "f", "s", "%", "e", "d", ",.2f", "+$,.3~s", "(.1%", " .0e", "-d", "~s", ".12~f"] {
            assert!(Specifier::parse("format", good).is_ok(), "{good}");
        }
        assert_eq!(
            Specifier::parse("o", "+$,.3~s").unwrap(),
            Specifier { sign: Sign::Plus, currency: true, comma: true, precision: Some(3), trim: true, kind: Kind::Si }
        );
    }

    /// REQ-I18N-030: without a pack, `.` decimal, `,` groups of three,
    /// ASCII digits and `-`.
    #[test]
    fn default_locale_is_plain() {
        let l = Locale::default();
        assert_eq!((l.decimal.as_str(), l.group.as_str(), l.grouping.as_slice(), l.minus.as_str()), (".", ",", &[3][..], "-"));
        assert!(l.numerals.is_none());
        assert_eq!(f(",.2f", -1234567.891), "-1,234,567.89");
        assert_eq!(f(",d", 1e9), "1,000,000,000");
        assert_eq!(f("$,.2f", 1234.5), "$1,234.50");
        assert_eq!(f("(.1f", -2.25), "(2.3)");
        assert_eq!(f(".1%", 0.1234), "12.3%");
        assert_eq!(f(".3s", 1234567.0), "1.23M");
        assert_eq!(f("~s", 1500.0), "1.5k");
        assert_eq!(f(".2e", 12345.0), "1.23e+4");
        assert_eq!(f("auto", 0.1 + 0.2), "0.3");
        assert!(f(",.2f", -1234.5).is_ascii());
    }

    /// REQ-FMT-030: labels never read NaN, inf or -0.
    #[test]
    fn no_label_reads_nan_inf_or_minus_zero() {
        for spec in ["f", ".2f", "+.2f", "s", "~s", "%", "e", "d", "auto", "$,.1f"] {
            for x in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.0, -1e-9, -0.004] {
                let s = f(spec, x);
                let minus_zero = (s.contains('-') || s.contains('(')) && !s.chars().any(|c| ('1'..='9').contains(&c));
                assert!(!s.contains("NaN") && !s.contains("nf") && !minus_zero, "{spec} {x}: {s}");
            }
        }
        assert_eq!(f("+.2f", -0.001), "+0.00");
    }

    proptest::proptest! {
        /// REQ-FMT-030: a label read back as a number is the value it
        /// labels, to its precision; `auto` alone keeps 12 significant
        /// digits, and `s` reads back through its prefix.
        #[test]
        fn formatted_numbers_parse_back(m in -1.0f64..1.0, e in -30i32..30, p in 0u32..=10) {
            let x = m * 10f64.powi(e);
            let s = f(&(".".to_string() + &p.to_string() + "f"), x);
            let back: f64 = s.parse().unwrap();
            proptest::prop_assert!((back - x).abs() <= 0.5 * 10f64.powi(-(p as i32)) * (1.0 + 1e-9) + x.abs() * 1e-15, "{x} {p} {s}");
            let s = f(".6e", x);
            let back: f64 = s.parse().unwrap();
            proptest::prop_assert!((back - x).abs() <= x.abs() * 5e-7, "{x} {s}");
            let s = f("auto", x);
            let back: f64 = s.parse().unwrap();
            proptest::prop_assert!((back - x).abs() <= x.abs() * 5e-12, "{x} {s}");
            let s = f(".6s", x);
            let unit = PREFIXES.iter().position(|&u| !u.is_empty() && s.ends_with(u)).map_or(0, |i| i as i32 * 3 - 24);
            let digits = s.trim_end_matches(|c: char| !c.is_ascii_digit());
            let back = digits.parse::<f64>().unwrap() * 10f64.powi(unit);
            proptest::prop_assert!((back - x).abs() <= x.abs() * 5e-6 + 1e-24, "{x} {s}");
        }
    }

    #[test]
    fn tick_formats_follow_the_step() {
        let l = Locale::default();
        let auto = Specifier::default();
        let fmt = tick_format(0.0, 1.0, 10.0, &auto, &l);
        assert_eq!(fmt.format(0.3), "0.3");
        let fmt = tick_format(0.0, 3.4e9, 7.0, &Specifier::parse("f", "s").unwrap(), &l);
        assert_eq!(fmt.format(5e8), "0.5G");
        let fmt = tick_format(0.0, 1e6, 5.0, &auto, &l);
        assert_eq!(fmt.format(200000.0), "200,000");
        let fmt = tick_format(0.0, 0.05, 5.0, &Specifier::parse("f", "%").unwrap(), &l);
        assert_eq!(fmt.format(0.01), "1%");
    }

    /// REQ-FMT-060, REQ-I18N-030: the default pack writes `-`, a pack may
    /// write U+2212, and a thin, narrow or no-break space between groups is
    /// one plain space cell.
    #[test]
    fn minus_and_group_separators() {
        use crate::locale::LocalePack;
        let spec = Specifier::parse("format", ",.1f").unwrap();
        let plain = NumberFormat::new(&spec, &Locale::from_pack(&LocalePack::default()));
        assert_eq!(plain.format(-12345.67), "-12,345.7");
        assert_eq!(Locale::from_pack(&LocalePack::default()), Locale { time: Locale::default().time, ..Locale::default() });
        for sep in ["\u{2009}", "\u{202F}", "\u{00A0}"] {
            let pack = LocalePack { minus: "\u{2212}".into(), group: sep.into(), decimal: ",".into(), ..LocalePack::default() };
            let f = NumberFormat::new(&spec, &Locale::from_pack(&pack));
            assert_eq!(f.format(-12345.67), "\u{2212}12 345,7", "group {sep:?}");
        }
        let pct = LocalePack { percent: "#\u{00A0}%".into(), ..LocalePack::default() };
        assert_eq!(NumberFormat::new(&Specifier::parse("f", ".0%").unwrap(), &Locale::from_pack(&pct)).format(0.5), "50 %");
    }

    /// REQ-FMT-040: SI units, IEC bytes, bits a second, durations from
    /// nanoseconds to days, prices to a tick size and signed percentages,
    /// one table a unit.
    #[test]
    fn unit_table() {
        let l = Locale::default();
        let check = |unit: Unit, digits: u32, table: &[(f64, &str)]| {
            let f = UnitFormat::new(unit.clone(), digits, &l);
            for &(v, want) in table {
                assert_eq!(f.format(v), want, "{unit:?} {digits} {v}");
            }
        };
        check(
            Unit::Si("W".into()),
            3,
            &[(0.0, "0 W"), (1.0, "1 W"), (999.0, "999 W"), (1500.0, "1.5 kW"), (999_950.0, "1 MW"), (-2.5e9, "-2.5 GW"), (0.02, "20 mW"), (4.7e-6, "4.7 µW")],
        );
        check(Unit::Si("Hz".into()), 4, &[(1_234_567.0, "1.235 MHz"), (60.0, "60 Hz")]);
        check(
            Unit::Bytes,
            3,
            &[
                (0.0, "0 B"),
                (512.0, "512 B"),
                (1023.0, "1023 B"),
                (1024.0, "1 KiB"),
                (1536.0, "1.5 KiB"),
                (1_048_575.0, "1 MiB"),
                (5.0 * 1_048_576.0, "5 MiB"),
                (1.5 * 1_073_741_824.0, "1.5 GiB"),
                (2.0f64.powi(40), "1 TiB"),
                (-3072.0, "-3 KiB"),
            ],
        );
        check(
            Unit::BitsPerSecond,
            3,
            &[(0.0, "0 b/s"), (999.0, "999 b/s"), (1000.0, "1 kb/s"), (12_000_000.0, "12 Mb/s"), (2.5e9, "2.5 Gb/s"), (1e12, "1 Tb/s")],
        );
        check(
            Unit::Duration,
            3,
            &[
                (0.0, "0 s"),
                (5e-10, "0.5 ns"),
                (42e-9, "42 ns"),
                (250e-6, "250 µs"),
                (0.0125, "12.5 ms"),
                (0.9999, "1 s"),
                (1.5, "1.5 s"),
                (90.0, "1.5 min"),
                (5400.0, "1.5 h"),
                (86_400.0, "1 d"),
                (3.0 * 86_400.0, "3 d"),
                (-30.0, "-30 s"),
            ],
        );
        check(Unit::Price(0.01), 0, &[(182.4, "182.40"), (1234.567, "1,234.57"), (-0.004, "0.00"), (0.0, "0.00")]);
        check(Unit::Price(0.25), 0, &[(101.3, "101.25"), (101.38, "101.50"), (99.0, "99.00")]);
        check(Unit::Price(1.0 / 32.0), 0, &[(99.5, "99-16"), (99.03125, "99-01"), (100.99, "101-00"), (-1.25, "-1-08")]);
        check(Unit::Price(1.0), 0, &[(1234.4, "1,234")]);
        check(Unit::SignedPercent, 2, &[(0.0125, "+1.25%"), (-0.004, "-0.40%"), (0.0, "0.00%"), (0.00001, "0.00%"), (-0.00001, "0.00%"), (1.5, "+150.00%")]);
        check(Unit::SignedPercent, 0, &[(0.5, "+50%"), (-0.123, "-12%")]);
        for unit in [Unit::Si("W".into()), Unit::Bytes, Unit::BitsPerSecond, Unit::Duration, Unit::Price(0.01), Unit::SignedPercent] {
            for v in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                assert_eq!(UnitFormat::new(unit.clone(), 3, &l).format(v), "", "{unit:?} {v}");
            }
        }
        // The locale's separators apply.
        let de = Locale::from_pack(&crate::locale::LocalePack { decimal: ",".into(), group: ".".into(), ..crate::locale::LocalePack::default() });
        assert_eq!(UnitFormat::new(Unit::Bytes, 3, &de).format(1536.0), "1,5 KiB");
        assert_eq!(UnitFormat::new(Unit::Price(0.01), 0, &de).format(1234.5), "1.234,50");
    }

    /// REQ-DATA-100: a column's scale and offset change what ticks,
    /// readouts and table cells show, never the stored values.
    #[test]
    fn unit_scale_and_offset_change_labels_only() {
        use crate::data::Column;
        let mut c = Column::f64("price", vec![18240.0, 10000.0, 20000.0]).unit("USD");
        c.scale = Some(0.01);
        let before = format!("{:?}", c.values);
        let d = Display::of(&c);
        let auto = Specifier::default();
        assert_eq!(d.readout(18240.0, &auto, &Locale::default()), "182.40");
        assert_eq!(d.with_unit(&d.readout(18240.0, &auto, &Locale::default())), "182.40 USD");
        let ticks = d.tick_labels([10000.0, 20000.0], 5.0, &auto, &Locale::default());
        let labels: Vec<&str> = ticks.iter().map(|(_, l)| l.as_str()).collect();
        assert_eq!(labels, ["100", "120", "140", "160", "180", "200"]);
        assert_eq!(ticks[1].0, 12000.0, "ticks are placed at stored values");
        assert_eq!(format!("{:?}", c.values), before, "the stored values are unchanged");
        // An offset: stored kelvin shown in °C.
        let k = Display::new(Some(1.0), Some(-273.15), Some("°C"));
        assert_eq!(k.readout(300.0, &Specifier::parse("format", ".1f").unwrap(), &Locale::default()), "26.9");
        assert_eq!(k.stored(k.shown(300.0)), 300.0);
        // No transform: the identity, and the chart's own format.
        let none = Display::of(&Column::f64("y", vec![1.0]));
        assert!(none.is_identity());
        assert_eq!(none.readout(0.1 + 0.2, &auto, &Locale::default()), "0.3");
        assert_eq!(none.domain([3.0, 4.0]), [3.0, 4.0]);
    }

    /// REQ-FMT-050: date labels accept the strftime subset and refuse any
    /// other directive with an error naming the option.
    #[test]
    fn strftime_subset() {
        use crate::time::{check_pattern, strftime, TimeLocale};
        let t = 1_792_328_421_005.0; // 2026-10-18 13:00:21.005 UTC, a Sunday
        let mut s = String::new();
        strftime("%Y %m %d %e %H %I %M %S %L %p %a %A %b %B %j %Z %%", t, &TimeLocale::default(), &mut s);
        assert_eq!(s, "2026 10 18 18 13 01 00 21 005 PM Sun Sunday Oct October 291 +0000 %");
        for good in ["%Y-%m-%d", "%-d %b", "%_H:%0M", "at %H o'clock", "100%%", ""] {
            assert_eq!(check_pattern("axes[0].format", good), Ok(()), "{good}");
        }
        for bad in ["%y", "%U", "%c", "%x", "%Y %q", "%", "%-", "%Y-%"] {
            let e = check_pattern("axes[0].format", bad).unwrap_err();
            assert_eq!(e.option, "axes[0].format");
            assert!(e.to_string().starts_with("axes[0].format: \"") && e.to_string().contains("not a date format"), "{e}");
        }
        assert!(crate::scale::AxisFormat::parse("axes[1].format", "%y", crate::scale::Kind::Time).is_err());
    }

    /// REQ-FMT-050: time labels are on a 24-hour clock unless the locale
    /// pack's hour cycle is 12-hour (en-US's, the default, is).
    #[test]
    fn hour_cycle_follows_the_locale() {
        use crate::locale::{HourCycle, LocalePack};
        use crate::time::{utc_tick_label, HOUR, MINUTE};
        let t0 = 1_792_281_600_000.0; // 2026-10-18 00:00 UTC
        let label = |l: &Locale, t: f64| {
            let mut s = String::new();
            utc_tick_label(t, &l.time, &mut s);
            s
        };
        let us = Locale::from_pack(&LocalePack::default());
        assert!(us.time.hour12);
        assert_eq!(label(&us, t0 + 15.0 * HOUR), "03 PM");
        assert_eq!(label(&us, t0 + 15.0 * HOUR + 30.0 * MINUTE), "03:30");
        let h23 = Locale::from_pack(&LocalePack { hour_cycle: HourCycle::H23, ..LocalePack::default() });
        assert!(!h23.time.hour12);
        assert_eq!(label(&h23, t0 + 15.0 * HOUR), "15:00");
        assert_eq!(label(&h23, t0 + 15.0 * HOUR + 30.0 * MINUTE), "15:30");
        assert_eq!(label(&h23, t0 + 9.0 * HOUR), "09:00");
        // Coarser labels do not depend on the clock.
        assert_eq!(label(&h23, t0), "Oct 18");
        assert_eq!(label(&h23, t0 + 7.0 * 1000.0), ":07");
    }
}
