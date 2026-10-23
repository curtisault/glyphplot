//! The `candles` family: open, high, low and close over time as candles,
//! hollow candles or OHLC bars, with a volume pane (REQ-CANDLE-010 to 050,
//! REQ-SAMPLE-040, REQ-SCALE-060, REQ-THEME-060).
//!
//! x is time in the view, the cursor and every event, as data units are
//! (REQ-VIEW-020). The candles sit on the index time scale by default
//! (`xScale: "index"`, ADR-131): consecutive candles at consecutive
//! positions whatever the time between them, so nights and weekends take no
//! columns; `xScale: "time"` places them by time. When the candles in view
//! do not fit one a cell they are resampled to a calendar interval
//! (`resample.rs`, REQ-SAMPLE-040), four cells in one ink (REQ-CANDLE-030),
//! and their glyphs follow the spacing (`draw.rs`, REQ-CANDLE-020). A
//! `volume` column draws a second pane under the prices sharing x
//! (REQ-CANDLE-040, REQ-LAYOUT-050). Under 20 rows a pane's ticks snap to
//! rows (REQ-TICK-050). The cursor snaps to the candle under the pointer
//! and to the nearest of its open, high, low and close, and names the row
//! that value came from; the readout writes the candle's `o h l c`, volume
//! and signed change (REQ-CANDLE-050).
//!
//! As the line chart, the base (candles, volume, axes, labels) is drawn
//! again only when the data, options, view domains, size or env change, and
//! the overlay is drawn over a copy of it every frame (REQ-OVERLAY-010).
//! Levels of candles are kept by data version, interval and zone, so a pan
//! at one interval reads no rows.

mod axes;
mod data;
mod draw;
mod readout;
mod resample;

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Arc;

use crate::cells::Axis;
use crate::chart::{Accepted, Appended, CellPos, Changes, Charset, Chart, Env, Hit, MakeError, Meta, Mono, Part, Size, Table};
use crate::data::{Data, DataError, Encode, Shape};
use crate::events::{Aggregate, Value};
use crate::format::{Display, Locale, Specifier};
use crate::grid::{Grid, NO_BACK};
use crate::hit::lower_bound;
use crate::input::Input;
use crate::layout::{fit_scale_ticks, layout, AxisInput, Layout, LayoutExtents, LayoutInput, PaneExtents, PaneInput, Rect, Side};
use crate::locale::{LocalePack, ZoneTable};
use crate::options::Options;
use crate::palette::GROUND;
use crate::reduce::{reduce, DragMode, Gesture, Interaction, Plot, Scene, WheelMode};
use crate::registry::{Entry, Family};
use crate::ring::Retain;
use crate::scale::{AxisFormat, Extents, Kind, Scale};
use crate::table::pick_rows;
use crate::ticks::Snap;
use crate::time::Zone;
use crate::view::{Cursor, Fit, View, ViewPatch};
use crate::Budget;

use data::Prepared;
use draw::{BaseInput, Geom};
use resample::{Level, Place};

pub use draw::Glyphs;

/// The `candles` type (REQ-CANDLE-010, REQ-CHART-020).
pub static META: Meta = Meta {
    slug: "candles",
    name: "candles",
    note: "open, high, low and close over time, as candles or bars",
    family: "candles",
    modes: &["candle", "hollow", "ohlc"],
    shapes: &[Shape::Ohlcv],
    options: Some(
        r#"{"mode":"candle","title":"","encode":{},"xScale":"index","snap":"auto","volume":true,"drag":"pan","wheel":"scroll","motion":false,"retain":{"rows":1000000}}"#,
    ),
    min: Size { cols: 26, rows: 10 },
    drop: &[Part::Title, Part::Axes],
    mono: Mono::Authored,
    charset: Charset::Extended,
    budget: Budget::DEFAULT,
};

/// The family's types, by hand (REQ-REG-010, ADR-125).
pub const REGISTRY: &[Entry] = &[Entry { meta: &META, make }];

/// The family's registry entry.
pub const FAMILY: Family = Family { name: "candles", budget: Budget::DEFAULT, charts: REGISTRY };

/// The rows from which the volume pane is drawn.
const VOLUME_ROWS: usize = 8;

/// The price pane's share of the rows to the volume pane's.
const WEIGHTS: (u32, u32) = (3, 1);

/// The cells a candle needs: one in colour (REQ-SAMPLE-040), four in one ink
/// (REQ-CANDLE-030).
const SPACING: (f64, f64) = (1.0, 4.0);

/// A fresh `candles` chart for options already layered on [`META`]'s.
pub fn make(options: &Options) -> Result<Box<dyn Chart>, MakeError> {
    Ok(Box::new(CandlesChart::new(options)?))
}

/// How candles are drawn: the `mode` option (REQ-CANDLE-010).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Candles coloured by close against open.
    Candle,
    /// Hollow candles: filled by close against open, coloured by close
    /// against the previous close.
    Hollow,
    /// OHLC bars.
    Ohlc,
}

