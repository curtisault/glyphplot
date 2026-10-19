//! Labels in whole cells: widths by `unicode-width`, labels clearing the cells
//! under them, and the draw order (REQ-TEXT-010, REQ-TEXT-020, REQ-I18N-040).
//!
//! A label's width is the sum of its drawn code points' widths by
//! `unicode-width` 0.2, one code point at a time. A zero-width or control
//! code point takes no cell. A grapheme of several scalars is drawn as its
//! first scalar: the core holds no grapheme segmentation, so [`glyphs`]
//! finds the clusters a chart's labels meet (a combining or zero-width mark,
//! a zero-width-joiner sequence, an emoji modifier, a regional-indicator
//! pair) and drops all but the first scalar of each. The accessible text
//! (the data table and the element's label) keeps the original string; only
//! the grid holds the shortened one (REQ-I18N-040, REQ-I18N-050).
//!
//! A [`Frame`] paints one frame in the z-order and no other: data,
//! gridlines on empty cells only, axes, labels, annotations, the crosshair,
//! the readout last (REQ-TEXT-020). Each stage is its own type, and a frame
//! only moves forward ([`Frame::then`]), so a chart cannot draw a gridline
//! over its data or data over its labels.
//!
//! ```compile_fail
//! use glyphplot::grid::{Grid, NO_BACK};
//! use glyphplot::text::{stage, Frame};
//!
//! let mut grid = Grid::new(20, 5);
//! let frame = Frame::new(&mut grid, NO_BACK).then::<stage::Labels>();
//! // A frame cannot go back from labels to data.
//! let frame = frame.then::<stage::Data>();
//! ```

use std::marker::PhantomData;

use unicode_width::UnicodeWidthChar;

use crate::canvas::Glyphs as DotGlyphs;
use crate::grid::{Grid, BLANK};
use crate::layout::{Layout, PlacedAxis, Rect, Side, X_DOTS, Y_DOTS};
use crate::palette::{INK, MUTED, RULE};

/// `─`, the x axis line.
const H_LINE: u32 = 0x2500;
/// `│`, the y axis line.
const V_LINE: u32 = 0x2502;
/// `┬`, a tick mark on the x axis.
const TICK_DOWN: u32 = 0x252C;
/// `┤`, a tick mark on a left axis.
const TICK_LEFT: u32 = 0x2524;
/// `├`, a tick mark on a right axis.
const TICK_RIGHT: u32 = 0x251C;
/// `└`, where a left axis meets the x axis.
const CORNER_LEFT: u32 = 0x2514;
/// `┘`, where a right axis meets the x axis.
const CORNER_RIGHT: u32 = 0x2518;
/// `…`, the end of a shortened label.
const ELLIPSIS: char = '\u{2026}';
/// The zero-width joiner, which joins the next scalar to its grapheme.
const ZWJ: char = '\u{200D}';

/// Whether `c` is an emoji modifier (a skin tone), which joins the scalar
/// before it.
fn is_modifier(c: char) -> bool {
    ('\u{1F3FB}'..='\u{1F3FF}').contains(&c)
}

/// Whether `c` is a regional indicator, two of which make a flag.
fn is_regional(c: char) -> bool {
    ('\u{1F1E6}'..='\u{1F1FF}').contains(&c)
}

/// The glyphs a label draws, in order, each with the cells it takes (1 or 2):
/// the first scalar of each grapheme that takes a cell (REQ-I18N-040). See
/// the module's notes for the clusters it recognises.
pub fn glyphs(text: &str) -> impl Iterator<Item = (char, usize)> + '_ {
    let mut joined = false;
    let mut flag_open = false;
    let mut drawn = false;
    text.chars().filter_map(move |c| {
        if c == ZWJ {
            joined = drawn;
            return None;
        }
        let w = c.width().unwrap_or(0);
        if w == 0 {
            return None;
        }
        let regional = is_regional(c);
        let part = joined || (drawn && is_modifier(c)) || (regional && flag_open);
        joined = false;
        if part {
            flag_open = false;
            return None;
        }
        flag_open = regional;
        drawn = true;
        Some((c, w))
    })
}

/// The cells a label takes: the sum of [`glyphs`]' widths (REQ-I18N-040).
pub fn width(text: &str) -> usize {
    glyphs(text).map(|(_, w)| w).sum()
}

