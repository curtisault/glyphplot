//! Linear and time scales: a domain value to a continuous cell coordinate and
//! back (REQ-SCALE-010 to 030).
//!
//! [`Scale::map`] and [`Scale::invert`] are d3-scale 4.0.2's continuous
//! scale on a two-number domain (`src/continuous.js`, `bimap`, with
//! d3-interpolate's `interpolateNumber`; ISC licence, © Mike Bostock), and
//! [`nice`] is its `linear().nice(count)` (`src/linear.js`), including the
//! domain it leaves unchanged when the step has not settled after ten rounds
//! (REQ-SCALE-030). A time scale is the same map over milliseconds since the
//! epoch, with d3-time's UTC ticks and d3-scale's UTC labels.
//!
//! The domain a scale shows is [`resolve_domain`], a pure function of the
//! scale's options, the data extents the caller measured and the view state
//! (REQ-SCALE-020).

use serde::Deserialize;

use crate::format::{self, FormatError, Locale, NumberFormat, Specifier};
use crate::ticks::ticks;
use crate::time::{utc_nice, utc_tick_label, utc_ticks, Zone, HOUR, SECOND};

/// Every scale type the options name (`spec/schemas/options.schema.json`,
/// `$defs/scale`); linear and time exist from v0.2.0, the rest arrive later.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ScaleType {
    /// Linear (v0.2.0).
    Linear,
    /// Logarithmic (v0.5.0).
    Log,
    /// Symmetric log (v0.5.0).
    Symlog,
    /// Time, in UTC unless a zone is given (v0.2.0).
    Time,
    /// Index time: consecutive bars (v0.5.0).
    Index,
    /// Band (v0.5.0).
    Band,
    /// Point (v0.5.0).
    Point,
    /// Sequential colour (v0.9.0).
    Sequential,
    /// Diverging colour (v0.9.0).
    Diverging,
    /// Categorical colour (v0.9.0).
    Categorical,
    /// Threshold colour (v0.9.0).
    Threshold,
    /// Wrapping angle (v0.12.0).
    Angle,
}

/// A scale's domain option (REQ-SCALE-020).
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Domain {
    /// `"data"`: the extent of every series bound to the scale.
    #[default]
    Data,
    /// `"view"`: the view state's range; for a y scale, the data visible in
    /// the x view.
    View,
    /// A fixed list of numbers: `[low, high]` for a continuous scale.
    Values(Vec<f64>),
    /// A fixed list of categories (band, point, categorical).
    Categories(Vec<String>),
    /// `{"follow": seconds}`: the last window of a stream, that long.
    Follow(f64),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FollowRepr {
    follow: f64,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum DomainRepr {
    Word(String),
    Values(Vec<f64>),
    Categories(Vec<String>),
    Follow(FollowRepr),
}

impl<'de> Deserialize<'de> for Domain {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Domain, D::Error> {
        use serde::de::Error;
        match DomainRepr::deserialize(d)? {
            DomainRepr::Word(w) if w == "data" => Ok(Domain::Data),
            DomainRepr::Word(w) if w == "view" => Ok(Domain::View),
            DomainRepr::Word(_) => Err(D::Error::custom("domain: expected \"data\", \"view\", a list or {\"follow\": seconds}")),
            DomainRepr::Values(v) if v.is_empty() => Err(D::Error::custom("domain: a list needs at least one value")),
            DomainRepr::Values(v) => Ok(Domain::Values(v)),
            DomainRepr::Categories(c) => Ok(Domain::Categories(c)),
            DomainRepr::Follow(f) if f.follow > 0.0 => Ok(Domain::Follow(f.follow)),
            DomainRepr::Follow(_) => Err(D::Error::custom("domain: follow takes a window above 0 seconds")),
        }
    }
}

/// One scale's options, as `spec/schemas/options.schema.json` shapes
/// `scales.<name>`.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ScaleOptions {
    /// The scale's type.
    #[serde(rename = "type")]
    pub kind: ScaleType,
    /// The domain; `"data"` when absent.
    #[serde(default)]
    pub domain: Domain,
    /// Widen the domain to round values, as d3-scale's `nice`.
    #[serde(default)]
    pub nice: Option<bool>,
    /// Include 0 in the domain.
    #[serde(default)]
    pub zero: Option<bool>,
    /// A fraction of the span added at each end.
    #[serde(default)]
    pub padding: Option<f64>,
    /// For a log scale, its base.
    #[serde(default)]
    pub base: Option<f64>,
    /// For a symlog scale, its linear region's half-width.
    #[serde(default)]
    pub constant: Option<f64>,
    /// For a time scale, its zone.
    #[serde(default)]
    pub zone: Option<String>,
    /// For a colour scale, its stops.
    #[serde(default)]
    pub scheme: Option<String>,
}

/// What a domain is resolved from besides the options: extents the caller
/// measured and the view state's range, each `[low, high]` in data units
/// (timestamps in epoch ms).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Extents {
    /// The extent of every series bound to the scale.
    pub data: Option<[f64; 2]>,
    /// For a y scale, the extent of the bound data whose x is in the x view.
    pub visible: Option<[f64; 2]>,
    /// The view state's range for this scale, when it holds one (a zoom or
    /// pan); null in the view state is None.
    pub view: Option<[f64; 2]>,
}

