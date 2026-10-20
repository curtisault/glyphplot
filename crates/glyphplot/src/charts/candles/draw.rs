//! Drawing the candlestick chart: its candles or bars and its volume (the
//! base's data), its axes and labels, and its crosshair and readout (the
//! overlay).
//!
//! **Glyphs by spacing** (REQ-CANDLE-020), the spacing being the plot's
//! cells a candle:
//! - **4 cells or more: box drawing.** A body three cells wide (five from six
//!   cells), rows by the price axis's rows: hollow as an outline `┌─┐ │ │
//!   └─┘` (one row: `├─┤`), the wick joining it with `┴` and `┬`; solid in
//!   `█`, `▀` and `▄` to the half row. The wick is `│`, ending in `╷` or `╵`
//!   at a half row. A bar (`ohlc`) is a `│` with its open `─┤` on the left and
//!   its close `├─` on the right.
//! - **2 or 3 cells: braille, a blank cell between.** One cell a candle: the
//!   wick in its left dot column, a solid body in both, a hollow body its
//!   right column at the body's top and bottom dots only. A bar is its left
//!   dot column with its close a dot on its right, its open left out; at 3
//!   cells it is its right dot column, its open a dot on the left and its
//!   close a dot in the next cell's left column.
//! - **1 cell: braille without the gap**, as at 2 cells.
//!
//! **Up and down** (REQ-THEME-060) are the palette's `UP` and `DOWN` tokens,
//! and are told apart by fill as well: a rising candle (close at or above
//! open) is hollow and a falling one solid in every mode and env. `candle`
//! colours by close against open; `hollow` by close against the previous
//! candle's close. A volume bar is `░` under a rising candle and `█` (with
//! eighths at its top) under a falling one. In one ink the chart resamples
//! so every candle has four cells (REQ-CANDLE-030), so one-ink candles are
//! always box drawing.

use std::ops::Range;
use std::sync::Arc;

use super::resample::{Level, Place};
use super::Mode;
use crate::canvas::ColorCanvas;
use crate::cells::Axis;
use crate::grid::Grid;
use crate::layout::{Layout, Rect};
use crate::overlay;
use crate::palette::{ACCENT, DOWN, INK, MUTED, UP};
use crate::text::{stage, Frame};
use crate::view::View;

/// `│`.
const V: u32 = 0x2502;
/// `─`.
const H: u32 = 0x2500;
/// `╷`, a line in a cell's lower half.
const V_LOW: u32 = 0x2577;
/// `╵`, a line in a cell's upper half.
const V_HIGH: u32 = 0x2575;
/// `┌ ┐ └ ┘`.
const CORNERS: [u32; 4] = [0x250C, 0x2510, 0x2514, 0x2518];
/// `┴`, a wick meeting a body's top.
const UP_T: u32 = 0x2534;
/// `┬`, a wick meeting a body's bottom.
const DOWN_T: u32 = 0x252C;
/// `┼`.
const CROSS: u32 = 0x253C;
/// `├`.
const RIGHT_T: u32 = 0x251C;
/// `┤`.
const LEFT_T: u32 = 0x2524;
/// `█`.
const FULL: u32 = 0x2588;
/// `▀`.
const UPPER: u32 = 0x2580;
/// `▄`.
const LOWER: u32 = 0x2584;
/// `░`, a rising candle's volume.
const LIGHT: u32 = 0x2591;
/// `▁`, the first of the lower eighth blocks `▁▂▃▄▅▆▇`.
const EIGHTH: u32 = 0x2581;

/// How candles are drawn at a spacing (REQ-CANDLE-020).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glyphs {
    /// Box drawing, 4 cells or more a candle.
    Box,
    /// Braille with a blank cell between, 2 or 3 cells.
    Gap,
    /// Braille without the gap, under 2 cells.
    Dense,
}

impl Glyphs {
    /// The glyphs of a spacing in cells.
    pub fn of(spacing: f64) -> Glyphs {
        if spacing >= 4.0 {
            Glyphs::Box
        } else if spacing >= 2.0 {
            Glyphs::Gap
        } else {
            Glyphs::Dense
        }
    }
}

