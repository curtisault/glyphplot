//! The bar chart's geometry: which categories are drawn and in which cells
//! (the band scale, REQ-SCALE-040), the value axis (snapped to rows under 20
//! rows, REQ-TICK-050), the layout in cells, and each bar's extent in eighths
//! of a cell (REQ-BAR-010).
//!
//! **Categories.** The view's x domain is in category indices (row `i` is
//! category `i`); the categories drawn are those whose index lies in it,
//! every one when it is null, and at most as many as the category axis has
//! cells for (one cell a bar, the shown series' cells a group when grouped).
//! Each drawn category takes the same whole cells (the core's [`Band`]); the
//! cells left over are split before the first and after the last. Within a
//! band at least one cell is left empty once a band has two cells or more,
//! so bars never merge.
//!
//! **Positions.** Category `i` stands at the middle of the cells its bars
//! take: its drawn domain ([`Cats::domain`]) is the one whose linear map over
//! the axis's cells puts each index at the centre of that middle cell, so the
//! cell mapping (`cells.rs`) finds a category's bar from its index.
//!
//! **Eighths.** A bar runs from the baseline to its value along the value
//! axis, both ends in eighths of a cell. Its free end lands on the eighth
//! nearest its value (`▁`–`▇` vertical, `▏`–`▉` horizontal); a value other
//! than the baseline that rounds onto it takes one eighth, so no bar
//! vanishes. Stacked segments meet on whole cells, a cell having one colour;
//! only the stack's free end is at an eighth (discovery/charts.md).

use std::ops::Range;

use super::data::Prepared;
use super::Config;
use crate::annotate;
use crate::cells::Axis;
use crate::chart::Size;
use crate::format::{Locale, Specifier};
use crate::layout::{
    fit_ticks, legend_spans, legend_width, Layout, LegendPosition, PaneLayout, PlacedAxis, Rect, Side, Tick, TickLabel, LEGEND_ROWS, MIN_COLS, MIN_ROWS,
    PANE_ROWS, TITLE_ROWS, X_CELLS_PER_TICK, X_GAP, X_LABEL_ROWS, Y_DOTS, Y_GAP, Y_ROWS_PER_TICK,
};
use crate::scale::{nice, AxisFormat, Band, Kind};
use crate::text;
use crate::ticks::{snap_rows, Snap};

/// The fewest cells a thinned category label is given before more are
/// thinned out: room for four glyphs and the `…`.
const MIN_LABEL: usize = 5;

/// The categories drawn along the category axis, and their band.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Cats {
    /// The first category drawn.
    pub first: usize,
    /// The band: `count` categories over the axis's cells (REQ-SCALE-040).
    pub band: Band,
    /// The cells of one series' bar.
    pub sub: usize,
    /// The bars across a band: the shown series when grouped, else 1.
    pub groups: usize,
    /// The cells of a band before its first bar.
    pub glead: usize,
}

/// The categories in the requested domain, `[first, last]`; when it holds
/// none, the one nearest its middle. None for no categories.
fn visible(n: usize, requested: Option<[f64; 2]>) -> Option<(usize, usize)> {
    if n == 0 {
        return None;
    }
    let last = (n - 1) as f64;
    let Some([a, b]) = requested.filter(|[a, b]| a.is_finite() && b.is_finite()) else { return Some((0, n - 1)) };
    let (a, b) = (a.min(b), a.max(b));
    let (lo, hi) = (a.ceil().max(0.0), b.floor().min(last));
    if lo <= hi {
        return Some((lo as usize, hi as usize));
    }
    let mid = within(((a + b) / 2.0).round(), 0.0, last) as usize;
    Some((mid, mid))
}

/// `v` held to `lo..=hi` by comparison, as REQ-LINT-040 asks of floats
/// (`f64::clamp` panics on NaN bounds); NaN stays NaN.
pub(super) fn within(v: f64, lo: f64, hi: f64) -> f64 {
    if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}

