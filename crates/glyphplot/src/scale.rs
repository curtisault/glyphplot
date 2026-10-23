//! Scales: a domain value to a continuous cell coordinate and back
//! (REQ-SCALE-010 to 060).
//!
//! [`Scale::map`] and [`Scale::invert`] are d3-scale 4.0.2's continuous
//! scale on a two-number domain (`src/continuous.js`, `bimap`, with
//! d3-interpolate's `interpolateNumber`; ISC licence, © Mike Bostock), and
//! [`nice`] is its `linear().nice(count)` (`src/linear.js`), including the
//! domain it leaves unchanged when the step has not settled after ten rounds
//! (REQ-SCALE-030). The kinds ([`Kind`]):
//!
//! - **linear**, and **time** over milliseconds since the epoch, with
//!   d3-time's UTC ticks and d3-scale's UTC labels, or local ticks and
//!   labels over a zone table ([`Zone`], REQ-TZ-030);
//! - **log**, any base above 1, mapped through the natural logarithm as d3's
//!   `scaleLog` maps (`src/log.js`), ticked as it ticks and labelled on
//!   powers of the base only (REQ-SCALE-040, REQ-TICK-060);
//! - **symlog**, d3's `scaleSymlog` map (`src/symlog.js`) with ticks at 0 and
//!   ± powers of ten (REQ-SCALE-050);
//! - **band**, whole cells a category, every category the same number
//!   ([`Band`], REQ-SCALE-040);
//! - **index** time: rows at consecutive indices whatever the time between
//!   them, ticked at calendar boundaries and labelled from each row's own
//!   timestamp, so gaps (nights, weekends) take no columns (REQ-SCALE-060,
//!   ADR-131).
//!
//! The domain a scale shows is [`resolve`], a pure function of the scale's
//! options, the data extents the caller measured and the view state
//! (REQ-SCALE-020).

// `!(x < y)` sends NaN down the same branch as d3 does; so does this port.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use std::ops::Range;
use std::sync::Arc;

use serde::Deserialize;

use crate::format::{self, js, FormatError, Kind as SpecKind, Locale, NumberFormat, Specifier};
use crate::hit::lower_bound;
use crate::ticks::{band_ticks, log_ticks, symlog_ticks, ticks, LogBase, LogTickFormat};
use crate::time::{check_pattern, Zone, HOUR, SECOND};

/// Every scale type the options name (`spec/schemas/options.schema.json`,
/// `$defs/scale`); [`Kind::from_type`] gives the ones the core draws.
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

impl ScaleOptions {
    /// The options of a scale of `kind` over the data, with nothing else set.
    pub fn new(kind: Kind) -> ScaleOptions {
        ScaleOptions {
            kind: kind.scale_type(),
            domain: Domain::Data,
            nice: None,
            zero: None,
            padding: None,
            base: None,
            constant: None,
            zone: None,
            scheme: None,
        }
    }

    /// The kind the options' `type` names, for a chart or the layout to
    /// choose the scale by (REQ-SCALE-040 to 060); an error for a type the
    /// core does not draw yet.
    pub fn scale_kind(&self) -> Result<Kind, DomainError> {
        Kind::from_type(self.kind).ok_or(DomainError { reason: "this scale type is not drawn yet" })
    }

    /// A log scale's base: `base`, 10 when absent, above 1 (REQ-SCALE-040).
    pub fn log_base(&self) -> Result<f64, DomainError> {
        match self.base {
            None => Ok(10.0),
            Some(b) if b > 1.0 && b.is_finite() => Ok(b),
            Some(_) => Err(DomainError { reason: "a log scale's base is a number above 1" }),
        }
    }

    /// A symlog scale's constant: `constant`, 1 when absent, above 0
    /// (REQ-SCALE-050).
    pub fn symlog_constant(&self) -> Result<f64, DomainError> {
        match self.constant {
            None => Ok(1.0),
            Some(c) if c > 0.0 && c.is_finite() => Ok(c),
            Some(_) => Err(DomainError { reason: "a symlog scale's constant is a number above 0" }),
        }
    }
}

/// What a domain is resolved from besides the options: extents the caller
/// measured and the view state's range, each `[low, high]` in data units
/// (timestamps in epoch ms; row indices on an index scale).
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

/// The kinds of scale the core draws, chosen by the options' `type`
/// ([`ScaleOptions::scale_kind`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Linear.
    Linear,
    /// Logarithmic, any base above 1 (REQ-SCALE-040).
    Log,
    /// Symmetric log (REQ-SCALE-050).
    Symlog,
    /// Time: milliseconds since the epoch, ticked and labelled in UTC or a
    /// zone table's local time.
    Time,
    /// Index time: row indices, labelled from each row's timestamp
    /// (REQ-SCALE-060).
    Index,
    /// Band: category indices, whole cells a category (REQ-SCALE-040).
    Band,
}

impl Kind {
    /// The kind of an options `type`; None for the types not drawn yet
    /// (point, the colour scales, angle).
    pub fn from_type(t: ScaleType) -> Option<Kind> {
        match t {
            ScaleType::Linear => Some(Kind::Linear),
            ScaleType::Log => Some(Kind::Log),
            ScaleType::Symlog => Some(Kind::Symlog),
            ScaleType::Time => Some(Kind::Time),
            ScaleType::Index => Some(Kind::Index),
            ScaleType::Band => Some(Kind::Band),
            _ => None,
        }
    }