/// The options as the chart reads them.
#[derive(Clone, Debug, PartialEq)]
struct Config {
    mode: Mode,
    title: String,
    encode: Encode,
    /// Index positions (the default) or time.
    index: bool,
    /// The `snap` option: rows, none, or by the pane's rows.
    snap: Option<Snap>,
    /// Whether a volume column draws its pane.
    volume: bool,
    interaction: Interaction,
    /// The `retain` option: the ring's rows and age (REQ-STREAM-020).
    retain: Retain,
}

/// The options read: every key has a default in [`META`]'s (REQ-OPT-010).
fn config(o: &Options) -> Result<Config, MakeError> {
    let mode = match o.mode(&META)? {
        "hollow" => Mode::Hollow,
        "ohlc" => Mode::Ohlc,
        _ => Mode::Candle,
    };
    let encode = o.object("encode").and_then(|m| serde_json::from_value::<Encode>(serde_json::Value::Object(m.clone())).ok()).unwrap_or_default();
    Ok(Config {
        mode,
        title: o.str("title").to_string(),
        encode,
        index: o.choice("xScale", &["index", "time"]) == "index",
        snap: match o.choice("snap", &["auto", "rows", "none"]) {
            "rows" => Some(Snap::Rows),
            "none" => Some(Snap::None),
            _ => None,
        },
        volume: o.bool("volume"),
        interaction: Interaction {
            drag: match o.choice("drag", &["pan", "brush", "zoom"]) {
                "brush" => DragMode::Brush,
                "zoom" => DragMode::Zoom,
                _ => DragMode::Pan,
            },
            wheel: if o.choice("wheel", &["scroll", "zoom"]) == "zoom" { WheelMode::Zoom } else { WheelMode::Scroll },
            motion: o.bool("motion"),
        },
        retain: Retain::from_options(o),
    })
}

/// What the layout and the candles drawn depend on (REQ-LAYOUT-010).
#[derive(Clone, Debug, PartialEq)]
struct GeomKey {
    cols: usize,
    rows: usize,
    opts: u64,
    data: u64,
    /// The time domain drawn, as bits.
    x: [u64; 2],
    y: Option<[u64; 2]>,
    fit: Fit,
    locale: u64,
    zone: u64,
    /// One ink: candles of four cells (REQ-CANDLE-030).
    mono: bool,
}

/// What the base depends on besides the geometry.
#[derive(Clone, Debug, PartialEq)]
struct BaseKey {
    geom: GeomKey,
    paper: bool,
}

/// The last frame's size, play time and env flags.
#[derive(Clone, Copy, Debug, PartialEq)]
struct FrameCtx {
    cols: usize,
    rows: usize,
    t: f64,
    paper: bool,
    mono: bool,
}

/// The readout's parts, and the candle and value the crosshair marks.
type Readout = (Vec<String>, Option<(usize, f64)>);

/// A `candles` chart (REQ-CANDLE-010): the [`Chart`] the registry makes for
/// the type `candles`.
pub struct CandlesChart {
    options: Options,
    cfg: Config,
    opts_version: u64,
    data: Prepared,
    data_version: u64,
    view: View,
    gesture: Gesture,
    levels: RefCell<resample::Cache>,
    /// The y extent of every row, by data version.
    all: RefCell<Option<(u64, Option<[f64; 2]>)>>,
    geom: Option<(GeomKey, Geom)>,
    ctx: Option<FrameCtx>,
    pack: LocalePack,
    locale: Locale,
    locale_version: u64,
    zone_table: ZoneTable,
    zone: Zone,
    zone_version: u64,
    base: Grid,
    base_key: Option<BaseKey>,
    base_draws: u64,
    canvas: crate::canvas::ColorCanvas,
}

/// The coarser of two ladder steps, the rows the finest.
fn coarser(a: Option<usize>, b: Option<usize>) -> Option<usize> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.max(y)),
        (x, None) | (None, x) => x,
    }
}

impl CandlesChart {
    /// A chart with no data for options layered on [`META`]'s; a mode not
    /// listed is an error naming the valid ones (REQ-CHART-030).
    pub fn new(options: &Options) -> Result<CandlesChart, MakeError> {
        let options = options.for_meta(&META);
        let cfg = config(&options)?;
        Ok(CandlesChart {
            options,
            cfg,
            opts_version: 0,
            data: Prepared::empty(),
            data_version: 0,
            view: View::default(),
            gesture: Gesture::default(),
            levels: RefCell::new(resample::Cache::default()),
            all: RefCell::new(None),
            geom: None,
            ctx: None,
            pack: LocalePack::default(),
            locale: Locale::from_pack(&LocalePack::default()),
            locale_version: 0,
            zone_table: ZoneTable::default(),
            zone: Zone::Utc,
            zone_version: 0,
            base: Grid::new(1, 1),
            base_key: None,
            base_draws: 0,
            canvas: crate::canvas::ColorCanvas::new(crate::canvas::Glyphs::Braille, 0, 0),
        })
    }

