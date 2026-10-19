//! The `line` family: series over a continuous x, as lines, steps or areas
//! (REQ-LINE-010 to 040). Its one type so far is `line`; the sparkline joins
//! it in v0.5.0.
//!
//! The chart draws from M4 aggregates of the rows in view, found by binary
//! search on x and kept by data version, x range and width (REQ-SAMPLE-010,
//! REQ-SAMPLE-020); its base (data, axes, labels, legend) is drawn again only
//! when the data, the options, the view's domains, hidden series or
//! selection, the size or the env change (REQ-PERF-020), and the overlay
//! (crosshair, readout, brush, time cursor) is drawn over a copy of it every
//! frame (REQ-OVERLAY-010). Its channels are found as `series.rs` says: x by
//! `encode`, role or name, else the first timestamp column, else the first
//! numeric column, so `{"t": […], "close": […]}` draws with no
//! configuration.

mod draw;
mod readout;
mod series;

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::ops::Range;

use crate::cells::Axis;
use crate::chart::{Accepted, Appended, CellPos, Changes, Charset, Chart, Env, Hit, MakeError, Meta, Mono, Part, Size, Table};
use crate::data::{Column, Data, DataError, Encode, Shape, Values};
use crate::downsample::{view_rows, Cache, Edges};
use crate::events::Value;
use crate::format::Locale;
use crate::grid::{Grid, NO_BACK};
use crate::hit::{column_aggregate, nearest};
use crate::input::Input;
use crate::layout::{layout, AxisInput, Layout, LayoutExtents, LayoutInput, PaneExtents, PlacedAxis, Rect};
use crate::locale::LocalePack;
use crate::options::Options;
use crate::palette::GROUND;
use crate::reduce::{reduce, DragMode, Gesture, Interaction, Plot, Scene, WheelMode};
use crate::registry::{Entry, Family};
use crate::scale::{resolve_domain, Domain, Extents, Kind, ScaleOptions, ScaleType};
use crate::table::pick_rows;
use crate::view::{Cursor, Fit, View, ViewPatch};
use crate::Budget;

use draw::{BaseInput, Geom};
use series::Prepared;

/// The `line` type (REQ-LINE-010, REQ-CHART-020).
pub static META: Meta = Meta {
    slug: "line",
    name: "line",
    note: "series over a continuous x, as lines, steps or areas",
    family: "line",
    modes: &["line", "step", "area"],
    shapes: &[Shape::Series],
    options: Some(
        r#"{"mode":"line","step":"after","spanGaps":false,"title":"","encode":{},"legend":{"position":"auto"},"sample":"m4","baseline":0,"drag":"pan","wheel":"scroll","patterns":"auto","motion":false}"#,
    ),
    min: Size { cols: 23, rows: 6 },
    drop: &[Part::Title, Part::Legend, Part::Axes],
    mono: Mono::Authored,
    charset: Charset::Extended,
    budget: Budget::DEFAULT,
};

/// The family's types, by hand (REQ-REG-010, ADR-125).
pub const REGISTRY: &[Entry] = &[Entry { meta: &META, make }];

/// The family's registry entry.
pub const FAMILY: Family = Family { name: "line", budget: Budget::DEFAULT, charts: REGISTRY };

/// The least share of the niced y domain the data must span for the chart
/// to keep it. `nice` widens a domain to a multiple of the tick step, and
/// at a few rows the step is coarse: multi-4's −8.4 to 11.4 is niced to −20
/// to 20 on four rows, leaving half of them empty. Below this share the y
/// domain is the data's, unniced, and the ticks are fitted inside it, so the
/// data fills the plot's height.
const NICE_FILL: f64 = 0.75;

/// A fresh `line` chart for options already layered on [`META`]'s.
pub fn make(options: &Options) -> Result<Box<dyn Chart>, MakeError> {
    Ok(Box::new(LineChart::new(options)?))
}

/// How the series are drawn: the `mode` option (REQ-CHART-030).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    /// Straight segments between data.
    Line,
    /// Steps, by the `step` option.
    Step,
    /// The line filled to the `baseline`.
    Area,
}

/// Where a step rises: the `step` option.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StepAt {
    /// At the earlier datum.
    Before,
    /// At the later datum (the default).
    After,
    /// Halfway.
    Middle,
}

/// The `patterns` option.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Patterns {
    /// Dash patterns in one ink, none in colour.
    Auto,
    /// Always.
    On,
    /// Never.
    Off,
}

