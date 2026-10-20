//! The `sparkline` type of the `line` family (REQ-SPARK-010): series drawn
//! small, with no axes, one series a band of rows, as lines, areas or
//! columns; each band may end in the series' label, its latest value and its
//! least and greatest.
//!
//! The chart holds its rows in a ring of `capacity` rows: a replace keeps the
//! last `capacity` rows of the data sorted by x, and an append adds its rows
//! (a late row dropped, an equal x replacing the last row, as the line chart
//! appends) and then drops the oldest past the capacity. Rows with a missing
//! x are never drawn and are not kept. A datum's row (`Hit::index`, the
//! cursor's `index`, the events' `index`) counts every row given since the
//! data was last replaced, kept or dropped, so a row keeps its number while
//! it is held. This ring is the sparkline's own until the streaming ring
//! (`retain.rows`, REQ-STREAM-020) arrives in v0.6.0; its default capacity is
//! that ring's, 1,000,000 rows.
//!
//! Layout, from the grid's size and the data alone: the rows are split among
//! the series in order, the first `rows % n` taking one more, so a grid of
//! fewer rows than series draws the first series a row each. The text column
//! on the right holds, on each band's middle row, the series' name in its ink
//! and then, in the ink, its latest value and `↓least ↑greatest` over the
//! rows held, as the `label`, `latest` and `extent` options ask; parts are
//! dropped (the extent first, then the latest value, then the label) until
//! the plot keeps half the columns and at least [`MIN_PLOT`]. The plot is
//! every other column over every row: its x axis shared, its y fitted in each
//! band to that series' data in view (`fit`) and padded by [`Y_PAD`], or the
//! view's y domain as it is.
//!
//! Marks (REQ-SAMPLE-010): a line is the line chart's path through the M4
//! aggregates of the rows in view (or every row with `sample: "none"`); an
//! area fills that line down to the band's foot in braille; a column draws a
//! bar a cell from the band's base (0, held inside the y domain, snapped to a
//! cell edge) to the least and greatest y in the cell, its free ends in
//! eighths above the base (`▁`–`█`) and in `▔`, `▀` or `█` below it, and
//! widens over the empty cells between it and its neighbours. A gap (a
//! missing y) breaks a line or an area and stops a column widening into it
//! (REQ-DATA-030). A lone datum in view sits on a muted dashed track across
//! its band at its level, and with no data the plot's middle row is that
//! track with `no data` over it. The overlay is the line chart's: the brush
//! across the plot, the time cursor, the crosshair in the cursor's band and
//! the readout at the top right of the whole grid, which a narrow plot could
//! not hold (REQ-OVERLAY-010, REQ-OVERLAY-020).

use std::collections::BTreeMap;
use std::ops::Range;

use super::draw::{self, path, walk, x_dot, Pt};
use super::readout;
use super::series::Prepared;
use super::{appended, Mode, StepAt};
use crate::canvas::{ColorCanvas, Glyphs};
use crate::cells::Axis;
use crate::chart::{Accepted, Appended, CellPos, Changes, Charset, Chart, Env, Hit, MakeError, Meta, Mono, Size, Table};
use crate::data::{Column, Data, DataError, Encode, Shape, Values};
use crate::downsample::{m4_into, view_rows, Cache, CacheKey, Edges, M4};
use crate::events::Value;
use crate::format::{Display, Kind as FormatKind, Locale, NumberFormat, Specifier};
use crate::grid::{Grid, NO_BACK};
use crate::hit::{column_aggregate, lower_bound, nearest};
use crate::input::Input;
use crate::layout::Rect;
use crate::locale::{LocalePack, ZoneTable};
use crate::options::Options;
use crate::palette::{GROUND, INK, MUTED};
use crate::reduce::{reduce, DragMode, Gesture, Interaction, Plot, Scene, WheelMode};
use crate::scale::{resolve_domain, Extents, Kind, ScaleOptions};
use crate::table::pick_rows;
use crate::text::{self, stage, Frame};
use crate::ticks::tick_step;
use crate::time::Zone;
use crate::view::{Cursor, Fit, View, ViewPatch};
use crate::Budget;

/// The `sparkline` type (REQ-SPARK-010, REQ-CHART-020), as
/// `spec/schemas/examples/meta.sparkline.json` gives it, with the defaults
/// of the options it reads.
pub static META: Meta = Meta {
    slug: "sparkline",
    name: "sparkline",
    note: "a small line, area or column series with no axes",
    family: "line",
    modes: &["line", "area", "column"],
    shapes: &[Shape::Series],
    options: Some(
        r#"{"mode":"line","capacity":1000000,"encode":{},"label":true,"latest":true,"extent":true,"sample":"m4","drag":"pan","wheel":"scroll","motion":false}"#,
    ),
    min: Size { cols: 8, rows: 1 },
    drop: &[],
    mono: Mono::Auto,
    charset: Charset::Extended,
    budget: Budget::DEFAULT,
};

/// The ring's capacity when the `capacity` option is not a whole number of
/// at least one: the streaming ring's default (REQ-STREAM-020).
pub const CAPACITY: usize = 1_000_000;

/// The most rows the `capacity` option may ask for.
pub const MAX_CAPACITY: usize = 100_000_000;

/// The fewest columns the text beside the plot leaves it: the type's
/// minimum width.
pub const MIN_PLOT: usize = 8;

/// The width of a column's top end, in eighths of a cell, by glyph: `▁` to
/// `█`.
const LOWER: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// A fresh `sparkline` chart for options already layered on [`META`]'s.
pub fn make(options: &Options) -> Result<Box<dyn Chart>, MakeError> {
    Ok(Box::new(SparkChart::new(options)?))
}

/// How the series are drawn: the `mode` option (REQ-CHART-030).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SparkMode {
    /// The line chart's line.
    Line,
    /// The line, filled to the band's foot.
    Area,
    /// A bar a cell.
    Column,
}

