//! Native frame, hover and streaming timings (release). The same scenarios
//! as js/bench-core.mjs runs in wasm.
//!
//!     cargo run --release --example native              every scenario, 5 runs
//!     cargo run --release --example native -- show      one frame as text

use ladder::line::LineChart;
use std::time::Instant;

const T0: f64 = 1_790_000_000_000.0; // 2026-09-21, ms
const FRAMES: usize = 300;

fn data(n: usize) -> (Vec<f64>, Vec<f64>) {
    let mut s: u32 = 1;
    let mut v = 0.0;
    let (mut x, mut y) = (Vec::with_capacity(n), Vec::with_capacity(n));
    for i in 0..n {
        s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        v += f64::from(s >> 8) / 16_777_216.0 - 0.5;
        x.push(T0 + i as f64 * 1000.0);
        y.push(v + 20.0 * (i as f64 / 5000.0).sin());
    }
    (x, y)
}

fn chart(cols: usize, rows: usize, n: usize, ds: bool) -> LineChart {
    let (x, y) = data(n);
    let mut c = LineChart::new(cols, rows, n);
    c.set_data(&x, &y);
    c.set_time_axis(true);
    c.set_downsample(ds);
    c.fit();
    c
}

fn stats(ms: &[f64]) -> (f64, f64) {
    (ms.iter().sum::<f64>() / ms.len() as f64, ms.iter().cloned().fold(0.0, f64::max))
}

/// 300 frames with the view panning: full data width, offset by up to 10%.
fn pan(c: &mut LineChart, n: usize) -> (f64, f64) {
    let span = (n.max(2) - 1) as f64 * 1000.0;
    c.frame();
    let mut ms = Vec::with_capacity(FRAMES);
    for k in 0..FRAMES {
        let off = 0.1 * span * (std::f64::consts::TAU * k as f64 / FRAMES as f64).sin();
        let at = Instant::now();
        c.set_view(T0 + off, T0 + off + span);
        c.frame();
        ms.push(at.elapsed().as_secs_f64() * 1000.0);
    }
    stats(&ms)
}

fn hover(c: &mut LineChart) -> (f64, f64) {
    c.frame();
    let cols = c.cols() as f64;
    let mut ms = Vec::with_capacity(FRAMES);
    for k in 0..FRAMES {
        let at = Instant::now();
        std::hint::black_box(c.hover(8.0 + (cols - 9.0) * k as f64 / FRAMES as f64, 5.0));
        ms.push(at.elapsed().as_secs_f64() * 1000.0);
    }
    stats(&ms)
}

/// Ten simulated seconds into a full ring of 1M: `batch` samples every
/// `every_ms`, a frame every 1000/60 ms. Returns busy ms a simulated second,
/// and the frames' average and worst.
fn stream(batch: usize, every_ms: usize, ds: bool) -> (f64, f64, f64) {
    let n = 1_000_000;
    let mut c = chart(320, 120, n, ds);
    let span = (n - 1) as f64 * 1000.0;
    let mut next = n;
    let (mut busy, mut frames) = (0.0, vec![]);
    let (bx, by): (Vec<f64>, Vec<f64>) = (vec![0.0; batch], vec![0.0; batch]);
    let (mut bx, mut by) = (bx, by);
    let mut next_frame = 0.0;
    for ms in 0..10_000usize {
        if ms % every_ms == 0 {
            for k in 0..batch {
                bx[k] = T0 + (next + k) as f64 * 1000.0;
                by[k] = ((next + k) as f64 / 777.0).sin() * 30.0;
            }
            let at = Instant::now();
            if batch == 1 {
                c.push(bx[0], by[0]);
            } else {
                c.push_batch(&bx, &by);
            }
            busy += at.elapsed().as_secs_f64() * 1000.0;
            next += batch;
        }
        if ms as f64 >= next_frame {
            next_frame += 1000.0 / 60.0;
            let end = T0 + (next - 1) as f64 * 1000.0;
            let at = Instant::now();
            c.set_view(end - span, end);
            c.frame();
            let t = at.elapsed().as_secs_f64() * 1000.0;
            busy += t;
            frames.push(t);
        }
    }
    let (avg, worst) = stats(&frames);
    (busy / 10.0, avg, worst)
}

fn summary(label: &str, runs: &mut [(f64, f64)]) {
    let mut avgs: Vec<f64> = runs.iter().map(|r| r.0).collect();
    let mut worsts: Vec<f64> = runs.iter().map(|r| r.1).collect();
    avgs.sort_by(f64::total_cmp);
    worsts.sort_by(f64::total_cmp);
    let mid = avgs.len() / 2;
    println!("{label:<34} avg ms min {:>8.3} med {:>8.3} | worst ms min {:>8.3} med {:>8.3}", avgs[0], avgs[mid], worsts[0], worsts[mid]);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("show") {
        for (cols, rows, ds) in [(80, 24, false), (80, 24, true)] {
            let mut c = chart(cols, rows, 100_000, ds);
            c.frame();
            println!("{}\n", c.text());
            c.hover(40.0, 5.0);
            println!("{}\n", c.text());
        }
        return;
    }
    let runs = args.first().and_then(|a| a.parse().ok()).unwrap_or(5);
    for &(cols, rows) in &[(80usize, 24usize), (320, 120)] {
        for &n in &[1_000usize, 10_000, 100_000, 1_000_000] {
            for ds in [false, true] {
                let mut c = chart(cols, rows, n, ds);
                let mut r: Vec<(f64, f64)> = (0..runs).map(|_| pan(&mut c, n)).collect();
                summary(&format!("frame {cols}x{rows} n={n} ds={ds}"), &mut r);
            }
        }
    }
    for &(cols, rows) in &[(80usize, 24usize), (320, 120)] {
        let mut c = chart(cols, rows, 1_000_000, true);
        let mut r: Vec<(f64, f64)> = (0..runs).map(|_| hover(&mut c)).collect();
        summary(&format!("hover {cols}x{rows} n=1000000"), &mut r);
    }
    for (batch, every) in [(1usize, 1usize), (1000, 100)] {
        for ds in [true, false] {
            let r: Vec<(f64, f64, f64)> = (0..runs).map(|_| stream(batch, every, ds)).collect();
            let mut busy: Vec<f64> = r.iter().map(|x| x.0).collect();
            busy.sort_by(f64::total_cmp);
            let mut fr: Vec<(f64, f64)> = r.iter().map(|x| (x.1, x.2)).collect();
            println!("stream batch={batch} every={every}ms ds={ds}: busy ms/s min {:.1} med {:.1}", busy[0], busy[busy.len() / 2]);
            summary("  its frames", &mut fr);
        }
    }
}
