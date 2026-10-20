//! One ink: how a chart tells series apart without colour (REQ-MONO-010 to
//! 040, ADR-123).
//!
//! Line series 1 to 4 draw with the dash patterns solid, 3 on 1 off, 1 on 1
//! off, and 3 on 1 off 1 on 1 off, counted in dot columns from the x scale's
//! origin ([`origin`]), so a pan by `n` whole dots shifts the pattern by `n`
//! dots and the dashes never crawl. Series 5 to 8 repeat the four and add a
//! marker each ([`marker`]: `●`, `○`, `■`, `□`; series 9 to 12 take `▲`, `△`,
//! `◆`, `◇`), placed every [`MARKER_EVERY`] cells along x counted from the
//! same origin ([`marker_column`]), at most one marker a cell ([`Markers`],
//! REQ-MONO-020). A marker replaces the braille cell it sits in.
//!
//! Bar and area series fill with `█`, `▓`, `▒` and `░` by series ([`fill`],
//! REQ-MONO-030); series 5 to 8 repeat the four.
//!
//! Each series has a one-cell legend key, drawn beside its name in its ink
//! wherever it is drawn with its pattern (REQ-MONO-040): for a line
//! ([`key`]) its pattern's box-drawing dash, `─`, `┄`, `┈`, `╌`, or from
//! series 5 its marker; for a bar or an area ([`fill_key`]) its fill. A
//! series drawn solid (in colour, by default) has the solid key `─`.
//!
//! [`Patterns`] is the `patterns` option every chart reads: on in one ink and
//! off in colour by default.

use crate::cells::Axis;
use crate::grid::Grid;
use crate::layout::Rect;

/// The dash patterns of line series 1 to 4, one entry a dot column: on or
/// off (REQ-MONO-010).
pub const PATTERNS: [&[bool]; 4] = [&[true], &[true, true, true, false], &[true, false], &[true, true, true, false, true, false]];

/// The legend keys of the patterns: `─` (U+2500), `┄` (U+2504), `┈` (U+2508)
/// and `╌` (U+254C) (REQ-MONO-040).
pub const KEYS: [char; 4] = ['\u{2500}', '\u{2504}', '\u{2508}', '\u{254C}'];

/// The markers of line series 5 to 12, in order: `●` (U+25CF), `○`
/// (U+25CB), `■` (U+25A0), `□` (U+25A1), `▲` (U+25B2), `△` (U+25B3), `◆`
/// (U+25C6) and `◇` (U+25C7) (REQ-MONO-020).
pub const MARKERS: [char; 8] = ['\u{25CF}', '\u{25CB}', '\u{25A0}', '\u{25A1}', '\u{25B2}', '\u{25B3}', '\u{25C6}', '\u{25C7}'];

/// The fills of bar and area series 1 to 4: `█` (U+2588), `▓` (U+2593), `▒`
/// (U+2592) and `░` (U+2591) (REQ-MONO-030).
pub const FILLS: [char; 4] = ['\u{2588}', '\u{2593}', '\u{2592}', '\u{2591}'];

/// The cells from one marker of a series to its next along x
/// (REQ-MONO-020). Series 5 to 8 start two cells apart, so their markers
/// never share a column.
pub const MARKER_EVERY: i64 = 8;

/// The `patterns` option: whether series are told apart by dash patterns,
/// markers and fills as well as by colour.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Patterns {
    /// On in one ink, off in colour (the default).
    #[default]
    Auto,
    /// Always.
    On,
    /// Never.
    Off,
}

impl Patterns {
    /// The option's value: `"on"`, `"off"`, anything else `auto`.
    pub fn from_option(value: &str) -> Patterns {
        match value {
            "on" => Patterns::On,
            "off" => Patterns::Off,
            _ => Patterns::Auto,
        }
    }

    /// Whether a frame drawn in one ink (`mono`) or in colour draws patterns.
    pub fn active(self, mono: bool) -> bool {
        match self {
            Patterns::Auto => mono,
            Patterns::On => true,
            Patterns::Off => false,
        }
    }
}

/// The pattern of series `index` (0 for the first): series 5 to 8 repeat
/// series 1 to 4's (REQ-MONO-010, REQ-MONO-020).
pub fn pattern(index: usize) -> &'static [bool] {
    PATTERNS[index % PATTERNS.len()]
}

/// The marker of line series `index` (0 for the first): none for series 1
/// to 4, then [`MARKERS`] in order from series 5, starting again after
/// series 12 (REQ-MONO-020).
pub fn marker(index: usize) -> Option<char> {
    index.checked_sub(PATTERNS.len()).map(|i| MARKERS[i % MARKERS.len()])
}