/// Why a domain could not be resolved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DomainError {
    /// What is wrong.
    pub reason: &'static str,
}

/// The two continuous kinds this version draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Linear.
    Linear,
    /// Time: milliseconds since the epoch, ticked and labelled in UTC.
    Time,
}

fn finite(r: Option<[f64; 2]>) -> Option<[f64; 2]> {
    r.filter(|[a, b]| a.is_finite() && b.is_finite())
}

/// The domain a linear or time scale shows, from its options, the data
/// extents and the view state, and nothing else (REQ-SCALE-020):
///
/// 1. A range in the view state (a zoom or pan) is used as it is.
/// 2. Otherwise the option gives it: `"data"` the data's extent; `"view"`
///    the visible extent, else the data's; `[a, b]` itself;
///    `{"follow": s}` the last s seconds up to the data's end (s × 1000 ms on
///    a time scale, s data units on a linear one). With no data, d3's
///    defaults: [0, 1], or 2000-01-01 to 2000-01-02 UTC.
/// 3. Equal ends are widened: by 1 each way around 0, a tenth of the value
///    otherwise, an hour on a time scale.
/// 4. `zero` takes in 0, `padding` adds that fraction of the span at each
///    end, and `nice` widens to round values for about `count` ticks.
pub fn resolve_domain(options: &ScaleOptions, kind: Kind, extents: &Extents, count: f64) -> Result<[f64; 2], DomainError> {
    if let Some(view) = finite(extents.view) {
        return Ok(view);
    }
    let data = finite(extents.data);
    let base = match &options.domain {
        Domain::Data => data,
        Domain::View => finite(extents.visible).or(data),
        Domain::Values(v) if v.len() == 2 => Some([v[0], v[1]]),
        Domain::Values(_) => return Err(DomainError { reason: "a continuous scale's domain list is [low, high]" }),
        Domain::Categories(_) => return Err(DomainError { reason: "a continuous scale's domain is numbers, not categories" }),
        Domain::Follow(s) => {
            let window = if kind == Kind::Time { s * SECOND } else { *s };
            data.map(|[_, hi]| [hi - window, hi])
        }
    };
    let [mut a, mut b] = base.unwrap_or(match kind {
        Kind::Linear => [0.0, 1.0],
        Kind::Time => [946_684_800_000.0, 946_771_200_000.0],
    });
    if !(a.is_finite() && b.is_finite()) {
        return Err(DomainError { reason: "the domain's ends must be finite" });
    }
    if a == b {
        let d = match kind {
            Kind::Time => HOUR,
            Kind::Linear if a == 0.0 => 1.0,
            Kind::Linear => a.abs() / 10.0,
        };
        (a, b) = (a - d, b + d);
    }
    if options.zero == Some(true) {
        if a <= b {
            (a, b) = (a.min(0.0), b.max(0.0));
        } else {
            (a, b) = (a.max(0.0), b.min(0.0));
        }
    }
    if let Some(p) = options.padding.filter(|p| *p > 0.0) {
        let span = b - a;
        (a, b) = (a - span * p, b + span * p);
    }
    if options.nice == Some(true) {
        [a, b] = match kind {
            Kind::Linear => nice(a, b, count),
            Kind::Time => utc_nice(a, b, count),
        };
    }
    Ok([a, b])
}