impl Cats {
    /// The categories of `n` drawn for the `requested` x domain (null for
    /// all) over `cells` cells, `groups` bars a band, `padding` the share of
    /// each band left empty; None when none fits.
    pub fn new(n: usize, requested: Option<[f64; 2]>, cells: usize, groups: usize, padding: f64) -> Option<Cats> {
        let (first, last) = visible(n, requested)?;
        let groups = groups.max(1);
        let count = (last - first + 1).min(cells / groups);
        if count == 0 {
            return None;
        }
        let per = cells / count;
        let padding = if padding.is_finite() { within(padding, 0.0, 1.0) } else { 0.0 };
        let mut pad = (per as f64 * padding).round() as usize;
        if padding > 0.0 && per >= 2 {
            pad = pad.max(1);
        }
        let pad = pad.min(per - groups);
        let inner = per - pad;
        let lead = pad / 2;
        let band = Band { count, cells, per, start: (cells - per * count) / 2, lead, inner };
        let sub = inner / groups;
        let glead = lead + (inner - sub * groups) / 2;
        Some(Cats { first, band, sub, groups, glead })
    }

    /// The categories drawn.
    pub fn count(&self) -> usize {
        self.band.count
    }

    /// The cell (from the axis's start) of the middle of the bars of drawn
    /// category `k`.
    pub fn middle(&self, k: usize) -> usize {
        self.band.start + k * self.band.per + self.glead + (self.sub * self.groups - 1) / 2
    }

    /// The cells of bar `j` (0 when not grouped) of drawn category `k`.
    pub fn cross(&self, k: usize, j: usize) -> Range<usize> {
        let s = self.band.start + k * self.band.per + self.glead + j * self.sub;
        s..s + self.sub
    }

    /// The drawn domain in category indices: a linear map of it over the
    /// axis's cells puts category `i` at the centre of [`Cats::middle`].
    pub fn domain(&self) -> [f64; 2] {
        let per = self.band.per as f64;
        let off = (self.band.start + self.glead + (self.sub * self.groups - 1) / 2) as f64 + 0.5;
        let lo = self.first as f64 - off / per;
        [lo, lo + self.band.cells as f64 / per]
    }

    /// The drawn category at cell `cell` of the axis, the nearest drawn one
    /// for cells outside every band: the band hit test, in O(1)
    /// (REQ-BAR-020).
    pub fn at(&self, cell: usize) -> usize {
        match self.band.at(cell) {
            Some(k) => k,
            None if cell < self.band.start => 0,
            None => self.count() - 1,
        }
    }

    /// The bar of a band at cell `cell`, nearest when between bars: by
    /// arithmetic, in O(1) (REQ-BAR-020).
    pub fn bar_at(&self, k: usize, cell: usize) -> usize {
        let s = self.band.start + k * self.band.per + self.glead;
        (cell.saturating_sub(s) / self.sub.max(1)).min(self.groups - 1)
    }
}

/// The value axis: its domain as displayed and its cells along the axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct ValueAxis {
    /// The domain, low to high, in displayed units.
    pub domain: [f64; 2],
    /// The cells along the axis.
    pub cells: usize,
}

impl ValueAxis {
    /// The eighths along the axis.
    pub fn eighths(&self) -> i64 {
        8 * self.cells as i64
    }

    /// The eighth nearest `v`, from the axis's start (bottom or left),
    /// clamped to the axis; None for a value that is not finite.
    pub fn eighth(&self, v: f64) -> Option<i64> {
        let [lo, hi] = self.domain;
        let spans = hi > lo;
        if !v.is_finite() || !spans {
            return None;
        }
        let e = ((v - lo) / (hi - lo) * self.eighths() as f64).round();
        Some(if e.is_nan() { 0 } else { within(e, 0.0, self.eighths() as f64) as i64 })
    }
}

/// One bar, or one segment of a stack: its cells across, its extent in
/// eighths along the value axis, and which end is free.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Seg {
    /// The series, by its index in the series list.
    pub series: usize,
    /// The cells across the category axis, from the plot's start.
    pub cross: Range<usize>,
    /// The eighths along the value axis from its start, `lo..hi`; empty
    /// for a missing value.
    pub lo: i64,
    /// See `lo`.
    pub hi: i64,
    /// Whether the free end is `hi` (a value at or above the baseline).
    pub up: bool,
}