/// The options as the chart reads them.
#[derive(Clone, Debug, PartialEq)]
struct Config {
    mode: SparkMode,
    capacity: usize,
    encode: Encode,
    label: bool,
    latest: bool,
    extent: bool,
    sample_none: bool,
    interaction: Interaction,
}

/// The options read: every key has a default in [`META`]'s (REQ-OPT-010).
fn config(o: &Options) -> Result<Config, MakeError> {
    let mode = match o.mode(&META)? {
        "area" => SparkMode::Area,
        "column" => SparkMode::Column,
        _ => SparkMode::Line,
    };
    let c = o.f64("capacity");
    let capacity = if c.is_finite() && c >= 1.0 && c.fract() == 0.0 { c.min(MAX_CAPACITY as f64) as usize } else { CAPACITY };
    let encode = o.object("encode").and_then(|m| serde_json::from_value::<Encode>(serde_json::Value::Object(m.clone())).ok()).unwrap_or_default();
    Ok(Config {
        mode,
        capacity,
        encode,
        label: o.bool("label"),
        latest: o.bool("latest"),
        extent: o.bool("extent"),
        sample_none: o.choice("sample", &["m4", "lttb", "none"]) == "none",
        interaction: Interaction {
            drag: match o.choice("drag", &["pan", "brush", "zoom"]) {
                "brush" => DragMode::Brush,
                "zoom" => DragMode::Zoom,
                _ => DragMode::Pan,
            },
            wheel: if o.choice("wheel", &["scroll", "zoom"]) == "zoom" { WheelMode::Zoom } else { WheelMode::Scroll },
            motion: o.bool("motion"),
        },
    })
}

/// `table`'s rows as the ring keeps them: sorted by x, the rows with a
/// missing x left out, the last `capacity` of the rest; with the number of
/// rows dropped before the first kept.
fn ring(table: crate::data::Table, encode: &Encode, capacity: usize) -> (Prepared, usize) {
    let p = Prepared::new(table, encode);
    let xs = p.table_xs();
    let len = p.rows();
    if xs.len() != len {
        // No x column: nothing is drawn, nothing to keep in order.
        return (p, 0);
    }
    let end = xs.iter().position(|x| x.is_nan()).unwrap_or(len);
    let start = end.saturating_sub(capacity);
    if start == 0 && end == len {
        return (p, 0);
    }
    let warnings = p.warnings.clone();
    let Some(t) = p.into_table() else { return (Prepared::empty(), 0) };
    let Ok(rows) = rows_of(&t, start..end) else { return (Prepared::new(t, encode), 0) };
    let mut kept = Prepared::new(rows, encode);
    for w in warnings.as_slice() {
        if !kept.warnings.as_slice().contains(w) {
            kept.warnings.push(w.clone());
        }
    }
    (kept, start)
}

/// The rows `range` of `t`, its columns' types, roles, units and categories
/// kept.
fn rows_of(t: &crate::data::Table, range: Range<usize>) -> Result<crate::data::Table, DataError> {
    let columns = t
        .columns()
        .iter()
        .map(|c| Column {
            name: c.name.clone(),
            kind: c.kind,
            role: c.role,
            unit: c.unit.clone(),
            scale: c.scale,
            offset: c.offset,
            time_unit: c.time_unit,
            origin: c.origin,
            zone: c.zone.clone(),
            categories: c.categories.clone(),
            values: match &c.values {
                Values::Num(v) => Values::Num(v[range.clone()].to_vec()),
                Values::Codes(v) => Values::Codes(v[range.clone()].to_vec()),
                Values::Text(v) => Values::Text(v[range.clone()].to_vec()),
            },
        })
        .collect();
    let mut out = crate::data::Table::from_columns(columns)?;
    out.sorted = t.sorted.clone();
    Ok(out)
}

/// One series' band of rows.
#[derive(Clone, Debug, PartialEq)]
struct Band {
    /// Its cells: the plot's columns over its rows.
    rect: Rect,
    /// Its y axis over its rows, 4 dots a cell.
    y: Axis,
    /// The step between ticks of its y domain, which the readout's digits
    /// start from.
    y_step: f64,
    /// Where its columns rise from: 0 held inside the y domain.
    base: f64,
}

/// The layout of a frame.
#[derive(Clone, Debug)]
struct Geom {
    /// The plot: the columns left of the text, over every row.
    plot: Rect,
    /// The x axis over the plot's columns, 2 dots a cell.
    x: Axis,
    /// The step between ticks of the x domain.
    x_step: f64,
    /// Each series' band; `None` for a series given no rows.
    bands: Vec<Option<Band>>,
    /// The text column's first cell, and each series' name and values,
    /// when the text is drawn.
    text: Option<(usize, Vec<(String, String)>)>,
}

impl Geom {
    /// The series whose band holds row `row`.
    fn band_at(&self, row: usize) -> Option<usize> {
        self.bands.iter().position(|b| b.as_ref().is_some_and(|b| row >= b.rect.y && row < b.rect.y + b.rect.h))
    }
}

/// What the layout depends on (REQ-LAYOUT-010).
#[derive(Clone, Debug, PartialEq)]
struct GeomKey {
    cols: usize,
    rows: usize,
    opts: u64,
    data: u64,
    x: [u64; 2],
    y: Option<[u64; 2]>,
    fit: Fit,
    hidden: Vec<String>,
    locale: u64,
    zone: u64,
}

/// What the base depends on besides the layout.
#[derive(Clone, Debug, PartialEq)]
struct BaseKey {
    geom: GeomKey,
    paper: bool,
    mono: bool,
}

/// The last frame's size and play time, which input and hit tests use.
#[derive(Clone, Copy, Debug, PartialEq)]
struct FrameCtx {
    cols: usize,
    rows: usize,
    t: f64,
}