/// d3-scale's `linear().domain([start, stop]).nice(count).domain()`: the
/// ends moved out to multiples of the tick increment, repeated until the
/// increment settles, at most ten times; a domain that has not settled by
/// then comes back unchanged, as in d3 (REQ-SCALE-030).
pub fn nice(start: f64, stop: f64, count: f64) -> [f64; 2] {
    let flip = stop < start;
    let (mut lo, mut hi) = if flip { (stop, start) } else { (start, stop) };
    let mut prestep = f64::NAN;
    for _ in 0..10 {
        let step = crate::ticks::tick_increment(lo, hi, count);
        if step == prestep {
            return if flip { [hi, lo] } else { [lo, hi] };
        } else if step > 0.0 {
            lo = (lo / step).floor() * step;
            hi = (hi / step).ceil() * step;
        } else if step < 0.0 {
            lo = (lo * step).ceil() / step;
            hi = (hi * step).floor() / step;
        } else {
            break;
        }
        prestep = step;
    }
    [start, stop]
}

/// d3-scale's `normalize(a, b)` then d3-interpolate's `interpolateNumber`:
/// `x` mapped from [d0, d1] to [r0, r1] as d3's `bimap` does it.
fn bimap(x: f64, [d0, d1]: [f64; 2], [r0, r1]: [f64; 2]) -> f64 {
    let (d0, d1, r0, r1) = if d1 < d0 { (d1, d0, r1, r0) } else { (d0, d1, r0, r1) };
    let span = d1 - d0;
    let t = if span != 0.0 {
        (x - d0) / span
    } else if span.is_nan() {
        f64::NAN
    } else {
        0.5
    };
    r0 * (1.0 - t) + r1 * t
}

/// A continuous scale: a domain value to a continuous cell coordinate over a
/// range and back (REQ-SCALE-010).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scale {
    /// Linear or time.
    pub kind: Kind,
    /// The domain, `[start, stop]` in data units (epoch ms for time); stop
    /// below start reverses the scale.
    pub domain: [f64; 2],
    /// The range, in cells (fractional: a cell's centre is a whole number).
    pub range: [f64; 2],
    /// For a time scale, the zone ticks and labels are in: UTC (REQ-TZ-010).
    pub zone: Zone,
}

impl Scale {
    /// A linear scale.
    pub fn linear(domain: [f64; 2], range: [f64; 2]) -> Scale {
        Scale { kind: Kind::Linear, domain, range, zone: Zone::Utc }
    }

    /// A time scale over epoch milliseconds, in UTC (REQ-TZ-010).
    pub fn time(domain: [f64; 2], range: [f64; 2]) -> Scale {
        Scale { kind: Kind::Time, domain, range, zone: Zone::Utc }
    }

    /// The continuous coordinate of a domain value; outside the domain it
    /// extrapolates.
    pub fn map(&self, v: f64) -> f64 {
        bimap(v, self.domain, self.range)
    }

    /// The domain value at a coordinate: `invert(map(v))` is `v` within
    /// 10⁻⁹ of the domain's magnitude (REQ-SCALE-010).
    pub fn invert(&self, c: f64) -> f64 {
        bimap(c, self.range, self.domain)
    }

    /// The scale with its domain niced for about `count` ticks
    /// (REQ-SCALE-030).
    pub fn nice(&self, count: f64) -> Scale {
        let [a, b] = self.domain;
        let domain = match self.kind {
            Kind::Linear => nice(a, b, count),
            Kind::Time => utc_nice(a, b, count),
        };
        Scale { domain, ..*self }
    }

    /// About `count` tick values: d3-array's linear ticks or d3-time's UTC
    /// ticks over the domain (REQ-TICK-010, REQ-TICK-020).
    pub fn ticks(&self, count: f64) -> Vec<f64> {
        let [a, b] = self.domain;
        match self.kind {
            Kind::Linear => ticks(a, b, count),
            Kind::Time => utc_ticks(a, b, count),
        }
    }

    /// The format of this scale's ticks at about `count` ticks
    /// (REQ-FMT-010): for a linear scale d3-scale's `tickFormat`, for a time
    /// scale its multi-scale UTC labels or the axis's strftime pattern.
    pub fn tick_format(&self, count: f64, format: &AxisFormat, locale: &Locale) -> TickFormat {
        let [a, b] = self.domain;
        match format {
            AxisFormat::Number(spec) => TickFormat::Number(format::tick_format(a, b, count, spec, locale)),
            AxisFormat::Time(pattern) => TickFormat::Time { pattern: pattern.clone(), locale: locale.clone() },
        }
    }

