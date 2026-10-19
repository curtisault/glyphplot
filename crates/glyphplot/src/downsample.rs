//! M4: the first, last, minimum and maximum of the rows in each dot column
//! (Jugel et al., "M4: A Visualization-Oriented Time Series Data
//! Aggregation", PVLDB 7(10), 2014), the reduction line-like charts draw
//! from (REQ-SAMPLE-010, REQ-SAMPLE-020, REQ-PERF-010, ADR-108).
//!
//! A line through a column's four points, in row order, lights the dots a
//! line through every row lights: within a column every row sits on one dot
//! column, so its segments cover the dots from the least y to the greatest,
//! which the four points' segments cover too; and the first and last points
//! carry the segments to the neighbouring columns. So a frame drawn from M4
//! equals, cell for cell, the frame drawn from every segment.
//!
//! The rows in view are found by binary search on x ([`view_rows`]) and
//! scanned once; a row's column is found by walking the columns' edge values
//! ([`Edges`], from `cells.rs`, REQ-CANVAS-020), never by an independent
//! floor. A missing y ends a column's bucket and marks a gap, so the line
//! breaks there (REQ-LINE-030) unless gaps are spanned. [`M4::merge`] joins
//! the M4 of adjacent row ranges into the M4 of their union, so a ring read
//! in segments reduces as one. [`Cache`] keeps the result by data version, x
//! range and width: a frame with all three unchanged reads no data
//! (REQ-SAMPLE-020).

use std::ops::Range;

use crate::cells::Axis;
use crate::hit::lower_bound;

/// The leading edge value of each dot column of an x axis, and its high end
/// last: `dots + 1` values, non-decreasing (REQ-CANVAS-020).
#[derive(Clone, Debug, PartialEq)]
pub struct Edges(Vec<f64>);

impl Edges {
    /// The edges of `axis`'s dot columns, by `Axis::dot_value`.
    pub fn new(axis: &Axis) -> Edges {
        let dots = axis.dots();
        let mut v: Vec<f64> = (0..dots as i64).map(|k| axis.dot_value(k)).collect();
        v.push(axis.hi());
        Edges(v)
    }

    /// The dot columns.
    pub fn dots(&self) -> usize {
        self.0.len().saturating_sub(1)
    }

    /// The values, `dots + 1` of them.
    pub fn values(&self) -> &[f64] {
        &self.0
    }

    /// The dot column of `x` inside the axis: the last column whose edge it
    /// has reached, the axis's high end in the last column, as
    /// `Axis::dot_in` places it; `None` outside the axis.
    pub fn column(&self, x: f64) -> Option<usize> {
        let dots = self.dots();
        if dots == 0 || !(x >= self.0[0] && x <= self.0[dots]) {
            return None;
        }
        Some(self.0[1..dots].partition_point(|&e| e <= x))
    }
}

/// A row and its y.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pick {
    /// The row.
    pub row: usize,
    /// Its y.
    pub y: f64,
}

/// The M4 aggregate of one run of rows in one dot column.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bucket {
    /// The dot column.
    pub col: usize,
    /// Rows in it.
    pub count: u64,
    /// Its first row.
    pub first: Pick,
    /// Its last row.
    pub last: Pick,
    /// Its row of least y, the first of equals.
    pub min: Pick,
    /// Its row of greatest y, the first of equals.
    pub max: Pick,
    /// Whether a gap (a missing y) comes before it: the line breaks there.
    pub gap_before: bool,
}

impl Bucket {
    fn new(col: usize, p: Pick, gap_before: bool) -> Bucket {
        Bucket { col, count: 1, first: p, last: p, min: p, max: p, gap_before }
    }

    fn push(&mut self, p: Pick) {
        self.count += 1;
        self.last = p;
        if p.y < self.min.y {
            self.min = p;
        }
        if p.y > self.max.y {
            self.max = p;
        }
    }

    /// This bucket followed by `next`, of the same column with no gap
    /// between: the bucket of both runs.
    pub fn merge(&mut self, next: &Bucket) {
        self.count += next.count;
        self.last = next.last;
        if next.min.y < self.min.y {
            self.min = next.min;
        }
        if next.max.y > self.max.y {
            self.max = next.max;
        }
    }

    /// Its points in row order, each row once: at most four.
    pub fn points(&self) -> impl Iterator<Item = Pick> {
        let mut p = [self.first, self.min, self.max, self.last];
        p.sort_by_key(|p| p.row);
        let mut n = 0;
        for i in 0..4 {
            if n == 0 || p[n - 1].row != p[i].row {
                p[n] = p[i];
                n += 1;
            }
        }
        p.into_iter().take(n)
    }
}

/// The M4 of a range of rows: its buckets in row order, and whether a gap
/// ends it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct M4 {
    /// The buckets, one a run of rows in a dot column.
    pub buckets: Vec<Bucket>,
    /// Whether a gap follows the last bucket in the range.
    pub gap_after: bool,
}

