//! Throwaway benchmark for discovery/interaction.md: what one pointer move costs.
//!
//! A line chart of `n` sorted points drawn in braille into a `cols × rows` grid
//! of `u32` code points and `u8` colours, with a y axis,
//! an x axis and a readout row. One pointer event is: pointer (in cells, with
//! fractions) to dot and to data value, a view-state update, a hit test, and
//! either the crosshair overlay composed over a cached base frame or a full
//! frame drawn again from the data. A scatter of `n` unsorted points gives the
//! spatial-grid hit test and its linear-scan baseline.

use wasm_bindgen::prelude::*;

const PLOT_LEFT: usize = 9; // y labels (8) and the axis line (1)
const PLOT_TOP: usize = 1; // the readout row
const PLOT_BOTTOM: usize = 2; // the axis line and x labels

/// A small deterministic PRNG: Vigna's SplitMix64 (a Weyl sequence through a
/// 64-bit finaliser), as public-domain reference code describes it.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// A float in [0, 1): the top 52 bits as the mantissa of a float in [1, 2), less one.
    fn f(&mut self) -> f64 {
        f64::from_bits(0x3FF0_0000_0000_0000 | (self.next() >> 12)) - 1.0
    }
}

/// Columns of a series.
pub struct Series {
    pub x: Vec<f64>,
    pub y: Vec<f64>,
}

/// `n` points, x ascending with irregular gaps (timestamps), y a random walk.
pub fn line_series(n: usize, seed: u64) -> Series {
    let mut r = Rng(seed | 1);
    let (mut x, mut y) = (Vec::with_capacity(n), Vec::with_capacity(n));
    let (mut t, mut v) = (0.0, 100.0);
    for _ in 0..n {
        t += 0.5 + r.f();
        v += r.f() - 0.5;
        x.push(t);
        y.push(v);
    }
    Series { x, y }
}

/// `n` points in no order, in a few clusters.
pub fn scatter_series(n: usize, seed: u64) -> Series {
    let mut r = Rng(seed | 1);
    let (mut x, mut y) = (Vec::with_capacity(n), Vec::with_capacity(n));
    for i in 0..n {
        let c = (i % 5) as f64;
        x.push(c * 20.0 + (r.f() + r.f() + r.f()) * 15.0);
        y.push(c * 10.0 + (r.f() + r.f() + r.f()) * 12.0);
    }
    Series { x, y }
}

/// The view state the pointer changes.
#[derive(Clone, Copy, Debug)]
pub struct View {
    pub x0: f64,
    pub x1: f64,
    pub y0: f64,
    pub y1: f64,
    /// Pointer in cells from the element's top left; None when it is outside.
    pub cursor: Option<(f64, f64)>,
    /// The datum under the crosshair.
    pub hit: Option<usize>,
}

/// The frame buffer: `cols x rows` cells, row by row, each a code point and a colour index.
pub struct Grid {
    pub cols: usize,
    pub rows: usize,
    pub cells: Vec<u32>,
    pub colors: Vec<u8>,
}

impl Grid {
    pub fn new(cols: usize, rows: usize) -> Grid {
        let mut g = Grid { cols, rows, cells: Vec::new(), colors: Vec::new() };
        g.cells.resize(cols * rows, u32::from(' '));
        g.colors.resize(cols * rows, 0);
        g
    }
    fn clear(&mut self) {
        self.cells.iter_mut().for_each(|c| *c = u32::from(' '));
        self.colors.iter_mut().for_each(|k| *k = 0);
    }
    /// Writes one cell; positions off the frame are dropped.
    fn set(&mut self, x: usize, y: usize, ch: u32, c: u8) {
        if y >= self.rows || x >= self.cols {
            return;
        }
        let at = x + self.cols * y;
        self.cells[at] = ch;
        self.colors[at] = c;
    }
    /// Writes `s` from (x, y) rightward, one char a cell, clipped at the right edge.
    fn text(&mut self, x: usize, y: usize, s: &str, c: u8) {
        let mut col = x;
        for ch in s.chars() {
            self.set(col, y, u32::from(ch), c);
            col += 1;
        }
    }
    /// Cells whose glyph or colour differ from `other`: what the canvas painter would redraw.
    pub fn diff(&self, other: &Grid) -> usize {
        let mut n = 0;
        for i in 0..self.cells.len() {
            if self.cells[i] != other.cells[i] || self.colors[i] != other.colors[i] {
                n += 1;
            }
        }
        n
    }
}

