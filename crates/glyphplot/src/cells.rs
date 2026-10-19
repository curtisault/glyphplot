//! The cell mappings: a data value to its column and dot, and a column and dot
//! back to a value. Frames, hit tests, readouts and reducers all call these
//! (REQ-CANVAS-020).
//!
//! An [`Axis`] spreads a domain `lo..=hi` over `cells` cells of `per` dots
//! each: x rightwards over columns, y downwards over rows, so a y axis has
//! `hi` at its top. Dot `k` begins at its leading edge (left for x, top for
//! y), the value `dot_value(k)`, and a value belongs to the dot whose leading
//! edge it has reached and whose next edge it has not. Membership is decided
//! by comparing the value with those edge values, never by an independent
//! floor, so a dot's or a cell's edge value always maps back into it. Cell
//! `c`'s edge is dot `c × per`'s edge exactly, so a value's cell is always its
//! dot divided by `per`.

/// The largest dot index the mappings give, in either direction: past it a
/// value is too far off the axis for whole dots to mean anything.
const LIMIT: i64 = 1 << 52;

/// A domain spread over cells and their dots, rightwards (x) or downwards (y)
/// (REQ-CANVAS-020).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Axis {
    lo: f64,
    hi: f64,
    cells: usize,
    per: usize,
    down: bool,
}

impl Axis {
    /// An x axis: `lo..=hi` over `cols` columns of `per` dots each, `lo` at
    /// the left. Panics when `per` is 0.
    pub fn x(lo: f64, hi: f64, cols: usize, per: usize) -> Axis {
        assert!(per > 0, "a cell has at least one dot");
        Axis { lo, hi, cells: cols, per, down: false }
    }

    /// A y axis: `lo..=hi` over `rows` rows of `per` dots each, `hi` at the
    /// top, as rows run downward. Panics when `per` is 0.
    pub fn y(lo: f64, hi: f64, rows: usize, per: usize) -> Axis {
        assert!(per > 0, "a cell has at least one dot");
        Axis { lo, hi, cells: rows, per, down: true }
    }

    /// The domain's low end.
    pub fn lo(&self) -> f64 {
        self.lo
    }

    /// The domain's high end.
    pub fn hi(&self) -> f64 {
        self.hi
    }

    /// The cells along the axis.
    pub fn cells(&self) -> usize {
        self.cells
    }

    /// The dots a cell along the axis.
    pub fn per(&self) -> usize {
        self.per
    }

    /// The dots along the axis: `cells × per`.
    pub fn dots(&self) -> usize {
        self.cells.saturating_mul(self.per)
    }

    /// Whether the domain is degenerate: not `lo < hi` with both ends and the
    /// span finite. A degenerate axis maps every finite value to its middle,
    /// as d3's scales map a degenerate domain to the middle of their range.
    pub fn is_degenerate(&self) -> bool {
        let span = self.hi - self.lo;
        !(self.lo < self.hi && span.is_finite())
    }

    /// The value's continuous position in cells from the leading end (0 at
    /// `lo` for x, at `hi` for y): cell `c` covers `c..c + 1`. NaN for a NaN
    /// value; `cells / 2` on a degenerate axis.
    pub fn position(&self, v: f64) -> f64 {
        if v.is_nan() {
            return f64::NAN;
        }
        if self.is_degenerate() {
            return self.cells as f64 / 2.0;
        }
        let span = self.hi - self.lo;
        let from = if self.down { self.hi - v } else { v - self.lo };
        from / span * self.cells as f64
    }

    /// The dot the value falls in, counted from the leading end and not
    /// clamped to the axis, so a value off the axis gives a dot off it (for
    /// lines that leave the plot). `None` for a non-finite value or an axis
    /// of no cells; the middle dot on a degenerate axis. Dot indices are held
    /// to ±2^52.
    pub fn dot(&self, v: f64) -> Option<i64> {
        let n = self.dots();
        if !v.is_finite() || n == 0 {
            return None;
        }
        if self.is_degenerate() {
            return Some((n / 2) as i64);
        }
        let guess = self.position(v) * self.per as f64;
        if !guess.is_finite() {
            return Some(if guess > 0.0 { LIMIT } else { -LIMIT });
        }
        let guess = guess.floor();
        if guess >= LIMIT as f64 {
            return Some(LIMIT);
        }
        if guess <= -(LIMIT as f64) {
            return Some(-LIMIT);
        }
        // The floor is within a dot of the answer; the comparisons with the
        // edges settle it.
        let mut k = guess as i64;
        for _ in 0..4 {
            if !self.reached(v, k) {
                k -= 1;
            } else if self.reached(v, k + 1) {
                k += 1;
            } else {
                break;
            }
        }
        Some(k)
    }

    /// The cell the value falls in, `dot(v)` divided by `per`, not clamped
    /// to the axis.
    pub fn cell(&self, v: f64) -> Option<i64> {
        self.dot(v).map(|k| k.div_euclid(self.per as i64))
    }

