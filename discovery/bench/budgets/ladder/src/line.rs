//! Step 1: a line chart over two Float64Array columns, drawn in braille with
//! box-drawing axes. Later steps add tick labels (`scale`), a time x axis
//! (`time`), options as JSON (`serde`) and M4 downsampling (`downsample`).

use crate::grid::{Dots, Grid, BLANK};
use crate::series::Series;
use wasm_bindgen::prelude::*;

/// Columns of y-axis labels left of the axis line (with `scale`): fixed, so
/// the layout does not jitter while the view pans and the plot's width is
/// known before the y domain is.
#[cfg(feature = "scale")]
pub(crate) const LABEL_COLS: usize = 7;
#[cfg(not(feature = "scale"))]
pub(crate) const LABEL_COLS: usize = 0;
pub(crate) const TOP: usize = 1;
pub(crate) const BOTTOM: usize = if cfg!(feature = "scale") { 2 } else { 1 };

/// The plot's place on the grid, in cells.
#[derive(Clone, Copy, Default)]
pub(crate) struct Plot {
    pub x: usize,
    pub y: usize,
    pub cols: usize,
    pub rows: usize,
}

pub(crate) fn plot_of(cols: usize, rows: usize) -> Plot {
    let x = LABEL_COLS + 1;
    Plot { x, y: TOP, cols: cols.saturating_sub(x).max(1), rows: rows.saturating_sub(TOP + BOTTOM).max(1) }
}

/// Axis lines, ticks and (with `scale`) labels. `y0..y1` is the y domain,
/// bottom to top; `x0..x1` the view.
#[allow(unused_variables)]
pub(crate) fn axes(grid: &mut Grid, p: Plot, x0: f64, x1: f64, y0: f64, y1: f64, time: bool, scratch: &mut Vec<f64>) {
    let ax = (p.x - 1) as i64;
    let ay = (p.y + p.rows) as i64;
    for r in p.y..p.y + p.rows {
        grid.set(ax, r as i64, '│', 0);
    }
    for c in p.x..p.x + p.cols {
        grid.set(c as i64, ay, '─', 0);
    }
    grid.set(ax, ay, '└', 0);
    #[cfg(feature = "scale")]
    {
        let mut s = String::new();
        let t = crate::scale::ticks(y0, y1, (p.rows / 3).max(2));
        for k in 0..t.count {
            let v = t.start + k as f64 * t.step;
            let row = p.y as f64 + (y1 - v) / (y1 - y0) * p.rows as f64;
            let row = (row.floor() as i64).min(ay - 1);
            grid.set(ax, row, '┤', 0);
            s.clear();
            crate::scale::tick_label(&mut s, v, t.step);
            let n = s.chars().count().min(LABEL_COLS) as i64;
            grid.text_at(ax - n, row, &s, 0);
        }
        let n = (p.cols / 12).max(2);
        let sx = p.cols as f64 / (x1 - x0);
        let put = |grid: &mut Grid, v: f64, s: &str| {
            let c = p.x as i64 + ((v - x0) * sx).floor() as i64;
            grid.set(c, ay, '┬', 0);
            let w = s.chars().count() as i64;
            grid.text_at((c - w / 2).clamp(p.x as i64, (p.x + p.cols) as i64 - w), ay + 1, s, 0);
        };
        #[cfg(feature = "time")]
        if time {
            let iv = crate::time::ticks(x0, x1, n, scratch);
            for &v in scratch.iter() {
                s.clear();
                crate::time::label(&mut s, v, iv);
                put(grid, v, &s);
            }
            return;
        }
        let t = crate::scale::ticks(x0, x1, n);
        for k in 0..t.count {
            let v = t.start + k as f64 * t.step;
            s.clear();
            crate::scale::tick_label(&mut s, v, t.step);
            put(grid, v, &s);
        }
    }
}

#[wasm_bindgen]
pub struct LineChart {
    grid: Grid,
    base: Grid,
    dots: Dots,
    series: Series,
    plot: Plot,
    view: (f64, f64),
    ydom: (f64, f64),
    time: bool,
    downsample: bool,
    fixed_y: Option<(f64, f64)>,
    scratch: Vec<f64>,
    #[cfg(feature = "downsample")]
    m4: crate::m4::M4,
}

