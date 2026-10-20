//! Layout in whole cells, from the outside in (REQ-LAYOUT-010 to 030).
//!
//! [`layout`] places, in this order: the title, the legend, the x axis, the
//! panes (split by weight, at least three rows each), the y gutters (one
//! width across every pane, so plots line up), the x ticks, and last the
//! annotations, which draw over their pane's plot and take no rows of their
//! own. It is a pure function of the grid's size, a [`LayoutInput`] and the
//! [`LayoutExtents`] (the data's extent and the view), so the same inputs give
//! the same layout on every surface (REQ-LAYOUT-010).
//!
//! Ticks are fitted by lowering the count (ADR-111, REQ-TICK-030):
//! [`fit_ticks`] starts at one tick per four cells on x and one per two rows on
//! y, and lowers the count until every label fits whole inside the axis with a
//! gap of two cells on x and one row on y. Labels are `scale.rs`'s tick labels,
//! measured in cells by `unicode-width` (REQ-I18N-040).

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
/// The rows a chart needs to draw its legend in a row of its own.
pub const LEGEND_ROWS: usize = 12;
/// The rows a pane keeps when the rows allow it, and needs for y labels.
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
    let scale = match kind {
        Kind::Linear => Scale::linear([lo, hi], range),
        Kind::Time => Scale::time([lo, hi], range),
    };
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

/// One axis's scale and format, as the options will give them (Phase 2,
/// REQ-OPT).
#[derive(Clone, Debug, PartialEq)]
pub struct AxisInput {
    /// How the axis is ticked and labelled.
    pub kind: Kind,
    /// The scale's domain options (`domain`, `nice`, `zero`, `padding`),
    /// resolved by `scale::resolve_domain`; its `type` is not read.
    pub scale: ScaleOptions,
    /// The labels' format.
    pub format: AxisFormat,
    /// Whether the axis is drawn; its scale is resolved either way.
    pub shown: bool,
}

impl AxisInput {
    /// A shown axis of `kind` over the data's extent, unniced, with the
    /// automatic format.
    pub fn new(kind: Kind) -> AxisInput {
        let (kind_type, format) = match kind {
            Kind::Linear => (ScaleType::Linear, AxisFormat::default()),
            Kind::Time => (ScaleType::Time, AxisFormat::Time(None)),
        };
        let scale =
            ScaleOptions { kind: kind_type, domain: Domain::Data, nice: None, zero: None, padding: None, base: None, constant: None, zone: None, scheme: None };
        AxisInput { kind, scale, format, shown: true }
    }
}

/// One pane: a share of the rows and its y scales.
#[derive(Clone, Debug, PartialEq)]
pub struct PaneInput {
    /// Its share of the rows; 0 counts as 1.
    pub weight: u32,
    /// Its y scale, drawn on the left.
    pub left: AxisInput,
    /// A second y scale, drawn on the right.
    pub right: Option<AxisInput>,
}

/// What a chart's layout depends on besides its size and its data's extent:
/// the part of the options layout reads (REQ-LAYOUT-010). Phase 2's options
/// fill it.
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutInput {
    /// The title.
    pub title: Option<String>,
    /// The legend's entries, one a series.
    pub legend: Vec<String>,
    /// The x scale, shared by every pane and drawn at the bottom.
    pub x: AxisInput,
    /// The panes, top first.
    pub panes: Vec<PaneInput>,
    /// The names and symbols labels are written with.
    pub locale: Locale,
}

