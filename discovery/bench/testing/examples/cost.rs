//! What the contract's data side costs: generating each fixture, and reducing
//! the largest to a 320-column braille frame (640 dot columns), by min-max
//! and by LTTB. Seven runs each; min and median in ms.
use glyphplot_testing_bench::{FIXTURES, fixture, lttb, minmax, ticks};
use std::hint::black_box;
use std::time::Instant;

fn time<T>(mut f: impl FnMut() -> T) -> (f64, f64) {
    let mut ms: Vec<f64> = (0..7)
        .map(|_| {
            let at = Instant::now();
            black_box(f());
            at.elapsed().as_secs_f64() * 1e3
        })
        .collect();
    ms.sort_by(f64::total_cmp);
    (ms[0], ms[3])
}

fn main() {
    println!("{:<34} {:>9} {:>9}", "step", "min ms", "median ms");
    let row = |name: &str, (min, med): (f64, f64)| println!("{name:<34} {min:>9.3} {med:>9.3}");
    let mut total = 0.0;
    for name in FIXTURES {
        let t = time(|| fixture(name));
        total += t.1;
        row(&format!("generate {name}"), t);
    }
    row("generate every fixture (sum of medians)", (total, total));
    let big = fixture("random-walk-1m");
    row("min-max 1M -> 640 columns", time(|| minmax(&big, 0.0, 1e6, 640)));
    row("min-max 1M -> 80 columns", time(|| minmax(&big, 0.0, 1e6, 80)));
    row("LTTB 1M -> 640", time(|| lttb(&big.x, &big.y, 640)));
    let mid = fixture("random-walk-100k");
    row("LTTB 100k -> 640", time(|| lttb(&mid.x, &mid.y, 640)));
    row("10k d3 ticks calls", time(|| (0..10_000).map(|i| ticks(0.0, 1.0 + i as f64, 10.0).len()).sum::<usize>()));
    let clone = time(|| big.clone());
    row("clone 1M points (append vs replace)", clone);
}
