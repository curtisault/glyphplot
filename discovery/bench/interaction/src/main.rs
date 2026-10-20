//! Native harness: per-event cost of each step of a pointer move, at two chart sizes, 1M points.
//! `cargo run --release` (see README.md).

use interaction_bench::{Bench, Grid, path};
use std::hint::black_box;
use std::time::Instant;

const RUNS: usize = 5;

fn stats(mut v: Vec<f64>) -> (f64, f64) {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    (v[0], v[v.len() / 2])
}

fn row(size: &str, what: &str, per_event_us: (f64, f64)) {
    let (min, med) = per_event_us;
    println!("| {size} | {what} | {min:.3} | {med:.3} | {:.3} % |", med / 16_667.0 * 100.0);
}

fn time_kind(b: &mut Bench, kind: u32, n: u32) -> (f64, f64) {
    let mut v = Vec::new();
    for _ in 0..RUNS {
        let t = Instant::now();
        black_box(b.run_inside(kind, n));
        v.push(t.elapsed().as_secs_f64() * 1e6 / n as f64);
    }
    stats(v)
}

fn main() {
    let n: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(1_000_000);
    println!("points: {n}, runs: {RUNS}; per-event microseconds, min and median of the runs; share of a 16.667 ms frame (median)\n");
    println!("| size | step | min µs | median µs | of 16.7 ms |");
    println!("|---|---|---|---|---|");
    for (cols, rows) in [(80usize, 24usize), (320, 120)] {
        let size = format!("{cols}×{rows}");
        let t = Instant::now();
        let mut b = Bench::new(n, cols, rows);
        let setup = t.elapsed();
        row(&size, "move_only (cell → dot, view.cursor)", time_kind(&mut b, 0, 100_000));
        row(&size, "hit_only (+ binary search, 1M sorted x)", time_kind(&mut b, 1, 100_000));
        row(&size, "pointer (+ readout + crosshair over cached base)", time_kind(&mut b, 2, 20_000));
        row(&size, "pointer_full (+ full frame from data, all 1M visible)", time_kind(&mut b, 3, 200));
        b.zoom_to(0.45, 0.55);
        row(&size, "pointer_full, zoomed to 10 % (100k visible)", time_kind(&mut b, 3, 1000));
        row(&size, "pan 0.5 cell (full frame + overlay), 10 % visible", time_kind(&mut b, 5, 1000));
        b.zoom_to(0.0, 1.0);
        row(&size, "pan 0.5 cell (full frame + overlay), all visible", time_kind(&mut b, 5, 200));

        // JSON-encoded events, parsed with serde_json in Rust.
        let evs: Vec<String> = (0..20_000u32)
            .map(|k| {
                let (x, y) = path(k, 20_000, cols as f64, rows as f64);
                format!("{{\"kind\":\"move\",\"x\":{x},\"y\":{y}}}")
            })
            .collect();
        let mut v = Vec::new();
        for _ in 0..RUNS {
            let t = Instant::now();
            for e in &evs {
                black_box(b.pointer_json(e));
            }
            v.push(t.elapsed().as_secs_f64() * 1e6 / evs.len() as f64);
        }
        row(&size, "pointer_json (serde_json parse + pointer)", stats(v));

        // Scatter: grid build per view change, grid query per event, linear scan per event.
        let mut v = Vec::new();
        for _ in 0..RUNS {
            let t = Instant::now();
            black_box(b.scatter_build());
            v.push(t.elapsed().as_secs_f64() * 1e6);
        }
        row(&size, "scatter_build (spatial grid over dots, 1M points; per view change)", stats(v));
        row(&size, "scatter_query (nearest in dot space, from the grid)", time_kind(&mut b, 4, 100_000));
        let mut v = Vec::new();
        for _ in 0..RUNS {
            let t = Instant::now();
            for k in 0..20u32 {
                let (x, y) = path(k, 20, cols as f64, rows as f64);
                black_box(b.scatter_linear(x, y));
            }
            v.push(t.elapsed().as_secs_f64() * 1e6 / 20.0);
        }
        row(&size, "scatter_linear (nearest by scanning 1M points)", stats(v));

        // Cells changed per event: what the canvas painter redraws (REQ-MOUNT-060 in unicel).
        let mut prev = Grid::new(cols, rows);
        let (mut sum, mut cnt) = (0usize, 0usize);
        for k in 0..2000u32 {
            let (x, y) = path(k, 2000, cols as f64, rows as f64);
            b.pointer(x, y);
            let o = b.out();
            if k > 0 {
                sum += o.diff(&prev);
                cnt += 1;
            }
            prev.cells.copy_from_slice(&o.cells);
            prev.colors.copy_from_slice(&o.colors);
        }
        let hover_diff = sum as f64 / cnt as f64;
        let (mut sum, mut cnt) = (0usize, 0usize);
        for k in 0..50u32 {
            b.pan(if k % 2 == 0 { 3.0 } else { -2.0 });
            let o = b.out();
            if k > 0 {
                sum += o.diff(&prev);
                cnt += 1;
            }
            prev.cells.copy_from_slice(&o.cells);
            prev.colors.copy_from_slice(&o.colors);
        }
        println!(
            "\n{size}: setup {:.1} ms; cells changed per hover event {:.1} of {}; per pan event {:.1}\n",
            setup.as_secs_f64() * 1e3,
            hover_diff,
            cols * rows,
            sum as f64 / cnt as f64
        );
        if cols == 80 {
            b.pointer(40.3, 10.6);
            println!("```\n{}```\n", b.text());
        }
    }
}
