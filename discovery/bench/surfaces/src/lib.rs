//! Throwaway: a hello chart for discovery/surfaces.md. A line of values drawn
//! in braille into a grid of code points and colour indices, with a y axis of
//! three labels and an x axis in box drawing. Shared by the CLIs and the wasm.

/// A frame: `cols × rows` code points and colour indices (0 axis, 1 series, 2 crosshair).
pub struct Grid {
    pub cols: usize,
    pub rows: usize,
    pub cells: Vec<u32>,
    pub colors: Vec<u8>,
}

impl Grid {
    pub fn new(cols: usize, rows: usize) -> Self {
        let n = rows * cols;
        Grid { cols, rows, cells: std::iter::repeat(u32::from(' ')).take(n).collect(), colors: std::iter::repeat(0).take(n).collect() }
    }

    /// Writes one cell; anything off the frame is dropped.
    fn put(&mut self, x: usize, y: usize, ch: char, color: u8) {
        if x >= self.cols || y >= self.rows {
            return;
        }
        let at = y * self.cols + x;
        self.cells[at] = u32::from(ch);
        self.colors[at] = color;
    }

    /// Writes a label in the axis colour, one char a cell rightward from (x, y).
    fn text_at(&mut self, x: usize, y: usize, s: &str) {
        let mut col = x;
        for ch in s.chars() {
            self.put(col, y, ch, 0);
            col += 1;
        }
    }

    /// The frame as text, a line a row, each ended by a newline.
    pub fn text(&self) -> String {
        let mut out = String::new();
        for y in 0..self.rows {
            for x in 0..self.cols {
                out.push(char::from_u32(self.cells[y * self.cols + x]).unwrap_or(' '));
            }
            out.push('\n');
        }
        out
    }
}

/// Every point from `from` to `to`, ends included: Bresenham's midpoint method,
/// one step along the longer axis each time.
fn segment(from: (i64, i64), to: (i64, i64), mut plot: impl FnMut(i64, i64)) {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let steps = dx.abs().max(dy.abs());
    let minor = dx.abs().min(dy.abs());
    let x_major = dx.abs() >= dy.abs();
    let (mut x, mut y) = from;
    let mut d = 2 * minor - steps;
    for _ in 0..=steps {
        plot(x, y);
        if d >= 0 {
            if x_major {
                y += dy.signum();
            } else {
                x += dx.signum();
            }
            d -= 2 * steps;
        }
        d += 2 * minor;
        if x_major {
            x += dx.signum();
        } else {
            y += dy.signum();
        }
    }
}

const LABEL: usize = 8;

fn label(v: f64) -> String {
    format!("{v:>7.1}")
}

/// Draws `values` as a braille line in a `cols × rows` grid, with a crosshair at column `cross`.
pub fn chart(values: &[f64], cols: usize, rows: usize, cross: Option<usize>) -> Grid {
    let mut g = Grid::new(cols, rows);
    if cols <= LABEL + 2 || rows < 3 {
        return g;
    }
    let (w, h) = (cols - LABEL - 1, rows - 2);
    let (lo, hi) = values.iter().filter(|v| v.is_finite()).fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &v| (a.min(v), b.max(v)));
    let (lo, hi) = if lo.is_finite() { (lo, if hi > lo { hi } else { lo + 1.0 }) } else { (0.0, 1.0) };
    // Axes.
    for y in 0..h {
        g.put(LABEL, y, '│', 0);
    }
    g.put(LABEL, h, '└', 0);
    for x in LABEL + 1..cols {
        g.put(x, h, '─', 0);
    }
    for (y, v) in [(0, hi), (h / 2, (lo + hi) / 2.0), (h - 1, lo)] {
        g.text_at(0, y, &label(v));
        g.put(LABEL, y, '┤', 0);
    }
    g.text_at(LABEL + 1, h + 1, "0");
    let n = values.len().to_string();
    g.text_at(cols - n.len(), h + 1, &n);
    // Dots: 2 × 4 a cell, each cell's byte the low byte of its braille code point
    // (dots 1-2-3 left 0x01/0x02/0x04, 4-5-6 right 0x08/0x10/0x20, 7 and 8 below 0x40/0x80).
    let (dw, dh) = ((2 * w) as i64, (4 * h) as i64);
    let mut dots = vec![0u8; w * h];
    let mut set = |x: i64, y: i64| {
        if (0..dw).contains(&x) && (0..dh).contains(&y) {
            let (col, row) = ((x % 2) as u32, (y % 4) as u32);
            let bit = if row < 3 { 1u8 << (row + 3 * col) } else { 0x40 << col };
            dots[(y / 4) as usize * w + (x / 2) as usize] |= bit;
        }
    };
    let pt = |i: usize, v: f64| -> (i64, i64) {
        let x = if values.len() > 1 { i as f64 * (dw - 1) as f64 / (values.len() - 1) as f64 } else { 0.0 };
        let y = (hi - v) / (hi - lo) * (dh - 1) as f64;
        (x.round() as i64, y.round() as i64)
    };
    let mut last: Option<(i64, i64)> = None;
    for (i, &v) in values.iter().enumerate() {
        if !v.is_finite() {
            last = None;
            continue;
        }
        let p = pt(i, v);
        segment(last.unwrap_or(p), p, &mut set);
        last = Some(p);
    }
    for (k, &d) in dots.iter().enumerate() {
        if d != 0 {
            g.put(LABEL + 1 + k % w, k / w, char::from_u32(0x2800 | u32::from(d)).unwrap_or(' '), 1);
        }
    }
    if let Some(c) = cross.filter(|&c| c > LABEL && c < cols) {
        for y in 0..h {
            if g.cells[y * cols + c] == ' ' as u32 {
                g.put(c, y, '│', 2);
            } else {
                g.colors[y * cols + c] = 2;
            }
        }
    }
    g
}

/// A demo series: a sine with a slow drift.
pub fn demo(n: usize) -> Vec<f64> {
    (0..n).map(|i| (i as f64 / 9.0).sin() * 10.0 + i as f64 / 20.0).collect()
}

/// Numbers, one a line (the last field of a CSV line); anything else is a gap.
pub fn parse(text: &str) -> Vec<f64> {
    text.lines().filter(|l| !l.trim().is_empty()).map(|l| l.rsplit(',').next().unwrap_or("").trim().parse().unwrap_or(f64::NAN)).collect()
}

#[cfg(feature = "wasm")]
mod wasm {
    use wasm_bindgen::prelude::*;

    /// The first frame as text: what a server would put in the element before it loads.
    #[wasm_bindgen]
    pub fn render_text(values: &[f64], cols: usize, rows: usize) -> String {
        super::chart(values, cols, rows, None).text()
    }
}
