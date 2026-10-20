//! The frame buffer and the braille dot canvas the ladder draws into, written
//! for this bench. A frame cell is a `u32` code point and a `u8` colour index;
//! the dot canvas keeps, for each cell, the low byte of its braille code point
//! (U+2800 plus the bits of the dots that are on).

/// The code point of an empty cell.
pub const BLANK: u32 = 0x20;

/// A `cols x rows` frame stored row by row.
pub struct Grid {
    pub cols: usize,
    pub rows: usize,
    pub cells: Vec<u32>,
    pub colors: Vec<u8>,
}

impl Grid {
    pub fn new(cols: usize, rows: usize) -> Self {
        let n = cols * rows;
        Grid { cols, rows, cells: vec![BLANK; n], colors: vec![0; n] }
    }

    /// The offset of cell (x, y), or None when it lies outside the frame.
    #[inline]
    fn offset(&self, x: i64, y: i64) -> Option<usize> {
        let (x, y) = (usize::try_from(x).ok()?, usize::try_from(y).ok()?);
        (x < self.cols && y < self.rows).then(|| x + y * self.cols)
    }

    pub fn clear(&mut self) {
        for c in self.cells.iter_mut() {
            *c = BLANK;
        }
        for k in self.colors.iter_mut() {
            *k = 0;
        }
    }

    /// Writes one cell; positions outside the frame are ignored.
    #[inline]
    pub fn set(&mut self, x: i64, y: i64, ch: char, color: u8) {
        if let Some(i) = self.offset(x, y) {
            self.cells[i] = u32::from(ch);
            self.colors[i] = color;
        }
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize) -> u32 {
        self.cells[x + y * self.cols]
    }

    #[inline]
    pub fn recolor(&mut self, x: usize, y: usize, color: u8) {
        let i = x + y * self.cols;
        self.colors[i] = color;
    }

    /// Writes `s` one char a cell rightward from (x, y), clipped to the frame.
    pub fn text_at(&mut self, x: i64, y: i64, s: &str, color: u8) {
        let mut cx = x;
        for ch in s.chars() {
            self.set(cx, y, ch, color);
            cx += 1;
        }
    }

    /// Takes every cell of `other`, which has the same size.
    pub fn copy_from(&mut self, other: &Grid) {
        self.cells.clone_from_slice(&other.cells);
        self.colors.clone_from_slice(&other.colors);
    }

    /// The frame as text: rows joined by newlines, no newline after the last.
    pub fn text(&self) -> String {
        let mut out = String::with_capacity((self.cols + 1) * self.rows);
        for y in 0..self.rows {
            if y != 0 {
                out.push('\n');
            }
            let row = &self.cells[y * self.cols..(y + 1) * self.cols];
            out.extend(row.iter().map(|&cp| char::from_u32(cp).unwrap_or('?')));
        }
        out
    }
}

/// The braille bit of the dot in column `col` (0, 1) and row `row` (0..4) of a
/// cell. Unicode numbers the dots 1-2-3 down the left and 4-5-6 down the right,
/// bits 0x01..0x20; the fourth row, dots 7 and 8, was added later as 0x40 and 0x80.
#[inline]
fn dot_bit(col: usize, row: usize) -> u8 {
    if row < 3 {
        (1u8 << row) << (3 * col)
    } else {
        0x40u8 << col
    }
}

/// Visits every point of the segment (x0, y0)-(x1, y1), ends included, with
/// Bresenham's midpoint decision variable stepping along the longer axis.
fn walk_line(x0: i64, y0: i64, x1: i64, y1: i64, mut visit: impl FnMut(i64, i64)) {
    let (ax, ay) = ((x1 - x0).abs(), (y1 - y0).abs());
    let (sx, sy) = ((x1 - x0).signum(), (y1 - y0).signum());
    if ax >= ay {
        let mut d = 2 * ay - ax;
        let mut y = y0;
        for i in 0..=ax {
            visit(x0 + i * sx, y);
            if d >= 0 {
                y += sy;
                d -= 2 * ax;
            }
            d += 2 * ay;
        }
    } else {
        let mut d = 2 * ax - ay;
        let mut x = x0;
        for i in 0..=ay {
            visit(x, y0 + i * sy);
            if d >= 0 {
                x += sx;
                d -= 2 * ay;
            }
            d += 2 * ax;
        }
    }
}

/// A braille canvas: 2 x 4 dots a cell over `cols x rows` cells.
pub struct Dots {
    pub cols: usize,
    pub rows: usize,
    bits: Vec<u8>,
}

impl Dots {
    pub fn new(cols: usize, rows: usize) -> Self {
        Dots { cols, rows, bits: vec![0u8; rows * cols] }
    }

    /// Width in dots.
    pub fn width(&self) -> i64 {
        2 * self.cols as i64
    }

    /// Height in dots.
    pub fn height(&self) -> i64 {
        4 * self.rows as i64
    }

    pub fn clear(&mut self) {
        for b in self.bits.iter_mut() {
            *b = 0;
        }
    }

    /// Turns on dot (x, y); dots off the canvas are ignored.
    #[inline]
    pub fn set(&mut self, x: i64, y: i64) {
        if (0..self.width()).contains(&x) && (0..self.height()).contains(&y) {
            let (x, y) = (x as usize, y as usize);
            self.bits[(x >> 1) + (y >> 2) * self.cols] |= dot_bit(x & 1, y & 3);
        }
    }

    pub fn line(&mut self, x0: i64, y0: i64, x1: i64, y1: i64) {
        walk_line(x0, y0, x1, y1, |x, y| self.set(x, y));
    }

    /// The dots of column `x` from `y0` to `y1`, both ends included.
    pub fn vline(&mut self, x: i64, y0: i64, y1: i64) {
        let top = y0.min(y1).max(0);
        let bottom = y0.max(y1).min(self.height() - 1);
        let mut y = top;
        while y <= bottom {
            self.set(x, y);
            y += 1;
        }
    }

    /// Writes each cell that has a dot on into `grid`, offset by (x, y).
    pub fn draw(&self, grid: &mut Grid, x: i64, y: i64, color: u8) {
        for (i, &b) in self.bits.iter().enumerate() {
            if b == 0 {
                continue;
            }
            let (cx, cy) = ((i % self.cols) as i64, (i / self.cols) as i64);
            let glyph = char::from_u32(0x2800 | u32::from(b)).unwrap_or(' ');
            grid.set(x + cx, y + cy, glyph, color);
        }
    }
}
