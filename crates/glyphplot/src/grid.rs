//! The frame buffer: `cols × rows` cells, each a code point, an ink index and
//! a back index (REQ-GRID-010 to 070).

use unicode_width::UnicodeWidthChar;

/// The glyph of an empty cell: a space (REQ-GRID-010).
pub const BLANK: u32 = 0x20;
/// The glyph of the right half of a double-width glyph, never a real code
/// point (REQ-GRID-010).
pub const WIDE_TAIL: u32 = 0;
/// The back index of a cell with no colour behind it (REQ-GRID-010).
pub const NO_BACK: u8 = 255;

/// `cols × rows` cells held row-major, each a `u32` code point, a `u8` ink
/// index and a `u8` back index (REQ-GRID-010). Every drawing method clips, so
/// marks may overhang the edges (REQ-GRID-040).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grid {
    cols: usize,
    rows: usize,
    glyphs: Vec<u32>,
    inks: Vec<u8>,
    backs: Vec<u8>,
}

impl Grid {
    /// A grid with every cell `BLANK` in ink 0 with `NO_BACK`. Panics on a
    /// zero width or height (REQ-GRID-030).
    pub fn new(cols: usize, rows: usize) -> Grid {
        let mut grid = Grid { cols: 0, rows: 0, glyphs: Vec::new(), inks: Vec::new(), backs: Vec::new() };
        grid.resize(cols, rows);
        grid
    }

    /// Makes the grid `cols × rows` with every cell blank, as `new` does,
    /// reusing the allocation when it is large enough. Panics on a zero width
    /// or height (REQ-GRID-020).
    pub fn resize(&mut self, cols: usize, rows: usize) {
        assert!(cols > 0 && rows > 0, "a grid needs a width and a height, not {cols} × {rows}");
        let n = cols.checked_mul(rows).expect("the grid's cell count overflows usize");
        self.cols = cols;
        self.rows = rows;
        self.glyphs.clear();
        self.glyphs.resize(n, BLANK);
        self.inks.clear();
        self.inks.resize(n, 0);
        self.backs.clear();
        self.backs.resize(n, NO_BACK);
    }

    /// Resets every cell to `BLANK` in ink 0 with `NO_BACK` (REQ-GRID-030).
    pub fn clear(&mut self) {
        self.glyphs.fill(BLANK);
        self.inks.fill(0);
        self.backs.fill(NO_BACK);
    }

    /// Sets every cell's glyph and ink, leaving the backs as they are
    /// (REQ-GRID-030).
    pub fn fill(&mut self, glyph: u32, ink: u8) {
        self.glyphs.fill(glyph);
        self.inks.fill(ink);
    }

    /// The width in cells.
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// The height in cells.
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// The code point of every cell, row-major (REQ-GRID-070).
    pub fn glyphs(&self) -> &[u32] {
        &self.glyphs
    }

    /// The ink index of every cell, row-major (REQ-GRID-070).
    pub fn inks(&self) -> &[u8] {
        &self.inks
    }

    /// The back index of every cell, row-major (REQ-GRID-070).
    pub fn backs(&self) -> &[u8] {
        &self.backs
    }

    /// The code point at column `x`, row `y`. Panics off the grid
    /// (REQ-GRID-070).
    pub fn glyph(&self, x: usize, y: usize) -> u32 {
        self.glyphs[self.index(x, y)]
    }

    /// The ink index at column `x`, row `y`. Panics off the grid
    /// (REQ-GRID-070).
    pub fn ink(&self, x: usize, y: usize) -> u8 {
        self.inks[self.index(x, y)]
    }

    /// The back index at column `x`, row `y`. Panics off the grid
    /// (REQ-GRID-070).
    pub fn back(&self, x: usize, y: usize) -> u8 {
        self.backs[self.index(x, y)]
    }

    /// Puts a one-column glyph at `(x, y)` in `ink`, ignored off the grid
    /// (REQ-GRID-040). Writing over either half of a double-width glyph blanks
    /// the other half, so no head is left without its tail or tail without
    /// its head.
    pub fn set(&mut self, x: i32, y: i32, glyph: u32, ink: u8) {
        if let Some(i) = self.clip(x, y) {
            self.unpair(i);
            self.glyphs[i] = glyph;
            self.inks[i] = ink;
        }
    }

