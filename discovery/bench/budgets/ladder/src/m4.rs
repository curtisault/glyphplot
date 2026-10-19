//! Step 6: M4 downsampling (first, last, min and max per dot column), binned
//! in one pass over the visible points, which also gives the y domain.

use crate::grid::Dots;
use crate::series::Series;

#[derive(Default)]
pub struct M4 {
    first: Vec<f64>,
    last: Vec<f64>,
    min: Vec<f64>,
    max: Vec<f64>,
    has: Vec<bool>,
}

impl M4 {
    /// Bins logical points `i0..i1` into `w` dot columns; returns the y range.
    pub fn bin(&mut self, s: &Series, i0: usize, i1: usize, w: usize, x0: f64, sx: f64) -> (f64, f64) {
        for v in [&mut self.first, &mut self.last, &mut self.min, &mut self.max] {
            v.clear();
            v.resize(w, 0.0);
        }
        self.has.clear();
        self.has.resize(w, false);
        let last_col = (w - 1) as f64;
        let (first, last, min, max, has) = (&mut self.first, &mut self.last, &mut self.min, &mut self.max, &mut self.has);
        s.each(i0, i1, |x, y| {
            if y.is_nan() {
                return;
            }
            let c = ((x - x0) * sx).floor().max(0.0).min(last_col) as usize;
            if has[c] {
                if y < min[c] {
                    min[c] = y;
                }
                if y > max[c] {
                    max[c] = y;
                }
            } else {
                has[c] = true;
                first[c] = y;
                min[c] = y;
                max[c] = y;
            }
            last[c] = y;
        });
        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        for c in 0..w {
            if self.has[c] {
                lo = lo.min(self.min[c]);
                hi = hi.max(self.max[c]);
            }
        }
        (lo, hi)
    }

    pub fn draw(&self, dots: &mut Dots, py: impl Fn(f64) -> i64) {
        for c in 0..self.has.len() {
            if !self.has[c] {
                continue;
            }
            let x = c as i64;
            dots.vline(x, py(self.min[c]), py(self.max[c]));
            if c > 0 && self.has[c - 1] {
                dots.line(x - 1, py(self.last[c - 1]), x, py(self.first[c]));
            }
        }
    }
}
