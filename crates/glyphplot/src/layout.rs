//! Layout in whole cells, from the outside in (REQ-LAYOUT-010 to 060,
//! REQ-AXIS-010).
//!
//! [`layout`] places, in this order: the title, the legend, the x axis, the
//! panes (split by weight, at least three rows each), the y gutters (one
//! width across every pane, so plots line up), the x ticks, and last the
//! annotations, which draw over their pane's plot and take no rows of their
//! own. It is a pure function of the grid's size, a [`LayoutInput`] and the
//! [`LayoutExtents`] (the data's extent and the view), so the same inputs give
//! the same layout on every surface (REQ-LAYOUT-010).
//!
//! Panes stack top to bottom and share the x scale: one x domain, one x axis
//! under the last pane, and every plot over the same columns
//! (REQ-LAYOUT-050). A pane has up to two y scales, one a side, each with its
//! own domain and ticks, and gridlines from one of them only
//! (REQ-AXIS-010). The `panes` and `axes` options bind scales to panes and
//! sides through [`bind_axes`].
//!
//! Ticks are fitted by lowering the count (ADR-111, REQ-TICK-030):
//! [`fit_ticks`] starts at one tick per four cells on x and one per two rows on
//! y, and lowers the count until every label fits whole inside the axis with a
//! gap of two cells on x and one row on y. Labels are `scale.rs`'s tick labels,
//! measured in cells by `unicode-width` (REQ-I18N-040). Layout reads a scale
//! only through [`Kind`], `scale::resolve_domain` and [`Scale`]'s ticks and
//! labels, so a new kind of scale is laid out once `scale.rs` ticks and
//! labels it.

use serde::Deserialize;

use crate::annotate::{self, Annotation, Mark};
use crate::cells::Axis;
use crate::format::Locale;
use crate::scale::{resolve_domain, AxisFormat, Domain, DomainError, Extents, Kind, Scale, ScaleOptions, ScaleType};
use crate::text;
use crate::ticks::mark;

/// Below this many columns a chart draws no axis, title or legend
/// (REQ-LAYOUT-020).
pub const MIN_COLS: usize = 16;
/// Below this many rows a chart draws no axis, title or legend
/// (REQ-LAYOUT-020).
pub const MIN_ROWS: usize = 5;
/// The rows a chart needs to draw its title (REQ-LAYOUT-020).
pub const TITLE_ROWS: usize = 10;
/// The rows a chart needs to draw x labels (REQ-LAYOUT-020).
pub const X_LABEL_ROWS: usize = 6;
/// The rows a chart needs to draw its legend in a row of its own
/// (REQ-LAYOUT-060).
pub const LEGEND_ROWS: usize = 12;
/// The rows a chart needs for a time axis's context row under the x labels
/// (REQ-TICK-070): a placeholder until the context row is built, chosen with
/// the title's threshold, not measured.
pub const CONTEXT_ROWS: usize = 10;
/// The rows a pane keeps when the rows allow it, and needs for y labels
/// (REQ-LAYOUT-050).
pub const PANE_ROWS: usize = 3;
/// The blank cells between two x labels (ADR-111, REQ-TICK-030).
pub const X_GAP: usize = 2;
/// The blank rows between two y labels (ADR-111, REQ-TICK-030).
pub const Y_GAP: usize = 1;
/// The cells a tick at the x fitting's starting count (ADR-111).
pub const X_CELLS_PER_TICK: usize = 4;
/// The rows a tick at the y fitting's starting count (ADR-111).
pub const Y_ROWS_PER_TICK: usize = 2;
/// The braille dots across a cell, which x gridlines are placed by
/// (REQ-TICK-040).
pub const X_DOTS: usize = 2;
/// The braille dots down a cell, which y gridlines are placed by
/// (REQ-TICK-040).
pub const Y_DOTS: usize = 4;
/// The cells of a legend entry's key, `■ `.
const LEGEND_KEY: usize = 2;
/// The blank cells between two legend entries.
const LEGEND_SPACING: usize = 2;

/// A rectangle of whole cells: `w × h` from column `x`, row `y`
/// (REQ-LAYOUT-010).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Rect {
    /// The first column.
    pub x: usize,
    /// The first row.
    pub y: usize,
    /// The width in cells.
    pub w: usize,
    /// The height in cells.
    pub h: usize,
}

impl Rect {
    /// The rectangle `w × h` from `(x, y)`.
    pub const fn new(x: usize, y: usize, w: usize, h: usize) -> Rect {
        Rect { x, y, w, h }
    }

    /// Whether it holds no cell.
    pub const fn is_empty(&self) -> bool {
        self.w == 0 || self.h == 0
    }

    /// Whether it and `other` share a cell.
    pub const fn overlaps(&self, other: &Rect) -> bool {
        !self.is_empty()
            && !other.is_empty()
            && self.x < other.x + other.w
            && other.x < self.x + self.w
            && self.y < other.y + other.h
            && other.y < self.y + self.h
    }
}

/// The side of a pane an axis is drawn on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Side {
    /// A y axis in the left gutter.
    Left,
    /// A y axis in the right gutter.
    Right,
    /// The x axis under the panes.
    Bottom,
}

/// A tick's label, placed along its axis (REQ-TICK-030).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TickLabel {
    /// The label, as the axis's format writes it.
    pub text: String,
    /// The first cell along the axis it takes: on x the column from the
    /// axis's left, on y the row from the axis's top.
    pub start: usize,
    /// The cells across it takes, by `unicode-width` (REQ-I18N-040).
    pub width: usize,
}

/// A tick placed on an axis (REQ-TICK-030, REQ-TICK-040).
#[derive(Clone, Debug, PartialEq)]
pub struct Tick {
    /// The value it marks, in data units (epoch ms for time).
    pub value: f64,
    /// The cell of its mark, from the axis's left (x) or top (y): the cell
    /// nearest its position (REQ-TICK-040).
    pub cell: usize,
    /// The braille dot of its gridline, from the same end: [`X_DOTS`] a cell
    /// on x, [`Y_DOTS`] on y (REQ-TICK-040).
    pub dot: usize,
    /// Its label, or None when the axis shows marks only.
    pub label: Option<TickLabel>,
}

/// The ticks of an axis `len` cells long over `domain`, fitted by lowering
/// the count (ADR-111, REQ-TICK-030). The count starts at one tick per
/// [`X_CELLS_PER_TICK`] cells on x, one per [`Y_ROWS_PER_TICK`] rows on y, and
/// falls by one until every label fits: on x each label is centred on its
/// tick's cell, pushed inside the axis at the ends, and kept [`X_GAP`] cells
/// from the next; on y each label sits on its tick's row with [`Y_GAP`] blank
/// rows to the next. Labels are the scale's tick labels and their widths
/// count cells by `unicode-width` (REQ-I18N-040). When no count fits, the
/// ticks at the lowest count keep their marks and have no labels; there is no
/// pass that hides some labels.
///
/// Ticks come in increasing value and lie inside the domain; their cells
/// never decrease on x and never increase on y, as rows count down. A domain
/// given high to low is fitted as its low-to-high form. No ticks for an axis
/// of no cells or a domain that is not finite.
pub fn fit_ticks(side: Side, kind: Kind, domain: [f64; 2], len: usize, format: &AxisFormat, locale: &Locale) -> Vec<Tick> {
    let (lo, hi) = (domain[0].min(domain[1]), domain[0].max(domain[1]));
    if len == 0 || !(lo.is_finite() && hi.is_finite()) {
        return Vec::new();
    }
    let x = side == Side::Bottom;
    let (per_tick, gap) = if x { (X_CELLS_PER_TICK, X_GAP) } else { (Y_ROWS_PER_TICK, Y_GAP) };
    let axis = if x { Axis::x(lo, hi, len, X_DOTS) } else { Axis::y(lo, hi, len, Y_DOTS) };
    let range = [0.0, len as f64];
    // The scale of `kind` over the axis: its ticks and labels are scale.rs's,
    // whatever the kind.
    let scale = Scale { kind, ..Scale::linear([lo, hi], range) };
    let mut fallback = Vec::new();
    let mut tried: Vec<f64> = Vec::new();
    for count in (1..=(len / per_tick).max(1)).rev() {
        // A count that gives the ticks the last one gave gives its labels
        // and its fit too (the format's precision comes from the same step):
        // skip it rather than format them again.
        let values = scale.ticks(count as f64);
        if !tried.is_empty() && values == tried {
            continue;
        }
        tried = values;
        let ticks = place(&scale, &axis, count as f64, format, locale, x);
        if ticks.is_empty() {
            continue;
        }
        if fits(&ticks, x, len, gap) {
            return ticks;
        }
        fallback = ticks;
    }
    strip(&mut fallback);
    fallback
}

/// The scale's ticks at `count`, each with its mark and its label placed.
fn place(scale: &Scale, axis: &Axis, count: f64, format: &AxisFormat, locale: &Locale, x: bool) -> Vec<Tick> {
    let len = axis.cells();
    let mut out: Vec<Tick> = Vec::new();
    for (value, text) in scale.tick_labels(count, format, locale) {
        if out.last().is_some_and(|t| value <= t.value) {
            continue;
        }
        let Some((cell, dot)) = mark(axis, value) else { continue };
        let width = text::width(&text);
        let start = if x { cell.saturating_sub(width.saturating_sub(1) / 2).min(len.saturating_sub(width)) } else { cell };
        out.push(Tick { value, cell, dot, label: Some(TickLabel { text, start, width }) });
    }
    out
}

