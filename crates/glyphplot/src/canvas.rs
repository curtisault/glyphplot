//! Sub-cell canvases: braille and octants at 2 × 4 dots a cell, sextants at
//! 2 × 3, quadrants at 2 × 2 (REQ-CANVAS-010, REQ-CANVAS-030).
//!
//! A canvas holds one dot mask a cell. Bit `row * 2 + col` is the dot at
//! `col` (0 left, 1 right) and `row` (0 top) inside the cell, which is the
//! order Unicode numbers the BLOCK SEXTANT and BLOCK OCTANT dots in; braille
//! renumbers it to its own dots when it draws.

mod octants;

use crate::grid::{line_points, Grid, BLANK};
use octants::OCTANTS;

/// The glyph set a canvas draws with, which fixes its dots a cell
/// (REQ-CANVAS-010).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Glyphs {
    /// Braille patterns, U+2800–28FF: 2 × 4 dots a cell.
    Braille,
    /// Block octants, U+1CD00–1CDE5 and the block elements they leave out:
    /// 2 × 4 dots a cell.
    Octants,
    /// Block sextants, U+1FB00–1FB3B and the four masks they leave out: 2 × 3
    /// dots a cell.
    Sextants,
    /// Quadrants, U+2596–259F and `▀ ▄ ▌ ▐ █`: 2 × 2 dots a cell.
    Quadrants,
}

/// The braille bit of each dot, by `row * 2 + col`: dots 1, 2, 3, 7 down the
/// left column and 4, 5, 6, 8 down the right (REQ-CANVAS-030).
const BRAILLE_BITS: [u8; 8] = [0x01, 0x08, 0x02, 0x10, 0x04, 0x20, 0x40, 0x80];

/// The quadrant glyph of each mask: bit 0 upper left, 1 upper right, 2 lower
/// left, 3 lower right (REQ-CANVAS-030).
const QUADRANTS: [u32; 16] = [BLANK, 0x2598, 0x259D, 0x2580, 0x2596, 0x258C, 0x259E, 0x259B, 0x2597, 0x259A, 0x2590, 0x259C, 0x2584, 0x2599, 0x259F, 0x2588];

/// The left half block, `▌`.
const LEFT_HALF: u32 = 0x258C;
/// The right half block, `▐`.
const RIGHT_HALF: u32 = 0x2590;
/// The full block, `█`.
const FULL: u32 = 0x2588;

impl Glyphs {
    /// Dots across a cell: 2 for every glyph set.
    pub const fn dots_x(self) -> usize {
        2
    }

    /// Dots down a cell: 4 for braille and octants, 3 for sextants, 2 for
    /// quadrants.
    pub const fn dots_y(self) -> usize {
        match self {
            Glyphs::Braille | Glyphs::Octants => 4,
            Glyphs::Sextants => 3,
            Glyphs::Quadrants => 2,
        }
    }

    /// The code point drawing a cell's dot mask, bit `row * 2 + col`; bits
    /// past the cell's dots are ignored. A mask with no dots is a space
    /// (REQ-CANVAS-030).
    pub fn glyph(self, mask: u8) -> u32 {
        let mask = mask & ((1u16 << (2 * self.dots_y())) - 1) as u8;
        if mask == 0 {
            return BLANK;
        }
        match self {
            Glyphs::Braille => 0x2800 + (0..8).filter(|bit| mask & (1 << bit) != 0).map(|bit| u32::from(BRAILLE_BITS[bit])).sum::<u32>(),
            Glyphs::Octants => OCTANTS[usize::from(mask)],
            Glyphs::Sextants => match mask {
                // Dots 1, 3, 5 and 2, 4, 6: the halves and the whole, which
                // the sextant block leaves out.
                0b01_0101 => LEFT_HALF,
                0b10_1010 => RIGHT_HALF,
                0b11_1111 => FULL,
                m => 0x1FB00 + u32::from(m) - 1 - u32::from(m > 0b01_0101) - u32::from(m > 0b10_1010),
            },
            Glyphs::Quadrants => QUADRANTS[usize::from(mask)],
        }
    }
}