    /// How many times the base has been drawn (REQ-PERF-020).
    pub fn base_draws(&self) -> u64 {
        self.base_draws
    }

    /// How many levels of candles have been built: each reads the rows it
    /// covers (REQ-SAMPLE-040).
    pub fn builds(&self) -> u64 {
        self.levels.borrow().builds()
    }

    /// The options as read, defaults filled in (REQ-OPT-010).
    pub fn options(&self) -> &Options {
        &self.options
    }

    /// The cells a candle of the last frame takes and the glyphs drawn at
    /// that spacing (REQ-CANDLE-020), and the interval resampled to, `None`
    /// for the rows themselves (REQ-SAMPLE-040).
    pub fn spacing(&self) -> Option<(f64, Glyphs, Option<String>)> {
        let (_, g) = self.geom.as_ref()?;
        Some((g.spacing, g.glyphs(), g.level.interval.map(|i| i.name())))
    }

    /// Index or time positions.
    fn place(&self) -> Place {
        if self.cfg.index && self.data.kind == Kind::Time {
            Place::Index
        } else {
            Place::Time
        }
    }

    fn rctx(&self) -> resample::Ctx<'_> {
        resample::Ctx { data: &self.data, version: self.data_version, zone: &self.zone, zone_version: self.zone_version, place: self.place() }
    }

    /// The time domain drawn for a requested domain (null for the fitted
    /// one) and `follow`.
    fn resolve_x(&self, requested: Option<[f64; 2]>, follow: bool) -> [f64; 2] {
        let fit = resample::fitted(&self.data).unwrap_or([0.0, 1.0]);
        let mut d = requested.unwrap_or(fit);
        if let (true, Some([a, b])) = (follow, requested) {
            d = [fit[1] - (b - a), fit[1]];
        }
        if d[0] < d[1] && (d[1] - d[0]).is_finite() {
            d
        } else {
            fit
        }
    }

    fn geom_key(&self, view: &View, t: f64, ctx: FrameCtx) -> GeomKey {
        let x = self.resolve_x(view.x_domain_at(t), view.x.follow);
        GeomKey {
            cols: ctx.cols,
            rows: ctx.rows,
            opts: self.opts_version,
            data: self.data_version,
            x: [x[0].to_bits(), x[1].to_bits()],
            y: view.y.domain.map(|d| [d[0].to_bits(), d[1].to_bits()]),
            fit: view.y.fit,
            locale: self.locale_version,
            zone: self.zone_version,
            mono: ctx.mono,
        }
    }

    /// The y extent of every row, kept by data version.
    fn all_extent(&self) -> Option<[f64; 2]> {
        let mut kept = self.all.borrow_mut();
        if let Some((v, e)) = *kept {
            if v == self.data_version {
                return e;
            }
        }
        let r = self.data.rows.clone();
        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        for k in 0..4 {
            for &v in self.data.price(k).slice(r.clone()).iter() {
                if v.is_finite() {
                    lo = lo.min(v);
                    hi = hi.max(v);
                }
            }
        }
        let e = (lo <= hi).then_some([lo, hi]);
        *kept = Some((self.data_version, e));
        e
    }

    /// The layout and candles for `key` (REQ-LAYOUT-010, REQ-SAMPLE-040):
    /// the interval is chosen for the plot's width, the layout made, and
    /// both again while the width the layout gives asks for another
    /// interval, three times at most, the last taking the coarser.
    fn make_geom(&self, key: &GeomKey, view: &View) -> Geom {
        let time = [f64::from_bits(key.x[0]), f64::from_bits(key.x[1])];
        let place = self.place();
        let rc = self.rctx();
        let drawable = self.data.drawable();
        let min_spacing = if key.mono { SPACING.1 } else { SPACING.0 };
        let mut cache = self.levels.borrow_mut();
        let mut w = key.cols;
        let mut last: Option<Option<usize>> = None;
        let mut pass = 0;
        loop {
            let chosen = if drawable { rc.choose(&mut cache, time, w as f64 / min_spacing) } else { None };
            let step = match last {
                Some(prev) if pass == 2 => coarser(prev, chosen),
                _ => chosen,
            };
            let level = if drawable { rc.level(&mut cache, step, time) } else { Arc::new(Level::default()) };
            let geom = self.geom_at(key, view, time, place, level);
            pass += 1;
            let again = drawable && pass < 3 && geom.price.w != w && rc.choose(&mut cache, time, geom.price.w as f64 / min_spacing) != step;
            if !again {
                return geom;
            }
            w = geom.price.w;
            last = Some(step);
        }
    }

    /// The geometry of `level` over `time`.
    fn geom_at(&self, key: &GeomKey, view: &View, time: [f64; 2], place: Place, level: Arc<Level>) -> Geom {
        let (cols, rows) = (key.cols, key.rows);
        let drawable = self.data.drawable() && !level.is_empty();
        let (pa, pb) = if drawable { (level.pos(place, time[0]), level.pos(place, time[1])) } else { (0.0, 1.0) };
        let shown = if drawable { level.in_range(place, pa, pb) } else { 0..0 };
        let shed = cols < usize::from(META.min.cols) || rows < usize::from(META.min.rows);
        let title = (!shed && !self.cfg.title.is_empty()).then(|| self.cfg.title.clone());
        let volume = self.data.has_volume() && self.cfg.volume && rows >= VOLUME_ROWS;
        // The extents of the candles drawn.
        let (mut lo, mut hi, mut vmax) = (f64::INFINITY, f64::NEG_INFINITY, 0.0f64);
        for k in shown.clone() {
            for c in &level.ohlc {
                if c[k].is_finite() {
                    lo = lo.min(c[k]);
                    hi = hi.max(c[k]);
                }
            }
            if level.volume[k].is_finite() {
                vmax = vmax.max(level.volume[k]);
            }
        }
        let visible = (lo <= hi).then_some([lo, hi]);
        let fixed = view.y.domain.filter(|d| d[0] < d[1]);
        let extent = match (fixed, view.y.fit) {
            (Some(d), _) => d,
            (None, Fit::All) => self.all_extent().or(visible).unwrap_or([0.0, 1.0]),
            _ => visible.unwrap_or([0.0, 1.0]),
        };
        let mut input = LayoutInput::new(AxisInput::named(Kind::Linear, "x"), AxisInput::named(Kind::Linear, "y"));
        input.panes[0].weight = WEIGHTS.0;
        if volume {
            input.panes.push(PaneInput { weight: WEIGHTS.1, left: Some(AxisInput::named(Kind::Linear, "volume")), right: None });
        }
        input.title = title.clone();
        input.locale = self.locale.clone();
        let ext = |d: [f64; 2]| Extents { data: Some(d), visible: None, view: Some(d) };
        let (xa, xb) = if pa < pb && pa.is_finite() && pb.is_finite() { (pa, pb) } else { (0.0, 1.0) };
        let extents = LayoutExtents {
            x: ext([xa, xb]),
            panes: vec![PaneExtents { left: ext([0.0, 1.0]), ..PaneExtents::default() }, PaneExtents { left: ext([0.0, 1.0]), ..PaneExtents::default() }],
        };
        let mut lay = layout(cols, rows, &input, &extents).unwrap_or_else(|_| Layout::empty(cols, rows));
        // Each pane's y axis, then the gutter and the x ticks.
        let spec = Specifier::default();
        let price_rows = lay.panes.first().map_or(0, |p| p.plot.h);
        let snap = Snap::resolve(self.cfg.snap, price_rows, true) == Snap::Rows && fixed.is_none();
        let fit_y = |extent: [f64; 2], rows: usize, snap: bool, zero: bool, display: &Display, spec: &Specifier| {
            axes::y_axis(&axes::YInput { extent, rows, snap, zero, display, spec, locale: &self.locale })
        };
        let price = if let Some(d) = fixed {
            let mut y = fit_y(d, price_rows, false, false, &self.data.price, &spec);
            y.domain = d;
            y
        } else {
            fit_y(extent, price_rows, snap, false, &self.data.price, &spec)
        };
        let si = Specifier::parse("volume", "~s").unwrap_or_default();
        let vol_rows = lay.panes.get(1).map_or(0, |p| p.plot.h);
        let vol = volume.then(|| fit_y([0.0, if vmax > 0.0 { vmax } else { 1.0 }], vol_rows, false, true, &self.data.vol, &si));
        let y_step = price.step;
        let stamps: Option<Arc<[f64]>> = (drawable && place == Place::Index).then(|| Arc::from(level.stamp.as_slice()));
        let zone = self.zone.clone();
        let locale = self.locale.clone();
        let kind = self.data.kind;
        axes::finish(&mut lay, &[Some(price), vol], |pw| {
            if !drawable {
                return Vec::new();
            }
            let (scale, format) = match (&stamps, kind) {
                (Some(s), _) => (Scale::index(Arc::clone(s), [xa, xb], [0.0, pw as f64]).with_zone(zone), AxisFormat::Time(None)),
                (None, Kind::Time) => (Scale::time(time, [0.0, pw as f64]).with_zone(zone), AxisFormat::Time(None)),
                _ => (Scale::linear(time, [0.0, pw as f64]), AxisFormat::default()),
            };
            fit_scale_ticks(Side::Bottom, &scale, pw, &format, &locale)
        });
        let pane = lay.panes.first();
        let plot = pane.map_or(Rect::default(), |p| p.plot);
        let vp = lay.panes.get(1).map(|p| p.plot);
        let last = lay.panes.last().map_or(plot, |p| p.plot);
        let all = Rect::new(plot.x, plot.y, plot.w, (last.y + last.h).saturating_sub(plot.y));
        let yd = pane.and_then(|p| p.left_domain).unwrap_or([0.0, 1.0]);
        let vy = lay.panes.get(1).and_then(|p| p.left_domain).map(|d| Axis::y(d[0], d[1], vp.map_or(0, |r| r.h), 4));
        let x = Axis::x(xa, xb, plot.w, 2);
        let spacing = match place {
            _ if plot.w == 0 => 0.0,
            Place::Index => plot.w as f64 / (xb - xa),
            Place::Time => {
                let near = level.middle.get(shown.clone()).map_or(f64::INFINITY, |m| m.windows(2).map(|w| w[1] - w[0]).fold(f64::INFINITY, f64::min));
                if near.is_finite() {
                    near * plot.w as f64 / (xb - xa)
                } else {
                    plot.w as f64
                }
            }
        };
        let before = match shown.start {
            0 if drawable && level.from > self.data.rows.start => {
                resample::candle_at(&self.data, level.interval, level.from - 1, &self.zone).ohlc[3].first().copied().unwrap_or(f64::NAN)
            }
            0 => f64::NAN,
            k => level.ohlc[3][k - 1],
        };
        Geom {
            x,
            time,
            y: Axis::y(yd[0].min(yd[1]), yd[0].max(yd[1]), plot.h, 4),
            vy,
            price: plot,
            volume: vp,
            all,
            level,
            place,
            shown,
            spacing,
            y_step,
            before,
            title: lay.title.and(title),
            layout: lay,
        }
    }

    /// Makes sure the kept geometry is the current view's at play time `t`.
    fn ensure_geom(&mut self, t: f64) {
        let Some(ctx) = self.ctx else {
            self.geom = None;
            return;
        };
        let key = self.geom_key(&self.view, t, ctx);
        if self.geom.as_ref().map(|g| &g.0) != Some(&key) {
            let g = self.make_geom(&key, &self.view);
            self.geom = Some((key, g));
        }
    }

    /// The geometry of the last frame, or of an 80 × 24 frame before any.
    fn with_geom<T>(&self, f: impl FnOnce(&Geom) -> T) -> T {
        let ctx = self.ctx.unwrap_or(FrameCtx { cols: 80, rows: 24, t: 0.0, paper: false, mono: false });
        let key = self.geom_key(&self.view, ctx.t, ctx);
        match &self.geom {
            Some((k, g)) if *k == key => f(g),
            _ => f(&self.make_geom(&key, &self.view)),
        }
    }

    /// The hit of candle `k` of `geom`'s level at price `j` (0 open to 3
    /// close), placed on the plot.
    fn hit_of(&self, geom: &Geom, level: &Level, k: usize, j: usize) -> Hit {
        let row = level.rows[k][2 + j];
        let y = level.ohlc[j][k];
        let same = std::ptr::eq(level, &*geom.level);
        let col = if same { geom.column(k).unwrap_or(geom.price.x) } else { geom.price.x };
        let rrow = geom.y.dot_in(y).map_or(geom.price.y, |d| geom.price.y + d / 4);
        let n = level.rows[k][1] + 1 - level.rows[k][0];
        let aggregate =
            (n > 1).then(|| Aggregate { count: n as u64, first: level.ohlc[0][k], last: level.ohlc[3][k], min: level.ohlc[2][k], max: level.ohlc[1][k] });
        Hit { series: self.data.names[j].clone(), series_index: j, index: row, x: self.data.xs()[row], y, col, row: rrow, aggregate }
    }

    /// The datum the cursor snaps to at `at` (REQ-HIT-010, REQ-CANDLE-050):
    /// the drawn candle nearest the dot column under `at`, then of its open,
    /// high, low and close the nearest `at`'s row, the first of equals.
    fn hit_in(&self, geom: &Geom, at: CellPos) -> Option<Hit> {
        let all = geom.all;
        let plot = Plot { rect: all, x: geom.x, y: geom.y };
        if !plot.contains(at) || geom.shown.is_empty() {
            return None;
        }
        let lv = &*geom.level;
        let target = geom.x.dot_center(((at.x - all.x as f64) * 2.0).floor() as i64);
        let k = match geom.place {
            Place::Index => target.round().max(geom.shown.start as f64).min((geom.shown.end - 1) as f64) as usize,
            Place::Time => crate::hit::nearest(&lv.middle, geom.shown.start, geom.shown.end, target)?,
        };
        // The nearest whole candle, outward.
        let k = (0..geom.shown.len()).flat_map(|d| [k.checked_sub(d), k.checked_add(d)]).flatten().find(|&c| geom.shown.contains(&c) && lv.whole(c))?;
        let py = ((at.y - geom.price.y as f64) * 4.0).max(0.0).min((4 * geom.price.h) as f64 - 0.5);
        let mut best: Option<(f64, usize)> = None;
        for j in 0..4 {
            let d = geom.y.dot(lv.ohlc[j][k]).map_or(f64::INFINITY, |d| (d as f64 + 0.5 - py).abs());
            if best.is_none_or(|(b, _)| d < b) {
                best = Some((d, j));
            }
        }
        let (_, j) = best?;
        Some(self.hit_of(geom, lv, k, j))
    }

    /// The candle holding table row `row` at `geom`'s interval: in its
    /// level, or a level of that one candle.
    fn candle_of(&self, geom: &Geom, row: usize) -> Option<(Arc<Level>, usize)> {
        if !self.data.rows.contains(&row) {
            return None;
        }
        match geom.level.holding(row) {
            Some(k) => Some((Arc::clone(&geom.level), k)),
            None => Some((Arc::new(resample::candle_at(&self.data, geom.level.interval, row, &self.zone)), 0)),
        }
    }

    /// The close before candle `k` of `level`.
    fn prev_close(&self, level: &Level, k: usize) -> f64 {
        if k > 0 {
            level.ohlc[3][k - 1]
        } else if level.from > self.data.rows.start {
            resample::candle_at(&self.data, level.interval, level.from - 1, &self.zone).ohlc[3][0]
        } else {
            f64::NAN
        }
    }

    /// The price index of a cursor's series: open 0 to close 3, close when
    /// it names none or one column gives all four.
    fn which(&self, series: Option<&str>) -> usize {
        if self.data.single {
            return 3;
        }
        series.and_then(|s| self.data.names.iter().position(|n| n == s)).unwrap_or(3)
    }

    /// The price index a cursor names on candle `k`: by its series, or when
    /// one column gives all four, by the row it names.
    fn which_at(&self, series: Option<&str>, row: usize, level: &Level, k: usize) -> usize {
        if self.data.single {
            return (0..4).rev().find(|&j| level.rows[k][2 + j] == row).unwrap_or(3);
        }
        self.which(series)
    }

    /// The readout's parts for the view's cursor, and the candle and value
    /// the crosshair marks.
    fn readout(&self, geom: &Geom) -> Option<Readout> {
        let c = self.view.cursor.as_ref()?;
        let row = usize::try_from(c.index?).ok()?;
        let (level, k) = self.candle_of(geom, row)?;
        let j = self.which_at(c.series.as_deref(), row, &level, k);
        let parts = readout::parts(&readout::Input {
            data: &self.data,
            level: &level,
            k,
            prev: self.prev_close(&level, k),
            step: geom.y_step,
            zone: &self.zone,
            locale: &self.locale,
        });
        let mark = Arc::ptr_eq(&level, &geom.level).then(|| (k, level.ohlc[j][k]));
        Some((parts, mark))
    }

    /// The time domain drawn now, at the end of any motion.
    fn domain_now(&self) -> [f64; 2] {
        self.resolve_x(self.view.x.domain, self.view.x.follow)
    }

    /// The chart's label: what it draws, its ranges and its count
    /// (REQ-A11Y-030).
    fn summary(&self, geom: &Geom) -> String {
        let what = match self.cfg.mode {
            Mode::Candle => "candlestick chart",
            Mode::Hollow => "hollow candlestick chart",
            Mode::Ohlc => "OHLC bar chart",
        };
        let title = if self.cfg.title.is_empty() { String::new() } else { format!("{}: ", self.cfg.title) };
        if !self.data.drawable() || geom.shown.is_empty() {
            return format!("{title}{what}, no data");
        }
        let lv = &*geom.level;
        let (a, b) = (geom.shown.start, geom.shown.end - 1);
        let t = |k: usize| readout::time_text(lv, k, &self.data, &self.zone, &self.locale);
        let f = readout::price_format(readout::price_digits(geom.y_step, &self.data.price), &self.locale);
        let p = |v: f64| f.format(self.data.price.shown(v));
        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        for k in geom.shown.clone() {
            if lv.ohlc[2][k].is_finite() {
                lo = lo.min(lv.ohlc[2][k]);
            }
            if lv.ohlc[1][k].is_finite() {
                hi = hi.max(lv.ohlc[1][k]);
            }
        }
        let every = lv.interval.map_or_else(String::new, |i| format!(" of {}", i.name()));
        let mut text = format!("{title}{what} of {}; {} from {} to {}", self.data.names[3], self.data.x_name, t(a), t(b));
        if lo <= hi {
            text.push_str(&format!("; prices from {} to {}", p(lo), p(hi)));
        }
        text.push_str(&format!("; {} candles{every} in view", geom.shown.len()));
        text
    }

    /// Re-derives the cursor's row from its x after the data changed.
    fn resnap(&mut self) {
        let Some(c) = self.view.cursor.clone() else { return };
        let Some(x) = c.x else { return };
        let r = self.data.rows.clone();
        let row = crate::hit::nearest(self.data.xs(), r.start, r.end, x);
        let j = self.which(c.series.as_deref());
        self.view.cursor = row.filter(|_| self.data.drawable()).map(|row| {
            let y = self.data.price(j)[row];
            Cursor {
                x: Some(self.data.xs()[row]),
                y: y.is_finite().then_some(y),
                series: Some(self.data.names[j].clone()),
                index: Some(row as u64),
                locked: c.locked,
            }
        });
        let rows = self.data.len() as u64;
        if let Some(sel) = &mut self.view.selection {
            sel.points.retain(|p| p.index < rows);
        }
    }

    fn take_data(&mut self, data: Data) -> Result<Accepted, DataError> {
        data.shape(META.shapes, &self.cfg.encode)?;
        let table = data.into_tables().into_iter().next().map(|(_, t)| t);
        let prepared = table.map_or_else(Prepared::empty, |t| Prepared::new(t, &self.cfg.encode, self.cfg.retain).0);
        self.geom = None;
        self.data = prepared;
        self.changed();
        Ok(Accepted { rows: self.data.len(), version: self.data_version, warnings: self.data.warnings.as_slice().to_vec() })
    }

    /// After the data changed: a new version, the levels and extents
    /// dropped, a following view moved to end at the newest candle with its
    /// span kept (REQ-VIEW-060), and the cursor snapped again.
    fn changed(&mut self) {
        self.geom = None;
        self.levels.get_mut().clear();
        *self.all.get_mut() = None;
        self.base_key = None;
        self.data_version += 1;
        if self.view.x.follow {
            let end = self.view.x.domain.map(|d| self.resolve_x(Some(d), true)[1]);
            self.view.follow_to(end);
        }
        self.resnap();
    }

    /// The hit of the candle holding `row` at the current interval, at price
    /// `j`.
    fn hit_at_row(&self, geom: &Geom, row: usize, j: usize) -> Option<Hit> {
        let (level, k) = self.candle_of(geom, row)?;
        level.whole(k).then(|| self.hit_of(geom, &level, k, j))
    }
}

