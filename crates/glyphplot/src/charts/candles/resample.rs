//! Calendar resampling (REQ-SAMPLE-040, ADR-109) and the candles' positions
//! on the x axis (REQ-SCALE-060, ADR-131).
//!
//! When the candles in view do not fit one a cell (four cells in one ink,
//! REQ-CANDLE-030), the rows are resampled to the smallest interval of the
//! ladder 1 s, 5 s, 15 s, 1 min, 5 min, 15 min, 1 h, 4 h, 1 d, 1 w, 1 mo at
//! which they fit: open the first row's, high the greatest, low the least,
//! close the last row's, volume the sum, over buckets aligned to the interval
//! in the zone's local time (weeks start on Monday). Past a month, which the
//! requirement does not reach, the ladder goes on with 3 months, then 1, 5,
//! 10, 50, 100, 500 and 1,000 years, so a long series still fits. An x that
//! is not a time steps through 1, 2 and 5 × 10ⁿ of its own units instead.
//!
//! A [`Level`] holds the candles of one interval (or the rows themselves,
//! `interval: None`) over a range of rows: every bucket when there are at
//! most [`WHOLE`] of them, kept by data version, interval and zone, so a pan
//! reads no rows; else the buckets around the view, made for it.
//!
//! **Positions.** On the index scale candle `k` sits at position `k`, and
//! position maps to time piecewise linearly: the candle owns the time from
//! the midpoint between its first row and the last row before it to the
//! midpoint between its last row and the next row, and the data's ends are
//! padded by half their first and last row spacing. A time in a gap (a night,
//! a weekend) falls in the candle it borders, so gaps take no columns
//! (REQ-SCALE-060), the map is continuous and increasing, and the data's
//! fitted range is `[−½, n − ½]` at every interval. On a time scale the
//! position is the time itself, a resampled candle at its bucket's middle.

use std::sync::Arc;

use super::data::Prepared;
use crate::hit::lower_bound;
use crate::scale::Kind;
use crate::time::{civil_from_days, days_from_civil, Zone, DAY, HOUR, MINUTE, SECOND};

/// The most buckets a level holds whole; past it a level is made for the
/// rows around the view.
pub const WHOLE: f64 = 65_536.0;

/// The levels kept at once.
const KEPT: usize = 16;

/// A bucket interval.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Interval {
    /// A fixed length in x units (ms on a time scale), aligned to multiples
    /// of it in local time.
    Fixed(f64),
    /// A local day.
    Day,
    /// A local week from Monday.
    Week,
    /// Local calendar months, `n` at a time, aligned to multiples of `n`
    /// from January of year 0.
    Months(u32),
}

/// The ladder of REQ-SAMPLE-040, then the steps past it.
const TIME_LADDER: [Interval; 18] = [
    Interval::Fixed(SECOND),
    Interval::Fixed(5.0 * SECOND),
    Interval::Fixed(15.0 * SECOND),
    Interval::Fixed(MINUTE),
    Interval::Fixed(5.0 * MINUTE),
    Interval::Fixed(15.0 * MINUTE),
    Interval::Fixed(HOUR),
    Interval::Fixed(4.0 * HOUR),
    Interval::Day,
    Interval::Week,
    Interval::Months(1),
    Interval::Months(3),
    Interval::Months(12),
    Interval::Months(60),
    Interval::Months(120),
    Interval::Months(600),
    Interval::Months(1_200),
    Interval::Months(12_000),
];

/// The least power of ten of a plain x's ladder.
const LINEAR_FROM: i32 = -12;
/// The steps of a plain x's ladder: 1, 2 and 5 × 10ⁿ for n from −12 to 300.
const LINEAR_STEPS: usize = 939;

/// The ladder's length for x of `kind`.
pub fn ladder_len(kind: Kind) -> usize {
    if kind == Kind::Time {
        TIME_LADDER.len()
    } else {
        LINEAR_STEPS
    }
}