    /// Tick values with their labels.
    pub fn tick_labels(&self, count: f64, format: &AxisFormat, locale: &Locale) -> Vec<(f64, String)> {
        let f = self.tick_format(count, format, locale);
        self.ticks(count).into_iter().map(|t| (t, f.format(t))).collect()
    }
}

/// An axis's `format` option, parsed for its scale.
#[derive(Clone, Debug, PartialEq)]
pub enum AxisFormat {
    /// A d3-format subset specifier, `auto` included (REQ-FMT-020).
    Number(Specifier),
    /// For a time scale: None for d3's multi-scale labels (`auto`), else a
    /// strftime pattern.
    Time(Option<String>),
}

impl AxisFormat {
    /// The `format` option `value`, from the option named `option`, for a
    /// scale of `kind`; a number format outside the subset is an error naming
    /// the option (REQ-FMT-020).
    pub fn parse(option: &str, value: &str, kind: Kind) -> Result<AxisFormat, FormatError> {
        match kind {
            Kind::Linear => Specifier::parse(option, value).map(AxisFormat::Number),
            Kind::Time => Ok(AxisFormat::Time((value != "auto").then(|| value.to_string()))),
        }
    }
}

impl Default for AxisFormat {
    fn default() -> AxisFormat {
        AxisFormat::Number(Specifier::default())
    }
}

/// The labels of one axis's ticks: one format for the whole axis.
#[derive(Clone, Debug, PartialEq)]
pub enum TickFormat {
    /// A number format.
    Number(NumberFormat),
    /// UTC dates: d3-scale's multi-scale labels, or a strftime pattern.
    Time {
        /// The pattern, or None for the multi-scale labels.
        pattern: Option<String>,
        /// The names dates are written with.
        locale: Locale,
    },
}

impl TickFormat {
    /// One label.
    pub fn format(&self, v: f64) -> String {
        let mut out = String::new();
        self.format_into(v, &mut out);
        out
    }