impl Chart for CandlesChart {
    fn set_data(&mut self, data: Data) -> Result<Accepted, DataError> {
        self.take_data(data)
    }

    fn append(&mut self, batch: Data) -> Result<Appended, DataError> {
        if self.data.ring.is_none() {
            let a = self.take_data(batch)?;
            return Ok(Appended { rows: a.rows, dropped: self.data.ring.as_ref().map_or(0, |r| r.first() as usize), late: 0, version: a.version });
        }
        let Some(new) = batch.first_table() else { return Err(DataError::Shape { expected: META.shapes.to_vec() }) };
        let counts = self.data.append(new)?;
        self.changed();
        Ok(Appended { rows: self.data.len(), dropped: counts.dropped, late: counts.late, version: self.data_version })
    }

    fn set_options(&mut self, options: &Options) -> Result<(), MakeError> {
        let layered = options.for_meta(&META);
        let cfg = config(&layered)?;
        let encode_changed = cfg.encode != self.cfg.encode;
        self.options = layered;
        self.cfg = cfg;
        self.opts_version += 1;
        if encode_changed {
            let table = std::mem::replace(&mut self.data, Prepared::empty()).into_table();
            self.data = table.map_or_else(Prepared::empty, |t| Prepared::new(t, &self.cfg.encode, self.cfg.retain).0);
            self.changed();
        } else if self.data.set_retain(self.cfg.retain) > 0 {
            self.changed();
        }
        Ok(())
    }

