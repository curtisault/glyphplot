//! The same properties with quickcheck (`--features quickcheck`).
#![cfg(feature = "quickcheck")]
use glyphplot_testing_bench::{Series, column_of, lttb, minmax, ticks};
use quickcheck::{TestResult, quickcheck};

fn series(y: Vec<f64>) -> Series {
    let y: Vec<f64> = y.into_iter().map(|v| if v.is_infinite() { 0.0 } else { v }).collect();
    Series { x: (0..y.len()).map(|i| i as f64).collect(), y }
}

#[test]
fn ticks_are_ascending_inside_the_domain_and_about_count() {
    fn prop(a: f64, b: f64, count: u8) -> TestResult {
        let count = (count % 49 + 1) as f64;
        if !(a.is_finite() && b.is_finite() && a < b && (b - a).is_finite()) {
            return TestResult::discard();
        }
        let t = ticks(a, b, count);
        TestResult::from_bool(t.windows(2).all(|w| w[0] < w[1]) && t.iter().all(|&v| v >= a && v <= b) && t.len() <= 2 * count as usize + 1)
    }
    quickcheck(prop as fn(f64, f64, u8) -> TestResult);
}

#[test]
fn every_point_lies_within_its_columns_min_and_max() {
    fn prop(y: Vec<f64>, cols: u16) -> bool {
        let s = series(y);
        let cols = cols as usize % 399 + 1;
        let x1 = s.x.len().max(1) as f64;
        let mm = minmax(&s, 0.0, x1, cols);
        s.x.iter().zip(&s.y).filter(|(_, y)| !y.is_nan()).all(|(&x, &y)| {
            let (lo, hi) = mm[column_of(x, 0.0, x1, cols)];
            lo <= y && y <= hi
        })
    }
    quickcheck(prop as fn(Vec<f64>, u16) -> bool);
}

#[test]
fn lttb_keeps_the_ends_and_threshold_points_in_order() {
    fn prop(n: u16, threshold: u16, seed: u64) -> TestResult {
        let (n, threshold) = (n as usize % 5000, threshold as usize % 700);
        if n < 3 || threshold < 3 {
            return TestResult::discard();
        }
        let s = glyphplot_testing_bench::random_walk(n, seed);
        let k = lttb(&s.x, &s.y, threshold);
        TestResult::from_bool(k.len() == threshold.min(n) && k[0] == 0 && *k.last().unwrap() == n - 1 && k.windows(2).all(|w| w[0] < w[1]))
    }
    quickcheck(prop as fn(u16, u16, u64) -> TestResult);
}

/// False on purpose, as in props_proptest.rs.
#[test]
#[ignore]
fn lttb_keeps_the_maximum_wrongly() {
    fn prop(y: Vec<i8>, threshold: u8) -> TestResult {
        let threshold = threshold as usize % 47 + 3;
        if y.len() < 3 {
            return TestResult::discard();
        }
        let y: Vec<f64> = y.into_iter().map(f64::from).collect();
        let x: Vec<f64> = (0..y.len()).map(|i| i as f64).collect();
        let k = lttb(&x, &y, threshold);
        let max = y.iter().cloned().fold(f64::MIN, f64::max);
        TestResult::from_bool(k.iter().any(|&i| y[i] == max))
    }
    quickcheck(prop as fn(Vec<i8>, u8) -> TestResult);
}
