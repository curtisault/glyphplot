//! Hit tests for x-ordered charts: the datum nearest the pointer's dot column
//! by binary search on x, and the dot column's aggregate when it holds more
//! than one datum (REQ-HIT-010, REQ-HIT-020, ADR-117).
//!
//! The pointer's position becomes a dot column through `cells.rs` (the one
//! cell mapping, REQ-CANVAS-020); the column's centre value is the target,
//! and [`nearest`] finds the row whose x is nearest it in `O(log n)`, 20
//! comparisons at 10⁶ rows. Rows past a column's ends are found the same way
//! for [`column_aggregate`].

use crate::events::Aggregate;

/// The datum a position snaps to (REQ-HIT-010).
#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    /// The series' name.
    pub series: String,
    /// The series' position in the chart's series list.
    pub series_index: usize,
    /// The datum's row in the data as held now.
    pub index: usize,
    /// The datum's x in data units (epoch ms for time).
    pub x: f64,
    /// The datum's y; NaN when it is missing.
    pub y: f64,
    /// The column of the cell the datum is drawn in.
    pub col: usize,
    /// The row of the cell the datum is drawn in, or of the position when
    /// its y is off the plot or missing.
    pub row: usize,
    /// The dot column's summary of y when it holds more than one datum
    /// (REQ-HIT-020).
    pub aggregate: Option<Aggregate>,
}

/// The first row of `xs[lo..hi]` whose x is at least `v`; `hi` when none.
/// Missing x (NaN) sorts last, so it counts as past every value.
pub fn lower_bound(xs: &[f64], lo: usize, hi: usize, v: f64) -> usize {
    lo + xs[lo..hi].partition_point(|&x| x < v)
}

/// The row of `xs[lo..hi]` whose x is nearest `target`, by binary search on
/// x sorted non-decreasing with missing x last; of two equally near, the
/// earlier. `None` when the range holds no finite x or the target is NaN
/// (REQ-HIT-010).
pub fn nearest(xs: &[f64], lo: usize, hi: usize, target: f64) -> Option<usize> {
    if target.is_nan() || lo >= hi {
        return None;
    }
    let at = lower_bound(xs, lo, hi, target);
    // The candidates: the last row below the target and the first at or
    // above it, the latter moved back to the first row of its x.
    let below = (at > lo).then(|| at - 1).filter(|&i| xs[i].is_finite());
    let above = (at < hi && xs[at].is_finite()).then_some(at);
    let first_of = |i: usize| lower_bound(xs, lo, i, xs[i]);
    match (below, above) {
        (None, None) => None,
        (Some(b), None) => Some(first_of(b)),
        (None, Some(a)) => Some(a),
        (Some(b), Some(a)) => {
            let (db, da) = (target - xs[b], xs[a] - target);
            Some(if db <= da { first_of(b) } else { a })
        }
    }
}

/// The summary of the finite `ys` of the rows whose x lies in
/// `[from, to)`, found by binary search in `xs[lo..hi]`; `None` unless the
/// column holds two or more data (REQ-HIT-020).
pub fn column_aggregate(xs: &[f64], ys: &[f64], lo: usize, hi: usize, from: f64, to: f64) -> Option<Aggregate> {
    let (a, b) = (lower_bound(xs, lo, hi, from), lower_bound(xs, lo, hi, to));
    let mut agg: Option<Aggregate> = None;
    for &y in ys[a..b].iter().filter(|y| y.is_finite()) {
        match &mut agg {
            None => agg = Some(Aggregate { count: 1, first: y, last: y, min: y, max: y }),
            Some(g) => {
                g.count += 1;
                g.last = y;
                if y < g.min {
                    g.min = y;
                }
                if y > g.max {
                    g.max = y;
                }
            }
        }
    }
    agg.filter(|g| g.count >= 2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cells::Axis;
    use crate::fixtures::{generate, SplitMix64};
    use proptest::prelude::*;

    /// The brute-force answer: the first row of least distance.
    fn brute(xs: &[f64], target: f64) -> Option<usize> {
        let mut best: Option<(usize, f64)> = None;
        for (i, &x) in xs.iter().enumerate().filter(|(_, x)| x.is_finite()) {
            let d = (x - target).abs();
            if best.is_none_or(|(_, bd)| d < bd) {
                best = Some((i, d));
            }
        }
        best.map(|(i, _)| i)
    }

    /// REQ-HIT-010: for every dot column of an 80-cell plot over the sorted
    /// fixtures, the binary search snaps to the row a linear scan finds.
    #[test]
    fn snap_matches_brute_force() {
        for name in ["one", "two", "sine-1k", "duplicate-x", "steps-10k", "unsorted-x", "nan-gaps"] {
            let data = generate(name).unwrap();
            let t = data.first_table().unwrap();
            let mut xs = t.columns()[0].nums().unwrap().to_vec();
            xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let (lo, hi) = (xs[0], xs[xs.len() - 1]);
            let axis = Axis::x(lo - 1.0, hi + 1.0, 80, 2);
            for k in -2..(axis.dots() as i64 + 2) {
                let target = axis.dot_center(k);
                let got = nearest(&xs, 0, xs.len(), target);
                assert_eq!(got.map(|i| xs[i]), brute(&xs, target).map(|i| xs[i]), "{name}, dot {k}");
                assert_eq!(got, brute(&xs, target), "{name}, dot {k}: the first of equal x");
            }
        }
        assert_eq!(nearest(&[], 0, 0, 1.0), None);
        assert_eq!(nearest(&[1.0, f64::NAN], 0, 2, 9.0), Some(0), "missing x is never the answer");
    }

    /// REQ-HIT-020: a dot column that holds several data carries their count,
    /// first, last, least and greatest y; one datum carries none.
    #[test]
    fn shared_column_carries_its_aggregate() {
        let xs = [0.0, 1.0, 1.2, 1.5, 1.9, 2.0, 3.0];
        let ys = [9.0, 5.0, f64::NAN, -2.0, 7.0, 1.0, 4.0];
        let g = column_aggregate(&xs, &ys, 0, xs.len(), 1.0, 2.0).unwrap();
        assert_eq!((g.count, g.first, g.last, g.min, g.max), (3, 5.0, 7.0, -2.0, 7.0));
        assert_eq!(column_aggregate(&xs, &ys, 0, xs.len(), 3.0, 4.0), None);
    }

    proptest! {
        #[test]
        fn nearest_is_a_nearest_row(seed in any::<u64>(), n in 0usize..300, target in -10.0f64..110.0) {
            let mut r = SplitMix64::new(seed);
            let mut xs: Vec<f64> = (0..n).map(|_| (r.next_f64() * 100.0).floor()).collect();
            xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
            prop_assert_eq!(nearest(&xs, 0, n, target), brute(&xs, target));
        }
    }
}