/// The largest radius `Canvas::circle` draws, in dots; a larger circle draws
/// nothing.
pub const MAX_RADIUS: i64 = 1 << 24;

/// A `cols × rows` cell canvas of dots, `cols × dots_x` wide and
/// `rows × dots_y` high (REQ-CANVAS-010). Every drawing method clips.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Canvas {
    glyphs: Glyphs,
    cols: usize,
    rows: usize,
    masks: Vec<u8>,
}

impl Canvas {
    /// An empty canvas of `cols × rows` cells drawing with `glyphs`.
    pub fn new(glyphs: Glyphs, cols: usize, rows: usize) -> Canvas {
        let mut canvas = Canvas { glyphs, cols: 0, rows: 0, masks: Vec::new() };
        canvas.resize(cols, rows);
        canvas
    }

    /// Makes the canvas `cols × rows` cells with no dots, reusing the
    /// allocation when it is large enough.
    pub fn resize(&mut self, cols: usize, rows: usize) {
        let n = cols.checked_mul(rows).expect("the canvas's cell count overflows usize");
        self.cols = cols;
        self.rows = rows;
        self.masks.clear();
        self.masks.resize(n, 0);
    }

    /// Clears every dot.
    pub fn clear(&mut self) {
        self.masks.fill(0);
    }

    /// The glyph set the canvas draws with.
    pub fn glyphs(&self) -> Glyphs {
        self.glyphs
    }

    /// The width in cells.
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// The height in cells.
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// The width in dots: `cols × 2`.
    pub fn width(&self) -> usize {
        self.cols * self.glyphs.dots_x()
    }

    /// The height in dots: `rows × dots_y`.
    pub fn height(&self) -> usize {
        self.rows * self.glyphs.dots_y()
    }

    /// Sets the dot at `(x, y)`, ignored off the canvas.
    pub fn set(&mut self, x: i64, y: i64) {
        if let Some((i, bit)) = self.locate(x, y) {
            self.masks[i] |= bit;
        }
    }

    /// Clears the dot at `(x, y)`, ignored off the canvas.
    pub fn unset(&mut self, x: i64, y: i64) {
        if let Some((i, bit)) = self.locate(x, y) {
            self.masks[i] &= !bit;
        }
    }

    /// Whether the dot at `(x, y)` is set; false off the canvas.
    pub fn get(&self, x: i64, y: i64) -> bool {
        self.locate(x, y).is_some_and(|(i, bit)| self.masks[i] & bit != 0)
    }

    /// The dot mask of the cell at column `col`, row `row`, bit
    /// `row * 2 + col` inside the cell. Panics off the canvas.
    pub fn mask(&self, col: usize, row: usize) -> u8 {
        assert!(col < self.cols && row < self.rows, "({col}, {row}) is off the {} × {} canvas", self.cols, self.rows);
        self.masks[row * self.cols + col]
    }

    /// Sets the dots of a Bresenham line from `(x0, y0)` to `(x1, y1)`, both
    /// ends included, clipping (REQ-CANVAS-010).
    pub fn line(&mut self, x0: i64, y0: i64, x1: i64, y1: i64) {
        let (w, h) = (self.width() as i64, self.height() as i64);
        line_points(x0, y0, x1, y1, w, h, |x, y| self.set(x, y));
    }

