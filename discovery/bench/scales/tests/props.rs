//! Property checks of the hand-written ticks, with a fixed LCG (no proptest, to
//! keep the throwaway crate small). 200 000 cases each.
use scales_bench::hand::{self, Placed};

struct Lcg(u64);
impl Lcg {
    fn f(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

fn check(p: &[Placed], a: f64, b: f64, len: i64, gap: i64) {
    let (lo, hi) = (a.min(b), a.max(b));
    for w in p.windows(2) {
        assert!(w[0].value < w[1].value, "monotone {a} {b} {len}: {p:?}");
        assert!(w[0].cell <= w[1].cell);
        assert!(w[1].left >= w[0].left + w[0].label.chars().count() as i64 + gap, "overlap {a} {b} {len}: {p:?}");
    }
    for t in p {
        assert!(t.value >= lo && t.value <= hi, "outside {a} {b}: {p:?}");
        assert!(t.left >= 0 && t.left + t.label.chars().count() as i64 <= len, "off the axis {len}: {p:?}");
        assert!(t.cell >= 0 && t.cell < len);
    }
}

#[test]
fn linear_ticks_in_domain_monotone_nice_and_labels_apart() {
    let mut r = Lcg(7);
    let mut empty = 0;
    for _ in 0..200_000 {
        let mag = 10f64.powi((r.f() * 30.0) as i32 - 15);
        let a = (r.f() * 2.0 - 1.0) * mag;
        let b = a + (r.f() + 1e-6) * mag * 4.0;
        let len = 4 + (r.f() * 300.0) as i64;
        let p = hand::fit_x(a, b, len, 1, false);
        check(&p, a, b, len, 1);
        if p.is_empty() {
            empty += 1;
        }
        // Nice: the step between ticks is 1, 2 or 5 times a power of ten.
        if p.len() >= 2 {
            let step = p[1].value - p[0].value;
            let m = step / 10f64.powf(step.log10().floor());
            assert!([1.0, 2.0, 5.0, 10.0].iter().any(|k| (m - k).abs() < 1e-6), "step {step} not nice");
        }
        // Ticks themselves (unfitted) are within the domain and monotone.
        let t = hand::ticks(a, b, 10.0);
        assert!(t.windows(2).all(|w| w[0] < w[1]) && t.iter().all(|&v| v >= a && v <= b));
    }
    eprintln!("linear: {empty} of 200000 axes had no label (too narrow)");
}

#[test]
fn time_ticks_fit() {
    let mut r = Lcg(11);
    let mut empty = 0;
    for _ in 0..200_000 {
        let a = (946_684_800_000.0 + r.f() * 1.0e12).floor();
        let b = a + (10f64.powf(1.0 + r.f() * 11.0)).floor();
        let len = 4 + (r.f() * 300.0) as i64;
        let p = hand::fit_x(a, b, len, 1, true);
        check(&p, a, b, len, 1);
        if p.is_empty() {
            empty += 1;
        }
    }
    eprintln!("time: {empty} of 200000 axes had no label (too narrow)");
}

#[test]
fn nice_contains_domain() {
    let mut r = Lcg(3);
    for _ in 0..200_000 {
        let mag = 10f64.powi((r.f() * 30.0) as i32 - 15);
        let a = (r.f() * 2.0 - 1.0) * mag;
        let b = a + (r.f() + 1e-6) * mag * 4.0;
        let n = 2.0 + (r.f() * 20.0).floor();
        let (x, y) = hand::nice(a, b, n);
        assert!(x <= a && y >= b, "nice({a}, {b}, {n}) = ({x}, {y})");
    }
}
