//! Drawing the line chart: its series as dot paths on a colour canvas, its
//! axes, labels and legend (the base), and its crosshair and readout (the
//! overlay).
//!
//! A series' path is its points in dot coordinates, the M4 points of the
//! rows in view (or every row with `sample: "none"`) between the last row
//! before the view and the first after it, so lines run to the plot's edges
//! (REQ-SAMPLE-010). A point's dot column is its x's column by the cell
//! mapping's edges; its dot row the y axis's dot. A gap (a missing y) breaks
//! the path (REQ-LINE-030). Series are drawn in z-order on one colour
//! canvas, a cell taking the ink of the last series lighting it
//! (REQ-CANVAS-040, REQ-CHART-060); in one ink, line series take the dash
//! patterns of REQ-MONO-010 counted from the x origin.

use std::ops::Range;

use super::series::Prepared;
use super::{Config, Mode, StepAt};
use crate::canvas::ColorCanvas;
use crate::cells::Axis;
use crate::downsample::{Cache, CacheKey, Edges, M4};
use crate::format::{Locale, NumberFormat, Specifier};
use crate::grid::{line_points, Grid};
use crate::layout::{Layout, Rect};
use crate::mono;
use crate::overlay;
use crate::palette::{self, INK, MUTED};
use crate::scale::Kind;
use crate::text::{stage, Frame};
use crate::view::View;

/// The plot and its axes, as a frame lays them out.
#[derive(Clone, Debug)]
pub(super) struct Geom {
    /// The layout.
    pub layout: Layout,
    /// The plot's cells.
    pub plot: Rect,
    /// The x axis over the plot's columns, 2 dots a cell.
    pub x: Axis,
    /// The y axis over the plot's rows, 4 dots a cell.
    pub y: Axis,
    /// The step between the x axis's ticks, which the readout's precision
    /// starts from.
    pub x_step: f64,
    /// The step between the y axis's ticks.
    pub y_step: f64,
    /// Each legend entry's cells, by series.
    pub legend: Vec<(usize, Rect)>,
    /// The title, when it is drawn.
    pub title: Option<String>,
}

/// A point of a path: its dot column and row, and whether a gap comes
/// before it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Pt {
    x: i64,
    y: i64,
    gap: bool,
}

/// The dot column of an x on the axis: inside the domain by the edges (the
/// high end in the last column), outside it unclamped.
fn x_dot(axis: &Axis, edges: &Edges, x: f64) -> Option<i64> {
    edges.column(x).map(|c| c as i64).or_else(|| axis.dot(x))
}

/// The row before `rows` to join the path from, and the row after it to
/// join it to: the neighbours, or with `span` the nearest rows with a y.
fn neighbours(xs: &[f64], ys: &[f64], rows: &Range<usize>, span: bool) -> (Option<usize>, Option<usize>) {
    let ok = |i: usize| xs[i].is_finite() && ys[i].is_finite();
    let before = if span { (0..rows.start).rev().find(|&i| ok(i)) } else { rows.start.checked_sub(1).filter(|&i| ok(i)) };
    let after = if span { (rows.end..xs.len()).find(|&i| ok(i)) } else { Some(rows.end).filter(|&i| i < xs.len() && ok(i)) };
    (before, after)
}

/// The path of series `s` over `rows` (the rows in view), from its M4 or
/// from every row, into `out`, whose memory is reused.
#[allow(clippy::too_many_arguments)]
fn path(data: &Prepared, s: usize, rows: &Range<usize>, m4: Option<&M4>, geom: &Geom, edges: &Edges, span: bool, out: &mut Vec<Pt>) {
    let (x, y) = (&geom.x, &geom.y);
    let (xs, ys) = (data.xs(s), data.ys(s));
    let (before, after) = neighbours(xs, ys, rows, span);
    // At most four points a bucket, or a point a row, and the two neighbours.
    out.clear();
    out.reserve(m4.map_or(rows.len(), |m| 4 * m.buckets.len()) + 2);
    let mut push = |xv: f64, yv: f64, gap: bool| {
        if let (Some(px), Some(py)) = (x_dot(x, edges, xv), y.dot(yv)) {
            out.push(Pt { x: px, y: py, gap });
        }
    };
    if let Some(b) = before {
        push(xs[b], ys[b], false);
    }
    let mut gap = false;
    match m4 {
        Some(m) => {
            for (p, g) in m.points() {
                push(xs[p.row], p.y, g);
            }
            gap = m.gap_after;
        }
        None => {
            for i in rows.clone() {
                let (xv, yv) = (xs[i], ys[i]);
                if !(xv >= edges.values()[0] && xv <= edges.values()[edges.dots()]) {
                    continue;
                }
                if !yv.is_finite() {
                    gap |= !span;
                    continue;
                }
                push(xv, yv, gap);
                gap = false;
            }
        }
    }
    if let Some(a) = after {
        push(xs[a], ys[a], gap);
    }
}