    /// The dot of a value inside the domain `lo..=hi`, with `hi` (or `lo` on a
    /// y axis) in the last dot; `None` outside the domain or for a non-finite
    /// value. On a degenerate axis only `lo` is inside.
    pub fn dot_in(&self, v: f64) -> Option<usize> {
        let n = self.dots();
        if n == 0 || !(v >= self.lo && v <= self.hi) || (self.is_degenerate() && v != self.lo) {
            return None;
        }
        let k = self.dot(v)?;
        Some(if k < 0 { 0 } else { (k as usize).min(n - 1) })
    }

    /// The cell of a value inside the domain, `dot_in(v)` divided by `per`.
    pub fn cell_in(&self, v: f64) -> Option<usize> {
        self.dot_in(v).map(|k| k / self.per)
    }

    /// The value at dot `k`'s leading edge (left for x, top for y), which maps
    /// back into dot `k`. Every dot's is `lo` on a degenerate axis.
    pub fn dot_value(&self, k: i64) -> f64 {
        if self.is_degenerate() || self.dots() == 0 {
            return self.lo;
        }
        self.edge(k as f64, self.dots() as f64)
    }

    /// The value at cell `c`'s leading edge, equal to its first dot's.
    pub fn cell_value(&self, c: i64) -> f64 {
        self.dot_value(c.saturating_mul(self.per as i64))
    }

    /// The value at the middle of dot `k`.
    pub fn dot_center(&self, k: i64) -> f64 {
        if self.is_degenerate() || self.dots() == 0 {
            return self.lo;
        }
        self.edge(2.0 * k as f64 + 1.0, 2.0 * self.dots() as f64)
    }

    /// The value at the middle of cell `c`.
    pub fn cell_center(&self, c: i64) -> f64 {
        if self.is_degenerate() || self.cells == 0 {
            return self.lo;
        }
        self.edge(2.0 * c as f64 + 1.0, 2.0 * self.cells as f64)
    }

    /// The value `num / den` of the way along the axis from its leading end.
    /// Monotone in `num`, as each step rounds monotonically, and the same
    /// for equal fractions, as each quotient is correctly rounded.
    fn edge(&self, num: f64, den: f64) -> f64 {
        let along = (self.hi - self.lo) * (num / den);
        if self.down {
            self.hi - along
        } else {
            self.lo + along
        }
    }

    /// Whether the value has reached dot `k`'s leading edge.
    fn reached(&self, v: f64, k: i64) -> bool {
        let edge = self.edge(k as f64, self.dots() as f64);
        if self.down {
            v <= edge
        } else {
            v >= edge
        }
    }
}