impl LayoutInput {
    /// One pane with the y scale `y` under the x scale `x`, no title and no
    /// legend.
    pub fn new(x: AxisInput, y: AxisInput) -> LayoutInput {
        LayoutInput { title: None, legend: Vec::new(), x, panes: vec![PaneInput { weight: 1, left: y, right: None }], locale: Locale::default() }
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
    /// Its cells: for the x axis, the plot's columns over its line row and
    /// its label row; for a y axis, its gutter beside the pane, the line in
    /// the column next to the plot.
    pub rect: Rect,
    /// Its scale's kind.
    pub kind: Kind,
    /// Its scale's domain.
    pub domain: [f64; 2],
    /// Its ticks, cells counted from the rect's left (x) or top (y).
    pub ticks: Vec<Tick>,
}

/// A pane as laid out.
#[derive(Clone, Debug, PartialEq)]
pub struct PaneLayout {
    /// The plot, where the data is drawn; annotations are clipped to it.
    pub plot: Rect,
    /// The left y scale's domain.
    pub left_domain: [f64; 2],
    /// The right y scale's domain.
    pub right_domain: Option<[f64; 2]>,
    /// The left axis, when it is drawn.
    pub left: Option<PlacedAxis>,
    /// The right axis, when it is drawn.
    pub right: Option<PlacedAxis>,
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
    /// The legend's row, above the panes or inside the first pane's top left.
    pub legend: Option<Rect>,
    /// The x scale's domain.
    pub x_domain: [f64; 2],
    /// The x axis, when it is drawn.
    pub x_axis: Option<PlacedAxis>,
    /// The panes, top first.
    pub panes: Vec<PaneLayout>,
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

/// `total` split in proportion to `w` by largest remainder, ties to the
/// earlier pane.
fn shares(total: usize, w: &[u128]) -> Vec<usize> {
    let sum: u128 = w.iter().sum();
    if sum == 0 {
        return vec![0; w.len()];
    }
    let t = total as u128;
    let mut out: Vec<usize> = w.iter().map(|&wi| (t * wi / sum) as usize).collect();
    let mut left = total - out.iter().sum::<usize>();
    let mut order: Vec<usize> = (0..w.len()).collect();
    order.sort_by_key(|&i| (std::cmp::Reverse(t * w[i] % sum), i));
    for &i in &order {
        if left == 0 {
            break;
        }
        out[i] += 1;
        left -= 1;
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
/// rows allow both, else split by weight alone with no separators.
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

/// The layout of a `cols × rows` chart (REQ-LAYOUT-010 to 030), placed from
/// the outside in:
///
/// 1. The title: the top row, when there is one and `rows ≥ 10`.
/// 2. The legend: the next row when it fits in one row and `rows ≥ 12`;
///    otherwise, at step 7, the first pane's top left when it takes at most a
///    third of the plot's width and the plot has three rows; otherwise none.
/// 3. The x axis, when shown: two rows at the bottom (its line and its
///    labels), one row with no labels below 6 rows.
/// 4. The panes: the rows left, by weight, at least three rows each, with a
///    blank row between panes when every pane keeps three.
/// 5. The y gutters: each y scale resolved and its ticks fitted to its
///    pane's rows (no labels on a pane under three rows); the left gutter is
///    the widest left label of any pane plus the line's cell, the right
///    likewise. When the gutters would take more than a third of the columns,
///    the right axis's labels drop, then the left's (REQ-LAYOUT-030); the
///    lines and marks stay.
/// 6. The x ticks, fitted to the columns between the gutters.
/// 7. Annotations, drawn over each pane's plot.
///
/// Below 16 columns or 5 rows there is no axis, title or legend, and the
/// panes take every cell (REQ-LAYOUT-020). Each scale's domain is resolved by
/// `scale::resolve_domain` from its options and extents, niced (when asked)
/// for the fitting's starting count. An error names the scale whose domain
/// does not resolve.
pub fn layout(cols: usize, rows: usize, input: &LayoutInput, extents: &LayoutExtents) -> Result<Layout, LayoutError> {
    let decorated = cols >= MIN_COLS && rows >= MIN_ROWS;
    let locale = &input.locale;

    // 1. The title.
    let title = (decorated && rows >= TITLE_ROWS && input.title.is_some()).then_some(Rect::new(0, 0, cols, 1));
    let mut top = usize::from(title.is_some());

    // 2. The legend.
    let legend_w = legend_width(&input.legend);
    let outside = decorated && legend_w > 0 && rows >= LEGEND_ROWS && legend_w <= cols;
    let mut legend = outside.then_some(Rect::new(0, top, legend_w, 1));
    top += usize::from(outside);

    // 3. The x axis.
    let x_rows = match (decorated && input.x.shown, rows >= X_LABEL_ROWS) {
        (false, _) => 0,
        (true, true) => 2,
        (true, false) => 1,
    };
    let bottom = rows.saturating_sub(x_rows).max(top);

    // 4. The panes.
    let weights: Vec<u32> = input.panes.iter().map(|p| p.weight).collect();
    let spans = split_rows(top, bottom - top, &weights);

    // 5. The y gutters.
    let mut panes = Vec::with_capacity(input.panes.len());
    for (i, (pane, &(y, h))) in input.panes.iter().zip(&spans).enumerate() {
        let ext = extents.panes.get(i).copied().unwrap_or_default();
        let count = (h / Y_ROWS_PER_TICK).max(1) as f64;
        let left_domain = resolve_domain(&pane.left.scale, pane.left.kind, &ext.left, count).map_err(|error| LayoutError { axis: AxisId::Left(i), error })?;
        let right_domain = match &pane.right {
            Some(a) => Some(resolve_domain(&a.scale, a.kind, &ext.right, count).map_err(|error| LayoutError { axis: AxisId::Right(i), error })?),
            None => None,
        };
        let fit = |a: &AxisInput, domain: [f64; 2], side: Side| {
            (decorated && a.shown && h > 0).then(|| {
                let mut ticks = fit_ticks(side, a.kind, domain, h, &a.format, locale);
                if h < PANE_ROWS {
                    strip(&mut ticks);
                }
                PlacedAxis { side, rect: Rect::new(0, y, 0, h), kind: a.kind, domain, ticks }
            })
        };
        let left = fit(&pane.left, left_domain, Side::Left);
        let right = pane.right.as_ref().zip(right_domain).and_then(|(a, d)| fit(a, d, Side::Right));
        panes.push(PaneLayout { plot: Rect::new(0, y, 0, h), left_domain, right_domain, left, right });
    }
    let mut left_w = gutter(panes.iter().filter_map(|p| p.left.as_ref()));
    let mut right_w = gutter(panes.iter().filter_map(|p| p.right.as_ref()));
    if 3 * (left_w + right_w) > cols {
        panes.iter_mut().filter_map(|p| p.right.as_mut()).for_each(|a| strip(&mut a.ticks));
        right_w = gutter(panes.iter().filter_map(|p| p.right.as_ref()));
    }
    if 3 * (left_w + right_w) > cols {
        panes.iter_mut().filter_map(|p| p.left.as_mut()).for_each(|a| strip(&mut a.ticks));
        left_w = gutter(panes.iter().filter_map(|p| p.left.as_ref()));
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
        PlacedAxis { side: Side::Bottom, rect: Rect::new(left_w, bottom, plot_w, x_rows), kind: x.kind, domain: x_domain, ticks }
    });

    // 7. Annotations take each pane's plot; a legend that had no row of its
    // own goes inside the first pane when it is small enough.
    if decorated && legend_w > 0 && !outside {
        if let Some(p) = panes.first().filter(|p| 3 * legend_w <= p.plot.w && p.plot.h >= PANE_ROWS) {
            legend = Some(Rect::new(p.plot.x, p.plot.y, legend_w, 1));
        }
    }

    Ok(Layout { cols, rows, decorated, title, legend, x_domain, x_axis, panes })
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
            PaneInput { weight: 3, left: AxisInput::new(Kind::Linear), right: Some(right) },
            PaneInput { weight: 1, left: AxisInput::new(Kind::Linear), right: None },
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
        input.panes[0].left = wide.clone();
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
        let axis = PlacedAxis { side, rect, kind, domain: d, ticks: ticks.clone() };
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
        let axis = PlacedAxis { side: Side::Bottom, rect: Rect::new(0, 0, 60, 2), kind: Kind::Time, domain: year, ticks: ticks.clone() };
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
}