/// The fill of bar or area series `index` (0 for the first): [`FILLS`] by
/// series, series 5 to 8 repeating them (REQ-MONO-030).
pub fn fill(index: usize) -> char {
    FILLS[index % FILLS.len()]
}

/// The legend key of line series `index` drawn with its pattern: its marker
/// from series 5, else its dash (REQ-MONO-040).
pub fn key(index: usize) -> char {
    marker(index).unwrap_or(KEYS[index % KEYS.len()])
}

/// The legend key of bar or area series `index` drawn with its pattern: its
/// fill (REQ-MONO-040).
pub fn fill_key(index: usize) -> char {
    fill(index)
}

/// Whether column `col` of a plot (cells from its left) carries series
/// `index`'s marker, for an x axis whose origin is `origin` dots
/// ([`origin`], two dots a cell): every [`MARKER_EVERY`] cells counted from
/// the origin, series 5 to 8 two cells apart, so a pan by whole cells
/// carries the markers with the data (REQ-MONO-020). False for a series with
/// no marker.
pub fn marker_column(index: usize, col: usize, origin: i64) -> bool {
    if marker(index).is_none() {
        return false;
    }
    let cell = (col as i64).wrapping_mul(2).wrapping_add(origin).div_euclid(2);
    let phase = 2 * (index % PATTERNS.len()) as i64;
    cell.wrapping_sub(phase).rem_euclid(MARKER_EVERY) == 0
}

/// The cells of a plot that hold a marker, so no cell takes two
/// (REQ-MONO-020). A chart resets it each frame, places every series'
/// markers through it after drawing its data, and a marker refused by a
/// taken cell is not drawn.
#[derive(Clone, Debug, Default)]
pub struct Markers {
    rect: Rect,
    taken: Vec<bool>,
}

impl Markers {
    /// No marker yet in `rect` (grid cells).
    pub fn new(rect: Rect) -> Markers {
        let mut m = Markers::default();
        m.reset(rect);
        m
    }

    /// No marker in `rect`, reusing the memory.
    pub fn reset(&mut self, rect: Rect) {
        self.rect = rect;
        self.taken.clear();
        self.taken.resize(rect.w * rect.h, false);
    }

    /// The cell's index in the rect, when it is inside it.
    fn index(&self, x: usize, y: usize) -> Option<usize> {
        let r = self.rect;
        (x >= r.x && y >= r.y && x < r.x + r.w && y < r.y + r.h).then(|| (y - r.y) * r.w + (x - r.x))
    }

    /// Whether the cell at `(x, y)` (grid cells) holds a marker.
    pub fn is_taken(&self, x: usize, y: usize) -> bool {
        self.index(x, y).is_some_and(|i| self.taken[i])
    }

    /// Draws `glyph` at `(x, y)` (grid cells) in `ink`, replacing the cell,
    /// when the cell is inside the rect and the grid and holds no marker yet;
    /// returns whether it was drawn.
    pub fn place(&mut self, grid: &mut Grid, x: usize, y: usize, glyph: char, ink: u8) -> bool {
        let Some(i) = self.index(x, y) else { return false };
        if self.taken[i] || x >= grid.cols() || y >= grid.rows() {
            return false;
        }
        self.taken[i] = true;
        grid.set(x as i32, y as i32, glyph as u32, ink);
        true
    }
}

/// The dot column of the x scale's origin, counted from the axis's first
/// dot: the number of whole dots from 0 to the axis's low end, so a dot
/// column `k` of the plot is dot `k + origin(axis)` counted from 0. Panning by
/// whole dots moves it by as many (REQ-MONO-010). Held to ±2^52; 0 on a
/// degenerate axis.
pub fn origin(axis: &Axis) -> i64 {
    let dots = axis.dots();
    if axis.is_degenerate() || dots == 0 {
        return 0;
    }
    let per_dot = (axis.hi() - axis.lo()) / dots as f64;
    let d = (axis.lo() / per_dot).round();
    let limit = (1i64 << 52) as f64;
    if d >= limit {
        1 << 52
    } else if d <= -limit {
        -(1 << 52)
    } else {
        d as i64
    }
}