/// The options as the chart reads them.
#[derive(Clone, Debug, PartialEq)]
struct Config {
    mode: Mode,
    step: StepAt,
    span_gaps: bool,
    title: String,
    encode: Encode,
    legend: bool,
    sample_none: bool,
    baseline: f64,
    interaction: Interaction,
    patterns: Patterns,
}

/// The options read: every key has a default in [`META`]'s (REQ-OPT-010).
fn config(o: &Options) -> Result<Config, MakeError> {
    let mode = match o.mode(&META)? {
        "step" => Mode::Step,
        "area" => Mode::Area,
        _ => Mode::Line,
    };
    let step = match o.choice("step", &["after", "before", "middle"]) {
        "before" => StepAt::Before,
        "middle" => StepAt::Middle,
        _ => StepAt::After,
    };
    let encode = o.object("encode").and_then(|m| serde_json::from_value::<Encode>(serde_json::Value::Object(m.clone())).ok()).unwrap_or_default();
    let legend = o.object("legend").and_then(|m| m.get("position")).and_then(|p| p.as_str()) != Some("none");
    let baseline = o.f64("baseline");
    Ok(Config {
        mode,
        step,
        span_gaps: o.bool("spanGaps"),
        title: o.str("title").to_string(),
        encode,
        legend,
        sample_none: o.choice("sample", &["m4", "lttb", "none"]) == "none",
        baseline: if baseline.is_finite() { baseline } else { 0.0 },
        interaction: Interaction {
            drag: match o.choice("drag", &["pan", "brush", "zoom"]) {
                "brush" => DragMode::Brush,
                "zoom" => DragMode::Zoom,
                _ => DragMode::Pan,
            },
            wheel: if o.choice("wheel", &["scroll", "zoom"]) == "zoom" { WheelMode::Zoom } else { WheelMode::Scroll },
            motion: o.bool("motion"),
        },
        patterns: match o.choice("patterns", &["auto", "on", "off"]) {
            "on" => Patterns::On,
            "off" => Patterns::Off,
            _ => Patterns::Auto,
        },
    })
}

/// What the layout and the plot depend on (REQ-LAYOUT-010).
#[derive(Clone, Debug, PartialEq)]
struct GeomKey {
    cols: usize,
    rows: usize,
    opts: u64,
    data: u64,
    /// The x domain drawn, as bits.
    x: [u64; 2],
    y: Option<[u64; 2]>,
    fit: Fit,
    hidden: Vec<String>,
    locale: u64,
}

/// What the base depends on besides the geometry.
#[derive(Clone, Debug, PartialEq)]
struct BaseKey {
    geom: GeomKey,
    paper: bool,
    mono: bool,
    raised: Vec<String>,
}

/// The last frame's size, play time and env flags, which input and hit tests
/// use.
#[derive(Clone, Copy, Debug, PartialEq)]
struct FrameCtx {
    cols: usize,
    rows: usize,
    t: f64,
    paper: bool,
    mono: bool,
}

/// A y extent kept by the key it was found for.
type Kept<K> = Option<(K, Option<[f64; 2]>)>;

/// The y extents a chart keeps by what they depend on.
#[derive(Clone, Debug, Default)]
struct Extents2 {
    /// By data version, x range bits and hidden series: the visible extent.
    visible: Kept<(u64, [u64; 2], Vec<String>)>,
    /// By data version and hidden series: the extent of all the data.
    all: Kept<(u64, Vec<String>)>,
}

/// A `line` chart (REQ-LINE-010): the [`Chart`] the registry makes for the
/// type `line`.
pub struct LineChart {
    options: Options,
    cfg: Config,
    opts_version: u64,
    data: Prepared,
    data_version: u64,
    view: View,
    gesture: Gesture,
    m4: Cache,
    extents: RefCell<Extents2>,
    geom: Option<(GeomKey, Geom, Vec<Range<usize>>)>,
    ctx: Option<FrameCtx>,
    pack: LocalePack,
    locale: Locale,
    locale_version: u64,
    base: Grid,
    base_key: Option<BaseKey>,
    base_draws: u64,
    canvas: crate::canvas::ColorCanvas,
    formats: readout::Formats,
}

impl LineChart {
    /// A chart with no data for options layered on [`META`]'s; a mode not
    /// listed is an error naming the valid ones (REQ-CHART-030).
    pub fn new(options: &Options) -> Result<LineChart, MakeError> {
        let options = options.for_meta(&META);
        let cfg = config(&options)?;
        Ok(LineChart {
            options,
            cfg,
            opts_version: 0,
            data: Prepared::empty(),
            data_version: 0,
            view: View::default(),
            gesture: Gesture::default(),
            m4: Cache::new(),
            extents: RefCell::new(Extents2::default()),
            geom: None,
            ctx: None,
            pack: LocalePack::default(),
            locale: Locale::from_pack(&LocalePack::default()),
            locale_version: 0,
            base: Grid::new(1, 1),
            base_key: None,
            base_draws: 0,
            canvas: crate::canvas::ColorCanvas::new(crate::canvas::Glyphs::Braille, 0, 0),
            formats: readout::Formats::default(),
        })
    }