/// Braille dot bits, `[row][col]`.
const BRAILLE: [[u8; 2]; 4] = [[0x01, 0x08], [0x02, 0x10], [0x04, 0x20], [0x40, 0x80]];

/// The chart: data, view, the base frame (cached while the view holds) and the frame shown.
#[wasm_bindgen]
pub struct Bench {
    line: Series,
    scatter: Series,
    view: View,
    pw: usize,
    ph: usize,
    masks: Vec<u8>,
    colmin: Vec<f64>,
    colmax: Vec<f64>,
    base: Grid,
    out: Grid,
    // Scatter spatial grid, CSR over dots of the plot: start[d]..start[d+1] in idx.
    sg_start: Vec<u32>,
    sg_idx: Vec<u32>,
    readout: String,
}

#[derive(serde::Deserialize)]
struct PointerJson {
    #[allow(dead_code)]
    kind: String,
    x: f64,
    y: f64,
}

#[wasm_bindgen]
impl Bench {
    /// `n` points each for the line and the scatter, drawn into `cols × rows` cells.
    #[wasm_bindgen(constructor)]
    pub fn new(n: usize, cols: usize, rows: usize) -> Bench {
        let line = line_series(n, 7);
        let scatter = scatter_series(n, 11);
        let (pw, ph) = (cols - PLOT_LEFT, rows - PLOT_TOP - PLOT_BOTTOM);
        let x0 = line.x[0];
        let x1 = line.x[n - 1];
        let (mut y0, mut y1) = (f64::INFINITY, f64::NEG_INFINITY);
        for &v in &line.y {
            y0 = y0.min(v);
            y1 = y1.max(v);
        }
        let mut b = Bench {
            line,
            scatter,
            view: View { x0, x1, y0, y1, cursor: None, hit: None },
            pw,
            ph,
            masks: vec![0; pw * ph],
            colmin: vec![0.0; pw * 2],
            colmax: vec![0.0; pw * 2],
            base: Grid::new(cols, rows),
            out: Grid::new(cols, rows),
            sg_start: Vec::new(),
            sg_idx: Vec::new(),
            readout: String::with_capacity(64),
        };
        b.full_frame();
        b
    }

    /// The x domain to show, as a fraction of the data's: a zoom.
    pub fn zoom_to(&mut self, from: f64, to: f64) {
        let (a, z) = (self.line.x[0], self.line.x[self.line.x.len() - 1]);
        self.view.x0 = a + (z - a) * from;
        self.view.x1 = a + (z - a) * to;
        self.full_frame();
    }

    /// Nothing: the cost of crossing into wasm with two numbers and back with one.
    pub fn noop(&mut self, x: f64, y: f64) -> u32 {
        (x + y) as u32
    }

    /// Pointer to view state only: cell → dot and value, stored as the cursor.
    pub fn move_only(&mut self, x: f64, y: f64) -> u32 {
        self.view.cursor = Some((x, y));
        let (dx, _dy) = self.dot(x, y);
        dx as u32
    }

    /// Pointer to view state and the nearest datum by x (binary search). Returns its index.
    pub fn hit_only(&mut self, x: f64, y: f64) -> u32 {
        self.view.cursor = Some((x, y));
        let i = self.hit_line(x);
        self.view.hit = Some(i);
        i as u32
    }

    /// The whole hover path: view state, hit test, readout text, crosshair over the cached base.
    pub fn pointer(&mut self, x: f64, y: f64) -> u32 {
        let i = self.hit_only(x, y);
        self.overlay();
        i
    }

    /// The same event as JSON text parsed with serde_json, for comparison.
    pub fn pointer_json(&mut self, s: &str) -> u32 {
        let p: PointerJson = serde_json::from_str(s).expect("pointer json");
        self.pointer(p.x, p.y)
    }

    /// The hover path, but drawing the whole frame from the data again.
    pub fn pointer_full(&mut self, x: f64, y: f64) -> u32 {
        let i = self.hit_only(x, y);
        self.full_frame();
        self.overlay();
        i
    }

    /// A pan by `dx` cells: the view moves, so the base frame is drawn again.
    pub fn pan(&mut self, dx: f64) -> u32 {
        let span = self.view.x1 - self.view.x0;
        let shift = dx / self.pw as f64 * span;
        self.view.x0 += shift;
        self.view.x1 += shift;
        self.full_frame();
        self.overlay();
        self.pw as u32
    }