/// Whether series `index` lights dot column `k` of a plot whose x axis has
/// the origin offset `origin`.
pub fn lit(index: usize, k: i64, origin: i64) -> bool {
    let p = pattern(index);
    p[(k.wrapping_add(origin)).rem_euclid(p.len() as i64) as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// REQ-MONO-010: the four patterns, counted from the x origin, so a pan
    /// by whole dots carries the pattern with the data.
    #[test]
    fn dash_patterns_follow_the_origin() {
        let row = |i: usize, o: i64| -> String { (0..12).map(|k| if lit(i, k, o) { '#' } else { '.' }).collect() };
        assert_eq!(row(0, 0), "############");
        assert_eq!(row(1, 0), "###.###.###.");
        assert_eq!(row(2, 0), "#.#.#.#.#.#.");
        assert_eq!(row(3, 0), "###.#.###.#.");
        // An axis of 100 dots over 0..100: a dot a unit. Panned by 3 units it
        // starts 3 dots further on, and so does the pattern.
        let a = Axis::x(0.0, 100.0, 50, 2);
        let b = Axis::x(3.0, 103.0, 50, 2);
        assert_eq!((origin(&a), origin(&b)), (0, 3));
        for k in 0..40 {
            assert_eq!(lit(1, k, origin(&b)), lit(1, k + 3, origin(&a)), "dot {k}");
        }
        assert_eq!(origin(&Axis::x(1.0, 1.0, 10, 2)), 0);
        assert_eq!(KEYS.iter().collect::<std::collections::BTreeSet<_>>().len(), 4);
    }

    /// REQ-MONO-020: series 5 to 8 repeat patterns 1 to 4 and add the
    /// markers `● ○ ■ □`; their marker columns follow the origin by whole
    /// cells and never coincide; a cell takes one marker.
    #[test]
    fn series_five_to_eight_add_markers() {
        for (s, &k) in KEYS.iter().enumerate() {
            assert_eq!(marker(s), None, "series {} has no marker", s + 1);
            assert_eq!(pattern(s + 4), pattern(s), "series {} repeats pattern {}", s + 5, s + 1);
            assert_eq!(key(s), k);
        }
        let five_to_eight: String = (4..8).map(|s| marker(s).unwrap()).collect();
        assert_eq!(five_to_eight, "\u{25CF}\u{25CB}\u{25A0}\u{25A1}");
        assert_eq!((4..12).map(|s| marker(s).unwrap()).collect::<String>(), MARKERS.iter().collect::<String>());
        assert_eq!(key(4), '\u{25CF}', "series 5's key is its marker");
        assert_eq!(MARKERS.iter().collect::<std::collections::BTreeSet<_>>().len(), 8);
        // Marker columns: every 8 cells, the four series in different ones.
        let cols = |s: usize, o: i64| (0..40).filter(|&c| marker_column(s, c, o)).collect::<Vec<_>>();
        assert_eq!(cols(4, 0), [0, 8, 16, 24, 32]);
        assert_eq!(cols(5, 0), [2, 10, 18, 26, 34]);
        assert!(cols(0, 0).is_empty(), "series 1 has no marker column");
        for c in 0..40 {
            assert!((4..8).filter(|&s| marker_column(s, c, 0)).count() <= 1, "column {c}");
        }
        // Panned by three cells (six dots), the markers move with the data.
        assert_eq!(cols(4, 6), [5, 13, 21, 29, 37]);
        // At most one marker a cell.
        let mut grid = Grid::new(10, 3);
        let mut m = Markers::new(Rect::new(1, 0, 8, 3));
        assert!(m.place(&mut grid, 2, 1, marker(4).unwrap(), 8));
        assert!(!m.place(&mut grid, 2, 1, marker(5).unwrap(), 9), "a second marker in the cell is refused");
        assert_eq!((grid.glyph(2, 1), grid.ink(2, 1)), (0x25CF, 8));
        assert!(!m.place(&mut grid, 0, 1, '\u{25CF}', 8), "outside the rect");
        assert!(m.is_taken(2, 1) && !m.is_taken(3, 1));
        m.reset(Rect::new(1, 0, 8, 3));
        assert!(!m.is_taken(2, 1));
    }

    /// REQ-MONO-030: bar and area series fill with `█ ▓ ▒ ░` by series, and
    /// their keys are their fills.
    #[test]
    fn fills_go_by_series() {
        assert_eq!((0..8).map(fill).collect::<String>(), "\u{2588}\u{2593}\u{2592}\u{2591}\u{2588}\u{2593}\u{2592}\u{2591}");
        assert_eq!((0..4).map(fill_key).collect::<Vec<_>>(), FILLS);
        assert!(Patterns::Auto.active(true) && !Patterns::Auto.active(false));
        assert!(Patterns::On.active(false) && !Patterns::Off.active(true));
        assert_eq!(Patterns::from_option("on"), Patterns::On);
        assert_eq!(Patterns::from_option("bogus"), Patterns::Auto);
    }
}