/// The cell and the dot nearest a continuous position `p` in cells, where
/// cell `c` covers `c..c + 1` and has `per` dots: the cell and dot that hold
/// it, whose middles are the nearest (REQ-TICK-040). The cell is the dot
/// divided by `per`, so the two always agree. `None` for a non-finite
/// position or `per` of 0; indices are held to ±2^52.
pub fn nearest(p: f64, per: usize) -> Option<(i64, i64)> {
    if !p.is_finite() || per == 0 {
        return None;
    }
    let d = (p * per as f64).floor();
    let dot = if d >= LIMIT as f64 {
        LIMIT
    } else if d <= -(LIMIT as f64) {
        -LIMIT
    } else {
        d as i64
    };
    Some((dot.div_euclid(per as i64), dot))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// Checks that every cell's and dot's leading edge maps back into it, and
    /// that each value lies between its cell's edges.
    fn check_agreement(axis: &Axis, values: &[f64]) -> Result<(), TestCaseError> {
        let (cells, per) = (axis.cells() as i64, axis.per() as i64);
        for c in 0..cells {
            let v = axis.cell_value(c);
            prop_assert_eq!(axis.cell_in(v), Some(c as usize), "cell {}'s edge {} of {:?}", c, v, axis);
            prop_assert_eq!(v, axis.dot_value(c * per));
        }
        for k in 0..cells * per {
            prop_assert_eq!(axis.dot_in(axis.dot_value(k)), Some(k as usize), "dot {} of {:?}", k, axis);
        }
        for &v in values {
            let c = axis.cell_in(v).expect("a value of the domain has a cell");
            let k = axis.dot_in(v).expect("a value of the domain has a dot");
            prop_assert_eq!(c, k / axis.per());
            let (here, next) = (axis.cell_value(c as i64), axis.cell_value(c as i64 + 1));
            if axis.down {
                prop_assert!(v <= here && (c as i64 == cells - 1 || v > next), "{} in row {} of {:?}", v, c, axis);
            } else {
                prop_assert!(v >= here && (c as i64 == cells - 1 || v < next), "{} in column {} of {:?}", v, c, axis);
            }
        }
        Ok(())
    }

    proptest! {
        #[test]
        fn column_and_x_agree(xs in prop::collection::vec(-1.0e6f64..1.0e6, 1..=2000), cols in 40usize..=640, rows in 10usize..=200) {
            let lo = xs.iter().copied().fold(f64::INFINITY, f64::min);
            let mut hi = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            if hi == lo {
                hi = lo + 1.0;
            }
            check_agreement(&Axis::x(lo, hi, cols, 1), &xs)?;
            check_agreement(&Axis::x(lo, hi, cols, 2), &xs)?;
            for per in [2, 3, 4] {
                check_agreement(&Axis::y(lo, hi, rows, per), &xs)?;
            }
        }
    }

    #[test]
    fn edges_that_float_badly_still_agree() {
        // 0.1 to 0.3 spans 0.19999999999999998: the edges are inexact.
        let axis = Axis::x(0.1, 0.3, 7, 2);
        for c in 0..7 {
            assert_eq!(axis.cell_in(axis.cell_value(c)), Some(c as usize));
        }
        assert_eq!(axis.cell_in(0.3), Some(6), "hi is in the last column");
        assert_eq!(axis.cell_in(0.1), Some(0));
        let y = Axis::y(0.1, 0.3, 3, 4);
        assert_eq!(y.cell_in(0.3), Some(0), "hi is at the top");
        assert_eq!(y.cell_in(0.1), Some(2));
        assert_eq!(y.dot_in(0.1), Some(11));
    }

    #[test]
    fn values_off_the_axis_give_cells_off_it() {
        let axis = Axis::x(0.0, 10.0, 10, 2);
        assert_eq!(axis.cell(-0.5), Some(-1));
        assert_eq!(axis.dot(-0.5), Some(-1));
        assert_eq!(axis.cell(25.0), Some(25));
        assert_eq!(axis.cell_in(-0.5), None);
        assert_eq!(axis.cell_in(10.5), None);
        assert_eq!(axis.dot(1e300), Some(LIMIT));
        assert_eq!(axis.dot(-1e300), Some(-LIMIT));
        let y = Axis::y(0.0, 10.0, 5, 4);
        assert_eq!(y.cell(10.0), Some(0));
        assert_eq!(y.cell(11.5), Some(-1));
        assert_eq!(y.cell(12.5), Some(-2));
        assert_eq!(y.cell(-0.1), Some(5));
    }

    #[test]
    fn degenerate_domains_and_non_finite_values_do_not_panic() {
        let flat = Axis::x(3.0, 3.0, 10, 2);
        assert!(flat.is_degenerate());
        assert_eq!(flat.dot(3.0), Some(10));
        assert_eq!(flat.cell(-7.0), Some(5));
        assert_eq!(flat.cell_in(3.0), Some(5));
        assert_eq!(flat.cell_in(4.0), None);
        assert_eq!(flat.cell_value(4), 3.0);
        assert_eq!(flat.position(1.0), 5.0);
        for axis in
            [Axis::x(0.0, 1.0, 10, 2), Axis::y(f64::NAN, 1.0, 10, 4), Axis::x(-f64::MAX, f64::MAX, 10, 2), Axis::x(2.0, 1.0, 10, 2), Axis::x(0.0, 1.0, 0, 2)]
        {
            for v in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                assert_eq!(axis.dot(v), None);
                assert_eq!(axis.cell_in(v), None);
            }
            let _ = (axis.dot(0.5), axis.cell_in(0.5), axis.cell_value(3), axis.dot_center(3), axis.cell_center(3), axis.position(0.5));
        }
        assert_eq!(Axis::x(0.0, 1.0, 0, 2).dot(0.5), None);
    }

    #[test]
    fn centres_lie_inside() {
        let axis = Axis::x(-2.0, 5.0, 13, 2);
        for c in 0..13 {
            assert_eq!(axis.cell_in(axis.cell_center(c)), Some(c as usize));
        }
        for k in 0..26 {
            assert_eq!(axis.dot_in(axis.dot_center(k)), Some(k as usize));
        }
        let y = Axis::y(0.0, 8.0, 2, 4);
        assert_eq!(y.cell_center(0), 6.0);
        assert_eq!(y.dot_center(7), 0.5);
    }

    #[test]
    fn nearest_takes_the_cell_and_dot_holding_a_position() {
        assert_eq!(nearest(3.7, 2), Some((3, 7)));
        assert_eq!(nearest(3.0, 4), Some((3, 12)));
        assert_eq!(nearest(-0.1, 4), Some((-1, -1)));
        assert_eq!(nearest(f64::NAN, 2), None);
        assert_eq!(nearest(1.0, 0), None);
        assert_eq!(nearest(1e300, 2), Some((LIMIT / 2, LIMIT)));
        // A value's position and its dot agree.
        let axis = Axis::x(0.0, 100.0, 40, 2);
        assert_eq!(nearest(axis.position(37.0), 2).map(|n| n.1), axis.dot(37.0));
    }
}