impl Seg {
    /// The cells along the value axis the segment inks.
    pub fn cells(&self) -> Range<i64> {
        if self.hi <= self.lo {
            return 0..0;
        }
        self.lo.div_euclid(8)..(self.hi - 1).div_euclid(8) + 1
    }

    /// The cell along the value axis holding the free end; the baseline's
    /// for an empty segment.
    pub fn end_cell(&self) -> i64 {
        match (self.hi > self.lo, self.up) {
            (true, true) => (self.hi - 1).div_euclid(8),
            (true, false) => self.lo.div_euclid(8),
            (false, _) => self.lo.div_euclid(8),
        }
    }
}

/// The plot, its axes and its bars, as a frame lays them out.
#[derive(Clone, Debug)]
pub(super) struct Geom {
    /// The layout: title, legend, axes, the pane and the annotations' marks.
    pub layout: Layout,
    /// The plot's cells.
    pub plot: Rect,
    /// Bars run left to right from a category axis on the left.
    pub horizontal: bool,
    /// The categories drawn, None when none is.
    pub cats: Option<Cats>,
    /// The value axis.
    pub value: ValueAxis,
    /// The shown series, in order.
    pub shown: Vec<usize>,
    /// The bars: drawn category `k`'s shown series `j` at `k × shown + j`.
    pub segs: Vec<Seg>,
    /// Each legend entry's cells, by series.
    pub legend: Vec<(usize, Rect)>,
    /// The title, when it is drawn.
    pub title: Option<String>,
    /// Up and down colours by sign, for a signed single series
    /// (REQ-THEME-060).
    pub signed: bool,
}

impl Geom {
    /// The cell mapping of the category axis over its cells, two dots a
    /// cell, in category indices.
    pub fn cat_axis(&self) -> Option<Axis> {
        let c = self.cats?;
        let [a, b] = c.domain();
        Some(Axis::x(a, b, c.band.cells, 2))
    }

    /// The segment of drawn category `k`'s shown series `j`.
    pub fn seg(&self, k: usize, j: usize) -> Option<&Seg> {
        self.segs.get(k * self.shown.len() + j)
    }

    /// The plot cell `(col, row)` of a cell `cross` across and `along`
    /// along the value axis (from its start).
    pub fn cell(&self, cross: usize, along: i64) -> Option<(usize, usize)> {
        let p = self.plot;
        let along = usize::try_from(along).ok()?;
        if self.horizontal {
            (cross < p.h && along < p.w).then(|| (p.x + along, p.y + cross))
        } else {
            (cross < p.w && along < p.h).then(|| (p.x + cross, p.y + p.h - 1 - along))
        }
    }
}

/// What the geometry is made from besides the data.
pub(super) struct GeomInput<'a> {
    /// The grid's width.
    pub cols: usize,
    /// The grid's height.
    pub rows: usize,
    /// The chart's minimum size with axes, below which it sheds its title
    /// and legend.
    pub min: Size,
    /// The options as read.
    pub cfg: &'a Config,
    /// The requested x domain in category indices, null for every one.
    pub x: Option<[f64; 2]>,
    /// The requested value domain, null to fit.
    pub y: Option<[f64; 2]>,
    /// The series shown, in order.
    pub shown: Vec<usize>,
    /// The labels' names and symbols.
    pub locale: &'a Locale,
}

/// Stacks one direction of a category's values: `ends` are each nonzero
/// segment's series and exact end in eighths, cumulative from `base`, moving
/// away from it (up the axis). Inner boundaries go to the nearest whole cell
/// no further out than the last full cell; the stack's end stays exact. Gives
/// each series' `lo..hi`.
fn stack_up(base: i64, ends: &[(usize, i64)]) -> Vec<(usize, i64, i64)> {
    let Some(&(_, top)) = ends.last() else { return Vec::new() };
    let full = top.div_euclid(8) * 8;
    let mut prev = base;
    let mut out = Vec::with_capacity(ends.len());
    for (idx, &(s, e)) in ends.iter().enumerate() {
        let end = if idx + 1 == ends.len() { top } else { ((e + 4).div_euclid(8) * 8).min(full).max(prev) };
        out.push((s, prev, end.max(prev)));
        prev = end.max(prev);
    }
    out
}