/// Step `i` of the ladder for x of `kind`.
pub fn ladder(kind: Kind, i: usize) -> Option<Interval> {
    if kind == Kind::Time {
        return TIME_LADDER.get(i).copied();
    }
    (i < LINEAR_STEPS).then(|| Interval::Fixed([1.0, 2.0, 5.0][i % 3] * crate::format::js::pow10(f64::from(LINEAR_FROM + (i / 3) as i32))))
}

impl Interval {
    /// The shortest a bucket can be, in ms: a local day may be 23 hours.
    pub fn min_len(&self) -> f64 {
        match *self {
            Interval::Fixed(len) => len,
            Interval::Day => DAY - HOUR,
            Interval::Week => 7.0 * DAY - HOUR,
            Interval::Months(m) => f64::from(m) * 28.0 * DAY - HOUR,
        }
    }

    /// Its name in the readout and the table: `5 s`, `1 min`, `4 h`, `1 d`,
    /// `1 w`, `1 mo`, `3 mo`, `1 y`.
    pub fn name(&self) -> String {
        let n = |v: f64| {
            let mut s = String::new();
            crate::format::js::to_string(v, &mut s);
            s
        };
        match *self {
            Interval::Fixed(len) if len >= HOUR && len % HOUR == 0.0 => format!("{} h", n(len / HOUR)),
            Interval::Fixed(len) if len >= MINUTE && len % MINUTE == 0.0 => format!("{} min", n(len / MINUTE)),
            Interval::Fixed(len) if len >= SECOND && len % SECOND == 0.0 => format!("{} s", n(len / SECOND)),
            Interval::Fixed(len) => n(len),
            Interval::Day => "1 d".into(),
            Interval::Week => "1 w".into(),
            Interval::Months(m) if m % 12 == 0 => format!("{} y", m / 12),
            Interval::Months(m) => format!("{m} mo"),
        }
    }

    /// The strftime pattern a candle's time is written in.
    pub fn pattern(&self) -> &'static str {
        match *self {
            Interval::Fixed(len) if len < MINUTE => "%Y-%m-%d %H:%M:%S",
            Interval::Fixed(_) => "%Y-%m-%d %H:%M",
            Interval::Day | Interval::Week => "%Y-%m-%d",
            Interval::Months(m) if m % 12 == 0 => "%Y",
            Interval::Months(_) => "%Y-%m",
        }
    }

    /// The start of the bucket holding local time `local`, in local ms, and
    /// the start of the next.
    fn bucket(&self, local: f64) -> (f64, f64) {
        // Calendars past a JavaScript date fall back to fixed lengths.
        let calendar = local.abs() <= 8.64e15;
        match *self {
            Interval::Fixed(len) => {
                let k = (local / len).floor() * len;
                (k, k + len)
            }
            Interval::Day => {
                let k = (local / DAY).floor() * DAY;
                (k, k + DAY)
            }
            Interval::Week => {
                let d = (local / DAY).floor();
                let from_monday = (d + 3.0).rem_euclid(7.0);
                let k = (d - from_monday) * DAY;
                (k, k + 7.0 * DAY)
            }
            Interval::Months(m) if calendar => {
                let (y, mo, _) = civil_from_days((local / DAY).floor() as i64);
                let idx = y * 12 + i64::from(mo) - 1;
                let first = idx - idx.rem_euclid(i64::from(m));
                let at = |i: i64| days_from_civil(i.div_euclid(12), (i.rem_euclid(12) + 1) as u32, 1) as f64 * DAY;
                (at(first), at(first + i64::from(m)))
            }
            Interval::Months(m) => {
                let len = f64::from(m) * 30.0 * DAY;
                let k = (local / len).floor() * len;
                (k, k + len)
            }
        }
    }
}

/// Where a time is in a zone: its offset in ms.
fn offset(zone: &Zone, time: bool, t: f64) -> f64 {
    if time {
        f64::from(zone.offset_minutes(t)) * MINUTE
    } else {
        0.0
    }
}

