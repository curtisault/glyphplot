//! The overlay: what a frame draws over its cached base at pointer rate, the
//! brush, the time cursor, the crosshair and the readout (REQ-OVERLAY-010,
//! REQ-OVERLAY-020, ADR-114).
//!
//! A chart draws its base (data, axes, legend) only when the data, the
//! options, the view's domains or hidden series, the size or the env change,
//! and keeps it; every frame copies the base into the grid
//! ([`crate::grid::Grid::copy_from`]) and draws the overlay over it with the
//! painters here, in the z-order (REQ-TEXT-020): the brush on the backs, the
//! time cursor and the crosshair in the crosshair stage, the readout last. A
//! hover that leaves the view's domains alone redraws only the overlay.
//!
//! The crosshair is drawn at dot precision where the plot is blank, a column
//! or a row of braille dots, so it sits on the datum; where the data holds a
//! cell, the glyph stays and takes the crosshair's ink. The readout stays at
//! one place, the top right of the plot, not under the pointer
//! (REQ-OVERLAY-020); it goes with the cursor, which Escape clears.

use crate::canvas::Glyphs;
use crate::grid::{Grid, BLANK};
use crate::layout::Rect;
use crate::palette::{ACCENT, INK, MUTED, SELECTION};
use crate::text::{self, stage, Frame};

/// Whether a cell holds nothing: blank, or a braille gridline.
fn is_empty(grid: &Grid, x: usize, y: usize) -> bool {
    grid.glyph(x, y) == BLANK
}

/// Puts the selection colour behind the plot's columns `from..=to` (cells
/// counted from the plot's left, clipped to it): a brushed range.
pub fn brush(grid: &mut Grid, plot: Rect, from: i64, to: i64) {
    let (from, to) = (from.min(to), from.max(to));
    if plot.w == 0 || to < 0 || from >= plot.w as i64 {
        return;
    }
    let (a, b) = (from.max(0) as usize, (to as usize).min(plot.w - 1));
    for row in plot.y..plot.y + plot.h {
        for col in plot.x + a..=plot.x + b {
            grid.set_back(col as i32, row as i32, SELECTION);
        }
    }
}

/// A vertical rule at dot column `dot` of the plot (2 a cell, from its left)
/// in `ink`: braille dots in blank cells, the glyph kept and given `ink` in
/// cells the base holds.
pub fn rule_x(frame: &mut Frame<'_, stage::Crosshair>, plot: Rect, dot: i64, ink: u8) {
    if dot < 0 || dot >= 2 * plot.w as i64 {
        return;
    }
    let (col, bit) = (plot.x + (dot / 2) as usize, (dot % 2) as u8);
    let mask = (0..4).fold(0u8, |m, row| m | 1 << (row * 2 + bit));
    let grid = frame.crosshair();
    for row in plot.y..plot.y + plot.h {
        if col >= grid.cols() || row >= grid.rows() {
            continue;
        }
        let glyph = if is_empty(grid, col, row) { Glyphs::Braille.glyph(mask) } else { grid.glyph(col, row) };
        grid.set(col as i32, row as i32, glyph, ink);
    }
}

/// A horizontal rule at dot row `dot` of the plot (4 a cell, from its top)
/// in `ink`, as [`rule_x`] draws a vertical one.
pub fn rule_y(frame: &mut Frame<'_, stage::Crosshair>, plot: Rect, dot: i64, ink: u8) {
    if dot < 0 || dot >= 4 * plot.h as i64 {
        return;
    }
    let (row, sub) = (plot.y + (dot / 4) as usize, (dot % 4) as u8);
    let mask = 0b11u8 << (sub * 2);
    let grid = frame.crosshair();
    for col in plot.x..plot.x + plot.w {
        if col >= grid.cols() || row >= grid.rows() {
            continue;
        }
        let glyph = if is_empty(grid, col, row) { Glyphs::Braille.glyph(mask) } else { grid.glyph(col, row) };
        grid.set(col as i32, row as i32, glyph, ink);
    }
}

/// The crosshair at dot column `x` and dot row `y` of the plot, either
/// absent, in the accent ink.
pub fn crosshair(frame: &mut Frame<'_, stage::Crosshair>, plot: Rect, x: Option<i64>, y: Option<i64>) {
    if let Some(y) = y {
        rule_y(frame, plot, y, ACCENT);
    }
    if let Some(x) = x {
        rule_x(frame, plot, x, ACCENT);
    }
}