    /// The options `type` of this kind.
    pub fn scale_type(self) -> ScaleType {
        match self {
            Kind::Linear => ScaleType::Linear,
            Kind::Log => ScaleType::Log,
            Kind::Symlog => ScaleType::Symlog,
            Kind::Time => ScaleType::Time,
            Kind::Index => ScaleType::Index,
            Kind::Band => ScaleType::Band,
        }
    }

    /// Whether its labels are dates: time and index time.
    pub fn is_temporal(self) -> bool {
        matches!(self, Kind::Time | Kind::Index)
    }
}

fn finite(r: Option<[f64; 2]>) -> Option<[f64; 2]> {
    r.filter(|[a, b]| a.is_finite() && b.is_finite())
}

/// [`resolve`] in UTC: the domain from the options, the data extents and the
/// view state, a time scale niced to UTC calendar intervals.
pub fn resolve_domain(options: &ScaleOptions, kind: Kind, extents: &Extents, count: f64) -> Result<[f64; 2], DomainError> {
    resolve(options, kind, extents, count, &Zone::Utc)
}

/// The domain a scale shows, from its options, the data extents and the
/// view state, and nothing else (REQ-SCALE-020):
///
/// 1. A range in the view state (a zoom or pan) is used as it is.
/// 2. Otherwise the option gives it: `"data"` the data's extent; `"view"`
///    the visible extent, else the data's; `[a, b]` itself; categories on a
///    band scale `[−½, n − ½]` in category indices; `{"follow": s}` the last
///    s seconds up to the data's end (s × 1000 ms on a time scale, s rows on
///    an index scale, s data units otherwise). With no data, d3's defaults:
///    [0, 1], [1, 10] for log, or 2000-01-01 to 2000-01-02 UTC.
/// 3. Equal ends are widened: by 1 each way around 0, a tenth of the value
///    otherwise, an hour on a time scale, a factor of the base on a log
///    scale.
/// 4. `zero` takes in 0 (not on a log scale, whose domain must stay on one
///    side of 0), `padding` adds that fraction of the span at each end (of
///    the logarithms' span on a log scale), and `nice` widens to round
///    values for about `count` ticks: d3's linear nice (symlog too), time
///    intervals in `zone`, powers of the base on a log scale; index and band
///    domains are not niced.
pub fn resolve(options: &ScaleOptions, kind: Kind, extents: &Extents, count: f64, zone: &Zone) -> Result<[f64; 2], DomainError> {
    if let Some(view) = finite(extents.view) {
        return Ok(view);
    }
    let data = finite(extents.data);
    let base = match &options.domain {
        Domain::Data => data,
        Domain::View => finite(extents.visible).or(data),
        Domain::Values(v) if v.len() == 2 => Some([v[0], v[1]]),
        Domain::Values(_) => return Err(DomainError { reason: "a continuous scale's domain list is [low, high]" }),
        Domain::Categories(c) if kind == Kind::Band => return Ok([-0.5, c.len() as f64 - 0.5]),
        Domain::Categories(_) => return Err(DomainError { reason: "a continuous scale's domain is numbers, not categories" }),
        Domain::Follow(s) => {
            let window = if kind == Kind::Time { s * SECOND } else { *s };
            data.map(|[_, hi]| [hi - window, hi])
        }
    };
    let [mut a, mut b] = base.unwrap_or(match kind {
        Kind::Time => [946_684_800_000.0, 946_771_200_000.0],
        Kind::Log => [1.0, 10.0],
        _ => [0.0, 1.0],
    });
    if !(a.is_finite() && b.is_finite()) {
        return Err(DomainError { reason: "the domain's ends must be finite" });
    }
    let log_base = if kind == Kind::Log { options.log_base()? } else { 10.0 };
    if a == b {
        (a, b) = match kind {
            Kind::Time => (a - HOUR, b + HOUR),
            Kind::Log if a > 0.0 => (a / log_base, a * log_base),
            Kind::Log => (a * log_base, a / log_base),
            _ if a == 0.0 => (a - 1.0, b + 1.0),
            _ => (a - a.abs() / 10.0, b + a.abs() / 10.0),
        };
    }
    if kind == Kind::Log {
        if !(a * b > 0.0) {
            return Err(DomainError { reason: "a log scale's domain must not include or cross 0" });
        }
    } else if options.zero == Some(true) {
        if a <= b {
            (a, b) = (a.min(0.0), b.max(0.0));
        } else {
            (a, b) = (a.max(0.0), b.min(0.0));
        }
    }
    if let Some(p) = options.padding.filter(|p| *p > 0.0) {
        if kind == Kind::Log {
            let s = if a < 0.0 { -1.0 } else { 1.0 };
            let (la, lb) = ((s * a).ln(), (s * b).ln());
            let span = lb - la;
            (a, b) = (s * (la - span * p).exp(), s * (lb + span * p).exp());
        } else {
            let span = b - a;
            (a, b) = (a - span * p, b + span * p);
        }
    }
    if options.nice == Some(true) {
        [a, b] = match kind {
            Kind::Linear | Kind::Symlog => nice(a, b, count),
            Kind::Time => zone.nice(a, b, count),
            Kind::Log => log_nice(a, b, log_base),
            Kind::Index | Kind::Band => [a, b],
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

/// d3-scale's `scaleLog().base(base).domain([start, stop]).nice().domain()`:
/// the ends moved out to powers of the base (REQ-SCALE-040).
pub fn log_nice(start: f64, stop: f64, base: f64) -> [f64; 2] {
    let lb = LogBase::new(base, start);
    let floor = |x: f64| lb.pows(lb.logs(x).floor());
    let ceil = |x: f64| lb.pows(lb.logs(x).ceil());
    if stop < start {
        [ceil(start), floor(stop)]
    } else {
        [floor(start), ceil(stop)]
    }
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

/// JavaScript's `Math.sign`: −1, 1, or the value itself for ±0 and NaN.
fn sign(x: f64) -> f64 {
    if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        x
    }
}

/// A band axis: `count` categories over `cells` cells, every category the
/// same whole number of cells, `per` (REQ-SCALE-040). The cells left over
/// are split before the first band and after the last; within a band,
/// `inner` cells are drawn and the rest is padding, split either side.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Band {
    /// The categories.
    pub count: usize,
    /// The cells along the axis.
    pub cells: usize,
    /// The cells a category.
    pub per: usize,
    /// The first band's first cell.
    pub start: usize,
    /// The padding cells before a band's drawn cells.
    pub lead: usize,
    /// The drawn cells of a band, at least 1.
    pub inner: usize,
}

impl Band {
    /// `count` categories over `cells` cells, `padding` (0 to 1) the share
    /// of each band left empty, rounded to whole cells and leaving at least
    /// one drawn; None when there are no categories or fewer cells than
    /// categories.
    pub fn new(count: usize, cells: usize, padding: f64) -> Option<Band> {
        if count == 0 || cells < count {
            return None;
        }
        let per = cells / count;
        let pad = if padding > 0.0 { js::round(per as f64 * padding.min(1.0)) as usize } else { 0 };
        let inner = per - pad.min(per - 1);
        Some(Band { count, cells, per, start: (cells - per * count) / 2, lead: (per - inner) / 2, inner })
    }

    /// The cells of category `i`'s slot, padding included.
    pub fn slot(&self, i: usize) -> Option<Range<usize>> {
        (i < self.count).then(|| {
            let s = self.start + i * self.per;
            s..s + self.per
        })
    }

    /// The cells category `i` draws in.
    pub fn bar(&self, i: usize) -> Option<Range<usize>> {
        self.slot(i).map(|s| s.start + self.lead..s.start + self.lead + self.inner)
    }

    /// The continuous position of category `i`'s centre, in cells.
    pub fn center(&self, i: usize) -> f64 {
        self.start as f64 + (i as f64 + 0.5) * self.per as f64
    }

    /// The category whose slot holds `cell`: the band hit test.
    pub fn at(&self, cell: usize) -> Option<usize> {
        let i = cell.checked_sub(self.start)? / self.per;
        (i < self.count).then_some(i)
    }

    /// The domain in category indices that a linear axis over the band's
    /// cells needs to put each index at its band's centre: `[−½ − start/per,
    /// −½ − start/per + cells/per]`.
    pub fn domain(&self) -> [f64; 2] {
        let per = self.per as f64;
        let lo = -0.5 - self.start as f64 / per;
        [lo, lo + self.cells as f64 / per]
    }
}

/// A scale: a domain value to a continuous cell coordinate over a range and
/// back (REQ-SCALE-010, REQ-SCALE-040 to 060).
#[derive(Clone, Debug, PartialEq)]
pub struct Scale {
    /// The kind.
    pub kind: Kind,
    /// The domain, `[start, stop]` in data units (epoch ms for time, row
    /// indices for index time, category indices for band); stop below start
    /// reverses the scale.
    pub domain: [f64; 2],
    /// The range, in cells (fractional: a cell's centre is a whole number
    /// and a half).
    pub range: [f64; 2],
    /// For a time or index scale, the zone ticks and labels are in: UTC
    /// unless a table is given (REQ-TZ-010, REQ-TZ-030).
    pub zone: Zone,
    /// For a log scale, its base (10 by default).
    pub base: f64,
    /// For a symlog scale, its constant (1 by default).
    pub constant: f64,
    /// For an index scale, each row's timestamp in epoch ms, increasing.
    pub stamps: Option<Arc<[f64]>>,
    /// For a band scale, the categories' names.
    pub names: Option<Arc<[String]>>,
}

impl Scale {
    /// A scale of `kind` with the defaults: base 10, constant 1, UTC, no
    /// timestamps or names.
    pub fn new(kind: Kind, domain: [f64; 2], range: [f64; 2]) -> Scale {
        Scale { kind, domain, range, zone: Zone::Utc, base: 10.0, constant: 1.0, stamps: None, names: None }
    }

    /// A linear scale.
    pub fn linear(domain: [f64; 2], range: [f64; 2]) -> Scale {
        Scale::new(Kind::Linear, domain, range)
    }

    /// A time scale over epoch milliseconds, in UTC (REQ-TZ-010).
    pub fn time(domain: [f64; 2], range: [f64; 2]) -> Scale {
        Scale::new(Kind::Time, domain, range)
    }

    /// A log scale of `base` (REQ-SCALE-040); its domain must not include or
    /// cross 0.
    pub fn log(domain: [f64; 2], range: [f64; 2], base: f64) -> Scale {
        Scale { base, ..Scale::new(Kind::Log, domain, range) }
    }

    /// A symlog scale with its linear region's half-width (REQ-SCALE-050).
    pub fn symlog(domain: [f64; 2], range: [f64; 2], constant: f64) -> Scale {
        Scale { constant, ..Scale::new(Kind::Symlog, domain, range) }
    }

    /// An index time scale over rows whose timestamps are `stamps`
    /// (increasing), its domain in row indices (REQ-SCALE-060): row `i` maps
    /// where index `i` does, whatever the time between rows.
    pub fn index(stamps: Arc<[f64]>, domain: [f64; 2], range: [f64; 2]) -> Scale {
        Scale { stamps: Some(stamps), ..Scale::new(Kind::Index, domain, range) }
    }

    /// A band scale over `band`'s cells (`range` from 0 to its cells, or its
    /// reverse), each category's index mapped to its band's centre
    /// (REQ-SCALE-040), labelled by `names`.
    pub fn band(band: &Band, names: Option<Arc<[String]>>) -> Scale {
        Scale { names, ..Scale::new(Kind::Band, band.domain(), [0.0, band.cells as f64]) }
    }

    /// The scale its options give, over `domain` and `range`, in `zone`:
    /// the kind from `type`, the base and constant from theirs.
    pub fn from_options(options: &ScaleOptions, domain: [f64; 2], range: [f64; 2], zone: Zone) -> Result<Scale, DomainError> {
        let kind = options.scale_kind()?;
        let base = if kind == Kind::Log { options.log_base()? } else { 10.0 };
        let constant = if kind == Kind::Symlog { options.symlog_constant()? } else { 1.0 };
        Ok(Scale { base, constant, zone, ..Scale::new(kind, domain, range) })
    }

    /// The scale in `zone`.
    pub fn with_zone(self, zone: Zone) -> Scale {
        Scale { zone, ..self }
    }

    /// The value as the scale spaces it: the natural logarithm on a log
    /// scale (of −v below 0, negated), `sign(v) × ln(1 + |v / c|)` on a
    /// symlog scale, the value itself otherwise. A layout places marks with
    /// a linear axis over the transformed domain.
    pub fn transform(&self, v: f64) -> f64 {
        match self.kind {
            Kind::Log if self.domain[0] < 0.0 => -js::log(-v),
            Kind::Log => js::log(v),
            Kind::Symlog => sign(v) * (v / self.constant).abs().ln_1p(),
            _ => v,
        }
    }

    /// The inverse of [`Scale::transform`].
    pub fn untransform(&self, y: f64) -> f64 {
        match self.kind {
            Kind::Log if self.domain[0] < 0.0 => -(-y).exp(),
            Kind::Log => y.exp(),
            Kind::Symlog => sign(y) * y.abs().exp_m1() * self.constant,
            _ => y,
        }
    }

    /// The transformed domain.
    pub fn transformed_domain(&self) -> [f64; 2] {
        [self.transform(self.domain[0]), self.transform(self.domain[1])]
    }

    /// The continuous coordinate of a domain value; outside the domain it
    /// extrapolates.
    pub fn map(&self, v: f64) -> f64 {
        bimap(self.transform(v), self.transformed_domain(), self.range)
    }

    /// The domain value at a coordinate: `invert(map(v))` is `v` within
    /// 10⁻⁹ of the domain's magnitude on linear and time scales
    /// (REQ-SCALE-010), and within 10⁻⁹ of `v` on log scales.
    pub fn invert(&self, c: f64) -> f64 {
        self.untransform(bimap(c, self.range, self.transformed_domain()))
    }

    /// The scale with its domain niced for about `count` ticks
    /// (REQ-SCALE-030): linear (and symlog) by d3's linear nice, time by the
    /// zone's calendar, log to powers of the base; index and band unchanged.
    pub fn nice(&self, count: f64) -> Scale {
        let [a, b] = self.domain;
        let domain = match self.kind {
            Kind::Linear | Kind::Symlog => nice(a, b, count),
            Kind::Time => self.zone.nice(a, b, count),
            Kind::Log => log_nice(a, b, self.base),
            Kind::Index | Kind::Band => self.domain,
        };
        Scale { domain, ..self.clone() }
    }

    /// About `count` tick values: d3-array's linear ticks, d3-time's
    /// calendar ticks in the zone (REQ-TICK-010, REQ-TICK-020, REQ-TZ-030),
    /// d3-scale's log ticks (every multiple d3 gives; [`Scale::tick_labels`]
    /// labels the powers), symlog's 0 and ± powers of ten, a band's category
    /// indices, or an index scale's rows at calendar boundaries.
    pub fn ticks(&self, count: f64) -> Vec<f64> {
        let [a, b] = self.domain;
        match self.kind {
            Kind::Linear => ticks(a, b, count),
            Kind::Time => self.zone.ticks(a, b, count),
            Kind::Log => log_ticks(a, b, count, self.base),
            Kind::Symlog => symlog_ticks(a, b, count, self.constant),
            Kind::Band => self.band_ticks(count),
            Kind::Index => self.index_ticks(count).into_iter().map(|(i, _)| i).collect(),
        }
    }

    /// A band scale's ticks: its categories inside the domain.
    fn band_ticks(&self, count: f64) -> Vec<f64> {
        let [a, b] = self.domain;
        let (lo, hi) = (a.min(b), a.max(b));
        let n = match &self.names {
            Some(names) => names.len(),
            None if hi >= 0.0 => (hi.floor() + 1.0).min(1e9) as usize,
            None => 0,
        };
        let mut t: Vec<f64> = band_ticks(n, count).into_iter().filter(|&i| i >= lo && i <= hi).collect();
        if b < a {
            t.reverse();
        }
        t
    }

    /// An index scale's ticks (REQ-SCALE-060): the zone's calendar ticks
    /// over the timestamps of the rows in the domain, each moved to the
    /// first row at or after it, once a row; with the time whose
    /// multi-scale pattern labels it (the calendar tick when the row falls
    /// before the next one, else the row's own time). Without timestamps,
    /// linear ticks.
    fn index_ticks(&self, count: f64) -> Vec<(f64, f64)> {
        let [a, b] = self.domain;
        let Some(stamps) = &self.stamps else { return ticks(a, b, count).into_iter().map(|i| (i, f64::NAN)).collect() };
        let (lo, hi) = (a.min(b), a.max(b));
        let n = stamps.len();
        let (i0, i1) = (lo.ceil().max(0.0), hi.floor().min(n as f64 - 1.0));
        if n == 0 || !(i0 <= i1) {
            return Vec::new();
        }
        let (i0, i1) = (i0 as usize, i1 as usize);
        let times = self.zone.ticks(stamps[i0], stamps[i1], count);
        let mut out: Vec<(f64, f64)> = Vec::new();
        for (k, &tau) in times.iter().enumerate() {
            let i = lower_bound(&stamps[..], i0, i1 + 1, tau);
            if i > i1 {
                break;
            }
            if out.last().is_some_and(|&(j, _)| j == i as f64) {
                continue;
            }
            let next = times.get(k + 1).copied().unwrap_or(f64::INFINITY);
            out.push((i as f64, if stamps[i] < next { tau } else { stamps[i] }));
        }
        if b < a {
            out.reverse();
        }
        out
    }

    /// The format of this scale's ticks at about `count` (REQ-FMT-010): for
    /// a linear scale d3-scale's `tickFormat`; for a log scale d3's log tick
    /// format; for symlog the specifier, `~s` for `auto`; for a time scale
    /// its multi-scale labels in the zone or the axis's strftime pattern;
    /// for an index scale the same from each row's timestamp; for a band its
    /// names.
    pub fn tick_format(&self, count: f64, format: &AxisFormat, locale: &Locale) -> TickFormat {
        let [a, b] = self.domain;
        match (self.kind, format) {
            (Kind::Log, AxisFormat::Number(spec)) => {
                let spec = (spec.kind != SpecKind::Auto || spec.comma || spec.currency || spec.precision.is_some()).then_some(spec);
                TickFormat::Log(LogTickFormat::new(self.domain, self.base, count, spec, locale))
            }
            (Kind::Symlog, AxisFormat::Number(spec)) if spec.kind == SpecKind::Auto => {
                TickFormat::Number(NumberFormat::new(&Specifier { kind: SpecKind::Si, trim: true, ..spec.clone() }, locale))
            }
            (Kind::Symlog, AxisFormat::Number(spec)) => TickFormat::Number(NumberFormat::new(spec, locale)),
            (Kind::Band, _) => TickFormat::Category(self.names.clone().unwrap_or_else(|| Arc::from(Vec::new()))),
            (Kind::Index, AxisFormat::Time(pattern)) if self.stamps.is_some() => TickFormat::Index {
                stamps: self.stamps.clone().unwrap_or_else(|| Arc::from(Vec::new())),
                pattern: pattern.clone(),
                zone: self.zone.clone(),
                locale: locale.clone(),
            },
            (_, AxisFormat::Number(spec)) => TickFormat::Number(format::tick_format(a, b, count, spec, locale)),
            (_, AxisFormat::Time(pattern)) => TickFormat::Time { pattern: pattern.clone(), locale: locale.clone(), zone: self.zone.clone() },
        }
    }

    /// Tick values with their labels. A log axis returns only the powers of
    /// its base, the ticks it labels (REQ-TICK-060; [`Scale::minor_ticks`]
    /// gives the rest); an index axis labels each row from its own
    /// timestamp with the pattern of the calendar tick that chose it
    /// (REQ-SCALE-060).
    pub fn tick_labels(&self, count: f64, format: &AxisFormat, locale: &Locale) -> Vec<(f64, String)> {
        let f = self.tick_format(count, format, locale);
        match (&f, self.kind) {
            (TickFormat::Log(log), _) => self
                .ticks(count)
                .into_iter()
                .filter(|&t| log.is_power(t))
                .map(|t| {
                    let mut s = String::new();
                    log.format_always(t, &mut s);
                    (t, s)
                })
                .collect(),
            (TickFormat::Index { stamps, pattern, zone, locale }, _) => self
                .index_ticks(count)
                .into_iter()
                .map(|(i, at)| {
                    let mut s = String::new();
                    let t = stamps[i as usize];
                    match pattern {
                        Some(p) => zone.strftime(p, t, &locale.time, &mut s),
                        None => {
                            if let Some(p) = zone.tick_pattern(at, &locale.time) {
                                zone.strftime(p, t, &locale.time, &mut s);
                            }
                        }
                    }
                    (i, s)
                })
                .collect(),
            _ => self.ticks(count).into_iter().map(|t| (t, f.format(t))).collect(),
        }
    }

    /// The ticks an axis may mark without labels: on a log axis the
    /// multiples between the powers (REQ-TICK-060); none on other kinds.
    pub fn minor_ticks(&self, count: f64) -> Vec<f64> {
        if self.kind != Kind::Log {
            return Vec::new();
        }
        let f = LogTickFormat::new(self.domain, self.base, count, None, &Locale::default());
        self.ticks(count).into_iter().filter(|&t| !f.is_power(t)).collect()
    }
}

/// An axis's `format` option, parsed for its scale.
#[derive(Clone, Debug, PartialEq)]
pub enum AxisFormat {
    /// A d3-format subset specifier, `auto` included (REQ-FMT-020).
    Number(Specifier),
    /// For a time or index scale: None for d3's multi-scale labels
    /// (`auto`), else a strftime pattern in the subset (REQ-FMT-050).
    Time(Option<String>),
}

impl AxisFormat {
    /// The `format` option `value`, from the option named `option`, for a
    /// scale of `kind`; a number format outside the subset (REQ-FMT-020), or
    /// a date pattern with a directive outside the strftime subset
    /// (REQ-FMT-050), is an error naming the option.
    pub fn parse(option: &str, value: &str, kind: Kind) -> Result<AxisFormat, FormatError> {
        if kind.is_temporal() {
            if value == "auto" {
                return Ok(AxisFormat::Time(None));
            }
            check_pattern(option, value)?;
            return Ok(AxisFormat::Time(Some(value.to_string())));
        }
        Specifier::parse(option, value).map(AxisFormat::Number)
    }

    /// The `auto` format of a scale of `kind`.
    pub fn default_for(kind: Kind) -> AxisFormat {
        if kind.is_temporal() {
            AxisFormat::Time(None)
        } else {
            AxisFormat::default()
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
    /// Dates: d3-scale's multi-scale labels, or a strftime pattern.
    Time {
        /// The pattern, or None for the multi-scale labels.
        pattern: Option<String>,
        /// The names dates are written with.
        locale: Locale,
        /// The zone they are written in.
        zone: Zone,
    },
    /// A log scale's labels, d3's: empty past the multiples it labels.
    Log(LogTickFormat),
    /// A band's category names, by index.
    Category(Arc<[String]>),
    /// An index scale's rows, by their timestamps.
    Index {
        /// Each row's timestamp.
        stamps: Arc<[f64]>,
        /// The pattern, or None for the multi-scale labels.
        pattern: Option<String>,
        /// The zone they are written in.
        zone: Zone,
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

    /// One label, appended to `out`. A band index or row index is rounded to
    /// the nearest; one outside the categories or rows writes nothing.
    pub fn format_into(&self, v: f64, out: &mut String) {
        let index = |n: usize| {
            let i = js::round(v);
            (i >= 0.0 && i < n as f64).then_some(i as usize)
        };
        match self {
            TickFormat::Number(f) => f.format_into(v, out),
            TickFormat::Time { pattern: None, locale, zone } => zone.tick_label(v, &locale.time, out),
            TickFormat::Time { pattern: Some(p), locale, zone } => zone.strftime(p, v, &locale.time, out),
            TickFormat::Log(f) => out.push_str(&f.format(v)),
            TickFormat::Category(names) => {
                if let Some(i) = index(names.len()) {
                    out.push_str(&names[i]);
                }
            }
            TickFormat::Index { stamps, pattern, zone, locale } => {
                if let Some(i) = index(stamps.len()) {
                    match pattern {
                        Some(p) => zone.strftime(p, stamps[i], &locale.time, out),
                        None => zone.tick_label(stamps[i], &locale.time, out),
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::DAY;
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

        /// REQ-SCALE-040: a log scale of any base above 1 inverts what it
        /// maps within 10⁻⁹ of the value, on positive and negative domains
        /// either way round.
        #[test]
        fn log_inverts_for_any_base(
            base in 1.0001f64..1000.0,
            e0 in -30.0f64..30.0,
            decades in 0.01f64..12.0,
            t in 0.0f64..=1.0,
            cells in 4.0f64..400.0,
            reverse in any::<bool>(),
            negative in any::<bool>(),
        ) {
            let s = if negative { -1.0 } else { 1.0 };
            let (lo, hi) = (10f64.powf(e0), 10f64.powf(e0 + decades));
            let domain = if reverse { [s * hi, s * lo] } else { [s * lo, s * hi] };
            let scale = Scale::log(domain, [0.0, cells], base);
            let v = s * lo * (hi / lo).powf(t);
            let back = scale.invert(scale.map(v));
            prop_assert!((back - v).abs() <= 1e-9 * v.abs(), "{v} → {} → {back}", scale.map(v));
            prop_assert!(scale.map(domain[0]).abs() < 1e-9 && (scale.map(domain[1]) - cells).abs() < 1e-9 * cells);
        }
    }

    /// REQ-SCALE-040: every category gets the same whole number of cells,
    /// the bands tile their slots in order, and the band hit test and a
    /// linear axis over [`Band::domain`] agree with the bands.
    #[test]
    fn band_cells_are_equal() {
        assert_eq!(Band::new(0, 10, 0.0), None);
        assert_eq!(Band::new(11, 10, 0.0), None);
        for count in 1..=40 {
            for cells in count..=200 {
                for padding in [0.0, 0.1, 0.25, 0.5, 1.0] {
                    let band = Band::new(count, cells, padding).unwrap();
                    let widths: Vec<usize> = (0..count).map(|i| band.bar(i).unwrap().len()).collect();
                    assert!(widths.iter().all(|&w| w == widths[0] && w >= 1), "{count} in {cells}: {widths:?}");
                    assert!(band.per * count <= cells && cells - band.per * count < count, "{count} in {cells}: {band:?}");
                    let axis = crate::cells::Axis::x(band.domain()[0], band.domain()[1], cells, 1);
                    for i in 0..count {
                        let slot = band.slot(i).unwrap();
                        let bar = band.bar(i).unwrap();
                        assert!(slot.start <= bar.start && bar.end <= slot.end);
                        assert!(slot.clone().all(|c| band.at(c) == Some(i)));
                        assert!((axis.position(i as f64) - band.center(i)).abs() < 1e-9, "{count} in {cells}: {i}");
                        let cell = axis.cell_in(i as f64).unwrap();
                        assert!(slot.contains(&cell), "index {i} at cell {cell}, slot {slot:?}");
                    }
                    assert_eq!(band.slot(count), None);
                }
            }
        }
        let names: Arc<[String]> = ["a", "b", "c"].map(String::from).into();
        let s = Scale::band(&Band::new(3, 12, 0.25).unwrap(), Some(names));
        let labels = s.tick_labels(10.0, &AxisFormat::default(), &Locale::default());
        assert_eq!(labels, [(0.0, "a".to_string()), (1.0, "b".into()), (2.0, "c".into())]);
        assert_eq!(s.map(1.0), 6.0);
    }

    /// REQ-SCALE-050: symlog ticks are 0 and ± powers of ten inside the
    /// domain, thinned to the count by exponent, and the map is d3's.
    #[test]
    fn symlog_ticks_are_zero_and_powers_of_ten() {
        let s = Scale::symlog([-1000.0, 1e6], [0.0, 100.0], 1.0);
        assert_eq!(s.ticks(20.0), [-1000.0, -100.0, -10.0, -1.0, 0.0, 1.0, 10.0, 100.0, 1000.0, 1e4, 1e5, 1e6]);
        assert_eq!(s.ticks(7.0), [-100.0, -1.0, 0.0, 1.0, 100.0, 1e4, 1e6]);
        assert_eq!(s.ticks(5.0), [-10.0, 0.0, 10.0, 1e6]);
        assert_eq!(Scale::symlog([5.0, 5000.0], [0.0, 1.0], 1.0).ticks(10.0), [10.0, 100.0, 1000.0]);
        assert_eq!(Scale::symlog([-0.5, 0.5], [0.0, 1.0], 1.0).ticks(10.0), [-0.1, 0.0, 0.1]);
        assert_eq!(Scale::symlog([1e6, -1000.0], [0.0, 1.0], 1.0).ticks(5.0), [1e6, 10.0, 0.0, -10.0]);
        for (lo, hi) in [(-1e9, 1e9), (-3.0, 7e12), (0.0, 1e-3), (-123.0, -4.0), (2.0, 3.0)] {
            for count in [1.0, 3.0, 10.0, 30.0] {
                for t in Scale::symlog([lo, hi], [0.0, 1.0], 1.0).ticks(count) {
                    let a = t.abs();
                    assert!(t >= lo && t <= hi, "{t} outside [{lo}, {hi}]");
                    assert!(t == 0.0 || js::pow10(js::log10(a).round()) == a, "{t} is not a power of ten");
                }
            }
        }
        // d3's symlog map: sign(x) log1p(|x / c|), linear in between.
        let s = Scale::symlog([-100.0, 100.0], [0.0, 2.0], 1.0);
        assert_eq!(s.map(0.0), 1.0);
        assert!((s.map(100.0) - 2.0).abs() < 1e-12 && (s.map(-100.0)).abs() < 1e-12);
        assert!((s.invert(s.map(42.0)) - 42.0).abs() < 1e-9);
        let labels: Vec<String> = s.tick_labels(10.0, &AxisFormat::default(), &Locale::default()).into_iter().map(|(_, l)| l).collect();
        assert_eq!(labels, ["-100", "-10", "-1", "0", "1", "10", "100"]);
    }

    /// REQ-SCALE-060: rows sit at consecutive indices whatever the time
    /// between them, so weekday rows across weekends leave no column empty,
    /// and ticks fall on rows and are labelled from their own timestamps,
    /// never naming a weekend.
    #[test]
    fn index_time_skips_gaps() {
        // 60 weekdays from Monday 2026-01-05, 14:30 UTC each.
        let monday = crate::time::days_from_civil(2026, 1, 5) as f64 * DAY + 14.5 * HOUR;
        let stamps: Vec<f64> =
            (0..84).map(|d| monday + f64::from(d) * DAY).filter(|&t| !crate::time::Civil::utc(t).unwrap().weekday.is_multiple_of(6)).collect();
        let n = stamps.len();
        assert_eq!(n, 60);
        let stamps: Arc<[f64]> = stamps.into();
        for cols in [20usize, 60, 61, 120, 240] {
            let s = Scale::index(stamps.clone(), [-0.5, n as f64 - 0.5], [0.0, cols as f64]);
            let mut used = vec![false; cols];
            if cols >= n {
                for i in 0..n {
                    let c = s.map(i as f64).floor() as usize;
                    used[c] = true;
                    // Consecutive rows are a constant distance apart, weekend or not.
                    if i > 0 {
                        assert!((s.map(i as f64) - s.map(i as f64 - 1.0) - cols as f64 / n as f64).abs() < 1e-9);
                    }
                }
                // No run of empty columns wider than a row's share.
                let mut run = 0;
                for &u in &used {
                    run = if u { 0 } else { run + 1 };
                    assert!(run < cols.div_ceil(n), "{cols} columns: an empty run of {run}");
                }
            }
            for count in [2.0, 5.0, 10.0, 30.0] {
                let labels = s.tick_labels(count, &AxisFormat::Time(None), &Locale::default());
                assert!(!labels.is_empty());
                for (i, label) in &labels {
                    assert_eq!(*i, i.trunc());
                    assert!(*i >= 0.0 && *i < n as f64);
                    assert!(!label.starts_with("Sat") && !label.starts_with("Sun"), "{label}");
                }
                assert!(labels.windows(2).all(|w| w[0].0 < w[1].0));
            }
        }
        let s = Scale::index(stamps.clone(), [-0.5, n as f64 - 0.5], [0.0, 120.0]);
        let labels = s.tick_labels(4.0, &AxisFormat::Time(None), &Locale::default());
        // Month starts: February 2 and March 2, 2026, Mondays; January 5 is the first row.
        assert_eq!(labels, [(20.0, "February".to_string()), (40.0, "March".into())]);
        let iso = AxisFormat::parse("axes[0].format", "%Y-%m-%d", Kind::Index).unwrap();
        assert_eq!(s.tick_labels(4.0, &iso, &Locale::default())[0].1, "2026-02-02");
        assert_eq!(s.tick_format(4.0, &iso, &Locale::default()).format(5.0), "2026-01-12");
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
        // The new kinds: chosen by `type`, log domains on one side of 0 and niced to powers.
        let log = options(r#"{"type": "log", "nice": true}"#);
        assert_eq!(log.scale_kind(), Ok(Kind::Log));
        assert_eq!(resolve_domain(&log, Kind::Log, &ext, 10.0), Ok([1.0, 100.0]));
        assert_eq!(resolve_domain(&options(r#"{"type": "log", "base": 2, "nice": true}"#), Kind::Log, &ext, 10.0), Ok([2.0, 128.0]));
        assert!(resolve_domain(&options(r#"{"type": "log", "domain": [-1, 10]}"#), Kind::Log, &ext, 10.0).is_err());
        assert!(resolve_domain(&options(r#"{"type": "log", "base": 1}"#), Kind::Log, &ext, 10.0).is_err());
        assert_eq!(resolve_domain(&options(r#"{"type": "log"}"#), Kind::Log, &none, 10.0), Ok([1.0, 10.0]));
        let band = options(r#"{"type": "band", "domain": ["a", "b", "c"]}"#);
        assert_eq!(resolve_domain(&band, Kind::Band, &ext, 10.0), Ok([-0.5, 2.5]));
        assert_eq!(options(r#"{"type": "point"}"#).scale_kind().map_err(|e| e.reason.is_empty()), Err(false));
        for kind in [Kind::Linear, Kind::Log, Kind::Symlog, Kind::Time, Kind::Index, Kind::Band] {
            assert_eq!(Kind::from_type(kind.scale_type()), Some(kind));
            assert_eq!(ScaleOptions::new(kind).scale_kind(), Ok(kind));
        }
    }

    #[test]
    fn nice_leaves_an_unsettled_domain() {
        assert_eq!(nice(0.2, 9.7, 10.0), [0.0, 10.0]);
        assert_eq!(nice(9.7, 0.2, 10.0), [10.0, 0.0]);
        // A count of 1 across zero never settles: d3 gives the domain back.
        assert_eq!(nice(-0.5, 0.75, 1.0), [-0.5, 0.75]);
        assert_eq!(log_nice(3.0, 4500.0, 10.0), [1.0, 10000.0]);
        assert_eq!(log_nice(-4500.0, -3.0, 10.0), [-10000.0, -1.0]);
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
        assert!(AxisFormat::parse("axes[1].format", "%Y %U", Kind::Time).is_err());
        assert_eq!(s.map(1.7e9), 19.5);
        assert_eq!(s.invert(19.5), 1.7e9);
        let log = Scale::log([1.0, 1e4], [0.0, 40.0], 10.0);
        assert_eq!(log.map(100.0), 20.0);
        let labels: Vec<String> = log.tick_labels(10.0, &AxisFormat::default(), &Locale::default()).into_iter().map(|(_, l)| l).collect();
        assert_eq!(labels, ["1", "10", "100", "1k", "10k"]);
        assert_eq!(log.minor_ticks(10.0).len(), 32);
    }
}
