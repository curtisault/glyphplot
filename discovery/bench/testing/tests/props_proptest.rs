//! Property tests with proptest (`--features proptest`).
#![cfg(feature = "proptest")]
use glyphplot_testing_bench::{Series, column_of, lttb, minmax, ticks};
use proptest::prelude::*;

fn finite() -> impl Strategy<Value = f64> {
    prop_oneof![-1e12f64..1e12, -1.0f64..1.0, -1e-9f64..1e-9]
}

fn series(max: usize) -> impl Strategy<Value = Series> {
    prop::collection::vec(prop_oneof![9 => finite(), 1 => Just(f64::NAN)], 0..max)
        .prop_map(|y| Series { x: (0..y.len()).map(|i| i as f64).collect(), y })
}

/// Two min-max reductions merged: what an append does to a cached one.
fn merge(a: &[(f64, f64)], b: &[(f64, f64)]) -> Vec<(f64, f64)> {
    a.iter().zip(b).map(|(&(l1, h1), &(l2, h2))| (l1.min(l2), h1.max(h2))).collect()
}

proptest! {
    #[test]
    fn ticks_are_ascending_inside_the_domain_and_about_count(a in finite(), b in finite(), count in 1u32..50) {
        prop_assume!(a < b);
        let t = ticks(a, b, count as f64);
        prop_assert!(t.windows(2).all(|w| w[0] < w[1]), "{t:?}");
        prop_assert!(t.iter().all(|&v| v >= a && v <= b), "{t:?}");
        prop_assert!(t.len() <= 2 * count as usize + 1, "{} ticks for {count}", t.len());
    }

    #[test]
    fn every_point_lies_within_its_columns_min_and_max(s in series(2000), cols in 1usize..400) {
        let x1 = s.x.len().max(1) as f64;
        let mm = minmax(&s, 0.0, x1, cols);
        for (&x, &y) in s.x.iter().zip(&s.y) {
            if y.is_nan() { continue; }
            let (lo, hi) = mm[column_of(x, 0.0, x1, cols)];
            prop_assert!(lo <= y && y <= hi);
        }
    }

    #[test]
    fn appending_equals_replacing(s in series(2000), split in 0usize..2000, cols in 1usize..400) {
        let split = split.min(s.y.len());
        let x1 = s.x.len().max(1) as f64;
        let head = Series { x: s.x[..split].to_vec(), y: s.y[..split].to_vec() };
        let tail = Series { x: s.x[split..].to_vec(), y: s.y[split..].to_vec() };
        let whole = minmax(&s, 0.0, x1, cols);
        let parts = merge(&minmax(&head, 0.0, x1, cols), &minmax(&tail, 0.0, x1, cols));
        let bits = |v: &[(f64, f64)]| v.iter().map(|(a, b)| (a.to_bits(), b.to_bits())).collect::<Vec<_>>();
        prop_assert_eq!(bits(&whole), bits(&parts));
    }

    #[test]
    fn lttb_keeps_the_ends_and_threshold_points_in_order(n in 3usize..5000, threshold in 3usize..700, seed in any::<u64>()) {
        let s = glyphplot_testing_bench::random_walk(n, seed);
        let k = lttb(&s.x, &s.y, threshold);
        prop_assert_eq!(k.len(), threshold.min(n));
        prop_assert_eq!(k[0], 0);
        prop_assert_eq!(*k.last().unwrap(), n - 1);
        prop_assert!(k.windows(2).all(|w| w[0] < w[1]));
    }

    /// False on purpose, to see what the failure and its shrink look like:
    /// LTTB does not keep the global maximum.
    #[test]
    #[ignore]
    fn lttb_keeps_the_maximum_wrongly(y in prop::collection::vec(-100i32..100, 3..200), threshold in 3usize..50) {
        let y: Vec<f64> = y.into_iter().map(f64::from).collect();
        let x: Vec<f64> = (0..y.len()).map(|i| i as f64).collect();
        let k = lttb(&x, &y, threshold);
        let max = y.iter().cloned().fold(f64::MIN, f64::max);
        prop_assert!(k.iter().any(|&i| y[i] == max));
    }
}