/// The value extent of the shown series over categories `range`, as
/// displayed, the baseline included: of each value when not stacked, else
/// of each category's sums above and below the baseline. None when no value
/// is finite.
pub(super) fn extent(data: &Prepared, shown: &[usize], range: Range<usize>, stack: bool, baseline: f64) -> Option<[f64; 2]> {
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    let mut any = false;
    let columns: Vec<(&[f64], &crate::format::Display)> = shown.iter().map(|&s| (data.values(s), &data.series[s].display)).collect();
    for i in range {
        let (mut up, mut down) = (baseline, baseline);
        for &(values, display) in &columns {
            let v = display.shown(values.get(i).copied().unwrap_or(f64::NAN));
            if !v.is_finite() {
                continue;
            }
            any = true;
            if stack {
                if v > 0.0 {
                    up += v;
                } else {
                    down += v;
                }
            } else {
                lo = lo.min(v);
                hi = hi.max(v);
            }
        }
        if stack {
            for v in [up, down] {
                if v.is_finite() {
                    lo = lo.min(v);
                    hi = hi.max(v);
                }
            }
        }
    }
    any.then(|| [lo.min(baseline), hi.max(baseline)])
}

/// A domain of some span about `[lo, hi]`: one unit up when it has none.
fn spread([lo, hi]: [f64; 2]) -> [f64; 2] {
    if hi > lo && (hi - lo).is_finite() {
        [lo, hi]
    } else if lo.is_finite() {
        [lo, lo + lo.abs().max(1.0)]
    } else {
        [0.0, 1.0]
    }
}

/// A y axis's ticks over `rows` rows: snapped to rows under 20 rows when the
/// domain is fitted (REQ-TICK-050), else d3's fitted to the rows; with the
/// domain they label.
fn y_ticks(ext: [f64; 2], fixed: Option<[f64; 2]>, rows: usize, locale: &Locale) -> ([f64; 2], Vec<Tick>) {
    let format = AxisFormat::default_for(Kind::Linear);
    let count = (rows / Y_ROWS_PER_TICK).max(1) as f64;
    if let Some(d) = fixed {
        return (d, fit_ticks(Side::Left, Kind::Linear, d, rows, &format, locale));
    }
    let [lo, hi] = spread(ext);
    if Snap::resolve(None, rows, true) == Snap::Rows {
        if let Some(rs) = snap_rows(lo, hi, rows) {
            let f = rs.tick_format(count, &Specifier::default(), locale);
            let ticks = rs
                .tick_rows(count)
                .into_iter()
                .map(|r| {
                    let value = rs.row_value(r);
                    let text = f.format(value);
                    let width = text::width(&text);
                    Tick { value, cell: r, dot: r * Y_DOTS + Y_DOTS / 2, label: Some(TickLabel { text, start: r, width }) }
                })
                .collect();
            return (rs.domain, ticks);
        }
    }
    let d = nice(lo, hi, count);
    let d = if d[1] > d[0] { d } else { [lo, hi] };
    (d, fit_ticks(Side::Left, Kind::Linear, d, rows, &format, locale))
}