    fn view(&self) -> &View {
        &self.view
    }

    fn set_view(&mut self, patch: &ViewPatch) {
        self.view.apply(patch);
    }

    fn input(&mut self, event: &Input) -> Changes {
        self.ensure_geom(event.t());
        let mut view = std::mem::take(&mut self.view);
        let mut gesture = std::mem::take(&mut self.gesture);
        let changes = reduce(&mut view, &mut gesture, event, &*self, self.cfg.interaction);
        self.view = view;
        self.gesture = gesture;
        changes
    }

    fn frame(&mut self, t: f64, env: &Env, grid: &mut Grid) {
        if *env.locale != self.pack {
            self.pack = env.locale.clone();
            self.locale = Locale::from_pack(env.locale);
            self.locale_version += 1;
        }
        if *env.zone != self.zone_table {
            self.zone_table = env.zone.clone();
            self.zone = self.zone_table.zone();
            self.zone_version += 1;
            self.levels.get_mut().clear();
        }
        let (cols, rows) = (grid.cols(), grid.rows());
        self.ctx = Some(FrameCtx { cols, rows, t, paper: env.paper, mono: env.mono });
        self.ensure_geom(t);
        let Some((key, geom)) = &self.geom else { return };
        let ground = if env.paper { GROUND } else { NO_BACK };
        let bkey = BaseKey { geom: key.clone(), paper: env.paper };
        if self.base_key.as_ref() != Some(&bkey) {
            if self.base.cols() != cols || self.base.rows() != rows {
                self.base.resize(cols, rows);
            }
            let input = BaseInput { mode: self.cfg.mode, geom, drawable: self.data.drawable(), ground };
            draw::base(&mut self.base, &mut self.canvas, &input);
            self.base_key = Some(bkey);
            self.base_draws += 1;
        }
        grid.copy_from(&self.base);
        let (parts, mark) = self.readout(geom).map_or((Vec::new(), None), |(p, m)| (p, m));
        let lines = readout::lines(&parts, geom.price.w);
        draw::overlay(grid, geom, &self.view, mark, &lines, ground);
    }

