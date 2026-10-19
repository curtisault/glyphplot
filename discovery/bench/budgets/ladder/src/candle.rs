//! Step 5: a candlestick chart in the same module: one candle a cell column,
//! bodies in half blocks (two price steps a cell), wicks in box drawing.
//! With `downsample`, candles that share a column merge (first open, last
//! close, highest high, lowest low); without, the last one wins.

use crate::grid::Grid;
use crate::line::{axes, plot_of, Plot};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct CandleChart {
    grid: Grid,
    plot: Plot,
    t: Vec<f64>,
    ohlc: [Vec<f64>; 4],
    buckets: Vec<[f64; 4]>,
    time: bool,
    scratch: Vec<f64>,
}

#[wasm_bindgen]
impl CandleChart {
    #[wasm_bindgen(constructor)]
    pub fn new(cols: usize, rows: usize) -> CandleChart {
        let (cols, rows) = (cols.max(4), rows.max(4));
        CandleChart { grid: Grid::new(cols, rows), plot: plot_of(cols, rows), t: vec![], ohlc: Default::default(), buckets: vec![], time: false, scratch: vec![] }
    }

    pub fn set_data(&mut self, t: &[f64], o: &[f64], h: &[f64], l: &[f64], c: &[f64]) {
        self.t = t.to_vec();
        self.ohlc = [o.to_vec(), h.to_vec(), l.to_vec(), c.to_vec()];
    }

    pub fn set_time_axis(&mut self, on: bool) {
        self.time = on;
    }

    pub fn cells_ptr(&self) -> *const u32 {
        self.grid.cells.as_ptr()
    }

    pub fn colors_ptr(&self) -> *const u8 {
        self.grid.colors.as_ptr()
    }

    pub fn text(&self) -> String {
        self.grid.text()
    }

    pub fn frame(&mut self) {
        self.grid.clear();
        let p = self.plot;
        let n = self.t.len();
        if n == 0 {
            return;
        }
        let (t0, t1) = (self.t[0], self.t[n - 1].max(self.t[0] + 1.0));
        let sx = (p.cols - 1) as f64 / (t1 - t0);
        self.buckets.clear();
        self.buckets.resize(p.cols, [f64::NAN; 4]);
        let [o, h, l, c] = &self.ohlc;
        for i in 0..n {
            let k = ((self.t[i] - t0) * sx).floor().max(0.0).min((p.cols - 1) as f64) as usize;
            let b = &mut self.buckets[k];
            if cfg!(feature = "downsample") && !b[0].is_nan() {
                b[1] = b[1].max(h[i]);
                b[2] = b[2].min(l[i]);
                b[3] = c[i];
            } else {
                *b = [o[i], h[i], l[i], c[i]];
            }
        }
        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        for b in &self.buckets {
            if !b[0].is_nan() {
                lo = lo.min(b[2]);
                hi = hi.max(b[1]);
            }
        }
        #[cfg(feature = "scale")]
        let (lo, hi) = crate::scale::nice(lo, hi, (p.rows / 3).max(2));
        let halves = (p.rows * 2) as f64;
        let hy = |v: f64| ((hi - v) / (hi - lo) * halves).floor().max(0.0).min(halves - 1.0) as i64;
        for (k, b) in self.buckets.iter().enumerate() {
            if b[0].is_nan() {
                continue;
            }
            let x = (p.x + k) as i64;
            let up = b[3] >= b[0];
            let color = if up { 3 } else { 4 };
            let (bt, bb) = (hy(b[0].max(b[3])), hy(b[0].min(b[3])));
            let (wt, wb) = (hy(b[1]), hy(b[2]));
            for r in wt / 2..=wb / 2 {
                let (top, bot) = (r * 2, r * 2 + 1);
                let body_top = top >= bt && top <= bb;
                let body_bot = bot >= bt && bot <= bb;
                let ch = match (body_top, body_bot) {
                    (true, true) => '█',
                    (true, false) => '▀',
                    (false, true) => '▄',
                    _ => '│',
                };
                self.grid.set(x, p.y as i64 + r, ch, color);
            }
        }
        axes(&mut self.grid, p, t0, t1, lo, hi, self.time, &mut self.scratch);
    }
}
