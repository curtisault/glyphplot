//! Contract rule 30, the time budgets, alone in a test binary of its own
//! (REQ-CONTRACT-050, ADR-126): cargo runs test binaries one after another,
//! so no other test shares the machine with these timings, and this binary
//! holds one test, so nothing in it runs beside them either.
//!
//! For every registered chart, at 80 × 24 and 320 × 120, with default
//! options (`tests/timing/mod.rs` says how each play is timed):
//! - 300 frames of a view panning across the 1,000,000-point fixture against
//!   `META.budget`'s `frameMs` and `worstMs` (REQ-PERF-030);
//! - the same play over the 100,000-point fixture against 1 ms on average
//!   and 8 ms the worst (REQ-PERF-040);
//! - 300 pointer moves across a chart holding the 1,000,000-point fixture
//!   against `inputMs` on average and 2 ms the worst (REQ-PERF-050).
//!
//! A play over budget is played twice more and the lowest average and the
//! lowest worst count. The test prints a table and the load average before
//! and after with `--nocapture`, takes `CHART=<type>[,…]`, and reports every
//! row over budget in one run.
//!
//! It times only a build without debug assertions, which the `contract`
//! profile is; elsewhere it passes at once, saying it skipped:
//! `cargo test -p glyphplot --profile contract --test budgets -- --nocapture`.

mod timing;

use glyphplot::data::Data;
use glyphplot::fixtures::Fixture;
use glyphplot::registry::Entry;
use timing::{Row, GRIDS};

/// A row: `play` of `fixture`, generated as `data`, at `grid` against its budget.
fn row(entry: &'static Entry, (fixture, data): (&'static Fixture, &Data), grid: (usize, usize), play: &'static str, req: &'static str) -> Row {
    let limit = if play == "move" { timing::input_limit(entry.meta) } else { timing::frame_limit(entry.meta, fixture.rows) };
    let timing = timing::timed(limit, || match play {
        "move" => timing::pointer(entry, data, grid.0, grid.1),
        _ => timing::pan(entry, data, grid.0, grid.1),
    });
    Row { chart: entry.meta.slug, play, fixture: fixture.name, points: fixture.rows, grid, timing, limit, req }
}

/// REQ-CONTRACT-050, REQ-PERF-030, REQ-PERF-040, REQ-PERF-050.
#[test]
fn every_chart_keeps_its_time_budgets() {
    if cfg!(debug_assertions) {
        println!("budgets: skipped: rule 30 times only a build without debug assertions; run `cargo test -p glyphplot --profile contract --test budgets`");
        return;
    }
    let charts = timing::charts(&[]).unwrap_or_else(|e| panic!("CHART: {e}"));
    let before = timing::load();
    let started = std::time::Instant::now();
    let mut rows = Vec::new();
    let mut missing = Vec::new();
    for entry in charts {
        let (big, small) = (timing::fixture(entry.meta, 1_000_000), timing::fixture(entry.meta, 100_000));
        if big.is_none() {
            missing.push(format!("{}: no 1,000,000-point fixture of one series in a shape it takes", entry.meta.slug));
        }
        if small.is_none() {
            missing.push(format!("{}: no 100,000-point fixture of one series in a shape it takes", entry.meta.slug));
        }
        // Each fixture generated once a chart; the million-row one is 16 MB.
        let big = big.map(|f| (f, f.generate()));
        let small = small.map(|f| (f, f.generate()));
        for grid in GRIDS {
            if let Some((f, d)) = &big {
                rows.push(row(entry, (f, d), grid, "frame", "REQ-PERF-030"));
            }
            if let Some((f, d)) = &small {
                rows.push(row(entry, (f, d), grid, "frame", "REQ-PERF-040"));
            }
            if let Some((f, d)) = &big {
                rows.push(row(entry, (f, d), grid, "move", "REQ-PERF-050"));
            }
        }
    }
    println!("budgets: load average {before} before, {} after; {:.1} s", timing::load(), started.elapsed().as_secs_f64());
    print!("{}", timing::table(&rows));
    let over: Vec<String> = rows
        .iter()
        .filter(|r| r.over())
        .map(|r| {
            let what = r.timing.as_ref().map_or_else(|e| e.clone(), |t| format!("{:.3} ms average, {:.3} ms worst over {} runs", t.avg, t.worst, t.runs));
            format!("{} {} {} at {}×{}: {what}, budget {} / {} ms ({})", r.chart, r.play, r.fixture, r.grid.0, r.grid.1, r.limit.avg, r.limit.worst, r.req)
        })
        .chain(missing)
        .collect();
    assert!(over.is_empty(), "{} over budget:\n{}", over.len(), over.join("\n"));
}