/// The rows of each band a series takes: `rows` split in order, the first
/// `rows % n` one more.
fn band_rows(rows: usize, n: usize) -> Vec<(usize, usize)> {
    if n == 0 {
        return Vec::new();
    }
    let (each, extra) = (rows / n, rows % n);
    let mut y = 0;
    (0..n)
        .map(|s| {
            let h = each + usize::from(s < extra);
            let band = (y, h);
            y += h;
            band
        })
        .collect()
}

/// A value of the text column, about four significant digits with zeros
/// trimmed, from the core's formatters (REQ-FMT-030): SI-prefixed from
/// 10,000 up (`.4~s`, `18.24k`), in exponent notation below 0.001 (`.3~e`),
/// else fixed at the digits that leave four significant (`,.3~f` for
/// 0.998, `,.0~f` for 1,235); under the column's display transform when it
/// has one (REQ-DATA-100).
fn compact(v: f64, display: &Display, locale: &Locale) -> String {
    if !display.is_identity() {
        return draw::shown(v, display, locale);
    }
    let a = v.abs();
    let spec = if a >= 1e4 {
        Specifier { precision: Some(4), trim: true, kind: FormatKind::Si, ..Specifier::default() }
    } else if a < 1e-3 && a > 0.0 {
        Specifier { precision: Some(3), trim: true, kind: FormatKind::Exponent, ..Specifier::default() }
    } else {
        let digits = if a == 0.0 { 0 } else { (3 - a.log10().floor() as i32).clamp(0, 6) as u32 };
        Specifier { comma: true, precision: Some(digits), trim: true, kind: FormatKind::Fixed, ..Specifier::default() }
    };
    display.with_unit(&NumberFormat::new(&spec, locale).format(v))
}

/// The share of a band's y span added at each end of its domain.
pub const Y_PAD: f64 = 0.02;

/// A band's y domain from the extent `[lo, hi]` it fits: [`Y_PAD`] of the
/// span added at each end (the next float out when that is lost to
/// rounding). The least value then falls inside the band's last dot row,
/// not one past it where `Axis::dot` puts a domain's low end; and in a tall
/// band the greatest stops short of the top dot, so a filled area or a
/// column reaching it does not fill its cell column from top to bottom. In a
/// band of a row or two the pad is under a dot, so its marks keep every dot.
fn padded(lo: f64, hi: f64) -> [f64; 2] {
    let d = (hi - lo) * Y_PAD;
    let (a, b) = (lo - d, hi + d);
    [if a < lo { a } else { lo.next_down() }, if b > hi { b } else { hi.next_up() }]
}

/// `v` held to `lo..=hi` by comparison, `lo` for NaN (REQ-LINT-040: no
/// `f64::clamp`, which panics on NaN bounds).
fn held(v: f64, lo: f64, hi: f64) -> f64 {
    if v > hi {
        hi
    } else if v >= lo {
        v
    } else {
        lo
    }
}

/// The eighths of a cell from a band's top to `v`, held to the band.
fn eighths(b: &Band, v: f64) -> f64 {
    let span = b.y.hi() - b.y.lo();
    let e = (b.y.hi() - v) / span * (8 * b.rect.h) as f64;
    if e.is_nan() {
        0.0
    } else {
        held(e, 0.0, (8 * b.rect.h) as f64)
    }
}

/// A column's ends in its band, rows from the band's top: the top cell of
/// the part above the base, the bottom cell of the part below it, or the
/// cell of the mark of a column of no height.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Ends {
    up: Option<usize>,
    down: Option<usize>,
    mark: Option<usize>,
}

/// The bar of a cell whose data run from `lo` to `hi` in band `b`: its cells
/// (row from the band's top, glyph) into `out`, and its ends.
fn bar(b: &Band, lo: f64, hi: f64, mut out: impl FnMut(usize, char)) -> Ends {
    let base = (eighths(b, b.base) / 8.0).round() * 8.0;
    let rb = (base / 8.0) as usize;
    let (et, eb) = (eighths(b, hi), eighths(b, lo));
    let mut ends = Ends::default();
    if et < base {
        let rt = (et / 8.0).floor() as usize;
        let n = held(((8 * (rt + 1)) as f64 - et).round(), 1.0, 8.0) as usize;
        out(rt, LOWER[n - 1]);
        for r in rt + 1..rb {
            out(r, '█');
        }
        ends.up = Some(rt);
    }
    if eb > base {
        let rd = ((eb / 8.0).ceil() as usize).saturating_sub(1).max(rb);
        let n = held((eb - (8 * rd) as f64).round(), 1.0, 8.0) as usize;
        for r in rb..rd {
            out(r, '█');
        }
        out(
            rd,
            match n {
                1 | 2 => '▔',
                3..=6 => '▀',
                _ => '█',
            },
        );
        ends.down = Some(rd);
    }
    if ends.up.is_none() && ends.down.is_none() {
        let (r, g) = if rb > 0 { (rb - 1, '▁') } else { (0, '▔') };
        out(r, g);
        ends.mark = Some(r);
    }
    ends
}

/// The cell whose bar each cell draws: a cell holding data its own; an
/// empty cell between two cells of data with no gap (a missing y) between
/// them the nearer one's, so bars widen to their spacing, with one blank
/// cell between them (two when the empty cells are even), as candles do; an
/// empty cell before the first or after the last cell of data, or in a gap,
/// none.
fn widened(ext: &[Option<[f64; 2]>], gap: &[bool]) -> Vec<Option<usize>> {
    let mut out: Vec<Option<usize>> = ext.iter().enumerate().map(|(c, e)| e.map(|_| c)).collect();
    let mut prev: Option<usize> = None;
    for c in (0..ext.len()).filter(|&c| ext[c].is_some()) {
        if let (Some(a), false) = (prev, gap[c]) {
            let k = (c - a - 1).saturating_sub(1) / 2;
            for o in &mut out[a + 1..=a + k] {
                *o = Some(a);
            }
            for o in &mut out[c - k..c] {
                *o = Some(c);
            }
        }
        prev = Some(c);
    }
    out
}

