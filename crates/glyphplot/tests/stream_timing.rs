//! REQ-STREAM-050's native half: the median time to append one row, and a
//! batch of 1,000, to a chart whose ring is full, by the path the boundary
//! takes (`Table::staged` from a staging buffer, then `Chart::append`), with
//! no frame drawn between. The Node half, through the module, is
//! `web/test/data.test.mjs`'s.
//!
//! Each chart is made with default options (`retain.rows` 1,000,000) and
//! filled with 1,000,000 rows of its shape (x, y for `line` and
//! `sparkline`; t, open, high, low, close, volume for `candles`; one value
//! a category for `bar`), so every append overwrites the oldest rows. Then
//! 10,001 appends of one row and 1,001 of 1,000 rows are timed one by one
//! with `Instant`, and the medians printed. A test binary of its own, so no
//! other test shares the machine; it times only a build without debug
//! assertions (`cargo test -p glyphplot --profile contract --test
//! stream_timing -- --nocapture`), and holds every chart to the Node
//! budget, 1 µs a row and 50 µs a batch.

use std::time::Instant;

use glyphplot::data::{Column, ColumnType, Data, Role, Table};
use glyphplot::registry;

/// The rows a full ring holds.
const HELD: usize = 1_000_000;

/// A chart type's columns, empty: the template staged rows are read by.
fn template(slug: &str) -> Table {
    let cols = match slug {
        "candles" => vec![
            Column::timestamps_ms("t", Vec::new()).role(Role::X),
            Column::f64("open", Vec::new()).role(Role::Open),
            Column::f64("high", Vec::new()).role(Role::High),
            Column::f64("low", Vec::new()).role(Role::Low),
            Column::f64("close", Vec::new()).role(Role::Close),
            Column::numbers("volume", ColumnType::I64, Vec::new()).role(Role::Volume),
        ],
        "bar" => vec![Column::f64("value", Vec::new()).role(Role::Value)],
        _ => vec![Column::f64("x", Vec::new()).role(Role::X), Column::f64("y", Vec::new()).role(Role::Y)],
    };
    Table::from_columns(cols).expect("a template")
}

/// `n` staged rows from row `from`: x the row's number (a minute apart for
/// a time), each other column a value of it.
fn stage(staging: &mut Vec<f64>, k: usize, from: usize, n: usize, time: bool) {
    staging.clear();
    staging.resize(k * n, 0.0);
    for j in 0..k {
        for i in 0..n {
            let r = (from + i) as f64;
            staging[j * n + i] = if j == 0 && time {
                r * 60_000.0
            } else if j == 0 && k > 1 {
                r
            } else {
                100.0 + (r / 97.0).sin() + j as f64
            };
        }
    }
}

/// The median of `v`, sorted in place.
fn median(v: &mut [f64]) -> f64 {
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

/// REQ-STREAM-050 (natively).
#[test]
fn appends_are_timed_natively() {
    if cfg!(debug_assertions) {
        println!(
            "stream timing: skipped: it times only a build without debug assertions; run `cargo test -p glyphplot --profile contract --test stream_timing`"
        );
        return;
    }
    let mut over = Vec::new();
    println!("{:<10} {:>12} {:>14}", "chart", "1 row, ns", "1,000 rows, µs");
    for slug in ["line", "sparkline", "candles", "bar"] {
        let t = template(slug);
        let k = t.columns().len();
        let time = slug == "candles";
        let (_, mut chart) = registry::make(slug, "{}").expect("a chart");
        let mut staging = Vec::new();
        stage(&mut staging, k, 0, HELD, time);
        chart.set_data(Data::from_table(t.staged(&staging, HELD).expect("staged"))).expect("filled");
        let mut at = HELD;
        let mut time_of = |n: usize, runs: usize| -> f64 {
            let mut times = Vec::with_capacity(runs);
            for _ in 0..runs {
                stage(&mut staging, k, at, n, time);
                let start = Instant::now();
                let rows = t.staged(&staging, n).expect("staged");
                let a = chart.append(Data::from_table(rows)).expect("appended");
                times.push(start.elapsed().as_secs_f64());
                assert_eq!((a.rows, a.late), (HELD, 0));
                at += n;
            }
            median(&mut times)
        };
        let one = time_of(1, 10_001) * 1e9;
        let batch = time_of(1_000, 1_001) * 1e6;
        println!("{slug:<10} {one:>12.0} {batch:>14.1}");
        if one > 1_000.0 || batch > 50.0 {
            over.push(format!("{slug}: {one:.0} ns a row, {batch:.1} µs a batch of 1,000"));
        }
    }
    assert!(over.is_empty(), "over the Node budget natively: {}", over.join("; "));
}