/// The plot, its panes and the candles drawn, as a frame lays them out.
#[derive(Clone, Debug)]
pub(super) struct Geom {
    /// The layout.
    pub layout: Layout,
    /// The price pane's plot.
    pub price: Rect,
    /// The volume pane's plot.
    pub volume: Option<Rect>,
    /// The panes' plots and what lies between them: where hits answer.
    pub all: Rect,
    /// Positions over the plot's columns, 2 dots a cell.
    pub x: Axis,
    /// The time domain drawn.
    pub time: [f64; 2],
    /// The price axis over the price pane's rows, 4 dots a cell.
    pub y: Axis,
    /// The volume axis over its pane's rows.
    pub vy: Option<Axis>,
    /// The candles' level.
    pub level: Arc<Level>,
    /// Index or time positions.
    pub place: Place,
    /// The candles drawn.
    pub shown: Range<usize>,
    /// Cells a candle.
    pub spacing: f64,
    /// The price axis's displayed tick step, which the readout's digits
    /// start from.
    pub y_step: f64,
    /// The close before the first candle drawn, for `hollow`'s colour.
    pub before: f64,
    /// The title, when drawn.
    pub title: Option<String>,
}

impl Geom {
    /// The plot column of candle `k`'s centre, when it is on the plot.
    pub fn column(&self, k: usize) -> Option<usize> {
        self.x.dot_in(self.level.centre(self.place, k)).map(|d| self.price.x + d / 2)
    }

    /// The glyphs the spacing asks for.
    pub fn glyphs(&self) -> Glyphs {
        Glyphs::of(self.spacing)
    }
}

/// Candle `k`'s ink and whether it is hollow, in `mode`, after a candle
/// that closed at `prev` (REQ-THEME-060).
pub(super) fn style(mode: Mode, level: &Level, k: usize, prev: f64) -> (u8, bool) {
    let (o, c) = (level.ohlc[0][k], level.ohlc[3][k]);
    let up = c >= o;
    let ink_up = match mode {
        Mode::Hollow if prev.is_finite() => c >= prev,
        _ => up,
    };
    (if ink_up { UP } else { DOWN }, up)
}

/// The half row of a value, 0 at the plot's top, held to the plot.
fn half(y: &Axis, v: f64) -> i64 {
    let rows = y.cells() as i64;
    let p = (y.position(v) * 2.0).floor();
    if p.is_nan() {
        return 0;
    }
    (p.max(0.0) as i64).min(2 * rows - 1)
}

/// Sets a cell of the plot, clipped to it.
fn put(grid: &mut Grid, plot: Rect, col: i64, row: i64, glyph: u32, ink: u8) {
    if col >= plot.x as i64 && col < (plot.x + plot.w) as i64 && row >= 0 && row < plot.h as i64 {
        grid.set(col as i32, (plot.y as i64 + row) as i32, glyph, ink);
    }
}

/// A candle's values as drawn: open, close, the wick's top and bottom.
struct Bar {
    o: f64,
    c: f64,
    top: f64,
    bottom: f64,
}

impl Bar {
    fn of(level: &Level, k: usize) -> Bar {
        let [o, h, l, c] = [0, 1, 2, 3].map(|i| level.ohlc[i][k]);
        Bar { o, c, top: h.max(o).max(c), bottom: l.min(o).min(c) }
    }
}