    fn hit(&self, at: CellPos) -> Option<Hit> {
        self.ctx?;
        self.with_geom(|g| self.hit_in(g, at))
    }

    fn table(&self, max_rows: usize) -> Table {
        let d = self.domain_now();
        self.with_geom(|g| {
            let lv = &*g.level;
            let mut columns = vec![self.data.x_name.clone()];
            columns.extend(self.data.names.iter().cloned());
            if self.data.has_volume() {
                columns.push(self.data.volume_name.clone());
            }
            let shown = if self.data.drawable() { g.shown.clone() } else { 0..0 };
            let total = shown.len();
            let picked = pick_rows(total, max_rows, |i| lv.ohlc[3][shown.start + i]);
            let f = readout::price_format(readout::price_digits(g.y_step, &self.data.price), &self.locale);
            let num = |v: f64| if v.is_finite() { f.format(self.data.price.shown(v)) } else { String::new() };
            let rows = picked
                .into_iter()
                .map(|i| {
                    let k = shown.start + i;
                    let mut cells = vec![readout::time_text(lv, k, &self.data, &self.zone, &self.locale)];
                    cells.extend((0..4).map(|j| num(lv.ohlc[j][k])));
                    if self.data.has_volume() {
                        let v = lv.volume[k];
                        cells.push(if v.is_finite() { readout::volume_text(v, &self.data.vol, &self.locale) } else { String::new() });
                    }
                    cells
                })
                .collect::<Vec<_>>();
            let name = if self.cfg.title.is_empty() { "candlestick chart" } else { self.cfg.title.as_str() };
            let caption = match lv.interval {
                Some(i) => format!("{name}, candles of {}", i.name()),
                None => name.to_string(),
            };
            let caption = if rows.len() < total { format!("{caption}, each bucket's least and greatest close kept") } else { caption };
            let value = self.readout(g).map(|(p, _)| p.join(", "));
            Table { caption, columns, rows, total: total as u64, summary: self.summary(g), value, domain: Some(d) }
        })
    }
}