/// Whether every label lies inside the axis, the gap apart.
fn fits(ticks: &[Tick], x: bool, len: usize, gap: usize) -> bool {
    let mut prev: Option<&TickLabel> = None;
    for l in ticks.iter().filter_map(|t| t.label.as_ref()) {
        if (x && l.start + l.width > len) || (!x && l.start >= len) {
            return false;
        }
        if let Some(p) = prev {
            let apart = if x { l.start >= p.start + p.width + gap } else { p.start > l.start + gap };
            if !apart {
                return false;
            }
        }
        prev = Some(l);
    }
    true
}

/// Drops every tick's label, keeping the marks.
fn strip(ticks: &mut [Tick]) {
    for t in ticks {
        t.label = None;
    }
}

/// The labels of a time axis's context row (REQ-TICK-070): the date at each
/// day boundary of `domain` when it spans under two days, placed along an
/// axis of `len` cells.
///
/// SEAM (REQ-TICK-070): the context row is built with the date formats in
/// `ticks.rs`, `format.rs` and `time.rs`; until then it has no labels.
/// Layout already gives the row (step 3) when the x axis asks for it
/// ([`AxisInput::context`]) and the rows allow ([`CONTEXT_ROWS`]), and
/// `text::Frame::axis_labels` draws whatever this returns in the axis's third
/// row, so filling this function in is the whole change.
fn context_labels(_kind: Kind, _domain: [f64; 2], _len: usize, _locale: &Locale) -> Vec<TickLabel> {
    Vec::new()
}

/// One axis's scale and format, as the options give them.
#[derive(Clone, Debug, PartialEq)]
pub struct AxisInput {
    /// The scale's name, which annotations and the view name (`x` for the x
    /// scale); empty for a scale no option names.
    pub name: String,
    /// How the axis is ticked and labelled.
    pub kind: Kind,
    /// The scale's domain options (`domain`, `nice`, `zero`, `padding`),
    /// resolved by `scale::resolve_domain`; its `type` is not read.
    pub scale: ScaleOptions,
    /// The labels' format.
    pub format: AxisFormat,
    /// Whether the axis is drawn; its scale is resolved either way.
    pub shown: bool,
    /// Whether gridlines come from its ticks: on x across every pane, on y
    /// across its pane, from one y axis a pane only (REQ-AXIS-010).
    pub grid: bool,
    /// For the x axis of a time scale: a context row under the labels
    /// (REQ-TICK-070, see [`CONTEXT_ROWS`]).
    pub context: bool,
}

impl AxisInput {
    /// A shown, unnamed axis of `kind` over the data's extent, unniced, with
    /// the automatic format and no gridlines.
    pub fn new(kind: Kind) -> AxisInput {
        // The option type is not read by layout; a time scale's is written
        // for the reader, any other kind's left linear.
        let kind_type = if kind == Kind::Time { ScaleType::Time } else { ScaleType::Linear };
        let format = if kind == Kind::Time { AxisFormat::Time(None) } else { AxisFormat::default() };
        let scale =
            ScaleOptions { kind: kind_type, domain: Domain::Data, nice: None, zero: None, padding: None, base: None, constant: None, zone: None, scheme: None };
        AxisInput { name: String::new(), kind, scale, format, shown: true, grid: false, context: false }
    }

    /// [`AxisInput::new`] with the scale's name.
    pub fn named(kind: Kind, name: &str) -> AxisInput {
        AxisInput { name: name.to_string(), ..AxisInput::new(kind) }
    }
}

/// One pane: a share of the rows and its y scales (REQ-LAYOUT-050,
/// REQ-AXIS-010).
#[derive(Clone, Debug, PartialEq)]
pub struct PaneInput {
    /// Its share of the rows; 0 counts as 1.
    pub weight: u32,
    /// The y scale drawn on the left.
    pub left: Option<AxisInput>,
    /// The y scale drawn on the right.
    pub right: Option<AxisInput>,
}

/// The `legend.position` option (REQ-LAYOUT-060).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum LegendPosition {
    /// Outside in one row when it fits and there are at least
    /// [`LEGEND_ROWS`] rows, else inside the first pane when it takes at most
    /// a third of its width and height, else left out.
    #[default]
    Auto,
    /// In a row of its own above the panes, entries past the width left out.
    Outside,
    /// Inside the first pane's top left, entries past its width left out.
    Inside,
    /// Not drawn.
    None,
}

impl LegendPosition {
    /// The option's value: `outside`, `inside`, `none`, anything else
    /// `auto`.
    pub fn from_option(value: &str) -> LegendPosition {
        match value {
            "outside" => LegendPosition::Outside,
            "inside" => LegendPosition::Inside,
            "none" => LegendPosition::None,
            _ => LegendPosition::Auto,
        }
    }
}

/// What a chart's layout depends on besides its size and its data's extent:
/// the part of the options layout reads (REQ-LAYOUT-010).
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutInput {
    /// The title.
    pub title: Option<String>,
    /// The legend's entries, one a series.
    pub legend: Vec<String>,
    /// Where the legend goes (REQ-LAYOUT-060).
    pub legend_position: LegendPosition,
    /// The x scale, shared by every pane and drawn at the bottom.
    pub x: AxisInput,
    /// The panes, top first.
    pub panes: Vec<PaneInput>,
    /// The annotations, placed last (REQ-ANNOT-010).
    pub annotations: Vec<Annotation>,
    /// The names and symbols labels are written with.
    pub locale: Locale,
}

impl LayoutInput {
    /// One pane with the y scale `y` on the left under the x scale `x`, no
    /// title, no legend and no annotations.
    pub fn new(x: AxisInput, y: AxisInput) -> LayoutInput {
        LayoutInput {
            title: None,
            legend: Vec::new(),
            legend_position: LegendPosition::Auto,
            x,
            panes: vec![PaneInput { weight: 1, left: Some(y), right: None }],
            annotations: Vec::new(),
            locale: Locale::default(),
        }
    }
}

/// The extents of one pane's y scales.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PaneExtents {
    /// The left scale's.
    pub left: Extents,
    /// The right scale's.
    pub right: Extents,
}

/// The data's extent and the view, a scale at a time (REQ-LAYOUT-010).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LayoutExtents {
    /// The x scale's.
    pub x: Extents,
    /// Each pane's, in the panes' order; a pane past the end has none.
    pub panes: Vec<PaneExtents>,
}

/// An axis as laid out: where it is drawn and its fitted ticks.
#[derive(Clone, Debug, PartialEq)]
pub struct PlacedAxis {
    /// Its side.
    pub side: Side,
    /// Its cells: for the x axis, the plot's columns over its line row, its
    /// label row and, when it has one, its context row; for a y axis, its
    /// gutter beside the pane, the line in the column next to the plot.
    pub rect: Rect,
    /// Its scale's kind.
    pub kind: Kind,
    /// Its scale's domain.
    pub domain: [f64; 2],
    /// Its ticks, cells counted from the rect's left (x) or top (y).
    pub ticks: Vec<Tick>,
    /// For the x axis, the context row's labels, placed as tick labels are
    /// (REQ-TICK-070); empty otherwise.
    pub context: Vec<TickLabel>,
}

/// A pane as laid out.
#[derive(Clone, Debug, PartialEq)]
pub struct PaneLayout {
    /// The plot, where the data is drawn; annotations are clipped to it.
    pub plot: Rect,
    /// The left y scale's domain, when the pane has one.
    pub left_domain: Option<[f64; 2]>,
    /// The right y scale's domain, when the pane has one.
    pub right_domain: Option<[f64; 2]>,
    /// The left y scale's name.
    pub left_name: Option<String>,
    /// The right y scale's name.
    pub right_name: Option<String>,
    /// The left axis, when it is drawn.
    pub left: Option<PlacedAxis>,
    /// The right axis, when it is drawn.
    pub right: Option<PlacedAxis>,
    /// The side whose ticks give the pane's gridlines: the left axis's when
    /// it asks for them, else the right's; never both (REQ-AXIS-010).
    pub grid: Option<Side>,
}

impl PaneLayout {
    /// The axis giving the pane's gridlines, when it is drawn
    /// (REQ-AXIS-010).
    pub fn grid_axis(&self) -> Option<&PlacedAxis> {
        match self.grid {
            Some(Side::Left) => self.left.as_ref(),
            Some(Side::Right) => self.right.as_ref(),
            _ => None,
        }
    }

    /// The cell mapping of the y scale on `side` over the plot's rows,
    /// [`Y_DOTS`] dots a cell, high values at the top; None for a side with
    /// no scale.
    pub fn y_mapping(&self, side: Side) -> Option<Axis> {
        let d = match side {
            Side::Left => self.left_domain,
            Side::Right => self.right_domain,
            Side::Bottom => None,
        }?;
        Some(Axis::y(d[0].min(d[1]), d[0].max(d[1]), self.plot.h, Y_DOTS))
    }
}

