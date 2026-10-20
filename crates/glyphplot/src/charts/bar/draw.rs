//! Drawing the bar chart: its bars in block glyphs, its axes, labels and
//! legend (the base), and the hovered bar and readout (the overlay).
//!
//! A cell a bar covers whole is its fill: `█` in colour, and in one ink (or
//! with `patterns: "on"`) the series' fill, `█ ▓ ▒ ░` by series
//! (REQ-MONO-030). A cell a bar covers in part holds an eighth block: from
//! the axis's start, `▁`–`▇` up and `▏`–`▉` right, the free end of a bar
//! above the baseline (REQ-BAR-010); from the far side, `▔`, `🮂`, `🮃`, `▀`,
//! `🮄`, `🮅`, `🮆` down and `▕`, `🮇`, `🮈`, `▐`, `🮉`, `🮊`, `🮋` left, the free
//! end of a bar below it and a baseline inside a cell. A cell holding both
//! ends takes the free end's block. A signed single series draws in the
//! theme's up and down tokens, and told apart by fill as well: `█` above the
//! baseline, `▒` below (REQ-THEME-060).

use super::data::Prepared;
use super::geom::{Geom, Seg};
use super::Config;
use crate::annotate;
use crate::grid::{Grid, BLANK};
use crate::mono;
use crate::overlay;
use crate::palette::{self, ACCENT, DOWN, INK, MUTED, UP};
use crate::text::{stage, Frame, LegendEntry};
use crate::view::View;

/// The full cell of a bar drawn solid.
const FULL: char = '\u{2588}';
/// The fill of a signed series below its baseline: `▒` (REQ-THEME-060).
const DOWN_FILL: char = '\u{2592}';
/// The upper eighths 1–7, a vertical bar's end on the far side.
const UPPER: [u32; 7] = [0x2594, 0x1FB82, 0x1FB83, 0x2580, 0x1FB84, 0x1FB85, 0x1FB86];
/// The right eighths 1–7, a horizontal bar's end on the far side.
const RIGHT: [u32; 7] = [0x2595, 0x1FB87, 0x1FB88, 0x2590, 0x1FB89, 0x1FB8A, 0x1FB8B];

/// The block of `k` eighths (1–7) grown from the axis's start: `▁`–`▇`
/// (U+2581–2587) up, `▏`–`▉` (U+258F–2589) right (REQ-BAR-010).
pub(super) fn near_block(k: i64, horizontal: bool) -> u32 {
    let k = k.clamp(1, 7) as u32;
    if horizontal {
        0x2590 - k
    } else {
        0x2580 + k
    }
}

/// The block of `k` eighths (1–7) grown from the far side of the cell.
pub(super) fn far_block(k: i64, horizontal: bool) -> u32 {
    let k = k.clamp(1, 7) as usize;
    if horizontal {
        RIGHT[k - 1]
    } else {
        UPPER[k - 1]
    }
}

/// The glyph of a cell covering eighths `a..b` of it (0–8, from the axis's
/// start), `full` when it covers the whole cell, the free end's block when
/// it holds both ends; None when it covers none.
pub(super) fn glyph(a: i64, b: i64, up: bool, horizontal: bool, full: char) -> Option<u32> {
    match (a, b) {
        _ if b <= a => None,
        (0, 8) => Some(full as u32),
        (0, b) => Some(near_block(b, horizontal)),
        (a, 8) => Some(far_block(8 - a, horizontal)),
        (_, b) if up => Some(near_block(b, horizontal)),
        (a, _) => Some(far_block(8 - a, horizontal)),
    }
}

/// The ink of series `s`.
pub(super) fn series_ink(s: usize) -> u8 {
    palette::series(s % palette::SERIES_COUNT as usize)
}

/// A segment's ink and full glyph: up or down by sign for a signed series,
/// else its series' colour and, when patterned, its fill.
fn look(seg: &Seg, geom: &Geom, patterned: bool) -> (u8, char) {
    if geom.signed {
        if seg.up {
            (UP, FULL)
        } else {
            (DOWN, DOWN_FILL)
        }
    } else {
        (series_ink(seg.series), if patterned { mono::fill(seg.series) } else { FULL })
    }
}

/// Calls `cell` with each plot cell a segment inks and its glyph.
pub(super) fn each_cell(geom: &Geom, seg: &Seg, full: char, mut cell: impl FnMut(usize, usize, u32)) {
    for c in seg.cells() {
        let (a, b) = ((seg.lo - 8 * c).max(0), (seg.hi - 8 * c).min(8));
        let Some(g) = glyph(a, b, seg.up, geom.horizontal, full) else { continue };
        for x in seg.cross.clone() {
            if let Some((col, row)) = geom.cell(x, c) {
                cell(col, row, g);
            }
        }
    }
}