impl M4 {
    /// The M4 of this range followed by the adjacent range `next`: equal to
    /// the M4 of both ranges scanned as one (`merged_equals_whole`).
    pub fn merge(mut self, next: M4) -> M4 {
        let mut rest = next.buckets.into_iter();
        if let Some(mut first) = rest.next() {
            first.gap_before |= self.gap_after;
            match self.buckets.last_mut() {
                Some(last) if last.col == first.col && !first.gap_before => last.merge(&first),
                _ => self.buckets.push(first),
            }
            self.gap_after = false;
        }
        self.buckets.extend(rest);
        self.gap_after |= next.gap_after;
        self
    }

    /// The points to draw, in row order, with whether a gap comes before
    /// each.
    pub fn points(&self) -> impl Iterator<Item = (Pick, bool)> + '_ {
        self.buckets.iter().flat_map(|b| b.points().enumerate().map(move |(i, p)| (p, i == 0 && b.gap_before)))
    }

    /// The rows it reduces.
    pub fn count(&self) -> u64 {
        self.buckets.iter().map(|b| b.count).sum()
    }
}

/// The rows of `xs[..n]` whose x lies in `[lo, hi]`, by binary search on x
/// sorted non-decreasing (REQ-SAMPLE-020).
pub fn view_rows(xs: &[f64], lo: f64, hi: f64) -> Range<usize> {
    let a = lower_bound(xs, 0, xs.len(), lo);
    let b = a + xs[a..].partition_point(|&x| x <= hi);
    a..b
}

/// The M4 of `rows` of the sorted `xs` and their `ys` over the dot columns
/// of `edges` (REQ-SAMPLE-010). A row whose x is missing or outside the
/// edges is skipped; a row whose y is missing or infinite ends its bucket
/// and marks a gap, unless `span_gaps`, when it is skipped (REQ-LINE-030).
pub fn m4(xs: &[f64], ys: &[f64], rows: Range<usize>, edges: &Edges, span_gaps: bool) -> M4 {
    let mut out = M4::default();
    let dots = edges.dots();
    if dots == 0 {
        return out;
    }
    let e = edges.values();
    let (lo, hi) = (e[0], e[dots]);
    let mut col = 0usize;
    let mut cur: Option<Bucket> = None;
    let mut gap = false;
    for row in rows {
        let x = xs[row];
        if !(x >= lo && x <= hi) {
            continue;
        }
        let y = ys[row];
        if !y.is_finite() {
            if !span_gaps {
                out.buckets.extend(cur.take());
                gap = true;
            }
            continue;
        }
        while col + 1 < dots && x >= e[col + 1] {
            col += 1;
        }
        let p = Pick { row, y };
        match &mut cur {
            Some(b) if b.col == col && !gap => b.push(p),
            _ => {
                out.buckets.extend(cur.take());
                cur = Some(Bucket::new(col, p, gap));
                gap = false;
            }
        }
    }
    out.buckets.extend(cur);
    out.gap_after = gap;
    out
}

/// What a [`Cache`] keeps its result by: the data version, the x range and
/// the width in dot columns, and whether gaps are spanned (REQ-SAMPLE-020).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CacheKey {
    /// The data version.
    pub version: u64,
    /// The x range's low end, as bits.
    pub lo: u64,
    /// The x range's high end, as bits.
    pub hi: u64,
    /// The dot columns.
    pub dots: usize,
    /// Whether gaps are spanned.
    pub span_gaps: bool,
}

impl CacheKey {
    /// The key of an M4 of data `version` over `range` at `dots` columns.
    pub fn new(version: u64, range: [f64; 2], dots: usize, span_gaps: bool) -> CacheKey {
        CacheKey { version, lo: range[0].to_bits(), hi: range[1].to_bits(), dots, span_gaps }
    }
}

/// The M4 of every series, kept by [`CacheKey`] (REQ-SAMPLE-020).
#[derive(Clone, Debug, Default)]
pub struct Cache {
    key: Option<CacheKey>,
    value: Vec<M4>,
    scans: u64,
}

impl Cache {
    /// An empty cache.
    pub fn new() -> Cache {
        Cache::default()
    }

    /// The M4 of every series for `key`: the kept result when the key is the
    /// one it was made for, else `compute`'s, which is kept.
    pub fn get(&mut self, key: CacheKey, compute: impl FnOnce() -> Vec<M4>) -> &[M4] {
        if self.key != Some(key) {
            self.value = compute();
            self.key = Some(key);
            self.scans += 1;
        }
        &self.value
    }

    /// The kept result for `key`, if it is the one kept.
    pub fn peek(&self, key: CacheKey) -> Option<&[M4]> {
        (self.key == Some(key)).then_some(&self.value[..])
    }