    /// Builds the scatter's spatial grid for the current view (once per view change).
    pub fn scatter_build(&mut self) -> u32 {
        let (dw, dh) = (self.pw * 2, self.ph * 4);
        let (sx0, sx1, sy0, sy1) = (0.0, 100.0 + 45.0, 0.0, 40.0 + 36.0);
        let mut count = vec![0u32; dw * dh + 1];
        let dot = |x: f64, y: f64| -> Option<usize> {
            let fx = (x - sx0) / (sx1 - sx0) * dw as f64;
            let fy = (1.0 - (y - sy0) / (sy1 - sy0)) * dh as f64;
            if fx < 0.0 || fy < 0.0 || fx >= dw as f64 || fy >= dh as f64 {
                return None;
            }
            Some(fy as usize * dw + fx as usize)
        };
        for k in 0..self.scatter.x.len() {
            if let Some(d) = dot(self.scatter.x[k], self.scatter.y[k]) {
                count[d + 1] += 1;
            }
        }
        for d in 0..dw * dh {
            count[d + 1] += count[d];
        }
        let mut fill = count.clone();
        let mut idx = vec![0u32; count[dw * dh] as usize];
        for k in 0..self.scatter.x.len() {
            if let Some(d) = dot(self.scatter.x[k], self.scatter.y[k]) {
                idx[fill[d] as usize] = k as u32;
                fill[d] += 1;
            }
        }
        self.sg_start = count;
        self.sg_idx = idx;
        self.sg_idx.len() as u32
    }

    /// Nearest scatter point to the pointer in dot space, from the spatial grid: rings outward.
    pub fn scatter_query(&mut self, x: f64, y: f64) -> u32 {
        let (dw, dh) = (self.pw * 2, self.ph * 4);
        let (px, py) = self.dot(x, y);
        let (px, py) = (px.clamp(0, dw as i64 - 1), py.clamp(0, dh as i64 - 1));
        let mut best = (u64::MAX, u32::MAX);
        let visit = |gx: i64, gy: i64, best: &mut (u64, u32)| {
            if gx < 0 || gy < 0 || gx >= dw as i64 || gy >= dh as i64 {
                return;
            }
            let d = gy as usize * dw + gx as usize;
            let (a, z) = (self.sg_start[d] as usize, self.sg_start[d + 1] as usize);
            if a < z {
                let dd = ((gx - px) * (gx - px) + (gy - py) * (gy - py)) as u64;
                if dd < best.0 {
                    *best = (dd, self.sg_idx[a]);
                }
            }
        };
        // Rings of Chebyshev radius r, perimeter only; stop once no ring can hold a nearer dot.
        for r in 0..(dw.max(dh) as i64) {
            if best.0 != u64::MAX && (r * r) as u64 > best.0 {
                break;
            }
            if r == 0 {
                visit(px, py, &mut best);
                continue;
            }
            for g in (px - r)..=(px + r) {
                visit(g, py - r, &mut best);
                visit(g, py + r, &mut best);
            }
            for g in (py - r + 1)..(py + r) {
                visit(px - r, g, &mut best);
                visit(px + r, g, &mut best);
            }
        }
        best.1
    }

    /// Nearest scatter point by scanning every point (the baseline the grid replaces).
    pub fn scatter_linear(&mut self, x: f64, y: f64) -> u32 {
        let (dw, dh) = (self.pw * 2, self.ph * 4);
        let (sx0, sx1, sy0, sy1) = (0.0, 145.0, 0.0, 76.0);
        let (px, py) = self.dot(x, y);
        let (px, py) = (px as f64 + 0.5, py as f64 + 0.5);
        let (mut bd, mut bi) = (f64::INFINITY, 0u32);
        for k in 0..self.scatter.x.len() {
            let fx = (self.scatter.x[k] - sx0) / (sx1 - sx0) * dw as f64 - px;
            let fy = (1.0 - (self.scatter.y[k] - sy0) / (sy1 - sy0)) * dh as f64 - py;
            let d = fx * fx + fy * fy;
            if d < bd {
                bd = d;
                bi = k as u32;
            }
        }
        bi
    }