    /// How many times the base has been drawn: once a change of data,
    /// options, view domains, size or env (REQ-PERF-020, REQ-OVERLAY-010).
    pub fn base_draws(&self) -> u64 {
        self.base_draws
    }

    /// How many times the M4 has read the data (REQ-SAMPLE-020).
    pub fn scans(&self) -> u64 {
        self.m4.scans()
    }

    /// The options as read, defaults filled in, recording each key read
    /// (REQ-OPT-010).
    pub fn options(&self) -> &Options {
        &self.options
    }

    fn shown(&self, view: &View, s: usize) -> bool {
        !view.is_hidden(&self.data.series[s].name)
    }

    /// The x domain drawn for a requested domain (null for the fitted one)
    /// and `follow`, as the layout resolves it.
    fn resolve_x(&self, requested: Option<[f64; 2]>, follow: bool) -> [f64; 2] {
        let data = self.data.x_extent();
        let mut view = requested;
        if let (true, Some([a, b]), Some([_, last])) = (follow, requested, data) {
            view = Some([last - (b - a), last]);
        }
        let opts = scale_options(self.data.kind, Domain::Data, None);
        resolve_domain(&opts, self.data.kind, &Extents { data, visible: None, view }, 1.0).unwrap_or([0.0, 1.0])
    }

    fn geom_key(&self, view: &View, t: f64, cols: usize, rows: usize) -> GeomKey {
        let x = self.resolve_x(view.x_domain_at(t), view.x.follow);
        GeomKey {
            cols,
            rows,
            opts: self.opts_version,
            data: self.data_version,
            x: [x[0].to_bits(), x[1].to_bits()],
            y: view.y.domain.map(|d| [d[0].to_bits(), d[1].to_bits()]),
            fit: view.y.fit,
            hidden: view.hidden.clone(),
            locale: self.locale_version,
        }
    }

    /// The y extent of the shown data in `x` (or all of it), kept by what it
    /// depends on; an area takes in its baseline.
    fn y_extent(&self, view: &View, x: Option<[f64; 2]>) -> Option<[f64; 2]> {
        let hidden = view.hidden.clone();
        let mut cache = self.extents.borrow_mut();
        let ext = match x {
            Some(r) => {
                let key = (self.data_version, [r[0].to_bits(), r[1].to_bits()], hidden);
                match &cache.visible {
                    Some((k, v)) if *k == key => *v,
                    _ => {
                        let v = self.data.y_extent(|s| self.shown(view, s), Some(r));
                        cache.visible = Some((key, v));
                        v
                    }
                }
            }
            None => {
                let key = (self.data_version, hidden);
                match &cache.all {
                    Some((k, v)) if *k == key => *v,
                    _ => {
                        let v = self.data.y_extent(|s| self.shown(view, s), None);
                        cache.all = Some((key, v));
                        v
                    }
                }
            }
        };
        match (ext, self.cfg.mode) {
            (Some([a, b]), Mode::Area) => Some([a.min(self.cfg.baseline), b.max(self.cfg.baseline)]),
            (e, _) => e,
        }
    }