/// The label as drawn in at most `cells` cells: its glyphs whole when they
/// fit, else as many as fit before a `…`. No glyph is cut in half.
pub fn truncate(text: &str, cells: usize) -> String {
    if width(text) <= cells {
        return glyphs(text).map(|(c, _)| c).collect();
    }
    let mut out = String::new();
    let mut used = 0;
    if cells == 0 {
        return out;
    }
    for (c, w) in glyphs(text) {
        if used + w > cells - 1 {
            break;
        }
        out.push(c);
        used += w;
    }
    out.push(ELLIPSIS);
    out
}

/// A layer of a frame. The variants are in draw order, so the derived order
/// is the z-order (REQ-TEXT-020).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Layer {
    /// The chart's marks.
    Data,
    /// Gridlines, drawn on empty cells only.
    Gridlines,
    /// Axis lines and tick marks.
    Axes,
    /// Tick labels, the title and the legend.
    Labels,
    /// Annotations over the data.
    Annotations,
    /// The crosshair.
    Crosshair,
    /// The readout, over everything.
    Readout,
}

impl Layer {
    /// Every layer, bottom first (REQ-TEXT-020).
    pub const ALL: [Layer; 7] = [Layer::Data, Layer::Gridlines, Layer::Axes, Layer::Labels, Layer::Annotations, Layer::Crosshair, Layer::Readout];
}

mod sealed {
    /// Keeps [`super::Stage`] to this module's stages.
    pub trait Sealed {}
}

/// A stage of a [`Frame`], one a [`Layer`] (REQ-TEXT-020).
pub trait Stage: sealed::Sealed {
    /// The layer this stage draws.
    const LAYER: Layer;
}

/// That a frame may go from this stage to the stage `T`: true only when `T`
/// is drawn later (REQ-TEXT-020).
pub trait Before<T: Stage>: Stage {}

/// The stages of a frame, one a [`Layer`], in draw order (REQ-TEXT-020).
pub mod stage {
    /// Drawing the data.
    #[derive(Debug)]
    pub struct Data;
    /// Drawing gridlines on empty cells.
    #[derive(Debug)]
    pub struct Gridlines;
    /// Drawing axis lines and tick marks.
    #[derive(Debug)]
    pub struct Axes;
    /// Drawing tick labels, the title and the legend.
    #[derive(Debug)]
    pub struct Labels;
    /// Drawing annotations.
    #[derive(Debug)]
    pub struct Annotations;
    /// Drawing the crosshair.
    #[derive(Debug)]
    pub struct Crosshair;
    /// Drawing the readout.
    #[derive(Debug)]
    pub struct Readout;
}

macro_rules! stages {
    ($($s:ident),*) => {
        $(
            impl sealed::Sealed for stage::$s {}
            impl Stage for stage::$s {
                const LAYER: Layer = Layer::$s;
            }
        )*
    };
}
stages!(Data, Gridlines, Axes, Labels, Annotations, Crosshair, Readout);

macro_rules! before {
    ($a:ident => $($b:ident),*) => {
        $(impl Before<stage::$b> for stage::$a {})*
    };
}
before!(Data => Gridlines, Axes, Labels, Annotations, Crosshair, Readout);
before!(Gridlines => Axes, Labels, Annotations, Crosshair, Readout);
before!(Axes => Labels, Annotations, Crosshair, Readout);
before!(Labels => Annotations, Crosshair, Readout);
before!(Annotations => Crosshair, Readout);
before!(Crosshair => Readout);

/// A grid coordinate as the grid's `i32`; one past `i32::MAX` clips off it.
fn at(v: usize) -> i32 {
    i32::try_from(v).unwrap_or(i32::MAX)
}

/// One frame painted onto a grid, stage by stage in the z-order
/// (REQ-TEXT-020), keeping a record of the cells labels hold so no two labels
/// share a cell (REQ-TEXT-010).
///
/// The frame starts by clearing the grid: every cell blank, in ink 0, with
/// the ground as its back. The ground is `NO_BACK` when the page's own
/// background shows through, or `palette::GROUND` for a chart with a ground
/// of its own.
#[derive(Debug)]
pub struct Frame<'g, S: Stage> {
    grid: &'g mut Grid,
    ground: u8,
    labels: Vec<bool>,
    stage: PhantomData<S>,
}