/// A chart's layout in cells (REQ-LAYOUT-010).
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    /// The grid's width.
    pub cols: usize,
    /// The grid's height.
    pub rows: usize,
    /// Whether the size allows axes, a title and a legend at all
    /// (REQ-LAYOUT-020).
    pub decorated: bool,
    /// The title's row.
    pub title: Option<Rect>,
    /// The legend's row, above the panes or inside the first pane's top
    /// left (REQ-LAYOUT-060); its entries are [`legend_spans`].
    pub legend: Option<Rect>,
    /// The x scale's name.
    pub x_name: String,
    /// The x scale's domain, shared by every pane (REQ-LAYOUT-050).
    pub x_domain: [f64; 2],
    /// The x axis, when it is drawn.
    pub x_axis: Option<PlacedAxis>,
    /// Whether the x axis's ticks give gridlines in every pane.
    pub x_grid: bool,
    /// The panes, top first.
    pub panes: Vec<PaneLayout>,
    /// The annotations' marks, in grid cells (REQ-ANNOT-010).
    pub marks: Vec<Mark>,
}

impl Layout {
    /// A layout of no parts: the whole grid one plot, with x and its y
    /// over [0, 1].
    pub fn empty(cols: usize, rows: usize) -> Layout {
        Layout {
            cols,
            rows,
            decorated: false,
            title: None,
            legend: None,
            x_name: String::new(),
            x_domain: [0.0, 1.0],
            x_axis: None,
            x_grid: false,
            panes: vec![PaneLayout {
                plot: Rect::new(0, 0, cols, rows),
                left_domain: Some([0.0, 1.0]),
                right_domain: None,
                left_name: None,
                right_name: None,
                left: None,
                right: None,
                grid: None,
            }],
            marks: Vec::new(),
        }
    }

    /// The cell mapping of the shared x scale over pane `pane`'s columns,
    /// [`X_DOTS`] dots a cell; the same for every pane (REQ-LAYOUT-050).
    pub fn x_mapping(&self, pane: usize) -> Option<Axis> {
        let p = self.panes.get(pane)?;
        let [a, b] = self.x_domain;
        Some(Axis::x(a.min(b), a.max(b), p.plot.w, X_DOTS))
    }
}

/// Which axis a layout error is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AxisId {
    /// The x axis.
    X,
    /// The left axis of the pane at this index.
    Left(usize),
    /// The right axis of the pane at this index.
    Right(usize),
}

/// A scale whose domain could not be resolved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LayoutError {
    /// The axis.
    pub axis: AxisId,
    /// Why.
    pub error: DomainError,
}

/// The cells a legend of `entries` takes in one row: `■ name` each, two cells
/// apart, widths by `unicode-width`.
pub fn legend_width(entries: &[String]) -> usize {
    let names: usize = entries.iter().map(|e| LEGEND_KEY + text::width(e)).sum();
    names + LEGEND_SPACING * entries.len().saturating_sub(1)
}

/// Each legend entry's cells in the legend's row `rect`, in order: the key's
/// cell, a blank and the name, two cells apart, an entry cut at the rect's
/// right edge and those past it left out (REQ-LAYOUT-060).
pub fn legend_spans(rect: Rect, entries: &[String]) -> Vec<Rect> {
    let end = rect.x + rect.w;
    let mut x = rect.x;
    let mut out = Vec::with_capacity(entries.len());
    for e in entries {
        if x >= end || rect.h == 0 {
            break;
        }
        let w = LEGEND_KEY + text::width(e);
        out.push(Rect::new(x, rect.y, w.min(end - x), 1));
        x += w + LEGEND_SPACING;
    }
    out
}

/// `total` split in proportion to `w` by largest remainder, ties to the
/// earlier pane.
fn shares(total: usize, w: &[u128]) -> Vec<usize> {
    let sum: u128 = w.iter().sum();
    if sum == 0 {
        return vec![0; w.len()];
    }
    let t = total as u128;
    let mut out: Vec<usize> = w.iter().map(|&wi| (t * wi / sum) as usize).collect();
    let left = total - out.iter().sum::<usize>();
    // The `left` largest remainders get one more each, the earlier pane on a
    // tie: picked one at a time rather than sorted, since a slice sort keyed
    // on u128 is about 10 KB of the wasm module and there are a few panes.
    let rem: Vec<u128> = w.iter().map(|&wi| t * wi % sum).collect();
    let mut given = vec![false; w.len()];
    for _ in 0..left {
        let best = (0..w.len()).filter(|&i| !given[i]).fold(None, |best: Option<usize>, i| if best.is_none_or(|b| rem[i] > rem[b]) { Some(i) } else { best });
        let Some(i) = best else { break };
        given[i] = true;
        out[i] += 1;
    }
    out
}

/// `total` split in proportion to `w` with every share at least `min`, for
/// `total ≥ min × w.len()`: a pane whose share falls short takes `min`, and
/// the rest is split among the others again.
fn shares_with_min(total: usize, w: &[u128], min: usize) -> Vec<usize> {
    let mut fixed = vec![false; w.len()];
    loop {
        let free: Vec<usize> = (0..w.len()).filter(|&i| !fixed[i]).collect();
        let rest = total - min * (w.len() - free.len());
        let s = shares(rest, &free.iter().map(|&i| w[i]).collect::<Vec<_>>());
        let short: Vec<usize> = free.iter().zip(&s).filter(|(_, &h)| h < min).map(|(&i, _)| i).collect();
        if short.is_empty() || short.len() == free.len() {
            let mut out = vec![min; w.len()];
            for (&i, &h) in free.iter().zip(&s) {
                out[i] = h.max(min);
            }
            return out;
        }
        for i in short {
            fixed[i] = true;
        }
    }
}

/// The panes' rows, `(first row, height)` from `top`: `total` split by weight,
/// each pane at least [`PANE_ROWS`] with a blank row between panes when the
/// rows allow both, else split by weight alone with no separators
/// (REQ-LAYOUT-050).
fn split_rows(top: usize, total: usize, weights: &[u32]) -> Vec<(usize, usize)> {
    let n = weights.len();
    if n == 0 {
        return Vec::new();
    }
    let w: Vec<u128> = weights.iter().map(|&w| u128::from(w.max(1))).collect();
    let sep = usize::from(total >= PANE_ROWS * n + (n - 1));
    let avail = total - sep * (n - 1);
    let heights = if sep == 1 { shares_with_min(avail, &w, PANE_ROWS) } else { shares(avail, &w) };
    let mut y = top;
    heights
        .into_iter()
        .map(|h| {
            let span = (y, h);
            y += h + sep;
            span
        })
        .collect()
}

/// The width of a gutter holding these axes: the widest label plus one cell
/// for the line, 0 for no axis.
fn gutter<'a>(axes: impl Iterator<Item = &'a PlacedAxis>) -> usize {
    let mut any = false;
    let mut widest = 0;
    for a in axes {
        any = true;
        widest = a.ticks.iter().filter_map(|t| t.label.as_ref()).map(|l| l.width).fold(widest, usize::max);
    }
    if any {
        widest + 1
    } else {
        0
    }
}