/// The candles of one interval over the rows `from..to`, as columns.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Level {
    /// The interval, or `None` for the rows themselves.
    pub interval: Option<Interval>,
    /// The first row it covers.
    pub from: usize,
    /// The row past the last it covers.
    pub to: usize,
    /// Each candle's first row's x.
    pub start: Vec<f64>,
    /// Each candle's last row's x.
    pub end: Vec<f64>,
    /// Each candle's time as labels write it: its bucket's start, or its
    /// row's x.
    pub stamp: Vec<f64>,
    /// Each candle's place on a time scale: its bucket's middle, or its
    /// row's x.
    pub middle: Vec<f64>,
    /// Open, high, low and close, each a column.
    pub ohlc: [Vec<f64>; 4],
    /// The summed volume, NaN without a volume column.
    pub volume: Vec<f64>,
    /// Each candle's rows: first, last, and the rows its open, high, low and
    /// close come from.
    pub rows: Vec<[usize; 6]>,
    /// The time before the first candle where its span starts.
    pub lead: f64,
    /// The time after the last candle where its span ends.
    pub tail: f64,
}

/// The candles' x positions: index or time (ADR-131).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    /// Consecutive indices, gaps in time taking no columns.
    Index,
    /// Time itself.
    Time,
}

impl Level {
    /// The candles.
    pub fn len(&self) -> usize {
        self.start.len()
    }

    /// Whether it holds no candle.
    pub fn is_empty(&self) -> bool {
        self.start.is_empty()
    }

    /// Whether candle `k` has all four prices.
    pub fn whole(&self, k: usize) -> bool {
        self.ohlc.iter().all(|c| c[k].is_finite())
    }

    /// Where candle `k`'s span starts (`k` = 0) or ends: the bound between
    /// candle `k` and `k + 1`, for `k` from −1 to `len − 1`.
    fn bound(&self, k: isize) -> f64 {
        let n = self.len() as isize;
        if k < 0 {
            self.lead
        } else if k >= n - 1 {
            self.tail
        } else {
            (self.end[k as usize] + self.start[k as usize + 1]) / 2.0
        }
    }

    /// The position of time `t` (REQ-SCALE-060); NaN with no candle.
    pub fn pos(&self, place: Place, t: f64) -> f64 {
        if place == Place::Time {
            return t;
        }
        if self.is_empty() || !t.is_finite() {
            return f64::NAN;
        }
        let j = self.start.partition_point(|&s| s <= t) as isize - 1;
        let k = if j < 0 {
            0
        } else if (j as usize) + 1 < self.len() && t >= self.bound(j) {
            j + 1
        } else {
            j
        };
        let (a, b) = (self.bound(k - 1), self.bound(k));
        if b > a {
            k as f64 - 0.5 + (t - a) / (b - a)
        } else {
            k as f64
        }
    }

    /// The time at position `p`, the inverse of [`Level::pos`].
    #[cfg(test)]
    pub fn time(&self, place: Place, p: f64) -> f64 {
        if place == Place::Time || self.is_empty() || !p.is_finite() {
            return p;
        }
        let n = self.len() as f64;
        let k = (p + 0.5).floor().max(0.0).min(n - 1.0) as isize;
        let (a, b) = (self.bound(k - 1), self.bound(k));
        a + (p - (k as f64 - 0.5)) * (b - a)
    }

    /// Candle `k`'s position.
    pub fn centre(&self, place: Place, k: usize) -> f64 {
        match place {
            Place::Index => k as f64,
            Place::Time => self.middle[k],
        }
    }

    /// The candles whose position lies in `[a, b]`.
    pub fn in_range(&self, place: Place, a: f64, b: f64) -> std::ops::Range<usize> {
        if self.is_empty() || a.is_nan() || b.is_nan() || a > b {
            return 0..0;
        }
        match place {
            Place::Index => {
                let n = self.len() as f64;
                let k0 = a.ceil().max(0.0);
                let k1 = (b.floor() + 1.0).min(n);
                if k0 < k1 {
                    k0 as usize..k1 as usize
                } else {
                    0..0
                }
            }
            Place::Time => {
                let k0 = self.middle.partition_point(|&m| m < a);
                let k1 = self.middle.partition_point(|&m| m <= b);
                k0..k1.max(k0)
            }
        }
    }