    /// Sets the dots of a midpoint circle of radius `r` around `(cx, cy)`,
    /// clipping; a radius of 0 is one dot, and a negative radius or one past
    /// `MAX_RADIUS` draws nothing (REQ-CANVAS-010).
    pub fn circle(&mut self, cx: i64, cy: i64, r: i64) {
        if !(0..=MAX_RADIUS).contains(&r) {
            return;
        }
        let (w, h) = (self.width() as i64, self.height() as i64);
        // A circle whose box misses the canvas draws nothing.
        if cx.saturating_add(r) < 0 || cy.saturating_add(r) < 0 || cx.saturating_sub(r) >= w || cy.saturating_sub(r) >= h {
            return;
        }
        let (mut x, mut y, mut err) = (r, 0i64, 1 - r);
        while x >= y {
            for (dx, dy) in [(x, y), (y, x), (-y, x), (-x, y), (-x, -y), (-y, -x), (y, -x), (x, -y)] {
                self.set(cx + dx, cy + dy);
            }
            y += 1;
            if err < 0 {
                err += 2 * y + 1;
            } else {
                x -= 1;
                err += 2 * (y - x) + 1;
            }
        }
    }

    /// Draws the canvas onto `grid` with its top left cell at `(x, y)`, in
    /// `ink`, clipping. Only cells with dots are set, so canvases layer over
    /// text and each other (REQ-CANVAS-010); a cell's back is left as it is.
    pub fn draw(&self, grid: &mut Grid, x: i32, y: i32, ink: u8) {
        for (row, masks) in self.masks.chunks_exact(self.cols.max(1)).enumerate() {
            for (col, &mask) in masks.iter().enumerate() {
                if mask != 0 {
                    let (gx, gy) = (i64::from(x) + col as i64, i64::from(y) + row as i64);
                    if let (Ok(gx), Ok(gy)) = (i32::try_from(gx), i32::try_from(gy)) {
                        grid.set(gx, gy, self.glyphs.glyph(mask), ink);
                    }
                }
            }
        }
    }