/// The least and greatest finite y of `xs`/`ys` in the dot columns `2c` and
/// `2c + 1` of `edges`, as M4 and the drawing assign rows to dot columns.
fn cell_extent(xs: &[f64], ys: &[f64], edges: &Edges, c: usize) -> Option<[f64; 2]> {
    let e = edges.values();
    let dots = edges.dots();
    if 2 * c >= dots {
        return None;
    }
    let last = 2 * c + 2 >= dots;
    let (from, to) = (e[2 * c], e[(2 * c + 2).min(dots)]);
    let mut out: Option<[f64; 2]> = None;
    for i in lower_bound(xs, 0, xs.len(), from)..xs.len() {
        let x = xs[i];
        if !(x < to || (last && x <= to)) {
            break;
        }
        let y = ys[i];
        if y.is_finite() {
            out = Some(out.map_or([y, y], |[a, b]| [a.min(y), b.max(y)]));
        }
    }
    out
}

/// A `sparkline` chart (REQ-SPARK-010): the [`Chart`] the registry makes for
/// the type `sparkline`.
pub struct SparkChart {
    options: Options,
    cfg: Config,
    opts_version: u64,
    data: Prepared,
    /// The rows dropped before the first held, so a held row's number is
    /// `row0` and its position.
    row0: usize,
    data_version: u64,
    view: View,
    gesture: Gesture,
    m4: Cache,
    pts: Vec<Pt>,
    geom: Option<(GeomKey, Geom, Vec<Range<usize>>)>,
    ctx: Option<FrameCtx>,
    pack: LocalePack,
    locale: Locale,
    locale_version: u64,
    zone_table: ZoneTable,
    zone: Zone,
    zone_version: u64,
    base: Grid,
    base_key: Option<BaseKey>,
    canvas: ColorCanvas,
    formats: readout::Formats,
}

impl SparkChart {
    /// A chart with no data for options layered on [`META`]'s; a mode not
    /// listed is an error naming the valid ones (REQ-CHART-030).
    pub fn new(options: &Options) -> Result<SparkChart, MakeError> {
        let options = options.for_meta(&META);
        let cfg = config(&options)?;
        Ok(SparkChart {
            options,
            cfg,
            opts_version: 0,
            data: Prepared::empty(),
            row0: 0,
            data_version: 0,
            view: View::default(),
            gesture: Gesture::default(),
            m4: Cache::new(),
            pts: Vec::new(),
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
            canvas: ColorCanvas::new(Glyphs::Braille, 0, 0),
            formats: readout::Formats::default(),
        })
    }

    /// The options as read, defaults filled in, recording each key read
    /// (REQ-OPT-010).
    pub fn options(&self) -> &Options {
        &self.options
    }

    /// The rows the ring holds.
    pub fn held(&self) -> usize {
        self.data.rows()
    }

    fn shown(&self, view: &View, s: usize) -> bool {
        !view.is_hidden(&self.data.series[s].name)
    }

    /// The x domain drawn for a requested domain (null for the fitted one)
    /// and `follow`, as the line chart resolves it.
    fn resolve_x(&self, requested: Option<[f64; 2]>, follow: bool) -> [f64; 2] {
        let data = self.data.x_extent();
        let mut view = requested;
        if let (true, Some([a, b]), Some([_, last])) = (follow, requested, data) {
            view = Some([last - (b - a), last]);
        }
        let opts = ScaleOptions::new(if self.data.kind == Kind::Time { Kind::Time } else { Kind::Linear });
        resolve_domain(&opts, self.data.kind, &Extents { data, visible: None, view }, 1.0).unwrap_or([0.0, 1.0])
    }