    /// The candle holding table row `row`, if this level covers it.
    pub fn holding(&self, row: usize) -> Option<usize> {
        if row < self.from || row >= self.to {
            return None;
        }
        let k = self.rows.partition_point(|r| r[0] <= row);
        k.checked_sub(1)
    }
}

/// The spacing of the first rows (`first`) or the last, the first that is
/// above 0 within 64 rows: what the data's end candles are padded by half
/// of; a second or 1 when the rows give none.
fn edge_gap(xs: &[f64], rows: &std::ops::Range<usize>, first: bool, kind: Kind) -> f64 {
    let n = rows.len();
    let mut gaps = (1..n.min(65)).map(|i| if first { xs[rows.start + i] - xs[rows.start + i - 1] } else { xs[rows.end - i] - xs[rows.end - i - 1] });
    gaps.find(|g| *g > 0.0 && g.is_finite()).unwrap_or(if kind == Kind::Time { SECOND } else { 1.0 })
}

/// The data's fitted x domain: its first and last rows padded by half
/// their spacing, which every level maps to `[−½, n − ½]`; None with no
/// rows.
pub fn fitted(data: &Prepared) -> Option<[f64; 2]> {
    let xs = data.xs();
    let r = data.rows.clone();
    if r.is_empty() {
        return None;
    }
    let lead = xs[r.start] - edge_gap(xs, &r, true, data.kind) / 2.0;
    let tail = xs[r.end - 1] + edge_gap(xs, &r, false, data.kind) / 2.0;
    Some([lead, tail])
}

/// Builds a level of `interval` (or of the rows) over rows `from..to`,
/// whose ends are bucket ends.
pub fn build(data: &Prepared, interval: Option<Interval>, from: usize, to: usize, zone: &Zone) -> Level {
    let xs = data.xs();
    let [o, h, l, c] = [0, 1, 2, 3].map(|k| data.price(k));
    let v = data.volume();
    let time = data.kind == Kind::Time;
    let mut lv = Level { interval, from, to, ..Level::default() };
    let rows = data.rows.clone();
    let [lead, tail] = fitted(data).unwrap_or([0.0, 0.0]);
    lv.lead = if from <= rows.start { lead } else { (xs[from - 1] + xs[from]) / 2.0 };
    lv.tail = if to >= rows.end { tail } else { (xs[to - 1] + xs[to]) / 2.0 };
    let est = (to - from).min(4096);
    for col in lv.ohlc.iter_mut() {
        col.reserve(est);
    }
    let key = |t: f64| -> (f64, f64, f64) {
        match interval {
            None => (t, t, t),
            Some(i) => {
                let off = offset(zone, time, t);
                let (a, b) = i.bucket(t + off);
                (a, a - off, b - off)
            }
        }
    };
    let mut r = from;
    while r < to {
        let (k, stamp, next) = key(xs[r]);
        let mut e = r + 1;
        if interval.is_some() {
            while e < to && key(xs[e]).0 == k {
                e += 1;
            }
        }
        // Open the first finite, close the last, high the greatest, low the
        // least, volume the sum (REQ-SAMPLE-040).
        let open = (r..e).find(|&i| o[i].is_finite());
        let close = (r..e).rev().find(|&i| c[i].is_finite());
        let (mut hi, mut lo) = (None::<usize>, None::<usize>);
        let mut vol = f64::NAN;
        for i in r..e {
            if h[i].is_finite() && hi.is_none_or(|j| h[i] > h[j]) {
                hi = Some(i);
            }
            if l[i].is_finite() && lo.is_none_or(|j| l[i] < l[j]) {
                lo = Some(i);
            }
            if let Some(&x) = v.get(i) {
                if x.is_finite() {
                    vol = if vol.is_nan() { x } else { vol + x };
                }
            }
        }
        let val = |col: &[f64], i: Option<usize>| i.map_or(f64::NAN, |i| col[i]);
        lv.start.push(xs[r]);
        lv.end.push(xs[e - 1]);
        lv.stamp.push(stamp);
        lv.middle.push(if interval.is_some() { stamp + (next - stamp) / 2.0 } else { xs[r] });
        lv.ohlc[0].push(val(o, open));
        lv.ohlc[1].push(val(h, hi));
        lv.ohlc[2].push(val(l, lo));
        lv.ohlc[3].push(val(c, close));
        lv.volume.push(vol);
        lv.rows.push([r, e - 1, open.unwrap_or(r), hi.unwrap_or(r), lo.unwrap_or(r), close.unwrap_or(r)]);
        r = e;
    }
    lv
}