    /// The layout and plot for `key`, and the rows in view a series.
    fn make_geom(&self, key: &GeomKey, view: &View) -> (Geom, Vec<Range<usize>>) {
        let xd = [f64::from_bits(key.x[0]), f64::from_bits(key.x[1])];
        let rows: Vec<Range<usize>> = (0..self.data.series.len()).map(|s| view_rows(self.data.xs(s), xd[0], xd[1])).collect();
        let shed = key.cols < usize::from(META.min.cols) || key.rows < usize::from(META.min.rows);
        let title = (!shed && !self.cfg.title.is_empty()).then(|| self.cfg.title.clone());
        let legend: Vec<String> =
            if !shed && self.cfg.legend && self.data.series.len() >= 2 { self.data.series.iter().map(|s| s.name.clone()).collect() } else { Vec::new() };
        let fit = view.y.fit;
        let visible = if fit == Fit::Visible { self.y_extent(view, Some(xd)) } else { None };
        let all = if fit != Fit::Visible || visible.is_none() { self.y_extent(view, None) } else { None };
        let y_domain = if fit == Fit::Visible { Domain::View } else { Domain::Data };
        let mut x_axis = AxisInput::new(self.data.kind);
        x_axis.scale = scale_options(self.data.kind, Domain::Data, None);
        let mut y_axis = AxisInput::new(Kind::Linear);
        y_axis.scale = scale_options(Kind::Linear, y_domain.clone(), Some(true));
        let mut input = LayoutInput::new(x_axis, y_axis);
        input.title = title.clone();
        input.legend = legend.clone();
        input.locale = self.locale.clone();
        let x_ext = Extents { data: Some(xd), visible: None, view: Some(xd) };
        let y_ext = Extents { data: all, visible, view: view.y.domain };
        let run = |input: &LayoutInput| {
            layout(key.cols, key.rows, input, &LayoutExtents { x: x_ext, panes: vec![PaneExtents { left: y_ext, ..PaneExtents::default() }] })
                .or_else(|_| layout(key.cols, key.rows, input, &LayoutExtents { x: x_ext, panes: vec![PaneExtents::default()] }))
                .unwrap_or_else(|_| empty_layout(key.cols, key.rows))
        };
        let mut lay = run(&input);
        // A nice domain the data fills too little of gives way to the data's
        // (NICE_FILL); a y domain in the view is drawn as it is.
        if view.y.domain.is_none() {
            let plain = resolve_domain(&scale_options(Kind::Linear, y_domain, None), Kind::Linear, &y_ext, 1.0);
            if let (Some(p), Ok([a, b])) = (lay.panes.first(), plain) {
                let [c, d] = p.left_domain;
                if (b - a).abs() < NICE_FILL * (d - c).abs() {
                    input.panes[0].left.scale.nice = None;
                    lay = run(&input);
                }
            }
        }
        let pane = lay.panes.first();
        let plot = pane.map_or(Rect::default(), |p| p.plot);
        let y = pane.map_or([0.0, 1.0], |p| p.left_domain);
        let x_step = tick_step_of(lay.x_axis.as_ref(), lay.x_domain, plot.w / crate::layout::X_CELLS_PER_TICK);
        let y_step = tick_step_of(pane.and_then(|p| p.left.as_ref()), y, plot.h / crate::layout::Y_ROWS_PER_TICK);
        let mut spans = Vec::new();
        if let Some(r) = lay.legend {
            let mut x = r.x;
            for (s, name) in legend.iter().enumerate() {
                let w = 2 + crate::text::width(name);
                spans.push((s, Rect::new(x, r.y, w, 1)));
                x += w + 2;
            }
        }
        let geom = Geom {
            plot,
            x: Axis::x(lay.x_domain[0], lay.x_domain[1], plot.w, 2),
            y: Axis::y(y[0].min(y[1]), y[0].max(y[1]), plot.h, 4),
            x_step,
            y_step,
            legend: spans,
            title: lay.title.and(title),
            layout: lay,
        };
        (geom, rows)
    }

    /// Makes sure the kept geometry is the current view's at play time `t`,
    /// at the last frame's size.
    fn ensure_geom(&mut self, t: f64) {
        let Some(ctx) = self.ctx else {
            self.geom = None;
            return;
        };
        let key = self.geom_key(&self.view, t, ctx.cols, ctx.rows);
        if self.geom.as_ref().map(|g| &g.0) != Some(&key) {
            let (g, rows) = self.make_geom(&key, &self.view);
            self.geom = Some((key, g, rows));
        }
    }

    /// The datum of series `s` at position `i`, placed on the plot.
    fn hit_of(&self, geom: Option<&Geom>, s: usize, i: usize, at: Option<CellPos>, aggregate: Option<crate::events::Aggregate>) -> Hit {
        let (xs, ys) = (self.data.xs(s), self.data.ys(s));
        let (x, y) = (xs[i], ys[i]);
        let (mut col, mut row) = at.map_or((0, 0), |p| (p.x.max(0.0) as usize, p.y.max(0.0) as usize));
        if let Some(g) = geom {
            let edges = Edges::new(&g.x);
            if let Some(c) = edges.column(x) {
                col = g.plot.x + c / 2;
            }
            if let Some(d) = g.y.dot_in(y) {
                row = g.plot.y + d / 4;
            } else if at.is_none() {
                row = g.plot.y;
            }
        }
        Hit { series: self.data.series[s].name.clone(), series_index: s, index: self.data.row(s, i), x, y, col, row, aggregate }
    }