/// A box-drawing candle or bar at column `c`, `hw` cells either side
/// (REQ-CANDLE-020).
#[allow(clippy::too_many_arguments)]
fn box_candle(grid: &mut Grid, plot: Rect, y: &Axis, c: i64, hw: i64, b: &Bar, ink: u8, hollow: bool, ohlc: bool) {
    let (hh, hl) = (half(y, b.top), half(y, b.bottom));
    let wick = |grid: &mut Grid, from: i64, to: i64| {
        for r in from / 2..=to / 2 {
            let g = if r == from / 2 && from % 2 == 1 && r == to / 2 && to % 2 == 0 {
                V
            } else if r == from / 2 && from % 2 == 1 {
                V_LOW
            } else if r == to / 2 && to % 2 == 0 {
                V_HIGH
            } else {
                V
            };
            put(grid, plot, c, r, g, ink);
        }
    };
    if ohlc {
        wick(grid, hh, hl);
        let (ro, rc) = (half(y, b.o) / 2, half(y, b.c) / 2);
        for d in 1..=hw {
            put(grid, plot, c - d, ro, H, ink);
            put(grid, plot, c + d, rc, H, ink);
        }
        put(grid, plot, c, ro, LEFT_T, ink);
        put(grid, plot, c, rc, if ro == rc { CROSS } else { RIGHT_T }, ink);
        return;
    }
    let (ht, hb) = (half(y, b.o.max(b.c)), half(y, b.o.min(b.c)));
    let (rt, rb) = (ht / 2, hb / 2);
    if hh / 2 < rt {
        wick(grid, hh, 2 * rt - 1);
    }
    if hl / 2 > rb {
        wick(grid, 2 * rb + 2, hl);
    }
    let (above, below) = (hh / 2 < rt, hl / 2 > rb);
    for r in rt..=rb {
        for d in -hw..=hw {
            let g = if !hollow {
                let (upper, lower) = ((ht..=hb).contains(&(2 * r)), (ht..=hb).contains(&(2 * r + 1)));
                match (upper, lower) {
                    (true, true) => FULL,
                    (true, false) => UPPER,
                    (false, true) => LOWER,
                    (false, false) => continue,
                }
            } else if rt == rb {
                match d {
                    _ if d == -hw => RIGHT_T,
                    _ if d == hw => LEFT_T,
                    0 if above && below => CROSS,
                    0 if above => UP_T,
                    0 if below => DOWN_T,
                    _ => H,
                }
            } else if r == rt || r == rb {
                let (l, rr) = if r == rt { (CORNERS[0], CORNERS[1]) } else { (CORNERS[2], CORNERS[3]) };
                match d {
                    _ if d == -hw => l,
                    _ if d == hw => rr,
                    0 if r == rt && above => UP_T,
                    0 if r == rb && below => DOWN_T,
                    _ => H,
                }
            } else if d == -hw || d == hw {
                V
            } else {
                continue;
            };
            put(grid, plot, c + d, r, g, ink);
        }
    }
}

/// A braille candle or bar in plot cell `cell` (REQ-CANDLE-020); `wide`
/// when there are 3 cells a candle, room for a bar's close in the next cell.
#[allow(clippy::too_many_arguments)]
fn dot_candle(canvas: &mut ColorCanvas, y: &Axis, cell: i64, b: &Bar, ink: u8, hollow: bool, ohlc: bool, wide: bool) {
    let dot = |v: f64| y.dot(v).unwrap_or(0);
    let (left, right) = (2 * cell, 2 * cell + 1);
    let (dh, dl) = (dot(b.top), dot(b.bottom));
    if ohlc {
        let bar = if wide { right } else { left };
        for d in dh..=dl {
            canvas.set(bar, d, ink);
        }
        if wide {
            canvas.set(left, dot(b.o), ink);
            canvas.set(right + 1, dot(b.c), ink);
        } else {
            canvas.set(right, dot(b.c), ink);
        }
        return;
    }
    for d in dh..=dl {
        canvas.set(left, d, ink);
    }
    let (dt, db) = (dot(b.o.max(b.c)), dot(b.o.min(b.c)));
    if hollow {
        canvas.set(right, dt, ink);
        canvas.set(right, db, ink);
    } else {
        for d in dt..=db {
            canvas.set(right, d, ink);
        }
    }
}

/// A volume bar over columns `cols` of the volume pane (REQ-CANDLE-040).
fn volume_bar(grid: &mut Grid, plot: Rect, vy: &Axis, cols: Range<i64>, v: f64, ink: u8, light: bool) {
    if v.is_nan() || v <= 0.0 {
        return;
    }
    let h = plot.h as f64;
    let top = vy.position(v).max(0.0).min(h);
    let eighths = ((h - top) * 8.0).round() as i64;
    for r in 0..plot.h as i64 {
        let from_bottom = plot.h as i64 - 1 - r;
        let fill = (eighths - 8 * from_bottom).min(8);
        if fill <= 0 {
            continue;
        }
        let g = if light {
            if fill < 4 {
                continue;
            }
            LIGHT
        } else if fill >= 8 {
            FULL
        } else {
            EIGHTH + fill as u32 - 1
        };
        for c in cols.clone() {
            put(grid, plot, c, r, g, ink);
        }
    }
}

/// Everything the base is drawn from besides the data.
pub(super) struct BaseInput<'a> {
    /// The mode.
    pub mode: Mode,
    /// The plot and axes.
    pub geom: &'a Geom,
    /// Whether the data has every price channel and a row.
    pub drawable: bool,
    /// The frame's ground.
    pub ground: u8,
}