/// The first row of the bucket holding row `r`, and the row past its last.
fn bucket_rows(data: &Prepared, interval: Option<Interval>, r: usize, zone: &Zone) -> (usize, usize) {
    let Some(i) = interval else { return (r, r + 1) };
    let xs = data.xs();
    let rows = data.rows.clone();
    let time = data.kind == Kind::Time;
    let key = |t: f64| i.bucket(t + offset(zone, time, t)).0;
    let k = key(xs[r]);
    let first = rows.start + xs[rows.start..=r].partition_point(|&t| key(t) < k);
    let past = r + xs[r..rows.end].partition_point(|&t| key(t) <= k);
    (first, past.max(r + 1))
}

/// The one candle of `interval` holding row `row`, as a level of one.
pub fn candle_at(data: &Prepared, interval: Option<Interval>, row: usize, zone: &Zone) -> Level {
    let (a, b) = bucket_rows(data, interval, row, zone);
    build(data, interval, a, b, zone)
}

/// What a level is kept by.
#[derive(Clone, Debug, PartialEq)]
struct Key {
    version: u64,
    zone: u64,
    step: Option<usize>,
    from: usize,
    to: usize,
}

/// The levels kept, by data version, zone, interval and rows.
#[derive(Debug, Default)]
pub struct Cache {
    kept: Vec<(Key, Arc<Level>)>,
    builds: u64,
}

impl Cache {
    /// Forgets every level.
    pub fn clear(&mut self) {
        self.kept.clear();
    }

    /// How many levels have been built: the rows read (tests).
    pub fn builds(&self) -> u64 {
        self.builds
    }
}

/// What resampling needs besides the data: its version, the zone and its
/// version.
#[derive(Clone, Copy)]
pub struct Ctx<'a> {
    /// The data.
    pub data: &'a Prepared,
    /// The data version.
    pub version: u64,
    /// The zone buckets align in.
    pub zone: &'a Zone,
    /// The zone's version.
    pub zone_version: u64,
    /// Index or time positions.
    pub place: Place,
}