/// The layout of a `cols × rows` chart (REQ-LAYOUT-010 to 060,
/// REQ-AXIS-010, REQ-ANNOT-010), placed from the outside in:
///
/// 1. The title: the top row, when there is one and `rows ≥ 10`.
/// 2. The legend, by its position (REQ-LAYOUT-060). At `auto`, the next row
///    when it fits in one row and `rows ≥ 12`; otherwise, at step 7, the
///    first pane's top left when it takes at most a third of the plot's
///    width and height; otherwise none (the chart's table and events keep
///    its series). `outside` always takes the row, `inside` always goes in
///    the pane, `none` draws none.
/// 3. The x axis, when shown: two rows at the bottom (its line and its
///    labels), one row with no labels below 6 rows; a third row for its
///    context row when it asks for one and `rows ≥` [`CONTEXT_ROWS`].
/// 4. The panes: the rows left, by weight, at least three rows each, with a
///    blank row between panes when every pane keeps three (REQ-LAYOUT-050).
/// 5. The y gutters: each y scale resolved and its ticks fitted to its
///    pane's rows (no labels on a pane under three rows); the left gutter is
///    the widest left label of any pane, or `hline` annotation label in it,
///    plus the line's cell, the right likewise, so every plot spans the same
///    columns. When the gutters would take more than a third of the columns,
///    the right axis's labels drop, then the left's (REQ-LAYOUT-030); the
///    lines and marks stay. A pane's gridlines come from its left axis when
///    that asks for them, else its right's (REQ-AXIS-010).
/// 6. The x ticks, fitted to the columns between the gutters, one x domain
///    for every pane.
/// 7. The annotations, placed over the panes by `annotate::place`, hiding the
///    tick labels their axis labels overlap (REQ-ANNOT-010).
///
/// Below 16 columns or 5 rows there is no axis, title, legend or annotation,
/// and the panes take every cell (REQ-LAYOUT-020). Each scale's domain is
/// resolved by `scale::resolve_domain` from its options and extents, niced
/// (when asked) for the fitting's starting count. An error names the scale
/// whose domain does not resolve.
pub fn layout(cols: usize, rows: usize, input: &LayoutInput, extents: &LayoutExtents) -> Result<Layout, LayoutError> {
    let decorated = cols >= MIN_COLS && rows >= MIN_ROWS;
    let locale = &input.locale;

    // 1. The title.
    let title = (decorated && rows >= TITLE_ROWS && input.title.is_some()).then_some(Rect::new(0, 0, cols, 1));
    let mut top = usize::from(title.is_some());

    // 2. The legend.
    let legend_w = legend_width(&input.legend);
    let wanted = decorated && legend_w > 0;
    let outside = wanted
        && match input.legend_position {
            LegendPosition::Auto => rows >= LEGEND_ROWS && legend_w <= cols,
            LegendPosition::Outside => true,
            LegendPosition::Inside | LegendPosition::None => false,
        };
    let mut legend = outside.then_some(Rect::new(0, top, legend_w.min(cols), 1));
    top += usize::from(outside);

    // 3. The x axis.
    let mut x_rows = match (decorated && input.x.shown, rows >= X_LABEL_ROWS) {
        (false, _) => 0,
        (true, true) => 2,
        (true, false) => 1,
    };
    // SEAM (REQ-TICK-070): the context row, see `context_labels`.
    if x_rows == 2 && input.x.context && rows >= CONTEXT_ROWS {
        x_rows = 3;
    }
    let bottom = rows.saturating_sub(x_rows).max(top);

    // 4. The panes.
    let weights: Vec<u32> = input.panes.iter().map(|p| p.weight).collect();
    let spans = split_rows(top, bottom - top, &weights);

    // 5. The y gutters.
    let mut panes = Vec::with_capacity(input.panes.len());
    for (i, (pane, &(y, h))) in input.panes.iter().zip(&spans).enumerate() {
        let ext = extents.panes.get(i).copied().unwrap_or_default();
        let count = (h / Y_ROWS_PER_TICK).max(1) as f64;
        let resolve = |a: &AxisInput, e: &Extents, axis: AxisId| resolve_domain(&a.scale, a.kind, e, count).map_err(|error| LayoutError { axis, error });
        let left_domain = pane.left.as_ref().map(|a| resolve(a, &ext.left, AxisId::Left(i))).transpose()?;
        let right_domain = pane.right.as_ref().map(|a| resolve(a, &ext.right, AxisId::Right(i))).transpose()?;
        let fit = |a: &AxisInput, domain: [f64; 2], side: Side| {
            (decorated && a.shown && h > 0).then(|| {
                let mut ticks = fit_ticks(side, a.kind, domain, h, &a.format, locale);
                if h < PANE_ROWS {
                    strip(&mut ticks);
                }
                PlacedAxis { side, rect: Rect::new(0, y, 0, h), kind: a.kind, domain, ticks, context: Vec::new() }
            })
        };
        let left = pane.left.as_ref().zip(left_domain).and_then(|(a, d)| fit(a, d, Side::Left));
        let right = pane.right.as_ref().zip(right_domain).and_then(|(a, d)| fit(a, d, Side::Right));
        let grid = if pane.left.as_ref().is_some_and(|a| a.grid) {
            Some(Side::Left)
        } else if pane.right.as_ref().is_some_and(|a| a.grid) {
            Some(Side::Right)
        } else {
            None
        };
        panes.push(PaneLayout {
            plot: Rect::new(0, y, 0, h),
            left_domain,
            right_domain,
            left_name: pane.left.as_ref().map(|a| a.name.clone()),
            right_name: pane.right.as_ref().map(|a| a.name.clone()),
            left,
            right,
            grid,
        });
    }
    // An `hline`'s label goes in a gutter, which widens to hold it.
    let names: Vec<[Option<&str>; 2]> = panes.iter().map(|p| [p.left_name.as_deref(), p.right_name.as_deref()]).collect();
    let drawn: Vec<[bool; 2]> = panes.iter().map(|p| [p.left.is_some(), p.right.is_some()]).collect();
    let notes = annotate::gutter_labels(&input.annotations, &names, &drawn);
    let widen = |w: usize, note: usize| if w > 0 && note > 0 { w.max(note + 1) } else { w };
    let mut left_w = widen(gutter(panes.iter().filter_map(|p| p.left.as_ref())), notes[0]);
    let mut right_w = widen(gutter(panes.iter().filter_map(|p| p.right.as_ref())), notes[1]);
    let mut keep = [true, true];
    if 3 * (left_w + right_w) > cols {
        panes.iter_mut().filter_map(|p| p.right.as_mut()).for_each(|a| strip(&mut a.ticks));
        right_w = gutter(panes.iter().filter_map(|p| p.right.as_ref()));
        keep[1] = false;
    }
    if 3 * (left_w + right_w) > cols {
        panes.iter_mut().filter_map(|p| p.left.as_mut()).for_each(|a| strip(&mut a.ticks));
        left_w = gutter(panes.iter().filter_map(|p| p.left.as_ref()));
        keep[0] = false;
    }
    let plot_w = cols.saturating_sub(left_w + right_w);
    for p in &mut panes {
        p.plot.x = left_w;
        p.plot.w = plot_w;
        if let Some(a) = &mut p.left {
            a.rect.w = left_w;
        }
        if let Some(a) = &mut p.right {
            a.rect.x = left_w + plot_w;
            a.rect.w = right_w;
        }
    }

    // 6. The x ticks.
    let x = &input.x;
    let count = (plot_w / X_CELLS_PER_TICK).max(1) as f64;
    let x_domain = resolve_domain(&x.scale, x.kind, &extents.x, count).map_err(|error| LayoutError { axis: AxisId::X, error })?;
    let x_axis = (x_rows > 0 && plot_w > 0).then(|| {
        let mut ticks = fit_ticks(Side::Bottom, x.kind, x_domain, plot_w, &x.format, locale);
        if x_rows < 2 {
            strip(&mut ticks);
        }
        let context = if x_rows >= 3 { context_labels(x.kind, x_domain, plot_w, locale) } else { Vec::new() };
        PlacedAxis { side: Side::Bottom, rect: Rect::new(left_w, bottom, plot_w, x_rows), kind: x.kind, domain: x_domain, ticks, context }
    });

    // 7. A legend that had no row of its own goes inside the first pane when
    // its position allows; then the annotations.
    if wanted && !outside {
        if let Some(p) = panes.first() {
            let inside = match input.legend_position {
                LegendPosition::Auto => 3 * legend_w <= p.plot.w && 3 <= p.plot.h,
                LegendPosition::Inside => !p.plot.is_empty(),
                LegendPosition::Outside | LegendPosition::None => false,
            };
            if inside {
                legend = Some(Rect::new(p.plot.x, p.plot.y, legend_w.min(p.plot.w), 1));
            }
        }
    }
    let mut out = Layout { cols, rows, decorated, title, legend, x_name: x.name.clone(), x_domain, x_axis, x_grid: x.grid, panes, marks: Vec::new() };
    out.marks = annotate::place(&mut out, &input.annotations, keep);
    Ok(out)
}

/// One entry of the `panes` option (`options.schema.json`).
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PaneSpec {
    /// The pane's name, which axes name.
    pub id: String,
    /// Its share of the rows, above 0; default 1.
    #[serde(default = "one")]
    pub weight: f64,
}

fn one() -> f64 {
    1.0
}

/// An axis's side, the `orient` of an `axes` entry.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Orient {
    /// A y axis on the left.
    Left,
    /// A y axis on the right.
    Right,
    /// The x axis, at the bottom.
    Bottom,
    /// The x axis, drawn at the bottom until a top axis is built.
    Top,
}

/// One entry of the `axes` option (`options.schema.json`): the keys layout
/// reads. Its other keys (`ticks`, `snap`, `title`, `labelGap`) are left to
/// the code that reads them.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AxisSpec {
    /// The scale it draws, by name.
    pub scale: String,
    /// The pane it belongs to, by id; default the first.
    #[serde(default)]
    pub pane: Option<String>,
    /// Its side; default the x axis for the scale `x`, else the first free y
    /// side of its pane, left then right.
    #[serde(default)]
    pub orient: Option<Orient>,
    /// Its labels' format.
    #[serde(default)]
    pub format: Option<String>,
    /// Whether gridlines come from its ticks.
    #[serde(default)]
    pub grid: Option<bool>,
    /// For a time x axis, the context row (REQ-TICK-070).
    #[serde(default)]
    pub context: Option<bool>,
}

/// One pane's axes as [`bind_axes`] binds them.
#[derive(Clone, Debug, PartialEq)]
pub struct PaneAxes<'a> {
    /// The pane's id; empty for the one pane when no `panes` option is given.
    pub id: &'a str,
    /// Its weight, in thousandths, at least 1 (REQ-LAYOUT-050).
    pub weight: u32,
    /// The axis on its left.
    pub left: Option<&'a AxisSpec>,
    /// The axis on its right.
    pub right: Option<&'a AxisSpec>,
    /// The side its gridlines come from: the first y axis given for the
    /// pane, when it asks for them (REQ-AXIS-010).
    pub grid: Option<Side>,
}