#[wasm_bindgen]
impl LineChart {
    /// A chart of `cols x rows` cells holding up to `capacity` points.
    #[wasm_bindgen(constructor)]
    pub fn new(cols: usize, rows: usize, capacity: usize) -> LineChart {
        let (cols, rows) = (cols.max(4), rows.max(4));
        let plot = plot_of(cols, rows);
        LineChart {
            grid: Grid::new(cols, rows),
            base: Grid::new(cols, rows),
            dots: Dots::new(plot.cols, plot.rows),
            series: Series::new(capacity),
            plot,
            view: (0.0, 1.0),
            ydom: (0.0, 1.0),
            time: false,
            downsample: false,
            fixed_y: None,
            scratch: Vec::new(),
            #[cfg(feature = "downsample")]
            m4: crate::m4::M4::default(),
        }
    }

    pub fn cols(&self) -> usize {
        self.grid.cols
    }

    pub fn rows(&self) -> usize {
        self.grid.rows
    }

    pub fn len(&self) -> usize {
        self.series.len()
    }

    /// Replaces the data, copying it (wasm-bindgen copies the Float64Arrays in first).
    pub fn set_data(&mut self, x: &[f64], y: &[f64]) {
        self.series.replace(x, y);
    }

    /// Zero-copy load: reserve `n`, write through Float64Array views at
    /// `x_ptr()` and `y_ptr()`, then `commit(n)`.
    pub fn reserve(&mut self, n: usize) {
        self.series.reserve(n);
    }

    pub fn x_ptr(&mut self) -> *mut f64 {
        self.series.x.as_mut_ptr()
    }

    pub fn y_ptr(&mut self) -> *mut f64 {
        self.series.y.as_mut_ptr()
    }

    pub fn commit(&mut self, n: usize) {
        self.series.commit(n);
    }

    pub fn push(&mut self, x: f64, y: f64) {
        self.series.push(x, y);
    }

    pub fn push_batch(&mut self, x: &[f64], y: &[f64]) {
        self.series.push_batch(x, y);
    }

    pub fn set_view(&mut self, x0: f64, x1: f64) {
        self.view = if x1 > x0 { (x0, x1) } else { (x0, x0 + 1.0) };
    }

    /// The view to the data's extent.
    pub fn fit(&mut self) {
        let n = self.series.len();
        if n > 0 {
            self.set_view(self.series.x_at(0), self.series.x_at(n - 1));
        }
    }

    pub fn set_time_axis(&mut self, on: bool) {
        self.time = on;
    }