impl<'g> Frame<'g, stage::Data> {
    /// A frame on `grid`, cleared to `ground`, at the data stage.
    pub fn new(grid: &'g mut Grid, ground: u8) -> Frame<'g, stage::Data> {
        grid.clear();
        if ground != crate::grid::NO_BACK {
            for y in 0..grid.rows() {
                for x in 0..grid.cols() {
                    grid.set_back(at(x), at(y), ground);
                }
            }
        }
        let labels = vec![false; grid.cols() * grid.rows()];
        Frame { grid, ground, labels, stage: PhantomData }
    }
}

impl<'g> Frame<'g, stage::Crosshair> {
    /// A frame on a `grid` that already holds a frame's base (data, axes and
    /// labels), resumed at the crosshair stage without clearing it, so the
    /// overlay is drawn over a cached base (REQ-OVERLAY-010, REQ-TEXT-020).
    /// The base's labels are not recorded: only the readout, which may cover
    /// a label, is drawn after this stage.
    pub fn overlay(grid: &'g mut Grid, ground: u8) -> Frame<'g, stage::Crosshair> {
        Frame { grid, ground, labels: Vec::new(), stage: PhantomData }
    }
}

impl<'g, S: Stage> Frame<'g, S> {
    /// The layer this frame is drawing.
    pub fn layer(&self) -> Layer {
        S::LAYER
    }

    /// The frame at a later stage; going back does not compile
    /// (REQ-TEXT-020).
    pub fn then<T: Stage>(self) -> Frame<'g, T>
    where
        S: Before<T>,
    {
        Frame { grid: self.grid, ground: self.ground, labels: self.labels, stage: PhantomData }
    }

    /// The grid as painted so far.
    pub fn grid(&self) -> &Grid {
        self.grid
    }

    /// The back a label clears its cells to: the frame's ground.
    pub fn ground(&self) -> u8 {
        self.ground
    }

    /// Whether a label holds the cell at `(x, y)`; false off the grid.
    pub fn is_label(&self, x: usize, y: usize) -> bool {
        x < self.grid.cols() && y < self.grid.rows() && self.labels.get(y * self.grid.cols() + x).copied().unwrap_or(false)
    }

    /// Clears the `w` cells from `(x, y)` to the ground, then draws `text`
    /// there in `ink`, clipping; wide glyphs go through `set_wide`
    /// (REQ-I18N-040).
    fn paint(&mut self, x: usize, y: usize, w: usize, text: &str, ink: u8) {
        for c in x..x.saturating_add(w) {
            self.grid.set(at(c), at(y), BLANK, INK);
            self.grid.set_back(at(c), at(y), self.ground);
        }
        let mut c = x;
        for (g, gw) in glyphs(text) {
            if gw == 2 {
                self.grid.set_wide(at(c), at(y), g as u32, ink);
            } else {
                self.grid.set(at(c), at(y), g as u32, ink);
            }
            c = c.saturating_add(gw);
        }
    }

    /// Draws a label at `(x, y)` in `ink` when every cell it takes is on the
    /// grid and held by no other label, and returns whether it was drawn
    /// (REQ-TEXT-010). The rule is first come, first drawn: a label that
    /// would overlap one drawn before it, or hang off the grid, is refused
    /// whole and nothing of it is drawn, so labels keep whole cells and never
    /// share one. A drawn label first clears its cells to the ground (blank,
    /// the frame's ground as the back), so it never shares a cell with data,
    /// gridlines or axes under it.
    fn place(&mut self, x: usize, y: usize, text: &str, ink: u8) -> bool {
        let w = width(text);
        let cols = self.grid.cols();
        if y >= self.grid.rows() || x.saturating_add(w) > cols {
            return false;
        }
        let row = y * cols;
        if self.labels[row + x..row + x + w].iter().any(|&taken| taken) {
            return false;
        }
        self.labels[row + x..row + x + w].fill(true);
        self.paint(x, y, w, text, ink);
        true
    }
}

impl Frame<'_, stage::Data> {
    /// The grid, to draw the chart's marks on.
    pub fn data(&mut self) -> &mut Grid {
        self.grid
    }
}

impl Frame<'_, stage::Gridlines> {
    /// Draws gridlines across `plot` in braille, in `RULE`: a vertical line
    /// at the dot of each of `x`'s ticks and a horizontal one at the dot of
    /// each of `y`'s, so they meet the data to a dot (REQ-TICK-040). A
    /// gridline is drawn only on an empty cell, blank with the ground behind
    /// it; a cell the data holds keeps its glyph (REQ-TEXT-020). `x` is a
    /// bottom axis over `plot`'s columns and `y` a left or right axis over
    /// its rows; call it once a pane.
    pub fn gridlines(&mut self, plot: Rect, x: Option<&PlacedAxis>, y: Option<&PlacedAxis>) {
        if plot.w == 0 || plot.h == 0 {
            return;
        }
        let mut masks = vec![0u8; plot.w * plot.h];
        for t in x.map_or(&[][..], |a| &a.ticks[..]) {
            let (col, bit) = (t.dot / X_DOTS, t.dot % X_DOTS);
            if col < plot.w {
                for row in 0..plot.h {
                    for dy in 0..Y_DOTS {
                        masks[row * plot.w + col] |= 1 << (dy * X_DOTS + bit);
                    }
                }
            }
        }
        for t in y.map_or(&[][..], |a| &a.ticks[..]) {
            let (row, dy) = (t.dot / Y_DOTS, t.dot % Y_DOTS);
            if row < plot.h {
                for m in &mut masks[row * plot.w..(row + 1) * plot.w] {
                    *m |= 0b11 << (dy * X_DOTS);
                }
            }
        }
        for (i, &mask) in masks.iter().enumerate() {
            let (cx, cy) = (plot.x + i % plot.w, plot.y + i / plot.w);
            if mask != 0 && cx < self.grid.cols() && cy < self.grid.rows() && self.grid.glyph(cx, cy) == BLANK && self.grid.back(cx, cy) == self.ground {
                self.grid.set(at(cx), at(cy), DotGlyphs::Braille.glyph(mask), RULE);
            }
        }
    }
}