/// The `axes` option bound to the `panes` option (REQ-LAYOUT-050,
/// REQ-AXIS-010): the x axis (the one oriented `bottom` or `top`, or on the
/// scale `x`), and each pane's y axes, one a side, an axis without `orient`
/// taking its pane's first free side, left then right. With no `panes`, one
/// pane. An axis naming no pane, a pane given twice, a weight not above 0,
/// two x axes, or a pane with two axes on one side is an error saying which.
pub fn bind_axes<'a>(panes: &'a [PaneSpec], axes: &'a [AxisSpec]) -> Result<(Option<&'a AxisSpec>, Vec<PaneAxes<'a>>), String> {
    let mut out: Vec<PaneAxes> = Vec::with_capacity(panes.len().max(1));
    for p in panes {
        if !(p.weight > 0.0 && p.weight.is_finite()) {
            return Err(format!("pane `{}`: weight must be above 0", p.id));
        }
        if out.iter().any(|q| q.id == p.id) {
            return Err(format!("pane `{}` is given twice", p.id));
        }
        let weight = (p.weight * 1000.0).round().max(1.0).min(f64::from(u32::MAX)) as u32;
        out.push(PaneAxes { id: &p.id, weight, left: None, right: None, grid: None });
    }
    if out.is_empty() {
        out.push(PaneAxes { id: "", weight: 1000, left: None, right: None, grid: None });
    }
    let mut x = None;
    for a in axes {
        let horizontal = matches!(a.orient, Some(Orient::Bottom | Orient::Top)) || (a.orient.is_none() && a.scale == "x");
        if horizontal {
            if x.replace(a).is_some() {
                return Err("only one x axis is drawn".to_string());
            }
            continue;
        }
        let pane = match &a.pane {
            Some(id) => out.iter_mut().find(|p| p.id == id).ok_or_else(|| format!("axis on `{}`: there is no pane `{id}`", a.scale))?,
            None => &mut out[0],
        };
        let side = match a.orient {
            Some(Orient::Right) => Side::Right,
            Some(_) => Side::Left,
            None if pane.left.is_none() => Side::Left,
            None => Side::Right,
        };
        let slot = if side == Side::Left { &mut pane.left } else { &mut pane.right };
        if slot.replace(a).is_some() {
            return Err(format!("pane `{}` holds one y axis a side", pane.id));
        }
        if pane.grid.is_none() && pane.left.is_some() != pane.right.is_some() && a.grid == Some(true) {
            pane.grid = Some(side);
        }
    }
    Ok((x, out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::{Grid, NO_BACK, WIDE_TAIL};
    use crate::palette::{ACCENT, INK, RULE, SERIES_START};
    use crate::text::{stage, Frame, Layer};
    use crate::time::{TimeLocale, DAY};
    use proptest::prelude::*;

    /// A linear x over 0..100 and a linear y over 0..10, from data extents.
    fn simple() -> (LayoutInput, LayoutExtents) {
        let input = LayoutInput::new(AxisInput::new(Kind::Linear), AxisInput::new(Kind::Linear));
        let ext = |lo, hi| Extents { data: Some([lo, hi]), ..Extents::default() };
        (input, LayoutExtents { x: ext(0.0, 100.0), panes: vec![PaneExtents { left: ext(0.0, 10.0), ..PaneExtents::default() }] })
    }

    /// Every rect of the layout that holds cells.
    fn rects(l: &Layout) -> Vec<Rect> {
        let mut out: Vec<Rect> = l.title.into_iter().chain(l.legend).chain(l.x_axis.as_ref().map(|a| a.rect)).collect();
        for p in &l.panes {
            out.push(p.plot);
            out.extend(p.left.as_ref().map(|a| a.rect));
            out.extend(p.right.as_ref().map(|a| a.rect));
        }
        out.retain(|r| !r.is_empty());
        out
    }

    /// REQ-LAYOUT-010: the same inputs give the same layout; parts lie in
    /// whole cells inside the grid, apart, title then legend then panes then
    /// the x axis down the rows, gutters beside plots that line up, and the
    /// view moves the x domain.
    #[test]
    fn layout_is_pure_and_ordered() {
        let (mut input, mut ext) = simple();
        input.title = Some("BTC-USD".into());
        input.legend = vec!["price".into(), "volume".into()];
        let mut right = AxisInput::new(Kind::Linear);
        right.format = AxisFormat::parse("format", ",.0f", Kind::Linear).unwrap();
        input.panes = vec![
            PaneInput { weight: 3, left: Some(AxisInput::new(Kind::Linear)), right: Some(right) },
            PaneInput { weight: 1, left: Some(AxisInput::new(Kind::Linear)), right: None },
        ];
        let y = |lo, hi| Extents { data: Some([lo, hi]), ..Extents::default() };
        ext.panes = vec![PaneExtents { left: y(98.37, 131.2), right: y(0.0, 25_000.0) }, PaneExtents { left: y(0.0, 3.4e6), ..PaneExtents::default() }];
        let l = layout(80, 24, &input, &ext).unwrap();
        assert_eq!(l, layout(80, 24, &input, &ext).unwrap(), "pure");
        assert_eq!(l.title, Some(Rect::new(0, 0, 80, 1)));
        assert_eq!(l.legend, Some(Rect::new(0, 1, legend_width(&input.legend), 1)));
        assert_eq!(legend_width(&input.legend), 7 + 2 + 8);
        let x = l.x_axis.as_ref().unwrap();
        assert_eq!((x.rect.y, x.rect.h), (22, 2), "the x axis takes the bottom two rows");
        let (a, b) = (&l.panes[0], &l.panes[1]);
        assert_eq!(a.plot.y, 2, "panes start under the legend");
        assert_eq!(b.plot.y, a.plot.y + a.plot.h + 1, "a blank row between panes");
        assert_eq!(b.plot.y + b.plot.h, x.rect.y, "the panes end on the x axis");
        assert_eq!((a.plot.h, b.plot.h), (14, 5), "20 rows less the separator, split 3 : 1");
        assert_eq!((a.plot.x, a.plot.w), (b.plot.x, b.plot.w), "plots line up across panes");
        assert_eq!((x.rect.x, x.rect.w), (a.plot.x, a.plot.w), "the x axis spans the plot");
        let gutter = a.left.as_ref().unwrap().rect.w;
        assert_eq!(gutter, b.left.as_ref().unwrap().rect.w, "left gutters align");
        let widest = l.panes.iter().flat_map(|p| &p.left.as_ref().unwrap().ticks).filter_map(|t| t.label.as_ref()).map(|t| t.width).max().unwrap();
        assert_eq!(gutter, widest + 1, "the widest left label plus the line");
        assert_eq!(a.plot.x, gutter);
        assert_eq!(a.right.as_ref().unwrap().rect.x, a.plot.x + a.plot.w, "the right gutter follows the plot");
        assert_eq!(a.plot.x + a.plot.w + a.right.as_ref().unwrap().rect.w, 80);
        let all = rects(&l);
        for (i, r) in all.iter().enumerate() {
            assert!(r.x + r.w <= 80 && r.y + r.h <= 24, "{r:?} is off the grid");
            for s in &all[i + 1..] {
                assert!(!r.overlaps(s), "{r:?} and {s:?} overlap");
            }
        }
        assert_eq!(l.x_domain, [0.0, 100.0]);
        ext.x.view = Some([10.0, 20.0]);
        let zoomed = layout(80, 24, &input, &ext).unwrap();
        assert_eq!(zoomed.x_domain, [10.0, 20.0], "the view sets the x domain");
        assert!(zoomed.x_axis.unwrap().ticks.iter().all(|t| (10.0..=20.0).contains(&t.value)));
        assert_eq!(zoomed.panes, l.panes, "the x view leaves the panes alone");
    }

    /// REQ-LAYOUT-020: below 16 × 5 no axis, title or legend; no title below
    /// 10 rows; no x labels below 6 rows.
    #[test]
    fn small_sizes_drop_axes_title_and_labels() {
        let (mut input, ext) = simple();
        input.title = Some("title".into());
        input.legend = vec!["a".into()];
        let bare = |l: &Layout| l.title.is_none() && l.legend.is_none() && l.x_axis.is_none() && l.panes.iter().all(|p| p.left.is_none() && p.right.is_none());
        for (cols, rows) in [(15, 24), (80, 4), (1, 1), (15, 4), (4, 30)] {
            let l = layout(cols, rows, &input, &ext).unwrap();
            assert!(bare(&l) && !l.decorated, "{cols} × {rows}: {l:?}");
            assert_eq!(l.panes[0].plot, Rect::new(0, 0, cols, rows), "the data takes every cell at {cols} × {rows}");
        }
        let at = |cols, rows| layout(cols, rows, &input, &ext).unwrap();
        assert!(at(16, 5).decorated && at(16, 5).x_axis.is_some());
        let labels = |l: &Layout| l.x_axis.as_ref().unwrap().ticks.iter().filter(|t| t.label.is_some()).count();
        let five = at(80, 5);
        assert_eq!(five.x_axis.as_ref().unwrap().rect.h, 1, "a line and no label row at 5 rows");
        assert_eq!(labels(&five), 0, "no x labels below 6 rows");
        assert!(!five.x_axis.as_ref().unwrap().ticks.is_empty(), "the marks stay");
        assert!(labels(&at(80, 6)) > 0, "x labels from 6 rows");
        assert!(at(80, 9).title.is_none(), "no title below 10 rows");
        assert_eq!(at(80, 10).title, Some(Rect::new(0, 0, 80, 1)));
        let eleven = at(80, 11);
        let plot = eleven.panes[0].plot;
        assert_eq!(eleven.legend, Some(Rect::new(plot.x, plot.y, 3, 1)), "under 12 rows the legend goes inside the pane");
        assert_eq!(at(80, 12).legend, Some(Rect::new(0, 1, 3, 1)), "from 12 rows it has a row");
        let mut no_x = input.clone();
        no_x.x.shown = false;
        assert!(layout(80, 24, &no_x, &ext).unwrap().x_axis.is_none());
        assert_eq!(layout(80, 24, &no_x, &ext).unwrap().x_domain, [0.0, 100.0], "a hidden axis still has its domain");
    }

    /// REQ-LAYOUT-030: when the gutters would take more than a third of the
    /// columns, the right axis's labels drop first, then the left's.
    #[test]
    fn wide_gutters_drop_the_right_axis_first() {
        let (mut input, mut ext) = simple();
        let mut wide = AxisInput::new(Kind::Linear);
        wide.format = AxisFormat::parse("format", ",.0f", Kind::Linear).unwrap();
        input.panes[0].left = Some(wide.clone());
        input.panes[0].right = Some(wide);
        let millions = Extents { data: Some([1_000_000.0, 9_000_000.0]), ..Extents::default() };
        ext.panes[0] = PaneExtents { left: millions, right: millions };
        let labelled = |a: &Option<PlacedAxis>| a.as_ref().unwrap().ticks.iter().any(|t| t.label.is_some());
        // "1,000,000" is 9 cells: a gutter of 10 a side.
        let both = layout(60, 24, &input, &ext).unwrap();
        assert!(labelled(&both.panes[0].left) && labelled(&both.panes[0].right), "20 of 60 columns is a third");
        assert_eq!(both.panes[0].plot, Rect::new(10, 0, 40, 22));
        let one = layout(59, 24, &input, &ext).unwrap();
        assert!(labelled(&one.panes[0].left) && !labelled(&one.panes[0].right), "20 of 59 is more: the right drops");
        assert_eq!(one.panes[0].right.as_ref().unwrap().rect.w, 1, "its line stays");
        assert!(!one.panes[0].right.as_ref().unwrap().ticks.is_empty(), "and its marks");
        assert_eq!(one.panes[0].plot, Rect::new(10, 0, 48, 22));
        let none = layout(32, 24, &input, &ext).unwrap();
        assert!(!labelled(&none.panes[0].left) && !labelled(&none.panes[0].right), "11 of 32 is more: the left drops too");
        assert_eq!(none.panes[0].plot, Rect::new(1, 0, 30, 22));
    }

    /// A finite domain of any size: scaled spans like d3's tests, or any two
    /// finite numbers.
    fn domain() -> impl Strategy<Value = [f64; 2]> {
        prop_oneof![
            (-15i32..15, -1.0f64..1.0, 1e-6f64..4.0).prop_map(|(mag, a, span)| {
                let m = 10f64.powi(mag);
                [a * m, a * m + span * m]
            }),
            (prop::num::f64::NORMAL | prop::num::f64::ZERO, prop::num::f64::NORMAL | prop::num::f64::ZERO).prop_map(|(a, b)| [a.min(b), a.max(b)]),
        ]
    }

    /// A UTC span from 10 ms to 10¹² ms (32 years), starting from 1950 to
    /// 2100.
    fn time_domain() -> impl Strategy<Value = [f64; 2]> {
        (-631_152_000_000.0f64..4_102_444_800_000.0, 1.0f64..12.0).prop_map(|(t, e)| [t, t + 10f64.powf(e)])
    }

    /// Checks the fitted ticks of an axis of `len` cells against
    /// REQ-TICK-030, then draws its labels into a frame, which refuses a
    /// label that would share a cell (REQ-TEXT-010).
    fn check(side: Side, kind: Kind, d: [f64; 2], len: usize) -> Result<(), TestCaseError> {
        let format = if kind == Kind::Time { AxisFormat::Time(None) } else { AxisFormat::default() };
        let ticks = fit_ticks(side, kind, d, len, &format, &Locale::default());
        let x = side == Side::Bottom;
        for t in &ticks {
            prop_assert!(t.value >= d[0] && t.value <= d[1], "{} outside {:?}", t.value, d);
            let per = if x { X_DOTS } else { Y_DOTS };
            prop_assert!(t.cell < len && t.dot < len * per, "{:?} off a {}-cell axis", t, len);
        }
        for w in ticks.windows(2) {
            prop_assert!(w[0].value < w[1].value, "values {} then {}", w[0].value, w[1].value);
            prop_assert!(if x { w[0].cell <= w[1].cell } else { w[0].cell >= w[1].cell }, "cells {} then {}", w[0].cell, w[1].cell);
        }
        let labels: Vec<&TickLabel> = ticks.iter().filter_map(|t| t.label.as_ref()).collect();
        prop_assert!(labels.is_empty() || labels.len() == ticks.len(), "all labelled or none");
        for l in &labels {
            prop_assert_eq!(l.width, text::width(&l.text));
            prop_assert!(if x { l.start + l.width <= len } else { l.start < len }, "{:?} leaves a {}-cell axis", l, len);
        }
        for w in labels.windows(2) {
            if x {
                prop_assert!(w[1].start >= w[0].start + w[0].width + X_GAP, "{:?} then {:?}", w[0], w[1]);
            } else {
                prop_assert!(w[0].start > w[1].start + Y_GAP, "{:?} then {:?}", w[0], w[1]);
            }
        }
        // Drawn into a frame, every label finds its cells free.
        let widest = labels.iter().map(|l| l.width).max().unwrap_or(0);
        let rect = if x { Rect::new(0, 0, len, 2) } else { Rect::new(0, 0, widest + 1, len) };
        let mut grid = Grid::new(rect.w, rect.h);
        let mut frame = Frame::new(&mut grid, NO_BACK).then::<stage::Labels>();
        let axis = PlacedAxis { side, rect, kind, domain: d, ticks: ticks.clone(), context: Vec::new() };
        prop_assert_eq!(frame.axis_labels(&axis), labels.len());
        Ok(())
    }

    proptest! {
        /// REQ-TICK-030, REQ-TEXT-010, REQ-PROP-010 (label non-overlap): for
        /// any axis from 4 to 400 cells and any finite domain, linear or
        /// time, x or y, placed ticks lie in the domain, increase strictly in
        /// value and never go back in cell, and their labels lie inside the
        /// axis at least the gap apart, so no two share a cell.
        #[test]
        fn labels_never_share_a_cell(len in 4usize..=400, d in domain(), t in time_domain(), y in any::<bool>()) {
            let side = if y { Side::Left } else { Side::Bottom };
            check(side, Kind::Linear, d, len)?;
            check(side, Kind::Time, t, len)?;
        }
    }

    /// REQ-I18N-040: label widths count cells by `unicode-width`, so a wide
    /// month name takes its two cells a glyph in the fitting, the gutters and
    /// the grid, where its glyphs go through `set_wide`.
    #[test]
    fn labels_count_unicode_width() {
        let locale =
            Locale { time: TimeLocale { months_short: std::array::from_fn(|i| format!("{}月", i + 1)), ..TimeLocale::default() }, ..Locale::default() };
        let format = AxisFormat::Time(Some("%b".into()));
        let year = [1_767_225_600_000.0, 1_767_225_600_000.0 + 364.0 * DAY];
        let ticks = fit_ticks(Side::Bottom, Kind::Time, year, 60, &format, &locale);
        let labels: Vec<&TickLabel> = ticks.iter().filter_map(|t| t.label.as_ref()).collect();
        assert!(labels.len() >= 4, "{labels:?}");
        for l in &labels {
            assert_eq!(l.width, l.text.chars().count() + 1, "{} has one wide glyph", l.text);
        }
        for w in labels.windows(2) {
            assert!(w[1].start >= w[0].start + w[0].width + X_GAP, "the gap is in cells: {:?} then {:?}", w[0], w[1]);
        }
        // A y gutter is as wide as its widest label in cells.
        let mut input = LayoutInput::new(AxisInput::new(Kind::Linear), AxisInput { format: format.clone(), ..AxisInput::new(Kind::Time) });
        input.locale = locale.clone();
        let ext = LayoutExtents {
            panes: vec![PaneExtents { left: Extents { data: Some(year), ..Extents::default() }, ..PaneExtents::default() }],
            ..LayoutExtents::default()
        };
        let l = layout(80, 40, &input, &ext).unwrap();
        let left = l.panes[0].left.as_ref().unwrap();
        let widest = left.ticks.iter().filter_map(|t| t.label.as_ref()).map(|l| l.width).max().unwrap();
        assert_eq!(left.rect.w, widest + 1);
        assert!(widest >= 3);
        // Drawn, the wide glyph's tail follows its head.
        let mut grid = Grid::new(60, 2);
        let mut frame = Frame::new(&mut grid, NO_BACK).then::<stage::Labels>();
        let axis = PlacedAxis { side: Side::Bottom, rect: Rect::new(0, 0, 60, 2), kind: Kind::Time, domain: year, ticks: ticks.clone(), context: Vec::new() };
        assert_eq!(frame.axis_labels(&axis), labels.len());
        let first = labels[0];
        let head = first.start + first.text.chars().count() - 1;
        assert_eq!(frame.grid().glyph(head, 1), '月' as u32);
        assert_eq!(frame.grid().glyph(head + 1, 1), WIDE_TAIL);
        assert_eq!(text::width(&frame.grid().text().lines().nth(1).unwrap().replace(' ', "")), labels.iter().map(|l| l.width).sum::<usize>());
    }

    /// REQ-TEXT-020: a frame paints data, gridlines on empty cells only,
    /// axes, labels, annotations, the crosshair and the readout, in that
    /// order and no other; each layer covers the ones before it where they
    /// meet, and gridlines cover nothing.
    #[test]
    fn draw_order_is_z_order() {
        assert_eq!(Layer::ALL, {
            let mut sorted = Layer::ALL;
            sorted.sort();
            sorted
        });
        let (input, ext) = simple();
        let l = layout(40, 12, &input, &ext).unwrap();
        let pane = &l.panes[0];
        let (x, y) = (l.x_axis.as_ref().unwrap(), pane.left.as_ref().unwrap());
        let label = x.ticks.iter().find_map(|t| t.label.as_ref().filter(|l| l.start > 0)).unwrap().clone();
        let tick = x.ticks.iter().find(|t| t.cell > 0 && t.cell + 1 < x.rect.w).unwrap();
        let (gx, gy) = (pane.plot.x + tick.cell, pane.plot.y);
        let (lx, ly) = (x.rect.x + label.start, x.rect.y + 1);
        let mut grid = Grid::new(40, 12);

        let mut f = Frame::new(&mut grid, NO_BACK);
        assert_eq!(f.layer(), Layer::Data);
        f.data().set(gx as i32, gy as i32, 0x25CF, SERIES_START);
        f.data().set(lx as i32, ly as i32, 0x25CF, SERIES_START);

        let mut f = f.then::<stage::Gridlines>();
        f.gridlines(pane.plot, Some(x), Some(y));
        assert_eq!(f.grid().glyph(gx, gy), 0x25CF, "a gridline leaves the data's cell");
        assert_eq!((f.grid().glyph(gx, gy + 1) & !0xFF, f.grid().ink(gx, gy + 1)), (0x2800, RULE), "and draws in braille on empty cells");

        let mut f = f.then::<stage::Axes>();
        f.axis(x);
        f.axis(y);
        assert_eq!(f.grid().glyph(gx, x.rect.y), 0x252C, "a tick mark on the x axis");

        let mut f = f.then::<stage::Labels>();
        assert!(f.axis_labels(x) > 0 && f.axis_labels(y) > 0);
        assert_eq!(f.grid().glyph(lx, ly), label.text.chars().next().unwrap() as u32, "a label covers the data");
        assert_eq!(f.grid().ink(lx, ly), INK);

        let mut f = f.then::<stage::Annotations>();
        assert!(!f.label(lx, ly, "hi", ACCENT), "an annotation label cannot take a tick label's cell");
        f.annotations().set(gx as i32, gy as i32, 0x25B2, ACCENT);
        assert_eq!(f.grid().glyph(gx, gy), 0x25B2, "an annotation covers the data");

        let mut f = f.then::<stage::Crosshair>();
        f.crosshair().rect(lx as i32, 0, 1, 12, 0x2502, ACCENT);
        assert_eq!(f.grid().glyph(lx, ly), 0x2502, "the crosshair covers the label");

        let mut f = f.then::<stage::Readout>();
        f.readout(lx, ly, "50", INK);
        assert_eq!(f.grid().glyph(lx, ly), '5' as u32, "the readout covers everything");
        assert_eq!(f.layer(), Layer::Readout);
    }

    #[test]
    fn panes_split_by_weight_with_a_floor() {
        assert_eq!(split_rows(0, 19, &[3, 1]), [(0, 14), (15, 4)]);
        assert_eq!(split_rows(2, 10, &[10, 1]), [(2, 6), (9, 3)], "the small pane keeps three rows");
        assert_eq!(split_rows(0, 6, &[1, 1]), [(0, 3), (3, 3)], "no separator when the floor needs every row");
        assert_eq!(split_rows(0, 4, &[1, 1]), [(0, 2), (2, 2)], "under the floor, by weight alone");
        assert_eq!(split_rows(0, 1, &[1, 0, 1]), [(0, 1), (1, 0), (1, 0)]);
        assert!(split_rows(0, 10, &[]).is_empty());
    }

    proptest! {
        /// `shares` gives what the largest-remainder split by a stable sort
        /// on (remainder descending, pane) gives, which it replaced to keep a
        /// slice sort out of the wasm module.
        #[test]
        fn shares_match_a_sorted_largest_remainder(total in 0usize..400, w in proptest::collection::vec(0u128..1000, 0..12)) {
            let sum: u128 = w.iter().sum();
            let expected = if sum == 0 {
                vec![0; w.len()]
            } else {
                let t = total as u128;
                let mut out: Vec<usize> = w.iter().map(|&wi| (t * wi / sum) as usize).collect();
                let left = total - out.iter().sum::<usize>();
                let mut order: Vec<usize> = (0..w.len()).collect();
                order.sort_by_key(|&i| (std::cmp::Reverse(t * w[i] % sum), i));
                for &i in order.iter().take(left) {
                    out[i] += 1;
                }
                out
            };
            prop_assert_eq!(shares(total, &w), expected);
        }
    }

    /// Two panes sharing x: a price pane of weight 3 and a volume pane of
    /// weight 1, both y scales on the left.
    fn price_and_volume() -> (LayoutInput, LayoutExtents) {
        let (mut input, mut ext) = simple();
        input.panes = vec![
            PaneInput { weight: 3, left: Some(AxisInput::named(Kind::Linear, "price")), right: None },
            PaneInput { weight: 1, left: Some(AxisInput::named(Kind::Linear, "volume")), right: None },
        ];
        let y = |lo, hi| Extents { data: Some([lo, hi]), ..Extents::default() };
        ext.panes = vec![PaneExtents { left: y(98.0, 131.0), ..PaneExtents::default() }, PaneExtents { left: y(0.0, 2_500_000.0), ..PaneExtents::default() }];
        (input, ext)
    }

    /// REQ-LAYOUT-050: panes split the rows by weight with at least three
    /// rows each, share the x scale (one domain, one axis under the last
    /// pane, the same columns), and align their plots to the same gutters,
    /// at every size from the smallest decorated one up.
    #[test]
    fn panes_split_by_weight() {
        let (input, ext) = price_and_volume();
        for rows in 5..=60 {
            for cols in [16, 23, 40, 80, 200] {
                let l = layout(cols, rows, &input, &ext).unwrap();
                let (a, b) = (&l.panes[0], &l.panes[1]);
                assert_eq!((a.plot.x, a.plot.w), (b.plot.x, b.plot.w), "{cols} × {rows}: the plots line up");
                assert_eq!(l.x_mapping(0), l.x_mapping(1), "{cols} × {rows}: one x mapping");
                if let Some(x) = &l.x_axis {
                    assert_eq!((x.rect.x, x.rect.w), (b.plot.x, b.plot.w), "the x axis spans every plot");
                    assert_eq!(x.rect.y, b.plot.y + b.plot.h, "under the last pane");
                }
                if let (Some(la), Some(lb)) = (&a.left, &b.left) {
                    assert_eq!(la.rect.w, lb.rect.w, "{cols} × {rows}: one left gutter");
                }
                let room = l.x_axis.as_ref().map_or(rows, |x| x.rect.y) - a.plot.y;
                if room > 2 * PANE_ROWS {
                    assert!(a.plot.h >= PANE_ROWS && b.plot.h >= PANE_ROWS, "{cols} × {rows}: {} and {} rows", a.plot.h, b.plot.h);
                    assert_eq!(b.plot.y, a.plot.y + a.plot.h + 1, "a blank row between panes");
                    assert!(a.plot.h >= b.plot.h, "{cols} × {rows}: the heavier pane is no smaller");
                }
            }
        }
        // By weight: 3 : 1 of the 21 rows left at 80 × 24 by the separator, the spare row to the larger remainder.
        let l = layout(80, 24, &input, &ext).unwrap();
        assert_eq!((l.panes[0].plot.h, l.panes[1].plot.h), (16, 5));
        assert_eq!(l.panes[0].left_domain, Some([98.0, 131.0]));
        assert_eq!(l.panes[1].left_domain, Some([0.0, 2_500_000.0]));
        // The volume's wider labels widen the one gutter for both panes.
        let widest = l.panes[1].left.as_ref().unwrap().ticks.iter().filter_map(|t| t.label.as_ref()).map(|t| t.width).max().unwrap();
        assert_eq!(l.panes[0].plot.x, widest + 1);
        // A pane below its floor keeps three rows while the rows allow.
        let mut thin = input.clone();
        thin.panes[0].weight = 50;
        let l = layout(80, 24, &thin, &ext).unwrap();
        assert_eq!(l.panes[1].plot.h, PANE_ROWS);
    }

    /// REQ-LAYOUT-060: at `auto` the legend goes outside in one row when it
    /// fits and rows ≥ 12, else inside the first pane when it takes at most
    /// a third of its width and height, else nowhere; `outside`, `inside`
    /// and `none` force theirs.
    #[test]
    fn auto_legend_placement() {
        let (mut input, ext) = simple();
        input.legend = vec!["alpha".into(), "beta".into()];
        let w = legend_width(&input.legend);
        assert_eq!(w, 7 + 2 + 6);
        let at = |input: &LayoutInput, cols, rows| layout(cols, rows, input, &ext).unwrap();
        // Outside: one row, at 12 rows and more, when it fits the width.
        let l = at(&input, 80, 12);
        assert_eq!(l.legend, Some(Rect::new(0, 0, w, 1)));
        assert_eq!(l.panes[0].plot.y, 1, "the panes start under it");
        let mut exact = input.clone();
        exact.legend = vec!["alpha".into(), "gamma".into()];
        assert_eq!(legend_width(&exact.legend), 16);
        assert_eq!(at(&exact, 16, 24).legend.map(|r| r.y), Some(0), "exactly the width fits");
        // Inside: under 12 rows, when it takes a third of the plot's width
        // and the plot has three rows (a third of its height for one row).
        let l = at(&input, 80, 11);
        let p = l.panes[0].plot;
        assert!(3 * w <= p.w && p.h >= 3);
        assert_eq!(l.legend, Some(Rect::new(p.x, p.y, w, 1)));
        // Too wide for a third of the plot: left out.
        let l = at(&input, 40, 11);
        assert!(3 * w > l.panes[0].plot.w);
        assert_eq!(l.legend, None, "left out of the frame");
        // Too wide for one row at 12 rows: inside if a third, else out.
        let mut long = input.clone();
        long.legend = (0..8).map(|i| format!("series-{i}")).collect();
        assert!(legend_width(&long.legend) > 60);
        assert_eq!(at(&long, 60, 30).legend, None);
        // A first pane of two rows is too short for it: a row is more than a
        // third of its height.
        let mut split = input.clone();
        split.panes.push(PaneInput { weight: 3, left: Some(AxisInput::new(Kind::Linear)), right: None });
        let l = at(&split, 80, 7);
        assert!(l.panes[0].plot.h < 3 && l.legend.is_none(), "{:?}", l.panes[0].plot);
        // Forced positions.
        let mut forced = input.clone();
        forced.legend_position = LegendPosition::Outside;
        assert_eq!(at(&forced, 80, 8).legend, Some(Rect::new(0, 0, w, 1)), "outside under 12 rows");
        assert_eq!(at(&forced, 10, 8).legend, None, "but never below 16 columns");
        forced.legend_position = LegendPosition::Inside;
        let l = at(&forced, 40, 24);
        assert_eq!(l.legend.map(|r| (r.x, r.y)), Some((l.panes[0].plot.x, l.panes[0].plot.y)), "inside with room for a row");
        forced.legend_position = LegendPosition::None;
        assert_eq!(at(&forced, 80, 24).legend, None);
        assert_eq!(LegendPosition::from_option("inside"), LegendPosition::Inside);
        assert_eq!(LegendPosition::from_option("whatever"), LegendPosition::Auto);
        // Entries in their row: key, blank, name, two cells apart, cut at
        // the edge.
        let spans = legend_spans(Rect::new(3, 1, w, 1), &input.legend);
        assert_eq!(spans, [Rect::new(3, 1, 7, 1), Rect::new(12, 1, 6, 1)]);
        assert_eq!(legend_spans(Rect::new(0, 0, 9, 1), &input.legend), [Rect::new(0, 0, 7, 1)], "the second starts past the edge");
        assert_eq!(legend_spans(Rect::new(0, 0, 10, 1), &input.legend)[1], Rect::new(9, 0, 1, 1), "the second keeps its key");
        assert_eq!(legend_spans(Rect::new(0, 0, 4, 1), &input.legend), [Rect::new(0, 0, 4, 1)], "the first is cut");
    }

    /// REQ-AXIS-010: a pane's left and right y axes resolve their own scales
    /// and tick their own domains, and its gridlines come from one axis only.
    #[test]
    fn left_and_right_axes_bind_their_scales() {
        let (mut input, mut ext) = simple();
        let mut left = AxisInput::named(Kind::Linear, "price");
        left.grid = true;
        let mut right = AxisInput::named(Kind::Linear, "rate");
        right.grid = true;
        input.panes[0].left = Some(left);
        input.panes[0].right = Some(right);
        let y = |lo, hi| Extents { data: Some([lo, hi]), ..Extents::default() };
        ext.panes[0] = PaneExtents { left: y(0.0, 1000.0), right: y(0.0, 7.0) };
        let l = layout(80, 24, &input, &ext).unwrap();
        let p = &l.panes[0];
        assert_eq!((p.left_domain, p.right_domain), (Some([0.0, 1000.0]), Some([0.0, 7.0])));
        assert_eq!((p.left_name.as_deref(), p.right_name.as_deref()), (Some("price"), Some("rate")));
        let (la, ra) = (p.left.as_ref().unwrap(), p.right.as_ref().unwrap());
        assert!(la.ticks.iter().all(|t| (0.0..=1000.0).contains(&t.value)) && la.ticks.iter().any(|t| t.value >= 100.0));
        assert!(ra.ticks.iter().all(|t| (0.0..=7.0).contains(&t.value)) && ra.ticks.iter().all(|t| t.value.fract() == 0.0));
        assert!(la.ticks.iter().any(|t| t.label.as_ref().is_some_and(|l| l.text == "1,000" || l.text == "1000")));
        assert!(ra.ticks.iter().any(|t| t.label.as_ref().is_some_and(|l| l.text == "7")));
        assert_eq!(ra.rect.x, p.plot.x + p.plot.w, "the right gutter follows the plot");
        assert_eq!(p.y_mapping(Side::Right).unwrap().hi(), 7.0);
        // Gridlines from the left axis only: rows at its ticks' dots, none at
        // the right's alone.
        assert_eq!(p.grid, Some(Side::Left));
        assert_eq!(p.grid_axis(), Some(la));
        let mut grid = Grid::new(80, 24);
        let mut f = Frame::new(&mut grid, NO_BACK).then::<stage::Gridlines>();
        f.pane_gridlines(&l);
        let lit_rows: Vec<usize> = (0..p.plot.h).filter(|&r| (p.plot.x..p.plot.x + p.plot.w).any(|c| f.grid().glyph(c, p.plot.y + r) != 0x20)).collect();
        let left_rows: Vec<usize> = la.ticks.iter().map(|t| t.dot / Y_DOTS).collect();
        assert_eq!(
            lit_rows,
            {
                let mut r = left_rows.clone();
                r.sort_unstable();
                r.dedup();
                r
            },
            "gridlines at the left's ticks"
        );
        let right_only: Vec<usize> = ra.ticks.iter().map(|t| t.dot / Y_DOTS).filter(|r| !left_rows.contains(r)).collect();
        assert!(!right_only.is_empty() && right_only.iter().all(|r| !lit_rows.contains(r)), "none at the right's");
        // The right's when the left does not ask; none when neither does.
        input.panes[0].left.as_mut().unwrap().grid = false;
        assert_eq!(layout(80, 24, &input, &ext).unwrap().panes[0].grid, Some(Side::Right));
        input.panes[0].right.as_mut().unwrap().grid = false;
        assert_eq!(layout(80, 24, &input, &ext).unwrap().panes[0].grid, None);
        // A pane with its one scale on the right.
        input.panes[0].left = None;
        let l = layout(80, 24, &input, &ext).unwrap();
        assert!(l.panes[0].left.is_none() && l.panes[0].left_domain.is_none());
        assert_eq!(l.panes[0].plot.x, 0, "no left gutter");
        assert_eq!(l.panes[0].right_domain, Some([0.0, 7.0]));
    }

    /// The `panes` and `axes` options bind scales to panes and sides
    /// (REQ-LAYOUT-050, REQ-AXIS-010).
    #[test]
    fn axes_bind_to_panes() {
        let panes: Vec<PaneSpec> = serde_json::from_str(r#"[{"id":"price","weight":3},{"id":"volume"}]"#).unwrap();
        let axes: Vec<AxisSpec> = serde_json::from_str(
            r#"[{"scale":"x","format":"%b"},{"scale":"price","pane":"price","orient":"right","grid":true},
                {"scale":"sma","pane":"price","grid":true},{"scale":"volume","pane":"volume","ticks":3}]"#,
        )
        .unwrap();
        let (x, bound) = bind_axes(&panes, &axes).unwrap();
        assert_eq!(x.map(|a| a.format.as_deref()), Some(Some("%b")));
        assert_eq!(bound.len(), 2);
        assert_eq!((bound[0].id, bound[0].weight, bound[1].weight), ("price", 3000, 1000));
        assert_eq!(bound[0].right.map(|a| a.scale.as_str()), Some("price"));
        assert_eq!(bound[0].left.map(|a| a.scale.as_str()), Some("sma"), "an axis without orient takes the first free side");
        assert_eq!(bound[0].grid, Some(Side::Right), "gridlines from the first axis given only");
        assert_eq!(bound[1].left.map(|a| a.scale.as_str()), Some("volume"));
        let (_, one) = bind_axes(&[], &[]).unwrap();
        assert_eq!(one.len(), 1, "one pane by default");
        let err =
            |p: &str, a: &str| bind_axes(&serde_json::from_str::<Vec<PaneSpec>>(p).unwrap(), &serde_json::from_str::<Vec<AxisSpec>>(a).unwrap()).unwrap_err();
        assert!(err("[]", r#"[{"scale":"y","pane":"nope"}]"#).contains("no pane `nope`"));
        assert!(err(r#"[{"id":"a"}]"#, r#"[{"scale":"y","orient":"left"},{"scale":"z","orient":"left"}]"#).contains("one y axis a side"));
        assert!(err("[]", r#"[{"scale":"a"},{"scale":"b"},{"scale":"c"}]"#).contains("one y axis a side"));
        assert!(err(r#"[{"id":"a","weight":0}]"#, "[]").contains("above 0"));
        assert!(err(r#"[{"id":"a"},{"id":"a"}]"#, "[]").contains("twice"));
        assert!(err("[]", r#"[{"scale":"x"},{"scale":"t","orient":"bottom"}]"#).contains("one x axis"));
    }
}