/// The category axis's ticks: a mark and a label at the middle of every
/// `k`th category drawn, `k` the least of 1, 2, 5, 10, … that gives each
/// label [`MIN_LABEL`] cells (or its whole width, when less) and `gap`
/// cells or rows between labels; each label truncated with `…` to the cells
/// it is given, at most `room` (the gutter, for a left axis).
fn cat_ticks(cats: &Cats, labels: &[String], side: Side, room: usize) -> Vec<Tick> {
    let per = cats.band.per;
    let len = cats.band.cells;
    let horizontal = side != Side::Bottom;
    let widest = labels.iter().map(|l| text::width(l)).max().unwrap_or(0);
    let want = if horizontal { 1 } else { widest.clamp(1, MIN_LABEL) };
    let gap = if horizontal { Y_GAP } else { X_GAP };
    let mut k = 1usize;
    let mut m = 1usize;
    while k * per < want + gap && k * per < len {
        k = [1, 2, 5][m % 3] * 10usize.pow((m / 3) as u32);
        m += 1;
    }
    let avail = if horizontal { room } else { (k * per).saturating_sub(gap).min(len).min(room) };
    let mut out = Vec::new();
    for (kk, label) in labels.iter().enumerate() {
        let i = cats.first + kk;
        if !i.is_multiple_of(k) {
            continue;
        }
        let cell = cats.middle(kk);
        // A bottom label's window: `avail` cells about its middle, cut to
        // the axis, so neighbours `k` bands apart stay `gap` cells apart.
        let (lo, hi) = if horizontal {
            (cell, cell)
        } else {
            let left = avail.saturating_sub(1) / 2;
            (cell.saturating_sub(left), (cell + avail.saturating_sub(1) - left).min(len.saturating_sub(1)))
        };
        let shown = text::truncate(label, if horizontal { avail } else { hi + 1 - lo });
        let width = text::width(&shown);
        let label = (width > 0).then(|| {
            let start = if horizontal { cell } else { cell.saturating_sub(width.saturating_sub(1) / 2).clamp(lo, (hi + 1).saturating_sub(width).max(lo)) };
            TickLabel { text: shown, start, width }
        });
        let dot = if horizontal { cell * Y_DOTS + Y_DOTS / 2 } else { cell * 2 };
        out.push(Tick { value: i as f64, cell, dot, label });
    }
    out
}

/// Drops every tick's label, keeping the marks.
fn strip(ticks: &mut [Tick]) {
    for t in ticks {
        t.label = None;
    }
}