    /// The datum the cursor snaps to at `at` (REQ-HIT-010, REQ-HIT-020): of
    /// the shown series, the datum whose x is nearest the centre of the dot
    /// column under `at`, by binary search; of equally near, the one whose y
    /// is nearest `at`'s, then the earlier series. The dot column's
    /// aggregate goes with it when the column holds more than one datum.
    fn hit_in(&self, geom: &Geom, view: &View, at: CellPos) -> Option<Hit> {
        let plot = geom.plot;
        let p = Plot { rect: plot, x: geom.x, y: geom.y };
        if !p.contains(at) {
            return None;
        }
        let k = ((at.x - plot.x as f64) * 2.0).floor() as i64;
        let target = geom.x.dot_center(k);
        let py = (at.y - plot.y as f64) * 4.0;
        let mut best: Option<(f64, f64, usize, usize)> = None;
        for s in (0..self.data.series.len()).filter(|&s| self.shown(view, s)) {
            let (xs, ys) = (self.data.xs(s), self.data.ys(s));
            let Some(i) = nearest(xs, 0, xs.len(), target) else { continue };
            let dx = (xs[i] - target).abs();
            let dy = geom.y.dot(ys[i]).map_or(f64::INFINITY, |d| (d as f64 + 0.5 - py).abs());
            if best.is_none_or(|(bx, by, _, _)| dx < bx || (dx == bx && dy < by)) {
                best = Some((dx, dy, s, i));
            }
        }
        let (_, _, s, i) = best?;
        let (lo, hi) = (geom.x.dot_value(k), if k + 1 >= geom.x.dots() as i64 { geom.x.hi().next_up() } else { geom.x.dot_value(k + 1) });
        let (xs, ys) = (self.data.xs(s), self.data.ys(s));
        let aggregate = column_aggregate(xs, ys, 0, xs.len(), lo, hi);
        Some(self.hit_of(Some(geom), s, i, Some(at), aggregate))
    }

    /// The x domain drawn now, at the end of any motion.
    fn domain_now(&self) -> [f64; 2] {
        self.resolve_x(self.view.x.domain, self.view.x.follow)
    }

    /// The cursor's value text: its x, then each shown series' value at the
    /// datum nearest it, up to four, then `and N more` (REQ-A11Y-030).
    fn value_text(&self) -> Option<String> {
        let c = self.view.cursor.as_ref()?;
        let x = c.x?;
        let span = {
            let d = self.domain_now();
            d[1] - d[0]
        };
        let mut parts = vec![draw::x_label(x, self.data.kind, span, &self.locale)];
        let shown: Vec<usize> = (0..self.data.series.len()).filter(|&s| self.shown(&self.view, s)).collect();
        for &s in shown.iter().take(4) {
            let xs = self.data.xs(s);
            let v = nearest(xs, 0, xs.len(), x).map(|i| self.data.ys(s)[i]).filter(|v| v.is_finite());
            let v = v.map_or_else(|| "no value".to_string(), |v| draw::number(v, &self.locale));
            parts.push(format!("{} {}", self.data.series[s].name, v));
        }
        let mut text = parts.join(", ");
        if shown.len() > 4 {
            text.push_str(&format!(", and {} more", shown.len() - 4));
        }
        Some(text)
    }

    /// The chart's label: what it draws, its series, its ranges and its
    /// count (REQ-A11Y-030).
    fn summary(&self) -> String {
        let names: Vec<&str> = self.data.series.iter().map(|s| s.name.as_str()).collect();
        let what = match self.cfg.mode {
            Mode::Line => "line chart",
            Mode::Step => "step chart",
            Mode::Area => "area chart",
        };
        let title = if self.cfg.title.is_empty() { String::new() } else { format!("{}: ", self.cfg.title) };
        if names.is_empty() || self.data.rows() == 0 {
            return format!("{title}{what}, no data");
        }
        let d = self.domain_now();
        let span = d[1] - d[0];
        let mut text = format!(
            "{title}{what} of {} series ({}); {} from {} to {}",
            names.len(),
            names.join(", "),
            self.data.x_name,
            draw::x_label(d[0], self.data.kind, span, &self.locale),
            draw::x_label(d[1], self.data.kind, span, &self.locale)
        );
        if let Some([a, b]) = self.y_extent(&self.view, Some(d)) {
            text.push_str(&format!("; values from {} to {}", draw::number(a, &self.locale), draw::number(b, &self.locale)));
        }
        let count: usize = (0..names.len()).filter(|&s| self.shown(&self.view, s)).map(|s| view_rows(self.data.xs(s), d[0], d[1]).len()).sum();
        text.push_str(&format!("; {} points in view", draw::count(count, &self.locale)));
        text
    }