impl Ctx<'_> {
    /// The rows whose candles a view `[ta, tb]` may show: one row past each
    /// end, so the candles at the edges are whole.
    fn view_rows(&self, ta: f64, tb: f64) -> (usize, usize) {
        let xs = self.data.xs();
        let rows = self.data.rows.clone();
        let a = lower_bound(xs, rows.start, rows.end, ta).saturating_sub(1).max(rows.start);
        let b = (lower_bound(xs, rows.start, rows.end, tb) + 1).min(rows.end);
        (a, b.max(a))
    }

    /// The level of ladder step `step` (`None`: the rows) for a view: every
    /// bucket when there are at most [`WHOLE`], else those around the view.
    pub fn level(&self, cache: &mut Cache, step: Option<usize>, view: [f64; 2]) -> Arc<Level> {
        let rows = self.data.rows.clone();
        let interval = step.and_then(|i| ladder(self.data.kind, i));
        let whole = match (interval, self.data.x_extent()) {
            (Some(i), Some([a, b])) => (b - a) / i.min_len() + 2.0 <= WHOLE,
            _ => false,
        };
        let (from, to) = if rows.is_empty() {
            (0, 0)
        } else if whole {
            (rows.start, rows.end)
        } else {
            let (a, b) = self.view_rows(view[0], view[1]);
            if a >= b {
                (a, a)
            } else {
                (bucket_rows(self.data, interval, a, self.zone).0, bucket_rows(self.data, interval, b - 1, self.zone).1)
            }
        };
        let key = Key { version: self.version, zone: self.zone_version, step, from, to };
        if let Some((_, l)) = cache.kept.iter().find(|(k, _)| *k == key) {
            return Arc::clone(l);
        }
        let level = Arc::new(build(self.data, interval, from, to, self.zone));
        cache.builds += 1;
        if cache.kept.len() == KEPT {
            cache.kept.remove(0);
        }
        cache.kept.push((key, Arc::clone(&level)));
        level
    }

    /// Whether the candles of `level` in `view` fit `cap` to the plot: their
    /// positions span at most `cap` (index), or no two neighbours are nearer
    /// than the view over `cap` (time).
    fn fits(&self, level: &Level, view: [f64; 2], cap: f64) -> bool {
        match self.place {
            Place::Index => level.pos(Place::Index, view[1]) - level.pos(Place::Index, view[0]) <= cap,
            Place::Time => {
                let r = level.in_range(Place::Time, view[0], view[1]);
                let near = (view[1] - view[0]) / cap;
                r.len() as f64 <= cap + 1.0 && level.middle[r].windows(2).all(|w| w[1] - w[0] >= near)
            }
        }
    }

    /// The ladder step the view is drawn at (REQ-SAMPLE-040): `None` when
    /// the rows fit `cap` (the plot's cells over the least spacing),
    /// otherwise the smallest step whose candles do.
    pub fn choose(&self, cache: &mut Cache, view: [f64; 2], cap: f64) -> Option<usize> {
        let kind = self.data.kind;
        let n = ladder_len(kind);
        if self.data.rows.is_empty() || cap.is_nan() || cap <= 0.0 {
            return None;
        }
        let span = view[1] - view[0];
        let (a, b) = self.view_rows(view[0], view[1]);
        if (b - a) as f64 <= cap + 3.0 {
            let raw = self.level(cache, None, view);
            if self.fits(&raw, view, cap) {
                return None;
            }
        }
        // The first step whose buckets cannot outnumber the cells: a view
        // touches at most span / length + 2 buckets.
        if self.place == Place::Time {
            // On a time scale a bucket is at least its length apart from the next.
            return (0..n).find(|&i| ladder(kind, i).is_some_and(|iv| iv.min_len() >= span / cap)).or(Some(n - 1));
        }
        let mut best = (0..n).find(|&i| ladder(kind, i).is_some_and(|iv| span / iv.min_len() + 2.0 <= cap)).unwrap_or(n - 1);
        // A finer step may fit too when the data has gaps.
        while best > 0 {
            let finer = self.level(cache, Some(best - 1), view);
            if !self.fits(&finer, view, cap) {
                break;
            }
            best -= 1;
        }
        Some(best)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::Encode;
    use crate::fixtures::generate;

    fn prep(name: &str) -> Prepared {
        Prepared::new(generate(name).unwrap().into_tables().remove(0).1, &Encode::default())
    }

    fn ctx(data: &Prepared) -> Ctx<'_> {
        static UTC: Zone = Zone::Utc;
        Ctx { data, version: 1, zone: &UTC, zone_version: 0, place: Place::Index }
    }

    /// REQ-SAMPLE-040: the smallest step of the ladder whose candles fit is
    /// chosen; 500 weekdays are 100 weeks and 24 months, a million seconds
    /// 70 four-hour buckets and 12 days.
    #[test]
    fn picks_the_smallest_fitting_interval() {
        let days = prep("ohlcv-days");
        let c = ctx(&days);
        let mut cache = Cache::default();
        let view = fitted(&days).unwrap();
        let name = |s: Option<usize>| s.and_then(|i| ladder(Kind::Time, i)).map_or("rows".to_string(), |i| i.name());
        assert_eq!(name(c.choose(&mut cache, view, 500.0)), "rows");
        assert_eq!(name(c.choose(&mut cache, view, 499.0)), "1 w");
        assert_eq!(name(c.choose(&mut cache, view, 100.0)), "1 w");
        assert_eq!(name(c.choose(&mut cache, view, 99.0)), "1 mo");
        assert_eq!(name(c.choose(&mut cache, view, 23.9)), "3 mo");
        assert_eq!(name(c.choose(&mut cache, view, 7.9)), "1 y");
        let step = c.choose(&mut cache, view, 99.0);
        let lv = c.level(&mut cache, step, view);
        assert_eq!(lv.len(), 24);
        assert_eq!((lv.pos(Place::Index, view[0]), lv.pos(Place::Index, view[1])), (-0.5, 23.5));
        let ticks = prep("ohlcv-ticks-1m");
        let c = ctx(&ticks);
        let view = fitted(&ticks).unwrap();
        assert_eq!(name(c.choose(&mut cache, view, 75.0)), "4 h");
        let step = c.choose(&mut cache, view, 75.0);
        assert_eq!(c.level(&mut cache, step, view).len(), 70);
        assert_eq!(name(c.choose(&mut cache, view, 60.0)), "1 d");
        assert_eq!(name(c.choose(&mut cache, view, 300.0)), "1 h");
    }

    /// REQ-SAMPLE-040: a bucket's open is its first row's, its high the
    /// greatest, its low the least, its close the last row's and its volume
    /// the sum, buckets aligned to the interval.
    #[test]
    fn buckets_aggregate_open_high_low_close_volume() {
        let days = prep("ohlcv-days");
        let lv = build(&days, Some(Interval::Week), days.rows.start, days.rows.end, &Zone::Utc);
        assert_eq!(lv.len(), 100);
        let rows = 0..5;
        let col = |k: usize| &days.price(k)[rows.clone()];
        assert_eq!(lv.ohlc[0][0], col(0)[0]);
        assert_eq!(lv.ohlc[1][0], col(1).iter().copied().fold(f64::MIN, f64::max));
        assert_eq!(lv.ohlc[2][0], col(2).iter().copied().fold(f64::MAX, f64::min));
        assert_eq!(lv.ohlc[3][0], col(3)[4]);
        assert_eq!(lv.volume[0], days.volume()[rows].iter().sum::<f64>());
        // Monday 2024-10-21 00:00 UTC.
        assert_eq!(lv.stamp[0], 20_017.0 * DAY);
        assert!(lv.rows.iter().all(|r| days.price(1)[r[3]] >= days.price(1)[r[0]]));
    }

    /// REQ-SCALE-060: on the index scale a weekend takes no position, and
    /// position and time are each other's inverse.
    #[test]
    fn gaps_take_no_position() {
        let days = prep("ohlcv-days");
        let lv = build(&days, None, days.rows.start, days.rows.end, &Zone::Utc);
        let xs = days.xs();
        // Friday row 4 and Monday row 5 are neighbours, three days apart:
        // each row's time lies in its own candle's span, `k ± ½`.
        assert_eq!(xs[5] - xs[4], 3.0 * DAY);
        for (k, &x) in xs.iter().enumerate().take(500) {
            let p = lv.pos(Place::Index, x);
            assert!(p > k as f64 - 0.5 && p < k as f64 + 0.5, "row {k} at {p}");
            assert_eq!(lv.centre(Place::Index, k), k as f64);
        }
        assert_eq!(lv.pos(Place::Index, (xs[4] + xs[5]) / 2.0), 4.5, "the weekend's middle is the bound");
        for p in [-0.5, 0.0, 3.25, 4.5, 499.5] {
            assert!((lv.pos(Place::Index, lv.time(Place::Index, p)) - p).abs() < 1e-9, "{p}");
        }
    }
}