    /// Puts a double-width glyph at `(x, y)` and `WIDE_TAIL` at `(x + 1, y)`,
    /// both in `ink`, only when both cells are on the grid. A debug build
    /// asserts the glyph is two columns wide by `unicode-width`
    /// (REQ-GRID-050).
    pub fn set_wide(&mut self, x: i32, y: i32, glyph: u32, ink: u8) {
        debug_assert!(char::from_u32(glyph).and_then(UnicodeWidthChar::width) == Some(2), "set_wide takes a two-column glyph, not U+{glyph:04X}");
        let (Some(head), Some(_)) = (self.clip(x, y), self.clip(x.saturating_add(1), y)) else { return };
        self.unpair(head);
        self.unpair(head + 1);
        self.glyphs[head] = glyph;
        self.glyphs[head + 1] = WIDE_TAIL;
        self.inks[head] = ink;
        self.inks[head + 1] = ink;
    }

    /// Writes `text` from `(x, y)` rightwards in `ink`, clipping, and returns
    /// the columns it takes. Columns are counted by `unicode-width` 0.2: a
    /// two-column code point goes through `set_wide`, and a zero-width or
    /// control code point takes no cell (REQ-GRID-040, REQ-I18N-040).
    pub fn write(&mut self, x: i32, y: i32, text: &str, ink: u8) -> i32 {
        let mut at = x;
        for c in text.chars() {
            match c.width() {
                Some(1) => {
                    self.set(at, y, c as u32, ink);
                    at = at.saturating_add(1);
                }
                Some(2) => {
                    let next = at.saturating_add(1);
                    if self.clip(at, y).is_some() && self.clip(next, y).is_some() {
                        self.set_wide(at, y, c as u32, ink);
                    } else {
                        // Half a wide glyph hangs over the edge: its visible
                        // half is left blank, never half a glyph.
                        self.set(at, y, BLANK, ink);
                        self.set(next, y, BLANK, ink);
                    }
                    at = at.saturating_add(2);
                }
                _ => {}
            }
        }
        at.saturating_sub(x)
    }

    /// Fills the `w × h` rectangle from `(x, y)` with `glyph` in `ink`,
    /// clipping (REQ-GRID-040).
    pub fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, glyph: u32, ink: u8) {
        let (x0, x1) = clip_span(x, w, self.cols);
        let (y0, y1) = clip_span(y, h, self.rows);
        for row in y0..y1 {
            for col in x0..x1 {
                self.set(col as i32, row as i32, glyph, ink);
            }
        }
    }

    /// Draws a Bresenham line of `glyph` in `ink` from `(x0, y0)` to
    /// `(x1, y1)`, both ends included, clipping (REQ-GRID-040).
    pub fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, glyph: u32, ink: u8) {
        let (cols, rows) = (self.cols as i64, self.rows as i64);
        line_points(x0.into(), y0.into(), x1.into(), y1.into(), cols, rows, |x, y| self.set(x as i32, y as i32, glyph, ink));
    }

    /// Sets the back index at `(x, y)`, ignored off the grid (REQ-GRID-040).
    pub fn set_back(&mut self, x: i32, y: i32, back: u8) {
        if let Some(i) = self.clip(x, y) {
            self.backs[i] = back;
        }
    }

    /// The grid as `rows` lines joined by `\n`, leaving out `WIDE_TAIL` cells
    /// and writing an invalid code point as U+FFFD (REQ-GRID-060).
    pub fn text(&self) -> String {
        let mut out = String::with_capacity(self.glyphs.len() + self.rows);
        for (r, row) in self.glyphs.chunks_exact(self.cols).enumerate() {
            if r > 0 {
                out.push('\n');
            }
            out.extend(row.iter().filter(|&&g| g != WIDE_TAIL).map(|&g| char::from_u32(g).unwrap_or(char::REPLACEMENT_CHARACTER)));
        }
        out
    }

    /// The index of `(x, y)`, panicking off the grid.
    fn index(&self, x: usize, y: usize) -> usize {
        assert!(x < self.cols && y < self.rows, "({x}, {y}) is off the {} × {} grid", self.cols, self.rows);
        y * self.cols + x
    }

    /// The index of `(x, y)`, or `None` off the grid.
    fn clip(&self, x: i32, y: i32) -> Option<usize> {
        let (x, y) = (usize::try_from(x).ok()?, usize::try_from(y).ok()?);
        (x < self.cols && y < self.rows).then(|| y * self.cols + x)
    }

    /// Blanks the other half of a double-width glyph that cell `i` is part of,
    /// before cell `i` is overwritten.
    fn unpair(&mut self, i: usize) {
        let x = i % self.cols;
        if self.glyphs[i] == WIDE_TAIL {
            if x > 0 {
                self.glyphs[i - 1] = BLANK;
            }
        } else if x + 1 < self.cols && self.glyphs[i + 1] == WIDE_TAIL {
            self.glyphs[i + 1] = BLANK;
        }
    }
}

