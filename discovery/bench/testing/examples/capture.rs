//! Writes the inputs the reference script runs through d3-array and the LTTB
//! reference implementation: ref/out/ticks-in.txt and ref/out/lttb-<name>.txt.
use glyphplot_testing_bench::{Rng, fixture, random_walk};
use std::fmt::Write as _;

fn main() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ref/out");
    std::fs::create_dir_all(&dir).unwrap();
    let mut t = String::new();
    // Steps on exact decades, where floor(log10(step)) decides the power.
    for k in -15..=15 {
        for count in [1.0, 2.0, 3.0, 5.0, 7.0, 10.0, 20.0] {
            let stop = 10f64.powi(k) * count;
            writeln!(t, "{:?} {:?} {:?}", 0.0, stop, count).unwrap();
            writeln!(t, "{:?} {:?} {:?}", -stop, stop, count).unwrap();
            writeln!(t, "{:?} {:?} {:?}", stop, 0.0, count).unwrap();
        }
    }
    // Random domains over 24 decades, counts 1-20 and fractional.
    let mut r = Rng::new(11);
    for _ in 0..20_000 {
        let mag = 10f64.powf(r.f64() * 24.0 - 12.0);
        let start = (r.f64() - 0.5) * 2.0 * mag * 10f64.powf(r.f64() * 3.0);
        let stop = start + (r.f64() - 0.3) * mag;
        let count = if r.f64() < 0.8 { (1 + r.next_u64() % 20) as f64 } else { 0.25 + r.f64() * 3.0 };
        writeln!(t, "{start:?} {stop:?} {count:?}").unwrap();
    }
    std::fs::write(dir.join("ticks-in.txt"), t).unwrap();
    for (name, s) in [("sine-1k", fixture("sine-1k")), ("walk-10k", random_walk(10_000, 9)), ("walk-100k", fixture("random-walk-100k"))] {
        let mut out = String::new();
        for (x, y) in s.x.iter().zip(&s.y) {
            writeln!(out, "{x:?} {y:?}").unwrap();
        }
        std::fs::write(dir.join(format!("lttb-{name}.txt")), out).unwrap();
    }
    println!("inputs written to {}", dir.display());
}