    /// Draws the base frame from the data (axes, labels, the line), then shows it.
    pub fn full_frame(&mut self) {
        let g = &mut self.base;
        g.clear();
        self.masks.fill(0);
        let (pw, ph) = (self.pw, self.ph);
        let (dw, dh) = (pw * 2, ph * 4);
        let v = self.view;
        // Visible range by binary search, then min and max of y per dot column (one pass: x is sorted).
        let i0 = self.line.x.partition_point(|&x| x < v.x0).saturating_sub(1);
        let i1 = (self.line.x.partition_point(|&x| x <= v.x1) + 1).min(self.line.x.len());
        self.colmin.fill(f64::INFINITY);
        self.colmax.fill(f64::NEG_INFINITY);
        let sx = dw as f64 / (v.x1 - v.x0);
        for k in i0..i1 {
            let c = ((self.line.x[k] - v.x0) * sx) as i64;
            if c < 0 || c >= dw as i64 {
                continue;
            }
            let c = c as usize;
            let yv = self.line.y[k];
            if yv < self.colmin[c] {
                self.colmin[c] = yv;
            }
            if yv > self.colmax[c] {
                self.colmax[c] = yv;
            }
        }
        let sy = (dh - 1) as f64 / (v.y1 - v.y0);
        let to_dot = |yv: f64| -> i64 { ((v.y1 - yv) * sy).round().clamp(0.0, (dh - 1) as f64) as i64 };
        let mut prev: Option<(i64, i64)> = None;
        for c in 0..dw {
            if self.colmin[c] > self.colmax[c] {
                continue;
            }
            let (mut top, mut bot) = (to_dot(self.colmax[c]), to_dot(self.colmin[c]));
            if let Some((pt, pb)) = prev {
                // join to the previous column so the line has no gaps
                top = top.min(pb.max(pt));
                bot = bot.max(pt.min(pb));
            }
            for d in top..=bot {
                let (cx, cy) = (c / 2, d as usize / 4);
                self.masks[cy * pw + cx] |= BRAILLE[d as usize % 4][c % 2];
            }
            prev = Some((to_dot(self.colmax[c]), to_dot(self.colmin[c])));
        }
        for cy in 0..ph {
            for cx in 0..pw {
                let m = self.masks[cy * pw + cx];
                if m != 0 {
                    g.set(PLOT_LEFT + cx, PLOT_TOP + cy, 0x2800 + m as u32, 1);
                }
            }
        }
        // Axes: a y axis line with five labels, an x axis line with labels every 16 cells.
        for cy in 0..ph {
            g.set(PLOT_LEFT - 1, PLOT_TOP + cy, '│' as u32, 2);
        }
        g.set(PLOT_LEFT - 1, PLOT_TOP + ph, '└' as u32, 2);
        for cx in 0..pw {
            g.set(PLOT_LEFT + cx, PLOT_TOP + ph, '─' as u32, 2);
        }
        let mut s = String::with_capacity(16);
        for k in 0..5 {
            let cy = k * (ph - 1) / 4;
            let val = v.y1 - (v.y1 - v.y0) * cy as f64 / (ph - 1) as f64;
            s.clear();
            use std::fmt::Write;
            let _ = write!(s, "{val:>8.2}");
            g.text(0, PLOT_TOP + cy, &s, 2);
        }
        let mut cx = 0;
        while cx + 8 < pw {
            let val = v.x0 + (v.x1 - v.x0) * cx as f64 / pw as f64;
            s.clear();
            use std::fmt::Write;
            let _ = write!(s, "{val:.0}");
            g.text(PLOT_LEFT + cx, PLOT_TOP + ph + 1, &s, 2);
            cx += 16;
        }
        self.out.cells.copy_from_slice(&self.base.cells);
        self.out.colors.copy_from_slice(&self.base.colors);
    }