    /// One label, appended to `out`.
    pub fn format_into(&self, v: f64, out: &mut String) {
        match self {
            TickFormat::Number(f) => f.format_into(v, out),
            TickFormat::Time { pattern: None, locale } => utc_tick_label(v, &locale.time, out),
            TickFormat::Time { pattern: Some(p), locale } => crate::time::strftime(p, v, &locale.time, out),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn options(json: &str) -> ScaleOptions {
        serde_json::from_str(json).unwrap()
    }

    proptest! {
        /// REQ-SCALE-010, REQ-PROP-010: inverting a mapped value gives it
        /// back within 10⁻⁹ of the domain's magnitude, on linear and time
        /// scales, either way round, over ranges from 4 to 400 cells.
        #[test]
        fn invert_undoes_map(
            mag in -12i32..15,
            a in -1.0f64..1.0,
            span in 1e-6f64..10.0,
            t in 0.0f64..=1.0,
            cells in 4.0f64..400.0,
            reverse in any::<bool>(),
            time in any::<bool>(),
        ) {
            let m = 10f64.powi(mag);
            let (lo, hi) = (a * m, a * m + span * m);
            let domain = if reverse { [hi, lo] } else { [lo, hi] };
            let range = [0.0, cells - 1.0];
            let s = if time { Scale::time(domain, range) } else { Scale::linear(domain, range) };
            let v = lo + (hi - lo) * t;
            let back = s.invert(s.map(v));
            let scale = lo.abs().max(hi.abs());
            prop_assert!((back - v).abs() <= 1e-9 * scale, "{v} → {} → {back}", s.map(v));
        }
    }

    /// REQ-SCALE-020: every domain form deserialises from the options' JSON
    /// and resolves from the extents, options and view alone.
    #[test]
    fn domain_forms_are_pure() {
        let ext = Extents { data: Some([3.0, 97.0]), visible: Some([40.0, 60.0]), view: None };
        let lin = |json: &str| resolve_domain(&options(json), Kind::Linear, &ext, 10.0);
        assert_eq!(lin(r#"{"type": "linear"}"#), Ok([3.0, 97.0]));
        assert_eq!(lin(r#"{"type": "linear", "domain": "data"}"#), Ok([3.0, 97.0]));
        assert_eq!(lin(r#"{"type": "linear", "domain": "view"}"#), Ok([40.0, 60.0]));
        assert_eq!(lin(r#"{"type": "linear", "domain": [0, 50]}"#), Ok([0.0, 50.0]));
        assert_eq!(lin(r#"{"type": "linear", "domain": {"follow": 10}}"#), Ok([87.0, 97.0]));
        assert_eq!(lin(r#"{"type": "linear", "nice": true}"#), Ok([0.0, 100.0]));
        assert_eq!(lin(r#"{"type": "linear", "domain": "view", "zero": true}"#), Ok([0.0, 60.0]));
        assert_eq!(lin(r#"{"type": "linear", "domain": [10, 20], "padding": 0.1}"#), Ok([9.0, 21.0]));
        assert_eq!(lin(r#"{"type": "linear", "domain": [5, 5]}"#), Ok([4.5, 5.5]));
        assert!(lin(r#"{"type": "linear", "domain": [1, 2, 3]}"#).is_err());
        assert!(lin(r#"{"type": "linear", "domain": ["a", "b"]}"#).is_err());
        let zoomed = Extents { view: Some([10.0, 20.0]), ..ext };
        assert_eq!(resolve_domain(&options(r#"{"type": "linear", "nice": true}"#), Kind::Linear, &zoomed, 10.0), Ok([10.0, 20.0]));
        let none = Extents::default();
        assert_eq!(resolve_domain(&options(r#"{"type": "linear"}"#), Kind::Linear, &none, 10.0), Ok([0.0, 1.0]));
        let t = Extents { data: Some([0.0, 600_000.0]), ..none };
        assert_eq!(resolve_domain(&options(r#"{"type": "time", "domain": {"follow": 60}}"#), Kind::Time, &t, 10.0), Ok([540_000.0, 600_000.0]));
        // The same inputs give the same domain, every time.
        let o = options(r#"{"type": "linear", "domain": "view", "nice": true, "padding": 0.05}"#);
        assert_eq!(resolve_domain(&o, Kind::Linear, &ext, 7.0), resolve_domain(&o, Kind::Linear, &ext, 7.0));
        for bad in [
            r#"{"type": "linear", "domain": "all"}"#,
            r#"{"type": "linear", "domain": {"follow": 0}}"#,
            r#"{"type": "linear", "domain": []}"#,
            r#"{"type": "linear", "colour": 1}"#,
        ] {
            assert!(serde_json::from_str::<ScaleOptions>(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn nice_leaves_an_unsettled_domain() {
        assert_eq!(nice(0.2, 9.7, 10.0), [0.0, 10.0]);
        assert_eq!(nice(9.7, 0.2, 10.0), [10.0, 0.0]);
        // A count of 1 across zero never settles: d3 gives the domain back.
        assert_eq!(nice(-0.5, 0.75, 1.0), [-0.5, 0.75]);
    }

    #[test]
    fn scales_tick_and_label() {
        let s = Scale::linear([0.0, 3.4e9], [0.0, 39.0]);
        let spec = AxisFormat::parse("axes[0].format", "s", Kind::Linear).unwrap();
        let labels: Vec<String> = s.tick_labels(7.0, &spec, &Locale::default()).into_iter().map(|(_, l)| l).collect();
        assert_eq!(labels, ["0.0G", "0.5G", "1.0G", "1.5G", "2.0G", "2.5G", "3.0G"]);
        let t = Scale::time([1_792_328_400_000.0, 1_792_328_400_000.0 + 6.0 * HOUR], [0.0, 79.0]);
        let auto = AxisFormat::parse("axes[1].format", "auto", Kind::Time).unwrap();
        let labels: Vec<String> = t.tick_labels(6.0, &auto, &Locale::default()).into_iter().map(|(_, l)| l).collect();
        assert_eq!(labels, ["01 PM", "02 PM", "03 PM", "04 PM", "05 PM", "06 PM", "07 PM"]);
        assert!(AxisFormat::parse("axes[0].format", ">8.2f", Kind::Linear).is_err());
        assert_eq!(s.map(1.7e9), 19.5);
        assert_eq!(s.invert(19.5), 1.7e9);
    }
}