    fn domain_now(&self) -> [f64; 2] {
        self.resolve_x(self.view.x.domain, self.view.x.follow)
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
            zone: self.zone_version,
        }
    }

    /// Series `s`'s name and its values' text as the options ask: the latest
    /// finite y held, and `↓least ↑greatest` of every finite y held.
    fn text_of(&self, s: usize, label: bool, latest: bool, extent: bool) -> (String, String) {
        let ys = self.data.ys(s);
        let display = self.data.display(s);
        let mut values: Vec<String> = Vec::new();
        if latest {
            if let Some(v) = ys.iter().rev().find(|v| v.is_finite()) {
                values.push(compact(*v, &display, &self.locale));
            }
        }
        if extent {
            let finite = ys.iter().filter(|v| v.is_finite());
            let (lo, hi) = finite.fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &v| (a.min(v), b.max(v)));
            if lo <= hi {
                let (lo, hi) = if display.scale < 0.0 { (hi, lo) } else { (lo, hi) };
                values.push(format!("↓{} ↑{}", compact(lo, &display, &self.locale), compact(hi, &display, &self.locale)));
            }
        }
        let name = if label { self.data.series[s].name.clone() } else { String::new() };
        (name, values.join(" "))
    }

    /// The layout for `key` (REQ-LAYOUT-010), and the rows in view a series.
    fn make_geom(&self, key: &GeomKey, view: &View) -> (Geom, Vec<Range<usize>>) {
        let (cols, rows) = (key.cols, key.rows);
        let xd = [f64::from_bits(key.x[0]), f64::from_bits(key.x[1])];
        let n = self.data.series.len();
        let in_view: Vec<Range<usize>> = (0..n).map(|s| view_rows(self.data.xs(s), xd[0], xd[1])).collect();
        // The text: parts dropped until the plot keeps half the columns and
        // at least MIN_PLOT.
        let cfg = &self.cfg;
        let tries = [(cfg.label, cfg.latest, cfg.extent), (cfg.label, cfg.latest, false), (cfg.label, false, false)];
        let mut text = None;
        if self.data.rows() > 0 && n > 0 {
            for (label, latest, extent) in tries {
                if !(label || latest || extent) {
                    break;
                }
                let texts: Vec<(String, String)> = (0..n).map(|s| self.text_of(s, label, latest, extent)).collect();
                let width = texts
                    .iter()
                    .map(|(a, b)| {
                        let gap = usize::from(!a.is_empty() && !b.is_empty());
                        text::width(a) + gap + text::width(b)
                    })
                    .max()
                    .unwrap_or(0);
                if width == 0 {
                    break;
                }
                let plot_w = cols.saturating_sub(width + 1);
                if plot_w >= MIN_PLOT && 2 * plot_w >= cols {
                    text = Some((plot_w + 1, texts));
                    break;
                }
            }
        }
        let plot_w = text.as_ref().map_or(cols, |(x, _)| x - 1);
        let plot = Rect::new(0, 0, plot_w, rows);
        let x = Axis::x(xd[0], xd[1], plot_w, 2);
        let x_step = tick_step(xd[0], xd[1], (plot_w / crate::layout::X_CELLS_PER_TICK).max(1) as f64).abs();
        let bands = band_rows(rows, n)
            .into_iter()
            .enumerate()
            .map(|(s, (y, h))| {
                (h > 0).then(|| {
                    let ext = match (view.y.domain, view.y.fit) {
                        (Some(d), _) => Some(d),
                        (None, Fit::Visible) => self.data.y_extent(|t| t == s, Some(xd)),
                        (None, _) => self.data.y_extent(|t| t == s, None),
                    };
                    let opts = ScaleOptions::new(Kind::Linear);
                    let [a, b] = resolve_domain(&opts, Kind::Linear, &Extents { data: ext, visible: None, view: None }, 1.0).unwrap_or([0.0, 1.0]);
                    // A y domain in the view is drawn as it is; a fitted one
                    // is padded.
                    let [lo, hi] = if view.y.domain.is_some() { [a.min(b), a.max(b)] } else { padded(a.min(b), a.max(b)) };
                    let y_step = tick_step(lo, hi, (h / crate::layout::Y_ROWS_PER_TICK).max(1) as f64).abs();
                    Band { rect: Rect::new(plot.x, y, plot.w, h), y: Axis::y(lo, hi, h, 4), y_step, base: held(0.0, lo, hi) }
                })
            })
            .collect();
        (Geom { plot, x, x_step, bands, text }, in_view)
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

    /// The row of the cell a column draws datum `y` at, in band `b`: the end
    /// of the column on the datum's side of the base.
    fn column_row(&self, g: &Geom, b: &Band, s: usize, x: f64, y: f64) -> Option<usize> {
        let edges = Edges::new(&g.x);
        let c = edges.column(x)? / 2;
        let [lo, hi] = cell_extent(self.data.xs(s), self.data.ys(s), &edges, c)?;
        let ends = bar(b, lo, hi, |_, _| {});
        let base = (eighths(b, b.base) / 8.0).round() * 8.0;
        let e = eighths(b, y);
        let row = if e < base {
            ends.up
        } else if e > base {
            ends.down
        } else {
            ends.up.or(ends.down)
        };
        row.or(ends.mark).or(ends.up).or(ends.down).map(|r| b.rect.y + r)
    }

    /// The datum of series `s` at position `i`, placed on the plot.
    fn hit_of(&self, geom: Option<&Geom>, s: usize, i: usize, at: Option<CellPos>, aggregate: Option<crate::events::Aggregate>) -> Hit {
        let (xs, ys) = (self.data.xs(s), self.data.ys(s));
        let (x, y) = (xs[i], ys[i]);
        let (mut col, mut row) = at.map_or((0, 0), |p| (p.x.max(0.0) as usize, p.y.max(0.0) as usize));
        if let Some(g) = geom {
            if let Some(c) = Edges::new(&g.x).column(x) {
                col = g.plot.x + c / 2;
            }
            if let Some(Some(b)) = g.bands.get(s) {
                let drawn = match self.cfg.mode {
                    SparkMode::Column if y.is_finite() => self.column_row(g, b, s, x, y),
                    _ => b.y.dot_in(y).map(|d| b.rect.y + d / 4),
                };
                match drawn {
                    Some(r) => row = r,
                    None if at.is_none() => row = b.rect.y,
                    None => {}
                }
            }
        }
        Hit { series: self.data.series[s].name.clone(), series_index: s, index: self.row0 + self.data.row(s, i), x, y, col, row, aggregate }
    }

    /// The datum the cursor snaps to at `at` (REQ-HIT-010, REQ-HIT-020): in
    /// the band under `at`, when its series is shown, the datum whose x is
    /// nearest the centre of the dot column under `at`, by binary search,
    /// with the dot column's aggregate when it holds more than one datum.
    fn hit_in(&self, g: &Geom, view: &View, at: CellPos) -> Option<Hit> {
        let plot = g.plot;
        let p = Plot { rect: plot, x: g.x, y: Axis::y(0.0, 1.0, plot.h, 4) };
        if !p.contains(at) {
            return None;
        }
        let s = g.band_at(at.y.floor() as usize)?;
        if !self.shown(view, s) {
            return None;
        }
        let k = ((at.x - plot.x as f64) * 2.0).floor() as i64;
        let target = g.x.dot_center(k);
        let (xs, ys) = (self.data.xs(s), self.data.ys(s));
        let i = nearest(xs, 0, xs.len(), target)?;
        let (lo, hi) = (g.x.dot_value(k), if k + 1 >= g.x.dots() as i64 { g.x.hi().next_up() } else { g.x.dot_value(k + 1) });
        let aggregate = column_aggregate(xs, ys, 0, xs.len(), lo, hi);
        Some(self.hit_of(Some(g), s, i, Some(at), aggregate))
    }

    /// The position in series `s` of datum row `index`, if it is held.
    fn pos_of(&self, s: usize, index: u64) -> Option<usize> {
        let row = usize::try_from(index).ok()?.checked_sub(self.row0)?;
        self.data.pos(s, row)
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
        let (lo, hi) = (self.row0 as u64, (self.row0 + self.data.rows()) as u64);
        if let Some(sel) = &mut self.view.selection {
            sel.points.retain(|p| p.index >= lo && p.index < hi);
        }
    }

    /// Takes `prepared` as the held data, `row0` rows dropped before it.
    fn take(&mut self, prepared: Prepared, row0: usize) -> Accepted {
        self.geom = None;
        self.m4.invalidate();
        self.base_key = None;
        self.data = prepared;
        self.row0 = row0;
        self.data_version += 1;
        self.resnap();
        Accepted { rows: self.data.rows(), version: self.data_version, warnings: self.data.warnings.as_slice().to_vec() }
    }

    fn take_data(&mut self, data: Data) -> Result<Accepted, DataError> {
        data.shape(META.shapes, &self.cfg.encode)?;
        let table = data.into_tables().into_iter().next().map(|(_, t)| t);
        let (prepared, row0) = table.map_or_else(|| (Prepared::empty(), 0), |t| ring(t, &self.cfg.encode, self.cfg.capacity));
        Ok(self.take(prepared, row0))
    }

    /// The cursor's value text: its x, then each shown series' value at the
    /// datum nearest it, up to four, then `and N more` (REQ-A11Y-030).
    fn value_text(&self) -> Option<String> {
        let x = self.view.cursor.as_ref()?.x?;
        let d = self.domain_now();
        let mut parts = vec![draw::x_label(x, self.data.kind, d[1] - d[0], &self.locale, &self.zone, &self.data.x_display())];
        let shown: Vec<usize> = (0..self.data.series.len()).filter(|&s| self.shown(&self.view, s)).collect();
        for &s in shown.iter().take(4) {
            let xs = self.data.xs(s);
            let v = nearest(xs, 0, xs.len(), x).map(|i| self.data.ys(s)[i]).filter(|v| v.is_finite());
            let v = v.map_or_else(|| "no value".to_string(), |v| draw::shown(v, &self.data.display(s), &self.locale));
            parts.push(format!("{} {}", self.data.series[s].name, v));
        }
        let mut text = parts.join(", ");
        if shown.len() > 4 {
            text.push_str(&format!(", and {} more", shown.len() - 4));
        }
        Some(text)
    }

    /// The chart's label: what it draws, its series, its range, each series'
    /// latest value and the count in view (REQ-A11Y-030).
    fn summary(&self) -> String {
        let what = match self.cfg.mode {
            SparkMode::Line => "sparkline",
            SparkMode::Area => "area sparkline",
            SparkMode::Column => "column sparkline",
        };
        let names: Vec<&str> = self.data.series.iter().map(|s| s.name.as_str()).collect();
        if names.is_empty() || self.data.rows() == 0 {
            return format!("{what}, no data");
        }
        let d = self.domain_now();
        let span = d[1] - d[0];
        let x_disp = self.data.x_display();
        let mut text = format!(
            "{what} of {} series ({}); {} from {} to {}",
            names.len(),
            names.join(", "),
            self.data.x_name,
            draw::x_label(d[0], self.data.kind, span, &self.locale, &self.zone, &x_disp),
            draw::x_label(d[1], self.data.kind, span, &self.locale, &self.zone, &x_disp)
        );
        let latest: Vec<String> = (0..names.len())
            .filter_map(|s| {
                let v = self.data.ys(s).iter().rev().find(|v| v.is_finite())?;
                Some(format!("{} {}", names[s], draw::shown(*v, &self.data.display(s), &self.locale)))
            })
            .collect();
        if !latest.is_empty() {
            text.push_str(&format!("; latest {}", latest.join(", ")));
        }
        let count: usize = (0..names.len()).filter(|&s| self.shown(&self.view, s)).map(|s| view_rows(self.data.xs(s), d[0], d[1]).len()).sum();
        text.push_str(&format!("; {} points in view", draw::count(count, &self.locale)));
        text
    }

    /// Draws the base: the marks, then the text (REQ-TEXT-020).
    fn draw_base(&mut self, env: &Env) {
        let Some((_, g, in_view)) = &self.geom else { return };
        let ground = if env.paper { GROUND } else { NO_BACK };
        let plot = g.plot;
        let mut f = Frame::new(&mut self.base, ground);
        let n = self.data.series.len();
        let any = self.data.rows() > 0 && n > 0;
        if any && plot.w > 0 && plot.h > 0 {
            let edges = Edges::new(&g.x);
            let key = CacheKey::new(self.data_version, [g.x.lo(), g.x.hi()], edges.dots(), false);
            let data = &self.data;
            let m4s: Option<&[M4]> = if self.cfg.sample_none {
                None
            } else {
                Some(self.m4.get_into(key, n, |s, m| m4_into(data.xs(s), data.ys(s), in_view[s].clone(), &edges, false, m)))
            };
            self.canvas.resize(plot.w, plot.h);
            let w = self.canvas.width() as i64;
            let mut cells: Vec<(usize, usize, char, u8)> = Vec::new();
            let mut tracks: Vec<usize> = Vec::new();
            for s in (0..n).filter(|&s| !self.view.is_hidden(&data.series[s].name)) {
                let Some(Some(b)) = g.bands.get(s) else { continue };
                let ink = draw::series_ink(s);
                let top = ((b.rect.y - plot.y) * 4) as i64;
                let h = (b.rect.h * 4) as i64;
                match self.cfg.mode {
                    SparkMode::Line | SparkMode::Area => {
                        // A lone datum in view, with no line to draw through
                        // it, sits on a muted dashed track across its band at
                        // its level, so a sample of one reads as a level.
                        let (xs, ys) = (data.xs(s), data.ys(s));
                        let mut finite = in_view[s].clone().filter(|&i| edges.column(xs[i]).is_some() && ys[i].is_finite());
                        if let (Some(i), None) = (finite.next(), finite.next()) {
                            if let Some(d) = b.y.dot_in(ys[i]) {
                                tracks.push(b.rect.y + d / 4);
                            }
                        }
                        path(data, s, &in_view[s], m4s.map(|m| &m[s]), (&g.x, &b.y), &edges, false, &mut self.pts);
                        if self.cfg.mode == SparkMode::Line {
                            let canvas = &mut self.canvas;
                            walk(&self.pts, Mode::Line, StepAt::After, w, h, |x, y| canvas.set(x, top + y, ink));
                        } else {
                            let mut tops = vec![i64::MAX; w as usize];
                            walk(&self.pts, Mode::Line, StepAt::After, w, h, |x, y| tops[x as usize] = tops[x as usize].min(y));
                            for (x, &t) in tops.iter().enumerate().filter(|(_, &t)| t != i64::MAX) {
                                for y in t..h {
                                    self.canvas.set(x as i64, top + y, ink);
                                }
                            }
                        }
                    }
                    SparkMode::Column => {
                        // Each cell's least and greatest y, and whether a gap
                        // (a missing y) comes between it and the cell of data
                        // before it: from the M4 buckets, or from every row,
                        // which give the same (REQ-SAMPLE-010).
                        let mut ext: Vec<Option<[f64; 2]>> = vec![None; plot.w];
                        let mut gap = vec![false; plot.w];
                        let mut last: Option<usize> = None;
                        let mut take = |c: usize, lo: f64, hi: f64, gap_before: bool| {
                            if c >= ext.len() {
                                return;
                            }
                            if last != Some(c) {
                                gap[c] = gap_before;
                                last = Some(c);
                            }
                            ext[c] = Some(ext[c].map_or([lo, hi], |[a, b]| [a.min(lo), b.max(hi)]));
                        };
                        match m4s {
                            Some(m) => {
                                for bk in &m[s].buckets {
                                    take(bk.col / 2, bk.min.y, bk.max.y, bk.gap_before);
                                }
                            }
                            None => {
                                let (xs, ys) = (data.xs(s), data.ys(s));
                                let mut pending = false;
                                for i in in_view[s].clone() {
                                    let Some(k) = edges.column(xs[i]) else { continue };
                                    if ys[i].is_finite() {
                                        take(k / 2, ys[i], ys[i], pending);
                                        pending = false;
                                    } else {
                                        pending = true;
                                    }
                                }
                            }
                        }
                        for (c, owner) in widened(&ext, &gap).into_iter().enumerate() {
                            if let Some([lo, hi]) = owner.and_then(|o| ext[o]) {
                                bar(b, lo, hi, |r, glyph| cells.push((plot.x + c, b.rect.y + r, glyph, ink)));
                            }
                        }
                    }
                }
            }
            self.canvas.draw(f.data(), plot.x as i32, plot.y as i32);
            for (x, y, glyph, ink) in cells {
                f.data().set(x as i32, y as i32, glyph as u32, ink);
            }
            for row in tracks {
                for c in plot.x..plot.x + plot.w {
                    if f.data().glyph(c, row) == crate::grid::BLANK {
                        f.data().set(c as i32, row as i32, '┄' as u32, MUTED);
                    }
                }
            }
        }
        if !any && plot.h > 0 {
            // With no data, a dashed rule across the plot's middle row, which
            // `no data` is written over: the empty track a sparkline would
            // run along.
            for c in plot.x..plot.x + plot.w {
                f.data().set(c as i32, (plot.y + plot.h / 2) as i32, '┄' as u32, MUTED);
            }
        }
        let f = f.then::<stage::Axes>();
        let mut f = f.then::<stage::Labels>();
        if let Some((tx, texts)) = &g.text {
            for (s, (name, values)) in texts.iter().enumerate() {
                let Some(Some(b)) = g.bands.get(s) else { continue };
                let row = b.rect.y + (b.rect.h - 1) / 2;
                let hidden = self.view.is_hidden(&self.data.series[s].name);
                let mut x = *tx;
                if !name.is_empty() {
                    f.label(x, row, name, if hidden { MUTED } else { draw::series_ink(s) });
                    x += text::width(name) + 1;
                }
                if !values.is_empty() {
                    f.label(x, row, values, if hidden { MUTED } else { INK });
                }
            }
        }
        if !any && plot.w >= 7 && plot.h >= 1 {
            let t = "no data";
            f.label(plot.x + (plot.w - t.len()) / 2, plot.y + plot.h / 2, t, MUTED);
        }
    }
}