    /// Re-derives the cursor's row from its x after the data changed.
    fn resnap(&mut self) {
        let Some(c) = self.view.cursor.clone() else { return };
        let (Some(x), Some(name)) = (c.x, c.series.clone()) else { return };
        let hit = self.data.find(&name).and_then(|s| {
            let xs = self.data.xs(s);
            nearest(xs, 0, xs.len(), x).map(|i| self.hit_of(None, s, i, None, None))
        });
        self.view.cursor =
            hit.map(|h| Cursor { x: Some(h.x), y: h.y.is_finite().then_some(h.y), series: Some(h.series), index: Some(h.index as u64), locked: c.locked });
        let rows = self.data.rows() as u64;
        if let Some(sel) = &mut self.view.selection {
            sel.points.retain(|p| p.index < rows);
        }
    }

    fn take_data(&mut self, data: Data) -> Result<Accepted, DataError> {
        data.shape(META.shapes, &self.cfg.encode)?;
        let table = data.into_tables().into_iter().next().map(|(_, t)| t);
        self.data = table.map_or_else(Prepared::empty, |t| Prepared::new(t, &self.cfg.encode));
        self.data_version += 1;
        self.resnap();
        Ok(Accepted { rows: self.data.rows(), version: self.data_version, warnings: self.data.warnings.as_slice().to_vec() })
    }
}

/// A scale's options of `kind` over `domain`, niced when asked.
fn scale_options(kind: Kind, domain: Domain, nice: Option<bool>) -> ScaleOptions {
    let kind = if kind == Kind::Time { ScaleType::Time } else { ScaleType::Linear };
    ScaleOptions { kind, domain, nice, zero: None, padding: None, base: None, constant: None, zone: None, scheme: None }
}

/// The step between an axis's drawn ticks; with fewer than two, d3's tick
/// step over `domain` at `count` (at least 1).
fn tick_step_of(axis: Option<&PlacedAxis>, domain: [f64; 2], count: usize) -> f64 {
    match axis.map(|a| a.ticks.as_slice()) {
        Some([a, b, ..]) => b.value - a.value,
        _ => crate::ticks::tick_step(domain[0], domain[1], count.max(1) as f64).abs(),
    }
}

/// A layout of no parts: the whole grid a plot over [0, 1] both ways.
fn empty_layout(cols: usize, rows: usize) -> Layout {
    Layout {
        cols,
        rows,
        decorated: false,
        title: None,
        legend: None,
        x_domain: [0.0, 1.0],
        x_axis: None,
        panes: vec![crate::layout::PaneLayout { plot: Rect::new(0, 0, cols, rows), left_domain: [0.0, 1.0], right_domain: None, left: None, right: None }],
    }
}

/// The rows of `batch` appended to `table`'s: the same columns, numbers held
/// without an origin (no `us` or `ns` timestamps); a row whose x is below the
/// last row's is late and dropped, one whose x equals it replaces it
/// (REQ-STREAM-040, the v0.6.0 ring aside).
fn appended(table: &crate::data::Table, batch: &crate::data::Table, x: Option<usize>) -> Result<(crate::data::Table, usize), DataError> {
    let cols = table.columns();
    if batch.columns().len() != cols.len() {
        return Err(DataError::Append(format!("rows have {} columns; the data has {}", batch.columns().len(), cols.len())));
    }
    let mut olds = Vec::with_capacity(cols.len());
    let mut news = Vec::with_capacity(cols.len());
    for c in cols {
        let new = batch.by_name(&c.name).filter(|b| b.kind == c.kind && b.origin == 0 && c.origin == 0);
        let (Some(old), Some(new)) = (c.nums(), new.and_then(Column::nums)) else {
            return Err(DataError::Append(format!(
                "column `{}` is missing, of another type or not held as numbers; until v0.6.0 append takes such columns only",
                c.name
            )));
        };
        olds.push(old);
        news.push(new);
    }
    let mut values: Vec<Vec<f64>> = olds
        .iter()
        .zip(&news)
        .map(|(o, n)| {
            let mut v = Vec::with_capacity(o.len() + n.len());
            v.extend_from_slice(o);
            v
        })
        .collect();
    let mut late = 0;
    let mut last = x.and_then(|xi| values[xi].iter().rev().find(|v| !v.is_nan()).copied());
    for r in 0..batch.len() {
        if let Some(xi) = x {
            let xv = news[xi][r];
            match last {
                Some(l) if xv < l => {
                    late += 1;
                    continue;
                }
                Some(l) if xv == l && values[xi].last() == Some(&l) => {
                    for (v, n) in values.iter_mut().zip(&news) {
                        if let Some(end) = v.last_mut() {
                            *end = n[r];
                        }
                    }
                    continue;
                }
                _ => {}
            }
            if !xv.is_nan() {
                last = Some(xv);
            }
        }
        for (v, n) in values.iter_mut().zip(&news) {
            v.push(n[r]);
        }
    }
    let columns = cols
        .iter()
        .zip(values)
        .map(|(c, v)| Column {
            name: c.name.clone(),
            kind: c.kind,
            role: c.role,
            unit: c.unit.clone(),
            scale: c.scale,
            offset: c.offset,
            time_unit: c.time_unit,
            origin: 0,
            zone: c.zone.clone(),
            categories: Vec::new(),
            values: Values::Num(v),
        })
        .collect();
    let mut t = crate::data::Table::from_columns(columns)?;
    t.sorted = table.sorted.clone();
    Ok((t, late))
}