/// Everything the base is drawn from besides the data.
pub(super) struct BaseInput<'a> {
    /// The parsed options.
    pub cfg: &'a Config,
    /// The plot, axes and bars.
    pub geom: &'a Geom,
    /// The view state.
    pub view: &'a View,
    /// One ink.
    pub mono: bool,
    /// The frame's ground.
    pub ground: u8,
}

/// Draws the base (bars, axes, labels, legend, annotations) into `grid`
/// (REQ-TEXT-020).
pub(super) fn base(grid: &mut Grid, data: &Prepared, input: &BaseInput) {
    let BaseInput { cfg, geom, view, mono, ground } = *input;
    let patterned = cfg.patterns.active(mono);
    let mut f = Frame::new(grid, ground);
    for seg in &geom.segs {
        let (ink, full) = look(seg, geom, patterned);
        let g = f.data();
        each_cell(geom, seg, full, |col, row, glyph| g.set(col as i32, row as i32, glyph, ink));
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
    // Each key is how its series is drawn: its fill when patterned, else
    // solid (REQ-MONO-040).
    let entries: Vec<LegendEntry> = geom
        .legend
        .iter()
        .map(|&(s, rect)| {
            let hidden = view.is_hidden(&data.series[s].name);
            LegendEntry {
                rect,
                key: if patterned { mono::fill_key(s) } else { FULL },
                key_ink: if hidden { MUTED } else { series_ink(s) },
                name: &data.series[s].name,
                name_ink: if hidden { MUTED } else { INK },
            }
        })
        .collect();
    f.legend(&entries);
    if let Some(a) = &layout.x_axis {
        f.axis_labels(a);
    }
    for p in &layout.panes {
        if let Some(a) = &p.left {
            f.axis_labels(a);
        }
    }
    let plot = geom.plot;
    if (data.len() == 0 || geom.shown.is_empty()) && plot.w >= 7 && plot.h >= 1 {
        let text = "no data";
        f.label(plot.x + (plot.w - text.len()) / 2, plot.y + plot.h / 2, text, MUTED);
    }
    let mut f = f.then::<stage::Annotations>();
    annotate::draw(&mut f, &layout.marks, mono);
}

/// Draws the overlay over the base already in `grid`: the brushed
/// categories, the hovered bar in the accent ink and the readout
/// (REQ-OVERLAY-010, REQ-OVERLAY-020).
pub(super) fn overlay(grid: &mut Grid, geom: &Geom, data: &Prepared, view: &View, readout: Option<&str>, ground: u8) {
    let plot = geom.plot;
    if plot.w == 0 || plot.h == 0 {
        return;
    }
    if let (Some([a, b]), Some(axis), false) = (view.selection.as_ref().and_then(|s| s.x), geom.cat_axis(), geom.horizontal) {
        if let (Some(ca), Some(cb)) = (axis.cell(a), axis.cell(b)) {
            overlay::brush(grid, plot, ca, cb);
        }
    }
    let mut f = Frame::overlay(grid, ground);
    let Some(c) = &view.cursor else { return };
    let seg = c.series.as_deref().and_then(|name| data.find(name)).zip(c.index).and_then(|(s, i)| {
        let cats = geom.cats?;
        let j = geom.shown.iter().position(|&x| x == s)?;
        let k = usize::try_from(i).ok()?.checked_sub(cats.first).filter(|&k| k < cats.count())?;
        geom.seg(k, j)
    });
    if let Some(seg) = seg {
        let g = f.crosshair();
        each_cell(geom, seg, FULL, |col, row, _| {
            if g.glyph(col, row) != BLANK {
                let glyph = g.glyph(col, row);
                g.set(col as i32, row as i32, glyph, ACCENT);
            }
        });
    }
    if let Some(text) = readout {
        overlay::readout(f, plot, text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// REQ-BAR-010: the free end at an eighth, `▁`–`▇` up and `▏`–`▉`
    /// right, and the far-side blocks for the other end.
    #[test]
    fn blocks_by_eighths() {
        let up: String = (1..8).map(|k| char::from_u32(near_block(k, false)).unwrap()).collect();
        assert_eq!(up, "▁▂▃▄▅▆▇");
        let right: String = (1..8).map(|k| char::from_u32(near_block(k, true)).unwrap()).collect();
        assert_eq!(right, "▏▎▍▌▋▊▉");
        assert_eq!(char::from_u32(far_block(4, false)), Some('▀'));
        assert_eq!(char::from_u32(far_block(4, true)), Some('▐'));
        assert_eq!(glyph(0, 8, true, false, '▓'), Some('▓' as u32));
        assert_eq!(glyph(0, 3, true, false, FULL), Some('▃' as u32));
        assert_eq!(glyph(4, 8, true, false, FULL), Some('▀' as u32), "a baseline mid-cell");
        assert_eq!(glyph(2, 5, true, false, FULL), Some('▅' as u32), "both ends: the free end's");
        assert_eq!(glyph(2, 5, false, false, FULL), Some(far_block(6, false)));
        assert_eq!(glyph(3, 3, true, false, FULL), None);
    }
}