    pub fn set_downsample(&mut self, on: bool) {
        self.downsample = on;
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

    /// Draws the visible data.
    pub fn frame(&mut self) {
        self.grid.clear();
        self.dots.clear();
        let (x0, x1) = self.view;
        let n = self.series.len();
        let i0 = self.series.lower_bound(x0).saturating_sub(1);
        let i1 = (self.series.lower_bound(x1) + 1).min(n);
        let (w, h) = (self.dots.width(), self.dots.height());
        let sx = w as f64 / (x1 - x0);
        let px = move |x: f64| ((x - x0) * sx).floor().max(-1.0).min(w as f64) as i64;

        #[cfg(feature = "downsample")]
        if self.downsample {
            let (lo, hi) = self.m4.bin(&self.series, i0, i1, w as usize, x0, sx);
            let (y0, y1) = self.domain(lo, hi);
            let sy = h as f64 / (y1 - y0);
            self.m4.draw(&mut self.dots, move |y: f64| ((y1 - y) * sy).floor() as i64);
            self.finish(y0, y1);
            return;
        }

        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        self.series.each(i0, i1, |_, y| {
            if y < lo {
                lo = y;
            }
            if y > hi {
                hi = y;
            }
        });
        let (y0, y1) = self.domain(lo, hi);
        let sy = h as f64 / (y1 - y0);
        let dots = &mut self.dots;
        let mut prev: Option<(i64, i64)> = None;
        self.series.each(i0, i1, |x, y| {
            if y.is_nan() {
                prev = None;
                return;
            }
            let p = (px(x), ((y1 - y) * sy).floor() as i64);
            match prev {
                Some(q) if q != p => dots.line(q.0, q.1, p.0, p.1),
                Some(_) => {}
                None => dots.set(p.0, p.1),
            }
            prev = Some(p);
        });
        self.finish(y0, y1);
    }

    /// Hit-tests the pointer at fractional cell `(col, row)`: the nearest
    /// point by x. Redraws the last frame with a crosshair on it and returns
    /// its index, or -1 with no data.
    pub fn hover(&mut self, col: f64, _row: f64) -> f64 {
        self.grid.copy_from(&self.base);
        let n = self.series.len();
        if n == 0 {
            return -1.0;
        }
        let p = self.plot;
        let (x0, x1) = self.view;
        let x = x0 + (col - p.x as f64) / p.cols as f64 * (x1 - x0);
        let i = self.series.lower_bound(x).min(n - 1);
        let i = if i > 0 && (x - self.series.x_at(i - 1)) < (self.series.x_at(i) - x) { i - 1 } else { i };
        let (xi, yi) = (self.series.x_at(i), self.series.y_at(i));
        let (y0, y1) = self.ydom;
        let cx = p.x as i64 + ((xi - x0) / (x1 - x0) * p.cols as f64).floor() as i64;
        let cy = p.y as i64 + ((y1 - yi) / (y1 - y0) * p.rows as f64).floor() as i64;
        for r in p.y..p.y + p.rows {
            self.cross(cx, r as i64, '│');
        }
        for c in p.x..p.x + p.cols {
            self.cross(c as i64, cy, '─');
        }
        self.cross(cx, cy, '┼');
        #[cfg(feature = "scale")]
        {
            let mut s = String::new();
            #[cfg(feature = "time")]
            if self.time {
                crate::time::label(&mut s, xi, 1000.0);
            }
            if s.is_empty() {
                crate::scale::si(&mut s, xi, 4);
            }
            s.push_str("  ");
            crate::scale::si(&mut s, yi, 4);
            let w = s.chars().count() as i64;
            self.grid.text_at(self.grid.cols as i64 - w, 0, &s, 2);
        }
        i as f64
    }
}

impl LineChart {
    fn cross(&mut self, c: i64, r: i64, ch: char) {
        if c < 0 || r < 0 || c as usize >= self.grid.cols || r as usize >= self.grid.rows {
            return;
        }
        let (c, r) = (c as usize, r as usize);
        if self.grid.get(c, r) == BLANK {
            self.grid.set(c as i64, r as i64, ch, 2);
        } else {
            self.grid.recolor(c, r, 2);
        }
    }

    fn domain(&mut self, lo: f64, hi: f64) -> (f64, f64) {
        if let Some(d) = self.fixed_y {
            return d;
        }
        let (lo, hi) = if lo.is_finite() && hi.is_finite() { (lo, hi) } else { (0.0, 1.0) };
        let (lo, hi) = if hi > lo { (lo, hi) } else { (lo - 0.5, lo + 0.5) };
        #[cfg(feature = "scale")]
        let (lo, hi) = crate::scale::nice(lo, hi, (self.plot.rows / 3).max(2));
        (lo, hi)
    }

    fn finish(&mut self, y0: f64, y1: f64) {
        self.ydom = (y0, y1);
        let p = self.plot;
        self.dots.draw(&mut self.grid, p.x as i64, p.y as i64, 1);
        axes(&mut self.grid, p, self.view.0, self.view.1, y0, y1, self.time, &mut self.scratch);
        self.base.copy_from(&self.grid);
    }

    #[cfg(feature = "serde")]
    pub(crate) fn apply(&mut self, o: &crate::opts::Options) {
        self.time = o.time_axis;
        self.downsample = o.downsample;
        self.fixed_y = match (o.y_min, o.y_max) {
            (Some(a), Some(b)) if b > a => Some((a, b)),
            _ => None,
        };
    }
}

#[cfg(feature = "serde")]
#[wasm_bindgen]
impl LineChart {
    /// Options as JSON (opts.rs); an error names the bad field.
    pub fn set_options(&mut self, json: &str) -> Result<(), JsError> {
        let o: crate::opts::Options = serde_json::from_str(json).map_err(|e| JsError::new(&e.to_string()))?;
        self.apply(&o);
        Ok(())
    }
}