/// The page's time cursor at dot column `x`, in the muted ink.
pub fn time_cursor(frame: &mut Frame<'_, stage::Crosshair>, plot: Rect, x: i64) {
    rule_x(frame, plot, x, MUTED);
}

/// Where the readout of `text` goes: the plot's top row, against its right
/// edge, shortened with `…` to the plot's width (REQ-OVERLAY-020). `None`
/// for a plot of no cells.
pub fn readout_place(plot: Rect, text: &str) -> Option<(usize, usize, String)> {
    if plot.w == 0 || plot.h == 0 {
        return None;
    }
    let shown = text::truncate(text, plot.w);
    let w = text::width(&shown);
    Some((plot.x + plot.w - w, plot.y, shown))
}

/// Draws the readout over everything at its fixed place (REQ-OVERLAY-020,
/// REQ-TEXT-020).
pub fn readout(frame: Frame<'_, stage::Crosshair>, plot: Rect, text: &str) {
    let mut f = frame.then::<stage::Readout>();
    if let Some((x, y, shown)) = readout_place(plot, text) {
        f.readout(x, y, &shown, INK);
    }
}

#[cfg(all(test, feature = "line"))]
mod tests {
    use crate::chart::{CellPos, Chart, Env};
    use crate::charts::line::LineChart;
    use crate::grid::Grid;
    use crate::input::{Input, Key, Mods, PointerType};
    use crate::options::Options;
    use crate::view::ViewPatch;

    fn chart(fixture: &str) -> LineChart {
        let mut c = LineChart::new(&Options::new()).unwrap();
        c.set_data(crate::fixtures::generate(fixture).unwrap()).unwrap();
        c
    }

    fn mv(t: f64, x: f64, y: f64) -> Input {
        Input::Move { t, x, y, pointer: PointerType::Mouse, buttons: 0, mods: Mods::NONE }
    }

    /// REQ-OVERLAY-010: hovers across the plot redraw the overlay only; the
    /// base is drawn once, and the frame equals a fresh chart's with the
    /// same view.
    #[test]
    fn hover_does_not_redraw_the_base() {
        let mut c = chart("random-walk-100k");
        let mut g = Grid::new(80, 24);
        c.frame(0.0, &Env::plain(), &mut g);
        assert_eq!(c.base_draws(), 1);
        let scans = c.scans();
        let mut hovers = 0;
        for i in 0..40 {
            let ch = c.input(&mv(0.1 * i as f64, 10.0 + i as f64, 8.5));
            assert!(!ch.base, "a hover changes no domain");
            hovers += usize::from(!ch.events.is_empty());
            c.frame(0.1 * i as f64, &Env::plain(), &mut g);
        }
        assert!(hovers > 20, "{hovers} hovers changed the datum");
        assert_eq!((c.base_draws(), c.scans()), (1, scans), "the base was drawn once and the data read once");
        let mut fresh = chart("random-walk-100k");
        fresh.set_view(&ViewPatch::whole(c.view()));
        let mut h = Grid::new(80, 24);
        fresh.frame(3.9, &Env::plain(), &mut h);
        assert_eq!(g, h);
        assert!(c.hit(CellPos::new(30.5, 8.5)).is_some());
    }

    /// REQ-OVERLAY-020: the readout stays at the plot's top right wherever
    /// the pointer is, and Escape hides it.
    #[test]
    fn readout_stays_and_hides_on_escape() {
        let mut c = chart("sine-1k");
        let mut g = Grid::new(80, 24);
        c.frame(0.0, &Env::plain(), &mut g);
        let blank = g.clone();
        let mut places = Vec::new();
        for x in [12.0, 40.0, 70.0] {
            c.input(&mv(0.0, x, 10.5));
            c.frame(0.0, &Env::plain(), &mut g);
            let row = (0..24).find(|&r| (0..80).any(|col| g.glyph(col, r) != blank.glyph(col, r) && (0x21..0x7F).contains(&g.glyph(col, r)))).unwrap();
            let right = (0..80).rev().find(|&col| g.glyph(col, row) != blank.glyph(col, row) && (0x21..0x7F).contains(&g.glyph(col, row))).unwrap();
            places.push((row, right));
        }
        assert!(places.windows(2).all(|w| w[0] == w[1]), "{places:?}");
        let ch = c.input(&Input::Key { t: 0.0, key: Key::Escape, mods: Mods::NONE });
        assert!(ch.overlay && c.view().cursor.is_none());
        c.frame(0.0, &Env::plain(), &mut g);
        assert_eq!(g, blank, "no readout and no crosshair");
    }
}