impl Chart for LineChart {
    fn set_data(&mut self, data: Data) -> Result<Accepted, DataError> {
        self.take_data(data)
    }

    fn append(&mut self, batch: Data) -> Result<Appended, DataError> {
        let Some(old) = self.data.table.as_ref() else {
            let a = self.take_data(batch)?;
            return Ok(Appended { rows: a.rows, dropped: 0, late: 0, version: a.version });
        };
        let Some(new) = batch.first_table() else { return Err(DataError::Shape { expected: META.shapes.to_vec() }) };
        let x = self.data.table.as_ref().and_then(|t| t.find(&self.data.x_name));
        let (table, late) = appended(old, new, x)?;
        let a = self.take_data(Data::from_table(table))?;
        Ok(Appended { rows: a.rows, dropped: 0, late, version: a.version })
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
            self.data = table.map_or_else(Prepared::empty, |t| Prepared::new(t, &self.cfg.encode));
            self.data_version += 1;
            self.resnap();
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
            self.formats.clear();
        }
        let (cols, rows) = (grid.cols(), grid.rows());
        self.ctx = Some(FrameCtx { cols, rows, t, paper: env.paper, mono: env.mono });
        self.ensure_geom(t);
        let Some((key, geom, in_view)) = &self.geom else { return };
        let ground = if env.paper { GROUND } else { NO_BACK };
        let raised = self.view.selection.as_ref().map(|s| s.points.iter().map(|p| p.series.clone()).collect()).unwrap_or_default();
        let bkey = BaseKey { geom: key.clone(), paper: env.paper, mono: env.mono, raised };
        if self.base_key.as_ref() != Some(&bkey) {
            if self.base.cols() != cols || self.base.rows() != rows {
                self.base.resize(cols, rows);
            }
            let input = BaseInput { cfg: &self.cfg, geom, view: &self.view, version: self.data_version, rows: in_view, mono: env.mono, ground };
            draw::base(&mut self.base, &mut self.canvas, &mut self.m4, &self.data, &input);
            self.base_key = Some(bkey);
            self.base_draws += 1;
        }
        grid.copy_from(&self.base);
        let text = readout::text(&self.data, &self.view, geom, &self.locale, &mut self.formats);
        draw::overlay(grid, geom, &self.view, text.as_deref(), ground);
    }

    fn hit(&self, at: CellPos) -> Option<Hit> {
        let ctx = self.ctx?;
        let key = self.geom_key(&self.view, ctx.t, ctx.cols, ctx.rows);
        match &self.geom {
            Some((k, g, _)) if *k == key => self.hit_in(g, &self.view, at),
            _ => {
                let (g, _) = self.make_geom(&key, &self.view);
                self.hit_in(&g, &self.view, at)
            }
        }
    }

    fn table(&self, max_rows: usize) -> Table {
        let d = self.domain_now();
        let span = d[1] - d[0];
        let mut columns = vec![self.data.x_name.clone()];
        columns.extend(self.data.series.iter().map(|s| s.name.clone()));
        let xs = self.data.table_xs();
        let rows = view_rows(xs, d[0], d[1]);
        let total = rows.len();
        // Each bucket keeps its extremes of the first shown series.
        let first = (0..self.data.series.len()).find(|&s| self.shown(&self.view, s));
        let value = |s: usize, row: usize| -> f64 { self.data.pos(s, row).map_or(f64::NAN, |i| self.data.ys(s)[i]) };
        let picked = pick_rows(total, max_rows, |i| first.map_or(f64::NAN, |s| value(s, rows.start + i)));
        let mut out = Vec::with_capacity(picked.len());
        for i in picked {
            let row = rows.start + i;
            let mut cells = vec![draw::x_label(xs[row], self.data.kind, span, &self.locale)];
            for s in 0..self.data.series.len() {
                let v = value(s, row);
                cells.push(if v.is_finite() { draw::number(v, &self.locale) } else { String::new() });
            }
            out.push(cells);
        }
        // The caption holds no count: the shell adds the rows of the total
        // when the view holds more (REQ-A11Y-020).
        let name = if self.cfg.title.is_empty() { "line chart" } else { self.cfg.title.as_str() };
        let caption = if out.len() < total { format!("{name}, each bucket's least and greatest kept") } else { name.to_string() };
        Table { caption, columns, rows: out, total: total as u64, summary: self.summary(), value: self.value_text() }
    }
}

