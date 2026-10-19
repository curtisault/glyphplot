//! Prints fitted x axes at several widths and times `fit_x`.
//!   cargo run --release --example fit
use scales_bench::hand;
use std::time::Instant;

fn draw(p: &[hand::Placed], len: i64) -> String {
    let mut axis: Vec<char> = vec!['─'; len as usize];
    let mut labels: Vec<char> = vec![' '; len as usize];
    for t in p {
        axis[t.cell as usize] = '┬';
        for (i, c) in t.label.chars().enumerate() {
            labels[t.left as usize + i] = c;
        }
    }
    format!("{}\n{}", axis.iter().collect::<String>(), labels.iter().collect::<String>())
}

fn main() {
    let t0 = 1_792_328_841_250.0; // 2026-10-18 13:07:21.250 UTC
    for (name, a, b, time) in [("price 98.37..131.2", 98.37, 131.2, false), ("bytes 0..3.4e9", 0.0, 3.4e9, false), ("6 hours", t0, t0 + 6.0 * 3.6e6, true), ("90 days", t0, t0 + 90.0 * 8.64e7, true)] {
        for len in [12, 24, 40, 80] {
            println!("{name}, {len} cells");
            println!("{}\n", draw(&hand::fit_x(a, b, len, 1, time), len));
        }
    }
    for len in [80i64, 320] {
        for time in [false, true] {
            let mut best = f64::MAX;
            let mut runs = vec![];
            for _ in 0..5 {
                let n = 20_000;
                let s = Instant::now();
                for i in 0..n {
                    let (a, b) = if time { (t0, t0 + 3.6e6 * (1.0 + (i % 100) as f64)) } else { (0.0, 1.0 + (i % 100) as f64 * 37.3) };
                    std::hint::black_box(hand::fit_x(a, b, len, 1, time));
                }
                let us = s.elapsed().as_secs_f64() * 1e6 / n as f64;
                best = best.min(us);
                runs.push(us);
            }
            runs.sort_by(|a, b| a.partial_cmp(b).unwrap());
            println!("fit_x len {len} time {time}: min {best:.2} µs median {:.2} µs (5 runs of 20000)", runs[2]);
        }
    }
}
