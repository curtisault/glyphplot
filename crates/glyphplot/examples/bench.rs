//! The native bench, `mise run bench [type …]` (REQ-BENCH-010): the load
//! average, then a row for each chart type × fixture size (1k, 100k, 1M) ×
//! grid (80 × 24, 320 × 120) with the average and worst frame of 300
//! panning frames, the budget and `OVER`, and a pointer-move row for each
//! grid at 1M; it exits 1 when a row is over budget.
//!
//! ```sh
//! cargo run -q -p glyphplot --profile contract --example bench            # every chart
//! cargo run -q -p glyphplot --profile contract --example bench -- line    # the types given
//! ```
//!
//! The plays are `tests/budgets.rs`'s (`tests/timing/mod.rs`), with its
//! retime: a row over budget is played twice more and the lowest average and
//! the lowest worst count. Budgets: `META.budget` above 100,000 points
//! (REQ-PERF-030), 1 ms average and 8 ms worst at or below (REQ-PERF-040),
//! `inputMs` and 2 ms for a pointer move (REQ-PERF-050). Built with debug
//! assertions it still runs, and says its numbers mean nothing.

#[path = "../tests/timing/mod.rs"]
mod timing;

use std::process::ExitCode;

use timing::{Row, GRIDS, POINTS};

fn main() -> ExitCode {
    let types: Vec<String> = std::env::args().skip(1).filter(|a| a != "--").collect();
    let charts = match timing::charts(&types) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("bench: {e}");
            return ExitCode::from(2);
        }
    };
    if cfg!(debug_assertions) {
        println!("bench: warning: built with debug assertions; run it with `--profile contract` for numbers the budgets mean");
    }
    println!("bench: load average {}", timing::load());
    let mut rows = Vec::new();
    for entry in charts {
        for points in POINTS {
            let Some(fixture) = timing::fixture(entry.meta, points) else {
                println!("bench: {}: no {points}-point fixture of one series in a shape it takes", entry.meta.slug);
                continue;
            };
            let data = fixture.generate();
            for grid in GRIDS {
                let limit = timing::frame_limit(entry.meta, points);
                let timing = timing::timed(limit, || timing::pan(entry, &data, grid.0, grid.1));
                let req = if points > timing::SMALL { "REQ-PERF-030" } else { "REQ-PERF-040" };
                rows.push(Row { chart: entry.meta.slug, play: "frame", fixture: fixture.name, points, grid, timing, limit, req });
            }
            if points == 1_000_000 {
                for grid in GRIDS {
                    let limit = timing::input_limit(entry.meta);
                    let timing = timing::timed(limit, || timing::pointer(entry, &data, grid.0, grid.1));
                    rows.push(Row { chart: entry.meta.slug, play: "move", fixture: fixture.name, points, grid, timing, limit, req: "REQ-PERF-050" });
                }
            }
        }
    }
    print!("{}", timing::table(&rows));
    let over = rows.iter().filter(|r| r.over()).count();
    println!("bench: load average {} after; {} of {} rows over budget", timing::load(), over, rows.len());
    if over > 0 {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