/// What drawing the base reuses from frame to frame, so a frame after a data
/// change allocates no new memory for it (REQ-MEM-030).
#[derive(Clone, Debug, Default)]
pub(super) struct Scratch {
    /// A series' path.
    pts: Vec<Pt>,
}

/// Calls `plot` for every dot of the path in `mode`, clipped to `w × h`.
fn walk(points: &[Pt], mode: Mode, step: StepAt, w: i64, h: i64, mut plot: impl FnMut(i64, i64)) {
    let inside = |x: i64, y: i64| x >= 0 && y >= 0 && x < w && y < h;
    for (i, p) in points.iter().enumerate() {
        let next = points.get(i + 1).filter(|q| !q.gap);
        let prev_joined = i > 0 && !p.gap;
        match next {
            Some(q) => match mode {
                Mode::Step => {
                    let (a, b) = (*p, *q);
                    match step {
                        StepAt::After => {
                            line_points(a.x, a.y, b.x, a.y, w, h, &mut plot);
                            line_points(b.x, a.y, b.x, b.y, w, h, &mut plot);
                        }
                        StepAt::Before => {
                            line_points(a.x, a.y, a.x, b.y, w, h, &mut plot);
                            line_points(a.x, b.y, b.x, b.y, w, h, &mut plot);
                        }
                        StepAt::Middle => {
                            let m = a.x + (b.x - a.x).div_euclid(2);
                            line_points(a.x, a.y, m, a.y, w, h, &mut plot);
                            line_points(m, a.y, m, b.y, w, h, &mut plot);
                            line_points(m, b.y, b.x, b.y, w, h, &mut plot);
                        }
                    }
                }
                Mode::Line | Mode::Area => line_points(p.x, p.y, q.x, q.y, w, h, &mut plot),
            },
            None if !prev_joined && inside(p.x, p.y) => plot(p.x, p.y),
            None => {}
        }
    }
}

/// Everything the base is drawn from besides the data.
pub(super) struct BaseInput<'a> {
    /// The parsed options.
    pub cfg: &'a Config,
    /// The plot and axes.
    pub geom: &'a Geom,
    /// The view state.
    pub view: &'a View,
    /// The data version, for the M4 cache.
    pub version: u64,
    /// The rows in view, a range a series.
    pub rows: &'a [Range<usize>],
    /// One ink.
    pub mono: bool,
    /// The frame's ground.
    pub ground: u8,
}

/// The ink of series `s`.
pub(super) fn series_ink(s: usize) -> u8 {
    palette::series(s % palette::SERIES_COUNT as usize)
}

