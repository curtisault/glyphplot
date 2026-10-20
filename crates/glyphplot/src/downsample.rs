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
//!
//! [`lttb`] is Largest-Triangle-Three-Buckets, offered as `sample: "lttb"`
//! and never a default ([`Sample`], REQ-SAMPLE-060), a port of the reference
//! implementation that `tests/reference.rs` compares bit for bit
//! (REQ-REF-010).

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
    m4_into(xs, ys, rows, edges, span_gaps, &mut out);
    out
}

/// As [`m4`], into `out`, whose buckets' memory is reused: a frame after a
/// data change allocates nothing new for its M4 once one was made at the
/// width (REQ-MEM-030).
pub fn m4_into(xs: &[f64], ys: &[f64], rows: Range<usize>, edges: &Edges, span_gaps: bool, out: &mut M4) {
    out.buckets.clear();
    out.gap_after = false;
    let dots = edges.dots();
    if dots == 0 {
        return;
    }
    // One bucket a dot column without gaps: allocated at its size.
    out.buckets.reserve(dots.min(rows.len()));
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

    /// The M4 of `n` series for `key`: the kept result when the key is the
    /// one it was made for, else `fill`'s, each series filled into the M4
    /// kept before, so its memory is reused (REQ-SAMPLE-020, REQ-MEM-030).
    pub fn get_into(&mut self, key: CacheKey, n: usize, mut fill: impl FnMut(usize, &mut M4)) -> &[M4] {
        if self.key != Some(key) {
            self.value.truncate(n);
            self.value.resize_with(n, M4::default);
            for (s, m) in self.value.iter_mut().enumerate() {
                fill(s, m);
            }
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

    /// Forgets the kept result's key, keeping its memory for the next
    /// [`Cache::get_into`] and the count of scans: for data that is replaced.
    pub fn invalidate(&mut self) {
        self.key = None;
    }
}

/// How a line-like chart reduces the rows in view: the `sample` option
/// (`options.schema.json`). M4 is every chart's default (REQ-SAMPLE-010);
/// LTTB is offered and is never a default (REQ-SAMPLE-060).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sample {
    /// M4 a dot column, exact to the cell.
    #[default]
    M4,
    /// Largest-Triangle-Three-Buckets ([`lttb`]): a fixed number of points,
    /// chosen for shape, not exact to the cell.
    Lttb,
    /// Every row.
    None,
}

impl Sample {
    /// The option's value by name: `m4`, `lttb` or `none`.
    pub fn from_name(name: &str) -> Option<Sample> {
        match name {
            "m4" => Some(Sample::M4),
            "lttb" => Some(Sample::Lttb),
            "none" => Some(Sample::None),
            _ => None,
        }
    }
}

/// Largest-Triangle-Three-Buckets (Steinarsson, "Downsampling Time Series
/// for Visual Representation", 2013): the indices of `threshold` points of
/// the series, the first and the last always, and from each of the
/// `threshold − 2` buckets between them the point making the largest
/// triangle with the point kept before it and the average of the next
/// bucket (REQ-SAMPLE-060).
///
/// A port of the reference implementation, flot-downsample's
/// `largestTriangleThreeBuckets` (MIT, © 2013 Sveinn Steinarsson, commit
/// af872e0; `scripts/fixtures/lttb-reference.js`), operation for operation,
/// so its choices equal the reference's bit for bit (REQ-REF-010): every
/// index when `threshold` is 0 or at least the number of points. The series
/// is `xs` and `ys` to the shorter's length, finite, x increasing.
pub fn lttb(xs: &[f64], ys: &[f64], threshold: usize) -> Vec<usize> {
    let n = xs.len().min(ys.len());
    if threshold >= n || threshold == 0 {
        return (0..n).collect();
    }
    let mut out = Vec::with_capacity(threshold);
    // Bucket size, leaving room for the first and last points.
    let every = (n - 2) as f64 / (threshold as f64 - 2.0);
    let mut a = 0usize;
    let (mut max_area_point, mut next_a) = (0usize, 0usize);
    out.push(a);
    for i in 0..threshold.saturating_sub(2) {
        // The average of the next bucket.
        let avg_start = ((i + 1) as f64 * every).floor() as usize + 1;
        let avg_end = (((i + 2) as f64 * every).floor() as usize + 1).min(n);
        let avg_len = avg_end.saturating_sub(avg_start) as f64;
        let (mut avg_x, mut avg_y) = (0.0, 0.0);
        for j in avg_start..avg_end {
            avg_x += xs[j];
            avg_y += ys[j];
        }
        avg_x /= avg_len;
        avg_y /= avg_len;
        // This bucket's range, and the point kept before it.
        let range_offs = (i as f64 * every).floor() as usize + 1;
        let range_to = ((i + 1) as f64 * every).floor() as usize + 1;
        let (pax, pay) = (xs[a], ys[a]);
        let mut max_area = -1.0;
        for r in range_offs..range_to.min(n) {
            let area = ((pax - avg_x) * (ys[r] - pay) - (pax - xs[r]) * (avg_y - pay)).abs() * 0.5;
            if area > max_area {
                max_area = area;
                max_area_point = r;
                next_a = r;
            }
        }
        out.push(max_area_point);
        a = next_a;
    }
    out.push(n - 1);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::SplitMix64;
    use proptest::prelude::*;

    proptest! {
        /// REQ-SAMPLE-060, REQ-PROP-010: LTTB keeps the first and last
        /// points, keeps `min(threshold, n)` of them (all for a threshold of
        /// 0), and keeps them in order, each once.
        #[test]
        fn lttb_keeps_ends_and_order(seed in any::<u64>(), n in 0usize..2000, threshold in 0usize..2100) {
            let mut r = SplitMix64::new(seed);
            let mut x = 0.0;
            let (mut xs, mut ys) = (Vec::with_capacity(n), Vec::with_capacity(n));
            let mut y = 0.0;
            for _ in 0..n {
                x += 0.5 + r.next_f64();
                y += r.next_f64() - 0.5;
                xs.push(x);
                ys.push(y);
            }
            let kept = lttb(&xs, &ys, threshold);
            let want = if threshold == 0 || threshold >= n { n } else { threshold.max(2) };
            prop_assert_eq!(kept.len(), want);
            if n > 0 {
                prop_assert_eq!(kept[0], 0);
                prop_assert_eq!(*kept.last().unwrap(), n - 1);
            }
            prop_assert!(kept.windows(2).all(|w| w[0] < w[1]), "{kept:?}");
        }
    }

    #[test]
    fn lttb_is_never_a_default() {
        assert_eq!(Sample::default(), Sample::M4);
        assert_eq!(Sample::from_name("lttb"), Some(Sample::Lttb));
        assert_eq!(Sample::from_name("LTTB"), None);
        // A spike survives three-point reduction where it makes the largest triangle.
        let xs: Vec<f64> = (0..9).map(f64::from).collect();
        let ys = [0.0, 0.0, 0.0, 0.0, 9.0, 0.0, 0.0, 0.0, 0.0];
        assert_eq!(lttb(&xs, &ys, 3), [0, 4, 8]);
    }

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

    /// REQ-SAMPLE-020, REQ-MEM-030: `get_into` gives `m4`'s result, refills
    /// only on a new key, and an invalidated key refills into the memory kept.
    #[test]
    fn get_into_refills_in_place() {
        let (xs, ys) = series(5, 10_000);
        let edges = Edges::new(&Axis::x(0.0, 2000.0, 80, 2));
        let mut cache = Cache::new();
        let key = CacheKey::new(1, [0.0, 2000.0], 160, false);
        let want = m4(&xs, &ys, 0..xs.len(), &edges, false);
        assert_eq!(cache.get_into(key, 1, |_, m| m4_into(&xs, &ys, 0..xs.len(), &edges, false, m)), std::slice::from_ref(&want));
        let at = cache.peek(key).unwrap()[0].buckets.as_ptr();
        cache.get_into(key, 1, |_, _| unreachable!("an unchanged key reads no data"));
        cache.invalidate();
        assert!(cache.peek(key).is_none());
        let again = cache.get_into(key, 1, |_, m| m4_into(&xs, &ys, 0..xs.len(), &edges, false, m));
        assert_eq!((again, again[0].buckets.as_ptr()), (std::slice::from_ref(&want), at), "the same result in the same memory");
        assert_eq!(cache.scans(), 2);
    }
}