impl Chart for SparkChart {
    fn set_data(&mut self, data: Data) -> Result<Accepted, DataError> {
        self.take_data(data)
    }

    fn append(&mut self, batch: Data) -> Result<Appended, DataError> {
        let Some(old) = self.data.table.as_ref() else {
            let a = self.take_data(batch)?;
            return Ok(Appended { rows: a.rows, dropped: 0, late: 0, version: a.version });
        };
        let Some(new) = batch.first_table() else { return Err(DataError::Shape { expected: META.shapes.to_vec() }) };
        let x = old.find(&self.data.x_name);
        let (table, late) = appended(old, new, x)?;
        let (prepared, dropped) = ring(table, &self.cfg.encode, self.cfg.capacity);
        let row0 = self.row0 + dropped;
        let a = self.take(prepared, row0);
        Ok(Appended { rows: a.rows, dropped, late, version: a.version })
    }

    fn set_options(&mut self, options: &Options) -> Result<(), MakeError> {
        let layered = options.for_meta(&META);
        let cfg = config(&layered)?;
        let reprepare = cfg.encode != self.cfg.encode || cfg.capacity < self.data.rows();
        self.options = layered;
        self.cfg = cfg;
        self.opts_version += 1;
        if reprepare {
            let table = std::mem::replace(&mut self.data, Prepared::empty()).into_table();
            let (prepared, dropped) = table.map_or_else(|| (Prepared::empty(), 0), |t| ring(t, &self.cfg.encode, self.cfg.capacity));
            let row0 = self.row0 + dropped;
            self.take(prepared, row0);
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
        if *env.zone != self.zone_table {
            self.zone_table = env.zone.clone();
            self.zone = env.zone.zone();
            self.zone_version += 1;
        }
        let (cols, rows) = (grid.cols(), grid.rows());
        self.ctx = Some(FrameCtx { cols, rows, t });
        self.ensure_geom(t);
        let Some((key, _, _)) = &self.geom else { return };
        let bkey = BaseKey { geom: key.clone(), paper: env.paper, mono: env.mono };
        if self.base_key.as_ref() != Some(&bkey) {
            if self.base.cols() != cols || self.base.rows() != rows {
                self.base.resize(cols, rows);
            }
            self.draw_base(env);
            self.base_key = Some(bkey);
        }
        grid.copy_from(&self.base);
        let Some((_, g, _)) = &self.geom else { return };
        let plot = g.plot;
        if plot.w == 0 || plot.h == 0 {
            return;
        }
        let ground = if env.paper { GROUND } else { NO_BACK };
        if let Some([a, b]) = self.view.selection.as_ref().and_then(|s| s.x) {
            if let (Some(ca), Some(cb)) = (g.x.cell(a), g.x.cell(b)) {
                crate::overlay::brush(grid, plot, ca, cb);
            }
        }
        let edges = Edges::new(&g.x);
        let mut f = Frame::overlay(grid, ground);
        if let Some(t) = self.view.time.and_then(|t| x_dot(&g.x, &edges, t)) {
            crate::overlay::time_cursor(&mut f, plot, t);
        }
        let Some(c) = &self.view.cursor else { return };
        let s = c.series.as_deref().and_then(|n| self.data.find(n));
        let band = s.and_then(|s| g.bands.get(s)).and_then(Option::as_ref);
        // A cursor naming a held datum is drawn at the datum's x and y, not
        // the view's copies, which may come back from JSON an ulp off
        // (contract rule 16), as the readout writes them.
        let datum = s.and_then(|s| Some((s, self.pos_of(s, c.index?)?))).map(|(s, i)| (self.data.xs(s)[i], self.data.ys(s)[i]));
        let (cx, cy) = match datum {
            Some((x, y)) => (Some(x).filter(|x| x.is_finite()).or(c.x), Some(y).filter(|y| y.is_finite()).or(c.y)),
            None => (c.x, c.y),
        };
        let xd = cx.and_then(|x| x_dot(&g.x, &edges, x));
        match band {
            Some(b) => crate::overlay::crosshair(&mut f, b.rect, xd, cy.and_then(|y| b.y.dot(y))),
            None => crate::overlay::crosshair(&mut f, plot, xd, None),
        }
        let y_step = band.map_or(1.0, |b| b.y_step);
        let place = readout::Place { x: &g.x, x_step: g.x_step, y_step, zone: &self.zone, row0: self.row0 };
        if let Some(text) = readout::text(&self.data, &self.view, place, &self.locale, &mut self.formats) {
            // Across the whole width, over the text column too: a sparkline's
            // plot is often narrower than its readout.
            crate::overlay::readout(f, Rect::new(0, plot.y, cols, plot.h), &text);
        }
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
        let first = (0..self.data.series.len()).find(|&s| self.shown(&self.view, s));
        let value = |s: usize, row: usize| -> f64 { self.data.pos(s, row).map_or(f64::NAN, |i| self.data.ys(s)[i]) };
        let picked = pick_rows(total, max_rows, |i| first.map_or(f64::NAN, |s| value(s, rows.start + i)));
        let x_disp = self.data.x_display();
        let displays: Vec<Display> = (0..self.data.series.len()).map(|s| self.data.display(s)).collect();
        let mut out = Vec::with_capacity(picked.len());
        for i in picked {
            let row = rows.start + i;
            let mut cells = vec![draw::x_label(xs[row], self.data.kind, span, &self.locale, &self.zone, &x_disp)];
            for (s, disp) in displays.iter().enumerate() {
                let v = value(s, row);
                cells.push(if v.is_finite() { draw::shown(v, disp, &self.locale) } else { String::new() });
            }
            out.push(cells);
        }
        let caption = if out.len() < total { "sparkline, each bucket's least and greatest kept".to_string() } else { "sparkline".to_string() };
        Table { caption, columns, rows: out, total: total as u64, summary: self.summary(), value: self.value_text(), domain: Some(d) }
    }
}

impl Scene for SparkChart {
    fn plot(&self) -> Option<Plot> {
        self.geom.as_ref().map(|(_, g, _)| Plot { rect: g.plot, x: g.x, y: Axis::y(0.0, 1.0, g.plot.h, 4) })
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
        let pos = self.pos_of(s, from.index?)?;
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
        hit.index.checked_sub(self.row0).map(|r| self.data.datum(r)).unwrap_or_default()
    }

    fn count(&self, view: &View, range: [f64; 2]) -> u64 {
        (0..self.data.series.len()).filter(|&s| self.shown(view, s)).map(|s| view_rows(self.data.xs(s), range[0], range[1]).len() as u64).sum()
    }

    fn legend_at(&self, _: CellPos) -> Option<usize> {
        None
    }
}

#[cfg(test)]
mod tests;