/// Draws the base (data, axes, labels, legend) into `grid` (REQ-TEXT-020).
pub(super) fn base(grid: &mut Grid, canvas: &mut ColorCanvas, cache: &mut Cache, scratch: &mut Scratch, data: &Prepared, input: &BaseInput) {
    let BaseInput { cfg, geom, view, version, rows, mono, ground } = *input;
    let plot = geom.plot;
    let mut f = Frame::new(grid, ground);
    let shown: Vec<usize> = (0..data.series.len()).filter(|&s| !view.is_hidden(&data.series[s].name)).collect();
    let any = data.rows() > 0 && !data.series.is_empty();
    let patterned = match cfg.patterns {
        super::Patterns::On => true,
        super::Patterns::Off => false,
        super::Patterns::Auto => mono,
    };
    if plot.w > 0 && plot.h > 0 && !shown.is_empty() {
        let edges = Edges::new(&geom.x);
        let span = cfg.span_gaps;
        let key = CacheKey::new(version, [geom.x.lo(), geom.x.hi()], edges.dots(), span);
        let m4s: Option<&[M4]> = if cfg.sample_none {
            None
        } else {
            Some(cache.get_into(key, data.series.len(), |s, m| crate::downsample::m4_into(data.xs(s), data.ys(s), rows[s].clone(), &edges, span, m)))
        };
        canvas.resize(plot.w, plot.h);
        let (w, h) = (canvas.width() as i64, canvas.height() as i64);
        let origin = mono::origin(&geom.x);
        // Z-order: the series list, with selected series raised to the top
        // (REQ-CHART-060).
        let raised: Vec<&str> = view.selection.as_ref().map(|s| s.points.iter().map(|p| p.series.as_str()).collect()).unwrap_or_default();
        let mut order: Vec<usize> = shown.iter().copied().filter(|&s| !raised.contains(&data.series[s].name.as_str())).collect();
        order.extend(shown.iter().copied().filter(|&s| raised.contains(&data.series[s].name.as_str())));
        let base_dot = geom.y.dot(cfg.baseline).unwrap_or(h);
        for s in order {
            path(data, s, &rows[s], m4s.map(|m| &m[s]), geom, &edges, span, &mut scratch.pts);
            let pts = &scratch.pts;
            let ink = series_ink(s);
            if cfg.mode == Mode::Area {
                let (mut top, mut bot) = (vec![i64::MAX; w as usize], vec![i64::MIN; w as usize]);
                walk(pts, Mode::Line, cfg.step, w, h, |x, y| {
                    top[x as usize] = top[x as usize].min(y);
                    bot[x as usize] = bot[x as usize].max(y);
                });
                let base_dot = base_dot.max(-1).min(h);
                for x in 0..w as usize {
                    if top[x] <= bot[x] {
                        let (a, b) = (top[x].min(base_dot).max(0), bot[x].max(base_dot).min(h - 1));
                        for y in a..=b {
                            canvas.set(x as i64, y, ink);
                        }
                    }
                }
            } else {
                walk(pts, cfg.mode, cfg.step, w, h, |x, y| {
                    if !patterned || mono::lit(s, x, origin) {
                        canvas.set(x, y, ink);
                    }
                });
            }
        }
        canvas.draw(f.data(), plot.x as i32, plot.y as i32);
    }
    let mut f = f.then::<stage::Axes>();
    let layout = &geom.layout;
    if let Some(a) = &layout.x_axis {
        f.axis(a);
    }
    for p in &layout.panes {
        for a in p.left.iter().chain(p.right.iter()) {
            f.axis(a);
        }
    }
    f.corners(layout);
    let mut f = f.then::<stage::Labels>();
    if let (Some(rect), Some(title)) = (layout.title, &geom.title) {
        f.label_in(rect, title, INK);
    }
    // Each key is the line its series is drawn with: its dash pattern when
    // patterned (one ink, or `patterns: "on"`), else solid (REQ-MONO-040).
    for (s, span) in &geom.legend {
        let hidden = view.is_hidden(&data.series[*s].name);
        let ink = if hidden { MUTED } else { series_ink(*s) };
        let mut key = [0u8; 4];
        let glyph = if patterned { mono::key(*s) } else { mono::KEYS[0] };
        f.label(span.x, span.y, glyph.encode_utf8(&mut key), ink);
        f.label(span.x + 2, span.y, &data.series[*s].name, if hidden { MUTED } else { INK });
    }
    if let Some(a) = &layout.x_axis {
        f.axis_labels(a);
    }
    for p in &layout.panes {
        for a in p.left.iter().chain(p.right.iter()) {
            f.axis_labels(a);
        }
    }
    if !any && plot.w >= 7 && plot.h >= 1 {
        let text = "no data";
        f.label(plot.x + (plot.w - text.len()) / 2, plot.y + plot.h / 2, text, MUTED);
    }
}

/// A number as readouts and tables write it: d3-format's default, twelve
/// significant digits with zeros trimmed, in the locale (REQ-FMT-030).
pub(super) fn number(v: f64, locale: &Locale) -> String {
    NumberFormat::new(&Specifier::default(), locale).format(v)
}

/// A count as captions and labels write it: whole, with the locale's group
/// separator.
pub(super) fn count(n: usize, locale: &Locale) -> String {
    match Specifier::parse("count", ",d") {
        Ok(spec) => NumberFormat::new(&spec, locale).format(n as f64),
        Err(_) => number(n as f64, locale),
    }
}

/// An x as readouts and tables write it: a date and time for a time scale,
/// to the millisecond when the view spans under ten seconds, else a number.
pub(super) fn x_label(v: f64, kind: Kind, span: f64, locale: &Locale) -> String {
    match kind {
        Kind::Linear => number(v, locale),
        Kind::Time => {
            let pattern = if span < 10_000.0 { "%Y-%m-%d %H:%M:%S.%L" } else { "%Y-%m-%d %H:%M:%S" };
            let mut out = String::new();
            crate::time::strftime(pattern, v, &locale.time, &mut out);
            out
        }
    }
}

/// Draws the overlay over the base already in `grid`: the brush, the time
/// cursor, the crosshair and the readout (REQ-OVERLAY-010, REQ-OVERLAY-020).
pub(super) fn overlay(grid: &mut Grid, geom: &Geom, view: &View, readout: Option<&str>, ground: u8) {
    let plot = geom.plot;
    if plot.w == 0 || plot.h == 0 {
        return;
    }
    let edges = Edges::new(&geom.x);
    if let Some([a, b]) = view.selection.as_ref().and_then(|s| s.x) {
        if let (Some(ca), Some(cb)) = (geom.x.cell(a), geom.x.cell(b)) {
            overlay::brush(grid, plot, ca, cb);
        }
    }
    let mut f = Frame::overlay(grid, ground);
    if let Some(t) = view.time.and_then(|t| x_dot(&geom.x, &edges, t)) {
        overlay::time_cursor(&mut f, plot, t);
    }
    let Some(c) = &view.cursor else { return };
    let xd = c.x.and_then(|x| x_dot(&geom.x, &edges, x));
    let yd = c.y.and_then(|y| geom.y.dot(y));
    overlay::crosshair(&mut f, plot, xd, yd);
    if let Some(text) = readout {
        overlay::readout(f, plot, text);
    }
}