/// The cells `[from, to)` of a span of `len` from `start`, clipped to
/// `0..size`.
fn clip_span(start: i32, len: i32, size: usize) -> (usize, usize) {
    let end = i64::from(start) + i64::from(len.max(0));
    let size = size as i64;
    let from = i64::from(start).max(0).min(size);
    let to = end.max(0).min(size);
    (from as usize, to.max(from) as usize)
}

/// Calls `plot` for each point of the line from `(x0, y0)` to `(x1, y1)`,
/// both ends included, that lies in `0..w × 0..h`. The points are the
/// Bresenham line's: along the major axis every step, along the minor axis the
/// nearest whole coordinate, a half rounding away from the start. The visible
/// stretch is found by search, so a line from far off the grid costs only its
/// visible points. The grid and the canvases both draw with it.
pub(crate) fn line_points(x0: i64, y0: i64, x1: i64, y1: i64, w: i64, h: i64, mut plot: impl FnMut(i64, i64)) {
    let (dx, dy) = (i128::from(x1) - i128::from(x0), i128::from(y1) - i128::from(y0));
    let steps = dx.abs().max(dy.abs());
    // The point `i` steps along: the major axis moves one a step, the minor
    // axis to round(i * d / steps), halves away from the start.
    let at = |i: i128| -> (i128, i128) {
        let along = |start: i64, d: i128| -> i128 {
            if steps == 0 {
                return i128::from(start);
            }
            let offset = (2 * i * d.abs() + steps).div_euclid(2 * steps);
            i128::from(start) + offset * d.signum()
        };
        (along(x0, dx), along(y0, dy))
    };
    // Each coordinate is monotone in `i`, so the steps inside the grid are
    // one interval: find its first step, then walk while inside.
    let inside = |(x, y): (i128, i128)| x >= 0 && y >= 0 && x < i128::from(w) && y < i128::from(h);
    let before = |(x, y): (i128, i128)| {
        // Not yet reached the grid along a coordinate moving towards it.
        (dx > 0 && x < 0) || (dx < 0 && x >= i128::from(w)) || (dy > 0 && y < 0) || (dy < 0 && y >= i128::from(h))
    };
    let (mut lo, mut hi) = (0i128, steps);
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if before(at(mid)) {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    let mut i = lo;
    while i <= steps {
        let p = at(i);
        if !inside(p) {
            break;
        }
        plot(p.0 as i64, p.1 as i64);
        i += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(grid: &Grid) -> Vec<String> {
        grid.text().split('\n').map(str::to_owned).collect()
    }

    #[test]
    fn cells_are_a_code_point_and_two_indices() {
        let mut grid = Grid::new(3, 2);
        grid.set(1, 1, 0x1FB00, 7);
        grid.set_back(1, 1, 9);
        assert_eq!(grid.glyph(1, 1), 0x1FB00);
        assert_eq!(grid.ink(1, 1), 7);
        assert_eq!(grid.back(1, 1), 9);
        let _: (&[u32], &[u8], &[u8]) = (grid.glyphs(), grid.inks(), grid.backs());
        assert_eq!(grid.glyphs().len(), 6);
    }

    #[test]
    fn constants_are_fixed() {
        assert_eq!(BLANK, 0x20);
        assert_eq!(WIDE_TAIL, 0);
        assert_eq!(NO_BACK, 255);
    }

    #[test]
    fn resize_blanks_and_reuses() {
        let mut grid = Grid::new(40, 10);
        grid.fill('x' as u32, 3);
        grid.set_back(0, 0, 4);
        let ptr = grid.glyphs().as_ptr();
        grid.resize(10, 5);
        assert_eq!((grid.cols(), grid.rows()), (10, 5));
        assert!(grid.glyphs().iter().all(|&g| g == BLANK));
        assert!(grid.inks().iter().all(|&i| i == 0));
        assert!(grid.backs().iter().all(|&b| b == NO_BACK));
        grid.resize(20, 20);
        assert_eq!(grid.glyphs().as_ptr(), ptr, "400 cells fit the first allocation");
        assert_eq!(grid.glyphs().len(), 400);
    }

    #[test]
    #[should_panic]
    fn resize_to_zero_panics() {
        Grid::new(4, 4).resize(0, 4);
    }

    #[test]
    fn new_and_clear_blank_every_cell() {
        let mut grid = Grid::new(5, 3);
        let blank = |g: &Grid| g.glyphs().iter().all(|&c| c == BLANK) && g.inks().iter().all(|&i| i == 0) && g.backs().iter().all(|&b| b == NO_BACK);
        assert!(blank(&grid));
        grid.rect(0, 0, 5, 3, '#' as u32, 2);
        grid.set_back(2, 2, 1);
        grid.clear();
        assert!(blank(&grid));
        assert!(std::panic::catch_unwind(|| Grid::new(0, 1)).is_err());
        assert!(std::panic::catch_unwind(|| Grid::new(1, 0)).is_err());
    }

    #[test]
    fn fill_keeps_backs() {
        let mut grid = Grid::new(3, 2);
        grid.set_back(1, 0, 6);
        grid.fill('.' as u32, 5);
        assert!(grid.glyphs().iter().all(|&g| g == '.' as u32));
        assert!(grid.inks().iter().all(|&i| i == 5));
        assert_eq!(grid.backs(), &[NO_BACK, 6, NO_BACK, NO_BACK, NO_BACK, NO_BACK]);
    }

    #[test]
    fn draws_clip_at_the_edges() {
        let mut grid = Grid::new(4, 3);
        grid.set(-1, 0, '!' as u32, 1);
        grid.set(4, 0, '!' as u32, 1);
        grid.set(0, 3, '!' as u32, 1);
        grid.set(i32::MIN, i32::MAX, '!' as u32, 1);
        grid.set_back(-1, -1, 1);
        assert_eq!(grid.text(), "    \n    \n    ");
        assert!(grid.backs().iter().all(|&b| b == NO_BACK));
        grid.write(-2, 0, "abcdef", 1);
        grid.rect(2, 1, 10, 10, '#' as u32, 1);
        grid.rect(-5, 2, 6, 1, '=' as u32, 1);
        assert_eq!(rows(&grid), ["cdef", "  ##", "= ##"]);
        let mut grid = Grid::new(4, 3);
        grid.line(-3, -3, 6, 6, '\\' as u32, 1);
        assert_eq!(rows(&grid), ["\\   ", " \\  ", "  \\ "]);
        let mut grid = Grid::new(5, 1);
        grid.line(i32::MIN, 0, i32::MAX, 0, '-' as u32, 1);
        assert_eq!(grid.text(), "-----");
    }

    #[test]
    fn lines_include_both_ends() {
        let mut grid = Grid::new(6, 3);
        grid.line(0, 0, 5, 2, '*' as u32, 1);
        assert_eq!(grid.glyph(0, 0), '*' as u32);
        assert_eq!(grid.glyph(5, 2), '*' as u32);
        assert_eq!(grid.glyphs().iter().filter(|&&g| g == '*' as u32).count(), 6, "one point a step along the major axis");
        let mut grid = Grid::new(3, 3);
        grid.line(1, 1, 1, 1, 'o' as u32, 1);
        assert_eq!(rows(&grid), ["   ", " o ", "   "]);
        let mut grid = Grid::new(3, 4);
        grid.line(2, 3, 0, 0, '|' as u32, 1);
        assert_eq!(grid.glyph(2, 3), '|' as u32);
        assert_eq!(grid.glyph(0, 0), '|' as u32);
        assert_eq!(grid.glyphs().iter().filter(|&&g| g == '|' as u32).count(), 4);
    }

    #[test]
    fn line_points_match_a_walk_and_clip_to_it() {
        // Every point of an unclipped line is one step from the last, and a
        // clipped line is the unclipped one's points inside the window.
        for (x0, y0, x1, y1) in [(0, 0, 7, 3), (7, 3, 0, 0), (-4, 9, 12, -2), (3, -8, 5, 20), (0, 0, 0, 9), (-30, 2, 30, 3)] {
            let mut all = Vec::new();
            line_points(x0 + 100, y0 + 100, x1 + 100, y1 + 100, i64::MAX, i64::MAX, |x, y| all.push((x - 100, y - 100)));
            let mut shifted = Vec::new();
            line_points(x0 + 1000, y0 + 1000, x1 + 1000, y1 + 1000, i64::MAX, i64::MAX, |x, y| shifted.push((x - 1000, y - 1000)));
            assert_eq!(all, shifted);
            assert_eq!(all.first(), Some(&(x0, y0)));
            assert_eq!(all.last(), Some(&(x1, y1)));
            assert!(all.windows(2).all(|p| (p[1].0 - p[0].0).abs() <= 1 && (p[1].1 - p[0].1).abs() <= 1));
            let mut clipped = Vec::new();
            line_points(x0 + 100, y0 + 100, x1 + 100, y1 + 100, 104, 105, |x, y| clipped.push((x - 100, y - 100)));
            let inside: Vec<_> = all.iter().copied().filter(|&(x, y)| (-100..4).contains(&x) && (-100..5).contains(&y)).collect();
            assert_eq!(clipped, inside);
        }
    }

    #[test]
    fn set_wide_writes_its_tail() {
        let mut grid = Grid::new(4, 1);
        grid.set_wide(1, 0, '世' as u32, 3);
        assert_eq!(grid.glyphs(), &[BLANK, '世' as u32, WIDE_TAIL, BLANK]);
        assert_eq!(grid.inks(), &[0, 3, 3, 0]);
        // Overwriting either half blanks the other.
        grid.set(2, 0, 'a' as u32, 1);
        assert_eq!(grid.glyphs(), &[BLANK, BLANK, 'a' as u32, BLANK]);
        grid.set_wide(2, 0, '界' as u32, 1);
        grid.set(2, 0, 'b' as u32, 1);
        assert_eq!(grid.glyphs(), &[BLANK, BLANK, 'b' as u32, BLANK]);
        let mut grid = Grid::new(6, 1);
        assert_eq!(grid.write(0, 0, "a世b", 2), 4);
        assert_eq!(grid.text(), "a世b  ");
        assert_eq!(grid.glyph(2, 0), WIDE_TAIL);
        // Zero-width and control code points take no cell.
        let mut grid = Grid::new(4, 1);
        assert_eq!(grid.write(0, 0, "e\u{301}\u{200B}\tz", 1), 2);
        assert_eq!(grid.text(), "ez  ");
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "two-column glyph")]
    fn set_wide_asserts_two_columns_in_debug() {
        Grid::new(3, 1).set_wide(0, 0, 'a' as u32, 1);
    }

    #[test]
    fn set_wide_at_the_edge_draws_nothing() {
        let mut grid = Grid::new(3, 1);
        grid.set_wide(2, 0, '世' as u32, 1);
        grid.set_wide(-1, 0, '世' as u32, 1);
        assert_eq!(grid.text(), "   ");
        assert_eq!(grid.write(1, 0, "世世", 1), 4);
        assert_eq!(grid.text(), " 世");
        let mut grid = Grid::new(3, 1);
        grid.write(-1, 0, "世ab", 1);
        assert_eq!(grid.glyphs(), &[BLANK, 'a' as u32, 'b' as u32]);
    }

    #[test]
    fn text_joins_the_rows() {
        let mut grid = Grid::new(3, 3);
        grid.write(0, 0, "abc", 1);
        grid.write(1, 2, "z", 1);
        assert_eq!(grid.text(), "abc\n   \n z ");
    }

    #[test]
    fn text_skips_tails_and_replaces_invalid() {
        let mut grid = Grid::new(4, 1);
        grid.set_wide(0, 0, '世' as u32, 1);
        grid.set(2, 0, 0xD800, 1);
        grid.set(3, 0, 0x11_0000, 1);
        assert_eq!(grid.text(), "世\u{FFFD}\u{FFFD}");
    }

    #[test]
    fn slices_are_row_major() {
        let mut grid = Grid::new(3, 2);
        grid.set(2, 0, 'a' as u32, 1);
        grid.set(0, 1, 'b' as u32, 2);
        grid.set_back(1, 1, 3);
        assert_eq!(grid.glyphs()[2], 'a' as u32);
        assert_eq!(grid.glyphs()[3], 'b' as u32);
        assert_eq!(grid.inks(), &[0, 0, 1, 2, 0, 0]);
        assert_eq!(grid.backs()[4], 3);
    }

    #[test]
    fn getters_panic_off_the_grid() {
        let grid = Grid::new(3, 2);
        assert!(std::panic::catch_unwind(|| grid.glyph(3, 0)).is_err(), "x past the row panics though the index is in the buffer");
        assert!(std::panic::catch_unwind(|| grid.ink(0, 2)).is_err());
        assert!(std::panic::catch_unwind(|| grid.back(5, 5)).is_err());
        assert_eq!(grid.glyph(2, 1), BLANK);
    }
}