impl Frame<'_, stage::Axes> {
    /// Draws a placed axis's line and tick marks in `MUTED` (REQ-TICK-040):
    /// a bottom axis as `─` along its first row with `┬` at each tick's
    /// cell; a left axis as `│` down its last column with `┤`; a right axis
    /// as `│` down its first column with `├`. Its labels are drawn at the
    /// label stage, by [`Frame::axis_labels`].
    pub fn axis(&mut self, axis: &PlacedAxis) {
        let r = axis.rect;
        if r.w == 0 || r.h == 0 {
            return;
        }
        match axis.side {
            Side::Bottom => {
                self.grid.rect(at(r.x), at(r.y), at(r.w), 1, H_LINE, MUTED);
                for t in &axis.ticks {
                    self.grid.set(at(r.x + t.cell), at(r.y), TICK_DOWN, MUTED);
                }
            }
            Side::Left | Side::Right => {
                let (col, mark) = if axis.side == Side::Left { (r.x + r.w - 1, TICK_LEFT) } else { (r.x, TICK_RIGHT) };
                self.grid.rect(at(col), at(r.y), 1, at(r.h), V_LINE, MUTED);
                for t in &axis.ticks {
                    self.grid.set(at(col), at(r.y + t.cell), mark, MUTED);
                }
            }
        }
    }
}

impl Frame<'_, stage::Axes> {
    /// Joins the x axis's line to the y axes' lines in `MUTED`: `└` where a
    /// left axis meets it and `┘` where a right axis does.
    pub fn corners(&mut self, layout: &Layout) {
        let Some(x) = &layout.x_axis else { return };
        let r = x.rect;
        if r.w == 0 || r.h == 0 {
            return;
        }
        let pane = layout.panes.last();
        if pane.and_then(|p| p.left.as_ref()).is_some_and(|a| a.rect.w > 0) && r.x > 0 {
            self.grid.set(at(r.x - 1), at(r.y), CORNER_LEFT, MUTED);
        }
        if pane.and_then(|p| p.right.as_ref()).is_some_and(|a| a.rect.w > 0) {
            self.grid.set(at(r.x + r.w), at(r.y), CORNER_RIGHT, MUTED);
        }
    }
}

impl Frame<'_, stage::Labels> {
    /// Draws a label at `(x, y)` in `ink` and returns whether it was drawn:
    /// refused whole when a cell it needs is off the grid or held by another
    /// label, and otherwise drawn over cleared cells (REQ-TEXT-010).
    pub fn label(&mut self, x: usize, y: usize, text: &str, ink: u8) -> bool {
        self.place(x, y, text, ink)
    }

    /// Draws `text` in the first row of `rect`, from its left, shortened with
    /// `…` to the rect's width, as a label (a title or a legend entry).
    pub fn label_in(&mut self, rect: Rect, text: &str, ink: u8) -> bool {
        rect.h > 0 && self.place(rect.x, rect.y, &truncate(text, rect.w), ink)
    }

    /// Draws a placed axis's tick labels in `INK` and returns how many were
    /// drawn: a bottom axis's in its second row at each label's start, a left
    /// axis's ending one cell before its line, a right axis's one cell after
    /// it. Labels go through [`Frame::label`], so none shares a cell with
    /// another (REQ-TEXT-010).
    pub fn axis_labels(&mut self, axis: &PlacedAxis) -> usize {
        let r = axis.rect;
        let mut drawn = 0;
        for l in axis.ticks.iter().filter_map(|t| t.label.as_ref()) {
            let (x, y) = match axis.side {
                Side::Bottom if r.h >= 2 => (r.x + l.start, r.y + 1),
                Side::Bottom => continue,
                Side::Left if r.w > l.width => (r.x + r.w - 1 - l.width, r.y + l.start),
                Side::Left => continue,
                Side::Right => (r.x + 1, r.y + l.start),
            };
            drawn += usize::from(self.place(x, y, &l.text, INK));
        }
        drawn
    }
}