impl Scene for LineChart {
    fn plot(&self) -> Option<Plot> {
        self.geom.as_ref().map(|(_, g, _)| Plot { rect: g.plot, x: g.x, y: g.y })
    }

    fn x_domain(&self, view: &View) -> [f64; 2] {
        self.resolve_x(view.x.domain, view.x.follow)
    }

    fn fitted_x(&self) -> [f64; 2] {
        self.resolve_x(None, false)
    }

    fn snap(&self, view: &View, at: CellPos) -> Option<Hit> {
        self.geom.as_ref().and_then(|(_, g, _)| self.hit_in(g, view, at))
    }

    fn step(&self, view: &View, from: &Cursor, step: i64) -> Option<Hit> {
        let s = self.data.find(from.series.as_deref()?)?;
        if !self.shown(view, s) {
            return None;
        }
        let pos = self.data.pos(s, usize::try_from(from.index?).ok()?)?;
        let j = usize::try_from(pos as i64 + step).ok()?;
        let xs = self.data.xs(s);
        (j < xs.len() && xs[j].is_finite()).then(|| self.hit_of(self.geom.as_ref().map(|g| &g.1), s, j, None, None))
    }

    fn near(&self, view: &View, x: f64, series: Option<&str>) -> Option<Hit> {
        let one = series.and_then(|n| self.data.find(n)).filter(|&s| self.shown(view, s));
        let candidates: Vec<usize> = match one {
            Some(s) => vec![s],
            None => (0..self.data.series.len()).filter(|&s| self.shown(view, s)).collect(),
        };
        let mut best: Option<(f64, usize, usize)> = None;
        for s in candidates {
            let xs = self.data.xs(s);
            if let Some(i) = nearest(xs, 0, xs.len(), x) {
                let d = (xs[i] - x).abs();
                if best.is_none_or(|(b, _, _)| d < b) {
                    best = Some((d, s, i));
                }
            }
        }
        best.map(|(_, s, i)| self.hit_of(self.geom.as_ref().map(|g| &g.1), s, i, None, None))
    }

    fn edge(&self, view: &View, domain: [f64; 2], last: bool) -> Option<Hit> {
        let mut best: Option<(f64, usize, usize)> = None;
        for s in (0..self.data.series.len()).filter(|&s| self.shown(view, s)) {
            let rows = view_rows(self.data.xs(s), domain[0], domain[1]);
            if rows.is_empty() {
                continue;
            }
            let i = if last { rows.end - 1 } else { rows.start };
            let x = self.data.xs(s)[i];
            if best.is_none_or(|(b, _, _)| if last { x > b } else { x < b }) {
                best = Some((x, s, i));
            }
        }
        best.map(|(_, s, i)| self.hit_of(self.geom.as_ref().map(|g| &g.1), s, i, None, None))
    }

    fn series(&self) -> Vec<String> {
        self.data.series.iter().map(|s| s.name.clone()).collect()
    }

    fn datum(&self, hit: &Hit) -> BTreeMap<String, Value> {
        self.data.datum(hit.index)
    }

    fn count(&self, view: &View, range: [f64; 2]) -> u64 {
        (0..self.data.series.len()).filter(|&s| self.shown(view, s)).map(|s| view_rows(self.data.xs(s), range[0], range[1]).len() as u64).sum()
    }

    fn legend_at(&self, at: CellPos) -> Option<usize> {
        let (_, g, _) = self.geom.as_ref()?;
        g.legend.iter().find(|(_, r)| at.x >= r.x as f64 && at.x < (r.x + r.w) as f64 && at.y >= r.y as f64 && at.y < (r.y + 1) as f64).map(|(s, _)| *s)
    }
}

#[cfg(test)]
mod tests;