/// The geometry of a frame `input.cols × input.rows` (REQ-LAYOUT-010 to
/// 030), laid out from the outside in as `layout::layout` lays a chart out:
/// the title, the legend, the bottom axis, the pane, the left gutter, the
/// bottom axis's ticks and the annotations. `ext` gives the value extent of
/// the shown series over a range of categories.
pub(super) fn make(input: &GeomInput, data: &Prepared, ext: &dyn Fn(Range<usize>) -> Option<[f64; 2]>) -> Geom {
    let GeomInput { cols, rows, min, cfg, x, y, ref shown, locale } = *input;
    let n = data.len();
    let m = shown.len();
    let decorated = cols >= MIN_COLS && rows >= MIN_ROWS;
    let shed = cols < usize::from(min.cols) || rows < usize::from(min.rows);
    // 1. The title.
    let title = (decorated && !shed && rows >= TITLE_ROWS && !cfg.title.is_empty()).then(|| cfg.title.clone());
    let title_rect = title.as_ref().map(|_| Rect::new(0, 0, cols, 1));
    let mut top = usize::from(title.is_some());
    // 2. The legend.
    let entries: Vec<String> =
        if !shed && cfg.legend != LegendPosition::None && data.series.len() >= 2 { data.series.iter().map(|s| s.name.clone()).collect() } else { Vec::new() };
    let legend_w = legend_width(&entries);
    let wanted = decorated && legend_w > 0;
    let outside = wanted
        && match cfg.legend {
            LegendPosition::Auto => rows >= LEGEND_ROWS && legend_w <= cols,
            LegendPosition::Outside => true,
            LegendPosition::Inside | LegendPosition::None => false,
        };
    let mut legend = outside.then(|| Rect::new(0, top, legend_w.min(cols), 1));
    top += usize::from(outside);
    // 3. The bottom axis.
    let x_rows = match (decorated, rows >= X_LABEL_ROWS) {
        (false, _) => 0,
        (true, true) => 2,
        (true, false) => 1,
    };
    let bottom = rows.saturating_sub(x_rows).max(top);
    // 4. The pane.
    let h = bottom - top;
    let groups = if !cfg.stack && m > 1 { m } else { 1 };
    let range = visible(n, x).map_or(0..0, |(a, b)| a..b + 1);
    let extent = ext(range.clone()).unwrap_or([cfg.baseline, cfg.baseline]);
    let labels_of = |c: &Cats| -> Vec<String> { (0..c.count()).map(|k| data.label(c.first + k, locale)).collect() };
    let (plot, cats, value, mut x_ticks, mut y_ticks_, x_domain, left_domain);
    if cfg.horizontal {
        // 5. The left gutter holds the categories, at most a third of the
        // columns (REQ-LAYOUT-030).
        cats = Cats::new(n, x, h, groups, cfg.padding);
        let labels = cats.as_ref().map(labels_of).unwrap_or_default();
        let room = if decorated && h >= PANE_ROWS { (cols / 3).saturating_sub(1).min(labels.iter().map(|l| text::width(l)).max().unwrap_or(0)) } else { 0 };
        let gutter = if decorated { room + 1 } else { 0 };
        plot = Rect::new(gutter, top, cols.saturating_sub(gutter), h);
        // 6. The bottom axis holds the values.
        let count = (plot.w / X_CELLS_PER_TICK).max(1) as f64;
        let format = AxisFormat::default_for(Kind::Linear);
        let d = match y {
            Some(d) => d,
            None => {
                let [lo, hi] = spread(extent);
                let d = nice(lo, hi, count);
                if d[1] > d[0] {
                    d
                } else {
                    [lo, hi]
                }
            }
        };
        value = ValueAxis { domain: d, cells: plot.w };
        x_ticks = if x_rows > 0 { fit_ticks(Side::Bottom, Kind::Linear, d, plot.w, &format, locale) } else { Vec::new() };
        y_ticks_ = match (&cats, gutter > 0) {
            (Some(c), true) => cat_ticks(c, &labels, Side::Left, room),
            _ => Vec::new(),
        };
        x_domain = d;
        left_domain = None;
    } else {
        // 5. The left gutter holds the values.
        let (d, mut ticks) = if decorated && h > 0 { y_ticks(extent, y, h, locale) } else { (y.unwrap_or_else(|| spread(extent)), Vec::new()) };
        if h < PANE_ROWS {
            strip(&mut ticks);
        }
        let widest = ticks.iter().filter_map(|t| t.label.as_ref()).map(|l| l.width).max().unwrap_or(0);
        let mut gutter = if decorated && h > 0 { widest + 1 } else { 0 };
        let notes = annotate::gutter_labels(&cfg.annotations, &[[Some("y"), None]], &[[gutter > 0, false]]);
        if gutter > 0 && notes[0] > 0 {
            gutter = gutter.max(notes[0] + 1);
        }
        if 3 * gutter > cols {
            strip(&mut ticks);
            gutter = usize::from(gutter > 0);
        }
        plot = Rect::new(gutter, top, cols.saturating_sub(gutter), h);
        value = ValueAxis { domain: d, cells: h };
        // 6. The bottom axis holds the categories.
        cats = Cats::new(n, x, plot.w, groups, cfg.padding);
        let labels = cats.as_ref().map(labels_of).unwrap_or_default();
        x_ticks = match (&cats, x_rows > 0) {
            (Some(c), true) => cat_ticks(c, &labels, Side::Bottom, usize::MAX),
            _ => Vec::new(),
        };
        y_ticks_ = ticks;
        x_domain = cats.as_ref().map_or([-0.5, n as f64 - 0.5], Cats::domain);
        left_domain = Some(d);
    }
    if x_rows < 2 {
        strip(&mut x_ticks);
    }
    if !decorated {
        strip(&mut y_ticks_);
    }
    let left = (plot.x > 0).then(|| PlacedAxis {
        side: Side::Left,
        rect: Rect::new(0, top, plot.x, h),
        kind: Kind::Linear,
        domain: left_domain.unwrap_or(x_domain),
        ticks: y_ticks_,
        context: Vec::new(),
    });
    let keep = left.as_ref().is_some_and(|a| a.ticks.iter().any(|t| t.label.is_some()));
    let x_axis = (x_rows > 0 && plot.w > 0).then(|| PlacedAxis {
        side: Side::Bottom,
        rect: Rect::new(plot.x, bottom, plot.w, x_rows),
        kind: Kind::Linear,
        domain: x_domain,
        ticks: x_ticks,
        context: Vec::new(),
    });
    // 7. A legend with no row of its own goes inside the pane when its
    // position allows (REQ-LAYOUT-060); then the annotations.
    if wanted && !outside {
        let inside = match cfg.legend {
            LegendPosition::Auto => 3 * legend_w <= plot.w && 3 <= plot.h,
            LegendPosition::Inside => !plot.is_empty(),
            LegendPosition::Outside | LegendPosition::None => false,
        };
        if inside {
            legend = Some(Rect::new(plot.x, plot.y, legend_w.min(plot.w), 1));
        }
    }
    let pane = PaneLayout {
        plot,
        left_domain,
        right_domain: None,
        left_name: Some(if cfg.horizontal { "x" } else { "y" }.to_string()),
        right_name: None,
        left,
        right: None,
        grid: None,
    };
    let mut layout = Layout {
        cols,
        rows,
        decorated,
        title: title_rect,
        legend,
        x_name: if cfg.horizontal { "y" } else { "x" }.to_string(),
        x_domain,
        x_axis,
        x_grid: false,
        panes: vec![pane],
        marks: Vec::new(),
    };
    layout.marks = annotate::place(&mut layout, &cfg.annotations, [keep, false]);
    let spans = legend.map(|r| legend_spans(r, &entries).into_iter().enumerate().collect()).unwrap_or_default();
    let signed = match cfg.signed {
        super::Signed::On => true,
        super::Signed::Off => false,
        // One series in the data, not one shown: hiding series leaves the
        // rest drawn as their legend keys say (rule 11).
        super::Signed::Auto => data.series.len() == 1 && m == 1 && data.series[shown[0]].least < cfg.baseline,
    };
    let segs = cats.as_ref().map(|c| bars(data, c, &value, shown, cfg)).unwrap_or_default();
    Geom { layout, plot, horizontal: cfg.horizontal, cats, value, shown: shown.clone(), segs, legend: spans, title, signed }
}