impl Scene for CandlesChart {
    fn plot(&self) -> Option<Plot> {
        self.geom.as_ref().map(|(_, g)| Plot { rect: g.all, x: Axis::x(g.time[0], g.time[1], g.all.w, 2), y: g.y })
    }

    fn x_domain(&self, view: &View) -> [f64; 2] {
        self.resolve_x(view.x.domain, view.x.follow)
    }

    fn fitted_x(&self) -> [f64; 2] {
        self.resolve_x(None, false)
    }

    fn snap(&self, _view: &View, at: CellPos) -> Option<Hit> {
        self.geom.as_ref().and_then(|(_, g)| self.hit_in(g, at))
    }

    fn step(&self, _view: &View, from: &Cursor, step: i64) -> Option<Hit> {
        let (_, g) = self.geom.as_ref()?;
        let row = usize::try_from(from.index?).ok()?;
        let j = self.which(from.series.as_deref());
        let (level, k) = self.candle_of(g, row)?;
        if step == 0 {
            return level.whole(k).then(|| self.hit_of(g, &level, k, j));
        }
        let target = k as i64 + step;
        if target >= 0 && (target as usize) < level.len() {
            let t = target as usize;
            return level.whole(t).then(|| self.hit_of(g, &level, t, j));
        }
        // Past the level's rows: the candle next to its first or last.
        let next = if step > 0 { level.rows[level.len() - 1][1] + 1 } else { level.rows[0][0].checked_sub(1)? };
        self.hit_at_row(g, next, j)
    }