    /// How many times the data was scanned: one each `get` whose key was not
    /// the kept one.
    pub fn scans(&self) -> u64 {
        self.scans
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::SplitMix64;
    use proptest::prelude::*;

    /// Sorted x with repeats and runs of missing y.
    fn series(seed: u64, n: usize) -> (Vec<f64>, Vec<f64>) {
        let mut r = SplitMix64::new(seed);
        let mut x = 0.0;
        let mut xs = Vec::with_capacity(n);
        let mut ys = Vec::with_capacity(n);
        for _ in 0..n {
            x += (r.below(4) as f64) * 0.25;
            xs.push(x);
            ys.push(if r.below(17) == 0 { f64::NAN } else { (r.next_f64() * 100.0).round() });
        }
        (xs, ys)
    }

    proptest! {
        /// Every point of a bucket lies in the bucket's column, every finite
        /// row in view is counted once, and the extremes are the column's.
        #[test]
        fn every_point_within_its_column(seed in any::<u64>(), n in 0usize..400, cols in 1usize..40, gaps in any::<bool>()) {
            let (xs, ys) = series(seed, n);
            let hi = xs.last().copied().unwrap_or(1.0).max(1.0);
            let axis = Axis::x(0.0, hi, cols, 2);
            let edges = Edges::new(&axis);
            let out = m4(&xs, &ys, 0..n, &edges, gaps);
            let mut counted = 0;
            for b in &out.buckets {
                for p in b.points() {
                    prop_assert_eq!(edges.column(xs[p.row]), Some(b.col));
                    prop_assert!(p.y >= b.min.y && p.y <= b.max.y);
                }
                counted += b.count;
            }
            prop_assert_eq!(counted, ys.iter().filter(|y| y.is_finite()).count() as u64);
            for i in 0..n {
                if ys[i].is_finite() {
                    let col = edges.column(xs[i]).unwrap();
                    let in_col: Vec<&Bucket> = out.buckets.iter().filter(|b| b.col == col && b.first.row <= i && i <= b.last.row).collect();
                    prop_assert_eq!(in_col.len(), 1, "row {} in one bucket", i);
                    prop_assert!(in_col[0].min.y <= ys[i] && ys[i] <= in_col[0].max.y);
                }
            }
        }

        /// The M4 of two adjacent ranges, merged, is the M4 of both scanned
        /// as one.
        #[test]
        fn merged_equals_whole(seed in any::<u64>(), n in 0usize..400, cut in 0usize..400, cols in 1usize..40, gaps in any::<bool>()) {
            let (xs, ys) = series(seed, n);
            let cut = cut.min(n);
            let hi = xs.last().copied().unwrap_or(1.0).max(1.0);
            let edges = Edges::new(&Axis::x(0.0, hi, cols, 2));
            let whole = m4(&xs, &ys, 0..n, &edges, gaps);
            let merged = m4(&xs, &ys, 0..cut, &edges, gaps).merge(m4(&xs, &ys, cut..n, &edges, gaps));
            prop_assert_eq!(merged, whole);
        }
    }

    #[test]
    fn edges_place_values_as_the_cell_mapping_does() {
        let axis = Axis::x(-3.0, 7.0, 13, 2);
        let edges = Edges::new(&axis);
        let mut r = SplitMix64::new(7);
        for _ in 0..2000 {
            let v = -3.0 + r.next_f64() * 10.0;
            assert_eq!(edges.column(v), axis.dot_in(v), "{v}");
        }
        assert_eq!((edges.column(7.0), axis.dot_in(7.0)), (Some(25), Some(25)));
        assert_eq!(edges.column(7.5), None);
        assert_eq!(view_rows(&[0.0, 1.0, 1.0, 2.0, 3.0, f64::NAN], 1.0, 2.0), 1..4);
    }

    /// REQ-SAMPLE-020: a second frame with the same data version, x range and
    /// width reads no data; a change of any of the three scans again.
    #[test]
    fn unchanged_view_reads_no_data() {
        let (xs, ys) = series(3, 10_000);
        let edges = Edges::new(&Axis::x(0.0, 2000.0, 80, 2));
        let mut cache = Cache::new();
        let reads = std::cell::Cell::new(0);
        let compute = || {
            reads.set(reads.get() + 1);
            vec![m4(&xs, &ys, 0..xs.len(), &edges, false)]
        };
        let key = CacheKey::new(1, [0.0, 2000.0], 160, false);
        let first = cache.get(key, compute).to_vec();
        assert_eq!(cache.scans(), 1);
        for _ in 0..5 {
            assert_eq!(cache.get(key, || unreachable!("an unchanged key reads no data")), &first[..]);
        }
        assert_eq!((cache.scans(), reads.get()), (1, 1));
        for changed in [CacheKey::new(2, [0.0, 2000.0], 160, false), CacheKey::new(2, [0.0, 1000.0], 160, false), CacheKey::new(2, [0.0, 1000.0], 158, false)] {
            cache.get(changed, Vec::new);
        }
        assert_eq!(cache.scans(), 4);
    }
}