/// Draws the base (candles, volume, axes, labels) into `grid`
/// (REQ-TEXT-020).
pub(super) fn base(grid: &mut Grid, canvas: &mut ColorCanvas, input: &BaseInput) {
    let BaseInput { mode, geom, drawable, ground } = *input;
    let mut f = Frame::new(grid, ground);
    let plot = geom.price;
    let lv = &*geom.level;
    if drawable && plot.w > 0 && plot.h > 0 && !geom.shown.is_empty() {
        let glyphs = geom.glyphs();
        let ohlc = mode == Mode::Ohlc;
        let hw = if geom.spacing >= 6.0 { 2 } else { 1 };
        canvas.resize(plot.w, plot.h);
        let mut prev = geom.before;
        for k in geom.shown.clone() {
            let (ink, hollow) = style(mode, lv, k, prev);
            prev = if lv.ohlc[3][k].is_finite() { lv.ohlc[3][k] } else { prev };
            let Some(col) = geom.column(k) else { continue };
            if lv.whole(k) {
                let b = Bar::of(lv, k);
                match glyphs {
                    Glyphs::Box => box_candle(f.data(), plot, &geom.y, col as i64, hw, &b, ink, hollow, ohlc),
                    _ => dot_candle(canvas, &geom.y, (col - plot.x) as i64, &b, ink, hollow, ohlc, geom.spacing >= 3.0),
                }
            }
            if let (Some(vp), Some(vy)) = (geom.volume, &geom.vy) {
                let cols = if glyphs == Glyphs::Box { col as i64 - hw..col as i64 + hw + 1 } else { col as i64..col as i64 + 1 };
                volume_bar(f.data(), vp, vy, cols, lv.volume[k], ink, hollow);
            }
        }
        if glyphs != Glyphs::Box {
            canvas.draw(f.data(), plot.x as i32, plot.y as i32);
        }
    }
    let mut f = f.then::<stage::Axes>();
    let layout = &geom.layout;
    if let Some(a) = &layout.x_axis {
        f.axis(a);
    }
    for p in &layout.panes {
        if let Some(a) = &p.left {
            f.axis(a);
        }
    }
    f.corners(layout);
    let mut f = f.then::<stage::Labels>();
    if let (Some(rect), Some(title)) = (layout.title, &geom.title) {
        f.label_in(rect, title, INK);
    }
    if let Some(a) = &layout.x_axis {
        f.axis_labels(a);
    }
    for p in &layout.panes {
        if let Some(a) = &p.left {
            f.axis_labels(a);
        }
    }
    if !drawable && plot.w >= 7 && plot.h >= 1 {
        let text = "no data";
        f.label(plot.x + (plot.w - text.len()) / 2, plot.y + plot.h / 2, text, MUTED);
    }
}

/// The dot column of a time on the plot, through the candles' positions.
fn time_dot(geom: &Geom, t: f64) -> Option<i64> {
    geom.x.dot(geom.level.pos(geom.place, t))
}

/// Draws the overlay over the base already in `grid`: the brush, the time
/// cursor, the crosshair and the readout's lines (REQ-OVERLAY-010,
/// REQ-OVERLAY-020).
pub(super) fn overlay(grid: &mut Grid, geom: &Geom, view: &View, cursor: Option<(usize, f64)>, readout: &[String], ground: u8) {
    let all = geom.all;
    if all.w == 0 || all.h == 0 {
        return;
    }
    if let Some([a, b]) = view.selection.as_ref().and_then(|s| s.x) {
        if let (Some(da), Some(db)) = (time_dot(geom, a), time_dot(geom, b)) {
            overlay::brush(grid, all, da.div_euclid(2), db.div_euclid(2));
        }
    }
    let mut f = Frame::overlay(grid, ground);
    if let Some(t) = view.time.and_then(|t| time_dot(geom, t)) {
        overlay::time_cursor(&mut f, all, t);
    }
    if let Some((k, v)) = cursor {
        if let Some(d) = geom.x.dot(geom.level.centre(geom.place, k)) {
            overlay::rule_x(&mut f, all, d, ACCENT);
        }
        if let Some(d) = geom.y.dot(v) {
            overlay::rule_y(&mut f, geom.price, d, ACCENT);
        }
    }
    let plot = geom.price;
    let mut f = f.then::<stage::Readout>();
    for (i, line) in readout.iter().enumerate().take(plot.h) {
        if let Some((x, y, shown)) = overlay::readout_place(plot, line) {
            f.readout(x, y + i, &shown, INK);
        }
    }
}