    fn near(&self, _view: &View, x: f64, series: Option<&str>) -> Option<Hit> {
        let (_, g) = self.geom.as_ref()?;
        let r = self.data.rows.clone();
        let row = crate::hit::nearest(self.data.xs(), r.start, r.end, x)?;
        self.hit_at_row(g, row, self.which(series))
    }

    fn edge(&self, _view: &View, domain: [f64; 2], last: bool) -> Option<Hit> {
        let (_, g) = self.geom.as_ref()?;
        let xs = self.data.xs();
        let r = self.data.rows.clone();
        let (a, b) = (lower_bound(xs, r.start, r.end, domain[0]), xs.slice(r.clone()).partition_point(|&x| x <= domain[1]) + r.start);
        if a >= b {
            return None;
        }
        self.hit_at_row(g, if last { b - 1 } else { a }, 3)
    }

    fn series(&self) -> Vec<String> {
        Vec::new()
    }

    fn datum(&self, hit: &Hit) -> BTreeMap<String, Value> {
        self.data.datum(hit.index)
    }

    fn count(&self, _view: &View, range: [f64; 2]) -> u64 {
        let xs = self.data.xs();
        let r = self.data.rows.clone();
        let a = lower_bound(xs, r.start, r.end, range[0]);
        let b = r.start + xs.slice(r.clone()).partition_point(|&x| x <= range[1]);
        b.saturating_sub(a) as u64
    }

    fn legend_at(&self, _at: CellPos) -> Option<usize> {
        None
    }
}

#[cfg(test)]
mod tests;