/// Every drawn category's bars, `k × shown + j` (REQ-BAR-010).
fn bars(data: &Prepared, cats: &Cats, value: &ValueAxis, shown: &[usize], cfg: &Config) -> Vec<Seg> {
    let m = shown.len();
    let base = value.eighth(cfg.baseline).unwrap_or(0);
    let top = value.eighths();
    let mut out = Vec::with_capacity(cats.count() * m);
    let mut ups: Vec<(usize, i64)> = Vec::with_capacity(m);
    let mut downs: Vec<(usize, i64)> = Vec::with_capacity(m);
    for k in 0..cats.count() {
        let i = cats.first + k;
        let at = out.len();
        if cfg.stack {
            // Each direction cumulative from the baseline (REQ-BAR-010).
            ups.clear();
            downs.clear();
            let (mut up, mut down) = (cfg.baseline, cfg.baseline);
            for (j, &s) in shown.iter().enumerate() {
                out.push(Seg { series: s, cross: cats.cross(k, 0), lo: base, hi: base, up: true });
                let v = data.shown(s, i);
                if !v.is_finite() || v == 0.0 {
                    continue;
                }
                if v > 0.0 {
                    up += v;
                    let e = value.eighth(up).unwrap_or(top);
                    ups.push((j, e));
                } else {
                    down += v;
                    let e = value.eighth(down).unwrap_or(0);
                    downs.push((j, -e));
                }
            }
            if let Some(last) = ups.last_mut() {
                if last.1 == base && base < top {
                    last.1 = base + 1;
                }
            }
            for (j, lo, hi) in stack_up(base, &ups) {
                out[at + j].lo = lo;
                out[at + j].hi = hi;
            }
            if let Some(last) = downs.last_mut() {
                if last.1 == -base && base > 0 {
                    last.1 = -base + 1;
                }
            }
            for (j, lo, hi) in stack_up(-base, &downs) {
                let seg = &mut out[at + j];
                seg.lo = -hi;
                seg.hi = -lo;
                seg.up = false;
            }
        } else {
            for (j, &s) in shown.iter().enumerate() {
                let cross = cats.cross(k, if cats.groups > 1 { j } else { 0 });
                let v = data.shown(s, i);
                let up = v >= cfg.baseline || v.is_nan();
                let seg = match value.eighth(v) {
                    None => Seg { series: s, cross, lo: base, hi: base, up },
                    Some(mut e) => {
                        // A value off the baseline that rounds onto it takes
                        // one eighth, so no bar vanishes.
                        if e == base && v != cfg.baseline {
                            e = if up && base < top {
                                base + 1
                            } else if !up && base > 0 {
                                base - 1
                            } else {
                                base
                            };
                        }
                        Seg { series: s, cross, lo: e.min(base), hi: e.max(base), up }
                    }
                };
                out.push(seg);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// REQ-SCALE-040: every drawn category has the same whole cells, the
    /// rest split at the ends, a gap in every band of two cells or more.
    #[test]
    fn bands_are_equal_whole_cells() {
        let c = Cats::new(12, None, 23, 1, 0.2).unwrap();
        assert_eq!((c.band.per, c.band.start, c.band.inner, c.count()), (1, 5, 1, 12));
        let c = Cats::new(12, None, 24, 1, 0.2).unwrap();
        assert_eq!((c.band.per, c.band.inner), (2, 1), "a cell a bar, a cell between");
        let c = Cats::new(12, None, 73, 3, 0.2).unwrap();
        assert_eq!((c.band.per, c.sub, c.groups), (6, 1, 3));
        assert!(Cats::new(12, None, 2, 3, 0.2).is_none(), "no group fits");
        let c = Cats::new(1000, None, 80, 1, 0.2).unwrap();
        assert_eq!((c.first, c.count()), (0, 80), "at most a cell a bar");
        let c = Cats::new(12, Some([3.1, 6.1]), 80, 1, 0.2).unwrap();
        assert_eq!((c.first, c.count()), (4, 3));
        let c = Cats::new(12, Some([3.2, 3.4]), 80, 1, 0.2).unwrap();
        assert_eq!((c.first, c.count()), (3, 1), "the nearest when none lies inside");
    }

    /// The drawn domain puts each category at its middle drawn cell.
    #[test]
    fn domain_maps_each_category_to_its_middle_cell() {
        for (cells, n, groups) in [(73, 12, 1), (24, 12, 1), (23, 12, 1), (80, 7, 3), (5, 5, 1), (317, 12, 2)] {
            let c = Cats::new(n, None, cells, groups, 0.2).unwrap();
            let [a, b] = c.domain();
            let axis = Axis::x(a, b, cells, 2);
            for k in 0..c.count() {
                assert_eq!(axis.cell_in((c.first + k) as f64), Some(c.middle(k)), "{cells} cells, {n} categories, category {k}");
                let bars = c.cross(k, 0).start..c.cross(k, c.groups - 1).end;
                assert!(bars.contains(&c.middle(k)));
                for cell in c.band.slot(k).unwrap() {
                    assert_eq!(c.at(cell), k);
                }
            }
        }
    }

    /// REQ-BAR-010: stacked segments meet on whole cells; the top is exact.
    #[test]
    fn stacks_meet_on_whole_cells() {
        assert_eq!(stack_up(0, &[(0, 13), (1, 30)]), vec![(0, 0, 16), (1, 16, 30)]);
        assert_eq!(stack_up(4, &[(0, 6), (1, 7)]), vec![(0, 4, 4), (1, 4, 7)], "inside one cell the last owns it");
        assert_eq!(stack_up(0, &[(0, 20), (2, 21)]), vec![(0, 0, 16), (2, 16, 21)]);
        assert!(stack_up(0, &[]).is_empty());
    }
}