    /// The cell index and bit of the dot at `(x, y)`, or `None` off the
    /// canvas.
    fn locate(&self, x: i64, y: i64) -> Option<(usize, u8)> {
        let (x, y) = (usize::try_from(x).ok()?, usize::try_from(y).ok()?);
        let (dx, dy) = (self.glyphs.dots_x(), self.glyphs.dots_y());
        if x >= self.width() || y >= self.height() {
            return None;
        }
        let i = (y / dy) * self.cols + x / dx;
        Some((i, 1 << ((y % dy) * 2 + x % dx)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The canvas mask of a sextant's or an octant's dot digits: dot `d` is
    /// bit `d - 1`.
    fn block_mask(digits: &str) -> u8 {
        digits.bytes().map(|d| 1u8 << (d - b'1')).fold(0, |a, b| a | b)
    }

    /// The canvas mask of a braille pattern's dot digits, by braille's
    /// numbering: 1, 2, 3, 7 down the left and 4, 5, 6, 8 down the right.
    fn braille_mask(digits: &str) -> u8 {
        let at = |d: u8| -> (u8, u8) {
            match d {
                b'1'..=b'3' => (d - b'1', 0),
                b'4'..=b'6' => (d - b'4', 1),
                b'7' => (3, 0),
                _ => (3, 1),
            }
        };
        digits.bytes().map(at).map(|(row, col)| 1u8 << (row * 2 + col)).fold(0, |a, b| a | b)
    }

    /// The canvas mask of a QUADRANT name's quadrants.
    fn quadrant_mask(name: &str) -> u8 {
        let parts = name.strip_prefix("QUADRANT ").unwrap().split(" AND ");
        parts
            .map(|p| match p {
                "UPPER LEFT" => 1,
                "UPPER RIGHT" => 2,
                "LOWER LEFT" => 4,
                "LOWER RIGHT" => 8,
                _ => panic!("{p}"),
            })
            .fold(0, |a, b| a | b)
    }

    #[test]
    fn glyphs_match_their_unicode_names() {
        // Name and code point pairs from Unicode 16.0.0's UnicodeData.txt.
        let braille = [
            ("BRAILLE PATTERN DOTS-1", 0x2801),
            ("BRAILLE PATTERN DOTS-2", 0x2802),
            ("BRAILLE PATTERN DOTS-3", 0x2804),
            ("BRAILLE PATTERN DOTS-4", 0x2808),
            ("BRAILLE PATTERN DOTS-5", 0x2810),
            ("BRAILLE PATTERN DOTS-6", 0x2820),
            ("BRAILLE PATTERN DOTS-7", 0x2840),
            ("BRAILLE PATTERN DOTS-8", 0x2880),
            ("BRAILLE PATTERN DOTS-1368", 0x28A5),
            ("BRAILLE PATTERN DOTS-12345678", 0x28FF),
        ];
        for (name, code) in braille {
            let digits = name.strip_prefix("BRAILLE PATTERN DOTS-").unwrap();
            assert_eq!(Glyphs::Braille.glyph(braille_mask(digits)), code, "{name}");
        }
        let sextants = [
            ("BLOCK SEXTANT-1", 0x1FB00),
            ("BLOCK SEXTANT-2", 0x1FB01),
            ("BLOCK SEXTANT-3", 0x1FB03),
            ("BLOCK SEXTANT-35", 0x1FB13),
            ("BLOCK SEXTANT-235", 0x1FB14),
            ("BLOCK SEXTANT-6", 0x1FB1E),
            ("BLOCK SEXTANT-146", 0x1FB27),
            ("BLOCK SEXTANT-1246", 0x1FB28),
            ("BLOCK SEXTANT-23456", 0x1FB3B),
        ];
        for (name, code) in sextants {
            assert_eq!(Glyphs::Sextants.glyph(block_mask(name.strip_prefix("BLOCK SEXTANT-").unwrap())), code, "{name}");
        }
        let octants = [
            ("BLOCK OCTANT-3", 0x1CD00),
            ("BLOCK OCTANT-23", 0x1CD01),
            ("BLOCK OCTANT-1235", 0x1CD0F),
            ("BLOCK OCTANT-248", 0x1CD7A),
            ("BLOCK OCTANT-3568", 0x1CDA0),
            ("BLOCK OCTANT-2345678", 0x1CDE5),
        ];
        for (name, code) in octants {
            assert_eq!(Glyphs::Octants.glyph(block_mask(name.strip_prefix("BLOCK OCTANT-").unwrap())), code, "{name}");
        }
        let quadrants = [
            ("QUADRANT LOWER LEFT", 0x2596),
            ("QUADRANT LOWER RIGHT", 0x2597),
            ("QUADRANT UPPER LEFT", 0x2598),
            ("QUADRANT UPPER LEFT AND LOWER LEFT AND LOWER RIGHT", 0x2599),
            ("QUADRANT UPPER LEFT AND LOWER RIGHT", 0x259A),
            ("QUADRANT UPPER LEFT AND UPPER RIGHT AND LOWER LEFT", 0x259B),
            ("QUADRANT UPPER LEFT AND UPPER RIGHT AND LOWER RIGHT", 0x259C),
            ("QUADRANT UPPER RIGHT", 0x259D),
            ("QUADRANT UPPER RIGHT AND LOWER LEFT", 0x259E),
            ("QUADRANT UPPER RIGHT AND LOWER LEFT AND LOWER RIGHT", 0x259F),
        ];
        for (name, code) in quadrants {
            assert_eq!(Glyphs::Quadrants.glyph(quadrant_mask(name)), code, "{name}");
        }
        // The masks the sextant and quadrant blocks leave out, and some the
        // octant block leaves out, drawn by block elements:
        // 2580 UPPER HALF BLOCK, 2582 LOWER ONE QUARTER BLOCK, 2584 LOWER
        // HALF BLOCK, 2586 LOWER THREE QUARTERS BLOCK, 2588 FULL BLOCK, 258C
        // LEFT HALF BLOCK, 2590 RIGHT HALF BLOCK, 1CEA0 RIGHT HALF LOWER ONE
        // QUARTER BLOCK, 1FB82 UPPER ONE QUARTER BLOCK, 1FB85 UPPER THREE
        // QUARTERS BLOCK, 1FBE6 MIDDLE LEFT ONE QUARTER BLOCK.
        assert_eq!(Glyphs::Sextants.glyph(block_mask("135")), 0x258C);
        assert_eq!(Glyphs::Sextants.glyph(block_mask("246")), 0x2590);
        assert_eq!(Glyphs::Sextants.glyph(block_mask("123456")), 0x2588);
        assert_eq!(Glyphs::Quadrants.glyph(0b0011), 0x2580);
        assert_eq!(Glyphs::Quadrants.glyph(0b1100), 0x2584);
        assert_eq!(Glyphs::Quadrants.glyph(0b0101), 0x258C);
        assert_eq!(Glyphs::Quadrants.glyph(0b1010), 0x2590);
        assert_eq!(Glyphs::Quadrants.glyph(0b1111), 0x2588);
        for (digits, code) in [
            ("1234", 0x2580),
            ("78", 0x2582),
            ("5678", 0x2584),
            ("345678", 0x2586),
            ("12345678", 0x2588),
            ("1357", 0x258C),
            ("2468", 0x2590),
            ("8", 0x1CEA0),
            ("12", 0x1FB82),
            ("123456", 0x1FB85),
            ("35", 0x1FBE6),
            ("13", 0x2598),
            ("1368", 0x259A),
        ] {
            assert_eq!(Glyphs::Octants.glyph(block_mask(digits)), code, "octant dots {digits}");
        }
    }

    #[test]
    fn every_mask_has_its_own_glyph() {
        for glyphs in [Glyphs::Braille, Glyphs::Octants, Glyphs::Sextants, Glyphs::Quadrants] {
            let masks = 1u16 << (2 * glyphs.dots_y());
            let mut codes: Vec<u32> = (0..masks).map(|m| glyphs.glyph(m as u8)).collect();
            codes.sort_unstable();
            codes.dedup();
            assert_eq!(codes.len(), usize::from(masks), "{glyphs:?}");
        }
        // The sextant block runs U+1FB00 to U+1FB3B with no gaps.
        let block: Vec<u32> = (1..63u8).filter(|m| ![21, 42].contains(m)).map(|m| Glyphs::Sextants.glyph(m)).collect();
        assert_eq!(block, (0x1FB00..=0x1FB3B).collect::<Vec<u32>>());
    }

    #[test]
    fn an_empty_cell_is_a_space() {
        for glyphs in [Glyphs::Braille, Glyphs::Octants, Glyphs::Sextants, Glyphs::Quadrants] {
            assert_eq!(glyphs.glyph(0), BLANK, "{glyphs:?}");
        }
        assert_eq!(Glyphs::Quadrants.glyph(0xF0), BLANK, "bits past the cell's dots are ignored");
    }

    #[test]
    fn dots_set_clear_and_read() {
        let mut canvas = Canvas::new(Glyphs::Braille, 2, 1);
        assert_eq!((canvas.width(), canvas.height()), (4, 4));
        canvas.set(3, 3);
        canvas.set(0, 0);
        assert!(canvas.get(3, 3) && canvas.get(0, 0) && !canvas.get(1, 0));
        assert_eq!(canvas.mask(1, 0), 0b1000_0000);
        canvas.unset(0, 0);
        assert!(!canvas.get(0, 0));
        assert!(!canvas.get(-1, 0) && !canvas.get(4, 0) && !canvas.get(0, 4));
        canvas.set(-1, 0);
        canvas.set(i64::MAX, i64::MIN);
        assert_eq!(canvas.mask(0, 0), 0);
        let mut grid = Grid::new(2, 1);
        canvas.draw(&mut grid, 0, 0, 1);
        assert_eq!(grid.text(), " \u{2880}", "braille dot 8 is the bottom right");
        assert_eq!(Canvas::new(Glyphs::Sextants, 3, 2).height(), 6);
        assert_eq!(Canvas::new(Glyphs::Quadrants, 3, 2).height(), 4);
    }

    #[test]
    fn lines_and_circles_clip() {
        let mut canvas = Canvas::new(Glyphs::Quadrants, 2, 2);
        canvas.line(-10, -10, 10, 10);
        let dots: Vec<(i64, i64)> = (0..4).flat_map(|y| (0..4).map(move |x| (x, y))).filter(|&(x, y)| canvas.get(x, y)).collect();
        assert_eq!(dots, [(0, 0), (1, 1), (2, 2), (3, 3)]);
        let mut canvas = Canvas::new(Glyphs::Braille, 3, 2);
        canvas.line(0, 7, 5, 7);
        assert!((0..6).all(|x| canvas.get(x, 7)), "both ends included");
        let mut canvas = Canvas::new(Glyphs::Octants, 4, 2);
        canvas.circle(4, 4, 3);
        for (x, y) in [(7, 4), (1, 4), (4, 1), (4, 7)] {
            assert!(canvas.get(x, y), "({x}, {y})");
        }
        assert!(!canvas.get(4, 4));
        // The radius-3 midpoint circle has 16 distinct dots, all on the canvas.
        let lit = (0..8).flat_map(|y| (0..8).map(move |x| (x, y))).filter(|&(x, y)| canvas.get(x, y)).count();
        assert_eq!(lit, 16);
        let mut clipped = Canvas::new(Glyphs::Octants, 4, 2);
        clipped.circle(0, 0, 3);
        assert!(clipped.get(3, 0) && clipped.get(0, 3) && clipped.get(2, 2));
        let mut zero = Canvas::new(Glyphs::Braille, 2, 2);
        zero.circle(1, 1, 0);
        zero.circle(1, 1, -1);
        zero.circle(i64::MAX, i64::MAX, MAX_RADIUS);
        assert_eq!(zero.mask(0, 0), 1 << 3);
    }

    #[test]
    fn drawing_sets_only_cells_with_dots() {
        let mut grid = Grid::new(4, 1);
        grid.write(0, 0, "abcd", 2);
        grid.set_back(1, 0, 7);
        let mut canvas = Canvas::new(Glyphs::Quadrants, 3, 1);
        canvas.set(1, 0);
        canvas.set(2, 1);
        canvas.set(3, 1);
        canvas.draw(&mut grid, 1, 0, 5);
        assert_eq!(grid.glyphs(), &['a' as u32, 0x259D, 0x2584, 'd' as u32]);
        assert_eq!(grid.inks(), &[2, 5, 5, 2]);
        assert_eq!(grid.back(1, 0), 7, "backs are left as they are");
        // A second canvas layers over the first where it has dots.
        let mut over = Canvas::new(Glyphs::Sextants, 4, 1);
        over.set(7, 2);
        over.draw(&mut grid, 0, 0, 6);
        assert_eq!(grid.glyphs(), &['a' as u32, 0x259D, 0x2584, 0x1FB1E]);
        // Drawn past the edges, it clips.
        canvas.draw(&mut grid, -1, 0, 1);
        canvas.draw(&mut grid, 3, 5, 1);
        assert_eq!(grid.glyph(0, 0), 0x2584);
    }

    #[test]
    fn resize_clears_and_reuses() {
        let mut canvas = Canvas::new(Glyphs::Octants, 10, 10);
        canvas.set(0, 0);
        canvas.resize(5, 5);
        assert!(!canvas.get(0, 0));
        assert_eq!((canvas.cols(), canvas.rows(), canvas.glyphs()), (5, 5, Glyphs::Octants));
        canvas.set(9, 19);
        canvas.clear();
        assert!(!canvas.get(9, 19));
    }
}