    /// Composes the crosshair and the readout over the cached base frame into the shown frame.
    pub fn overlay(&mut self) {
        self.out.cells.copy_from_slice(&self.base.cells);
        self.out.colors.copy_from_slice(&self.base.colors);
        let (Some((x, _y)), Some(i)) = (self.view.cursor, self.view.hit) else { return };
        let v = self.view;
        let cx = (x.floor() as i64).clamp(PLOT_LEFT as i64, (PLOT_LEFT + self.pw - 1) as i64) as usize;
        let dh = self.ph * 4;
        let yv = self.line.y[i];
        let dy = (((v.y1 - yv) / (v.y1 - v.y0)) * (dh - 1) as f64).round().clamp(0.0, (dh - 1) as f64) as usize;
        let cy = PLOT_TOP + dy / 4;
        let cols = self.out.cols;
        // Vertical rule where the plot is blank; the line's own glyph keeps its place, recoloured.
        for r in PLOT_TOP..PLOT_TOP + self.ph {
            let k = r * cols + cx;
            if self.out.cells[k] == 0x20 {
                self.out.cells[k] = '┊' as u32;
            }
            self.out.colors[k] = 3;
        }
        for c in PLOT_LEFT..PLOT_LEFT + self.pw {
            let k = cy * cols + c;
            if self.out.cells[k] == 0x20 {
                self.out.cells[k] = '┄' as u32;
            }
            self.out.colors[k] = 3;
        }
        self.out.set(cx, cy, '┼' as u32, 4);
        self.readout.clear();
        use std::fmt::Write;
        let _ = write!(self.readout, "x {:.1}  y {:.3}  #{}", self.line.x[i], yv, i);
        let w = self.readout.chars().count();
        let at = cols.saturating_sub(w + 1);
        let s = std::mem::take(&mut self.readout);
        self.out.text(at, 0, &s, 4);
        self.readout = s;
    }

    /// Runs `n` events of one kind inside wasm (no crossing per event); returns a checksum.
    /// kind: 0 move_only, 1 hit_only, 2 pointer, 3 pointer_full, 4 scatter_query, 5 pan.
    pub fn run_inside(&mut self, kind: u32, n: u32) -> f64 {
        let mut sum = 0.0;
        let (c, r) = (self.out.cols as f64, self.out.rows as f64);
        for k in 0..n {
            let (x, y) = path(k, n, c, r);
            sum += match kind {
                0 => self.move_only(x, y),
                1 => self.hit_only(x, y),
                2 => self.pointer(x, y),
                3 => self.pointer_full(x, y),
                4 => self.scatter_query(x, y),
                _ => self.pan(if k % 2 == 0 { 0.5 } else { -0.5 }),
            } as f64;
        }
        sum
    }

    pub fn cols(&self) -> usize {
        self.out.cols
    }
    pub fn rows(&self) -> usize {
        self.out.rows
    }
    pub fn cells_ptr(&self) -> *const u32 {
        self.out.cells.as_ptr()
    }
    pub fn colors_ptr(&self) -> *const u8 {
        self.out.colors.as_ptr()
    }
    /// The shown frame as text.
    pub fn text(&self) -> String {
        let mut s = String::new();
        for y in 0..self.out.rows {
            for x in 0..self.out.cols {
                s.push(char::from_u32(self.out.cells[y * self.out.cols + x]).unwrap_or('?'));
            }
            s.push('\n');
        }
        s
    }
}

impl Bench {
    /// Pointer in cells (fractions inside a cell) to a dot of the plot's braille canvas.
    fn dot(&self, x: f64, y: f64) -> (i64, i64) {
        (((x - PLOT_LEFT as f64) * 2.0).floor() as i64, ((y - PLOT_TOP as f64) * 4.0).floor() as i64)
    }

    /// Nearest line datum by x to the pointer's column: dot centre → value → binary search.
    fn hit_line(&self, x: f64) -> usize {
        let dw = (self.pw * 2) as f64;
        let (dx, _) = self.dot(x, 0.0);
        let dx = (dx as f64).clamp(0.0, dw - 1.0) + 0.5;
        let val = self.view.x0 + (self.view.x1 - self.view.x0) * dx / dw;
        let xs = &self.line.x;
        let j = xs.partition_point(|&v| v < val);
        if j == 0 {
            0
        } else if j >= xs.len() {
            xs.len() - 1
        } else if val - xs[j - 1] <= xs[j] - val {
            j - 1
        } else {
            j
        }
    }

    /// The shown frame, for the native harness.
    pub fn out(&self) -> &Grid {
        &self.out
    }
}

/// A pointer path over the element: a sweep left to right with a vertical wiggle, in cells.
pub fn path(k: u32, n: u32, cols: f64, rows: f64) -> (f64, f64) {
    let f = k as f64 / n as f64;
    let x = PLOT_LEFT as f64 + 0.01 + f * (cols - PLOT_LEFT as f64 - 0.02);
    let y = PLOT_TOP as f64 + (rows - 4.0) * (0.5 + 0.4 * (f * 37.0).sin());
    (x, y)
}