impl Frame<'_, stage::Annotations> {
    /// The grid, to draw annotations on.
    pub fn annotations(&mut self) -> &mut Grid {
        self.grid
    }

    /// Draws an annotation's label as [`Frame::label`] does, refused when it
    /// would share a cell with a label drawn before it (REQ-TEXT-010).
    pub fn label(&mut self, x: usize, y: usize, text: &str, ink: u8) -> bool {
        self.place(x, y, text, ink)
    }
}

impl Frame<'_, stage::Crosshair> {
    /// The grid, to draw the crosshair on.
    pub fn crosshair(&mut self) -> &mut Grid {
        self.grid
    }
}

impl Frame<'_, stage::Readout> {
    /// Draws the readout at `(x, y)` in `ink` over everything, its cells
    /// cleared to the ground first and clipped at the grid's edge. The
    /// readout is last, so it is the one text that may cover a label
    /// (REQ-TEXT-020).
    pub fn readout(&mut self, x: usize, y: usize, text: &str, ink: u8) {
        let w = width(text);
        self.paint(x, y, w, text, ink);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::{NO_BACK, WIDE_TAIL};
    use crate::palette::{ACCENT, GROUND, SERIES_START};

    #[test]
    fn widths_count_cells_and_drop_joined_scalars() {
        assert_eq!(width("100"), 3);
        assert_eq!(width("10月"), 4, "a wide glyph takes two cells");
        assert_eq!(width("e\u{301}"), 1, "a combining mark takes no cell");
        assert_eq!(width("a\u{200B}b"), 2, "a zero-width space takes no cell");
        let family = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}";
        assert_eq!(glyphs(family).collect::<Vec<_>>(), [('\u{1F468}', 2)], "a ZWJ sequence draws its first scalar");
        assert_eq!(glyphs("\u{1F44D}\u{1F3FD}").count(), 1, "a skin tone joins its emoji");
        let flags = "\u{1F1EF}\u{1F1F5}\u{1F1EB}\u{1F1F7}";
        assert_eq!(glyphs(flags).map(|(c, _)| c).collect::<String>(), "\u{1F1EF}\u{1F1EB}", "each flag draws its first indicator");
        assert_eq!(truncate("temperature", 6), "tempe\u{2026}");
        assert_eq!(truncate("temp", 6), "temp");
        assert_eq!(truncate("月月月", 4), "月\u{2026}", "no glyph is cut in half");
        assert_eq!(truncate("abc", 0), "");
    }

    #[test]
    fn labels_clear_their_cells_and_refuse_overlap() {
        let mut grid = Grid::new(12, 2);
        let mut f = Frame::new(&mut grid, GROUND);
        f.data().rect(0, 0, 12, 2, 0x2588, SERIES_START);
        f.data().set_back(3, 0, SERIES_START);
        let mut f = f.then::<stage::Labels>();
        assert!(f.label(2, 0, "abc", INK));
        assert_eq!(f.grid().back(3, 0), GROUND, "a label clears the back under it");
        assert!(!f.label(4, 0, "xy", INK), "a label overlapping another is refused");
        assert_eq!(f.grid().glyph(5, 0), 0x2588, "and nothing of it is drawn");
        assert!(f.label(5, 0, "xy", INK), "the next free cell is fine");
        assert!(!f.label(11, 1, "zz", INK), "a label hanging off the grid is refused");
        assert!(f.label(0, 1, "1月", INK));
        assert_eq!(f.grid().glyph(1, 1), '月' as u32);
        assert_eq!(f.grid().glyph(2, 1), WIDE_TAIL, "a wide glyph is drawn with set_wide");
        assert!(f.is_label(2, 1) && !f.is_label(3, 1));
        assert_eq!(f.grid().text().lines().next(), Some("██abcxy█████"));
    }

    #[test]
    fn gridlines_and_readouts_keep_their_layers() {
        let mut grid = Grid::new(6, 3);
        let mut f = Frame::new(&mut grid, NO_BACK);
        f.data().set(2, 1, 0x25CF, SERIES_START);
        let mut f = f.then::<stage::Readout>();
        f.readout(1, 1, "ab", ACCENT);
        assert_eq!(f.grid().text(), "      \n ab   \n      ");
        assert_eq!(f.layer(), Layer::Readout);
    }
}
