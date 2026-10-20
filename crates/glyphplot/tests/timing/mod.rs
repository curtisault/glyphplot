//! The timed plays of contract rule 30, shared by `tests/budgets.rs` (the
//! gate, REQ-CONTRACT-050) and `examples/bench.rs` (`mise run bench`,
//! REQ-BENCH-010), so the two time the same thing.
//!
//! - [`pan`]: 300 frames of a view panning across a fixture: the x domain is
//!   the fitted one offset by up to 10 % of its span on a sine, so 90–100 %
//!   of the points are in view and no frame can reuse the one before
//!   (discovery/budgets.md, "Frame time"). One untimed frame at the default
//!   view first. A frame is timed with its `set_view`.
//! - [`pointer`]: 300 pointer moves left to right across the grid at
//!   mid-height over the default view, each timed as the shell plays it: the
//!   input, then the frame it asks for (the crosshair and readout over the
//!   kept base, REQ-OVERLAY-010).
//!
//! Each play makes a fresh chart with default options from the registry and
//! draws with [`Env::plain`], over the data [`source`] gives it for the
//! play's size: the fixture of one series and that many rows in a shape the
//! chart takes; else, for a chart of a shape no fixture of that size has,
//! data derived from a fixture without changing the budget it is held to:
//! - `ohlcv` (`candles`): the first rows of `ohlcv-ticks-1m`, the one OHLCV
//!   fixture of a million rows (all of it at 1,000,000, its first 100,000
//!   rows at 100,000);
//! - `values` (`bar`): the `y` of the series fixture of that many rows
//!   (`random-walk-1m`, `random-walk-100k`, `sine-1k`) as one `value`
//!   column, each row its own category, as `bar`'s own timing test built
//!   it before rule 30 timed `bar`.
//!
//! Times are wall clock (`Instant`), in ms. A play over its limit is played
//! twice more, and the lowest average and the lowest worst count
//! (REQ-CONTRACT-050, ADR-126).
// Each of the two uses a part.
#![allow(dead_code)]

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::process::Command;
use std::time::Instant;

use glyphplot::chart::{Chart, Env, Meta};
use glyphplot::data::{Column, Data, Role, Shape, Table, Values};
use glyphplot::fixtures::{Fixture, FIXTURES};
use glyphplot::grid::Grid;
use glyphplot::input::{Input, Mods, PointerType};
use glyphplot::registry::{self, Entry};
use glyphplot::view::{Patch, ViewPatch, XPatch};

/// Frames, or pointer moves, a play.
pub const FRAMES: usize = 300;

/// The grid sizes a play is timed at (REQ-PERF-030).
pub const GRIDS: [(usize, usize); 2] = [(80, 24), (320, 120)];

/// The fixture sizes the bench plays (REQ-BENCH-010).
pub const POINTS: [usize; 3] = [1_000, 100_000, 1_000_000];

/// Points at or below which the frame budget is REQ-PERF-040's.
pub const SMALL: usize = 100_000;

/// REQ-PERF-040: the average and worst frame, in ms, at up to 100,000 points.
pub const SMALL_FRAME: Limit = Limit { avg: 1.0, worst: 8.0 };

/// REQ-PERF-050: no pointer move over this, in ms.
pub const INPUT_WORST_MS: f64 = 2.0;

/// The most a play's average and its worst frame may take, in ms.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Limit {
    /// The average.
    pub avg: f64,
    /// The worst.
    pub worst: f64,
}

/// What a play took, in ms: the lowest average and the lowest worst over its
/// runs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Timing {
    /// The average frame or move.
    pub avg: f64,
    /// The slowest frame or move.
    pub worst: f64,
    /// Runs played: 1, or 3 when the first was over.
    pub runs: usize,
}

impl Timing {
    /// Whether the average or the worst passes `limit`.
    pub fn over(&self, limit: Limit) -> bool {
        self.avg > limit.avg || self.worst > limit.worst
    }
}

/// The frame budget of a chart drawing `points`: `META.budget` above
/// 100,000 points (REQ-PERF-030), REQ-PERF-040's at or below.
pub fn frame_limit(meta: &Meta, points: usize) -> Limit {
    if points > SMALL {
        Limit { avg: meta.budget.frame_ms, worst: meta.budget.worst_ms }
    } else {
        SMALL_FRAME
    }
}

/// The pointer-move budget: `META.budget.inputMs` on average, none over 2 ms
/// (REQ-PERF-050).
pub fn input_limit(meta: &Meta) -> Limit {
    Limit { avg: meta.budget.input_ms, worst: INPUT_WORST_MS }
}

/// How a play's data comes from its fixture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Derive {
    /// The fixture as it is.
    Same,
    /// Its first rows.
    First(usize),
    /// Its `y` as one `value` column.
    Values,
}

/// The data a play draws: a fixture, or data derived from one (the module
/// says how).
#[derive(Clone, Copy, Debug)]
pub struct Source {
    /// Its name in the table: the fixture's, and how it was derived.
    pub name: &'static str,
    /// Its rows.
    pub points: usize,
    fixture: &'static Fixture,
    derive: Derive,
}

impl Source {
    /// The data, generated afresh.
    pub fn generate(&self) -> Data {
        let data = self.fixture.generate();
        let Some(t) = data.first_table() else { return data };
        let columns: Vec<Column> = match self.derive {
            Derive::Same => return data,
            Derive::First(n) => t
                .columns()
                .iter()
                .map(|c| {
                    let mut k = c.clone();
                    k.values = match &c.values {
                        Values::Num(v) => Values::Num(v[..n.min(v.len())].to_vec()),
                        Values::Codes(v) => Values::Codes(v[..n.min(v.len())].to_vec()),
                        Values::Text(v) => Values::Text(v[..n.min(v.len())].to_vec()),
                    };
                    k
                })
                .collect(),
            Derive::Values => {
                let Some(y) = t.columns().iter().find(|c| c.role == Some(Role::Y)) else { return data };
                vec![Column::f64("value", y.nums().unwrap_or(&[]).to_vec()).role(Role::Value)]
            }
        };
        Table::from_columns(columns).map(Data::from_table).unwrap_or(data)
    }
}

/// The data a chart's play of `points` rows draws: the fixture of one series
/// and `points` rows in a shape the chart takes, first in the fixtures'
/// table order (`random-walk-1m`, `random-walk-100k`, `sine-1k` for a series
/// chart); else the first `points` rows of an OHLCV fixture at least that
/// long for a chart of `ohlcv`; else a series fixture of `points` rows as
/// `values` for a chart of `values`. `None` when there is none.
pub fn source(meta: &Meta, points: usize) -> Option<Source> {
    let takes = |s: Shape| meta.shapes.contains(&s);
    let one = |f: &&Fixture| f.series == 1;
    let named = |f: &'static Fixture, how: &str| -> &'static str {
        if how.is_empty() {
            f.name
        } else {
            // A few names a run; leaked so a source stays `Copy`.
            Box::leak(format!("{} {how}", f.name).into_boxed_str())
        }
    };
    if let Some(f) = FIXTURES.iter().filter(one).find(|f| f.rows == points && takes(f.shape)) {
        return Some(Source { name: f.name, points, fixture: f, derive: Derive::Same });
    }
    if takes(Shape::Ohlcv) {
        if let Some(f) = FIXTURES.iter().filter(one).find(|f| f.shape == Shape::Ohlcv && f.rows >= points) {
            let name = named(f, &format!("first {}k", points / 1000));
            return Some(Source { name, points, fixture: f, derive: Derive::First(points) });
        }
    }
    if takes(Shape::Values) {
        if let Some(f) = FIXTURES.iter().filter(one).find(|f| f.shape == Shape::Series && f.rows == points) {
            return Some(Source { name: named(f, "as values"), points, fixture: f, derive: Derive::Values });
        }
    }
    None
}

/// The registered charts, or those `CHART=<type>[,…]` or `only` names; an
/// unknown name is an error.
pub fn charts(only: &[String]) -> Result<Vec<&'static Entry>, String> {
    let env = std::env::var("CHART").unwrap_or_default();
    let mut wanted: Vec<String> = only.to_vec();
    wanted.extend(env.split(',').map(str::trim).filter(|s| !s.is_empty()).map(String::from));
    for w in &wanted {
        if registry::entries().all(|e| e.meta.slug != w) {
            return Err(format!("there is no chart named {w:?}"));
        }
    }
    Ok(registry::entries().filter(|e| wanted.is_empty() || wanted.iter().any(|w| w == e.meta.slug)).collect())
}

/// The machine's load averages over 1, 5 and 15 minutes, as `uptime` prints
/// them, or `unknown`.
pub fn load() -> String {
    let out = Command::new("uptime").output().ok().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
    match out.split_once("load average") {
        Some((_, rest)) => rest.trim_start_matches('s').trim_start_matches(':').replace(',', " ").split_whitespace().collect::<Vec<_>>().join(" "),
        None => std::fs::read_to_string("/proc/loadavg").ok().map(|l| l.split_whitespace().take(3).collect::<Vec<_>>().join(" ")).unwrap_or("unknown".into()),
    }
}

/// The average and the worst of `ms`.
fn stats(ms: &[f64]) -> (f64, f64) {
    (ms.iter().sum::<f64>() / ms.len().max(1) as f64, ms.iter().copied().fold(0.0, f64::max))
}

/// A fresh chart of `entry` with default options holding `data`, after one
/// frame at its default view into `grid`.
fn ready(entry: &Entry, data: &Data, grid: &mut Grid) -> Result<Box<dyn Chart>, String> {
    let (_, mut c) = registry::make(entry.meta.slug, "").map_err(|e| format!("make: {e}"))?;
    c.set_data(data.clone()).map_err(|e| format!("set_data: {e}"))?;
    c.frame(0.0, &Env::plain(), grid);
    Ok(c)
}

/// Plays `play` once, and twice more when it is over `limit`, keeping the
/// lowest average and the lowest worst (REQ-CONTRACT-050). A panic is an
/// error naming its message.
pub fn timed(limit: Limit, mut play: impl FnMut() -> Result<(f64, f64), String>) -> Result<Timing, String> {
    let mut once = || catch_unwind(AssertUnwindSafe(&mut play)).unwrap_or_else(|p| Err(format!("panicked: {}", panic_text(&*p))));
    let (avg, worst) = once()?;
    let mut t = Timing { avg, worst, runs: 1 };
    if t.over(limit) {
        for _ in 0..2 {
            let (avg, worst) = once()?;
            t = Timing { avg: t.avg.min(avg), worst: t.worst.min(worst), runs: t.runs + 1 };
        }
    }
    Ok(t)
}

/// A panic payload's message.
fn panic_text(p: &(dyn std::any::Any + Send)) -> String {
    p.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| p.downcast_ref::<String>().cloned()).unwrap_or_else(|| "a panic".into())
}

/// One run of [`FRAMES`] frames panning across `data` at `cols × rows`: the
/// average and the worst frame in ms.
pub fn pan(entry: &Entry, data: &Data, cols: usize, rows: usize) -> Result<(f64, f64), String> {
    let mut grid = Grid::new(cols, rows);
    let mut c = ready(entry, data, &mut grid)?;
    let [lo, hi] = c.table(0).domain.ok_or("the chart reports no x domain to pan")?;
    let span = hi - lo;
    let env = Env::plain();
    let mut ms = Vec::with_capacity(FRAMES);
    for k in 0..FRAMES {
        let off = 0.1 * span * (std::f64::consts::TAU * k as f64 / FRAMES as f64).sin();
        let patch = ViewPatch { x: Patch::Set(XPatch { domain: Patch::Set([lo + off, hi + off]), follow: None }), ..ViewPatch::default() };
        let t = k as f64 / 60.0;
        let at = Instant::now();
        c.set_view(&patch);
        c.frame(t, &env, &mut grid);
        ms.push(at.elapsed().as_secs_f64() * 1e3);
        std::hint::black_box(&grid);
    }
    Ok(stats(&ms))
}

/// One run of [`FRAMES`] pointer moves across a chart of `data` at
/// `cols × rows`, each the input and the frame it asks for: the average and
/// the worst move in ms.
pub fn pointer(entry: &Entry, data: &Data, cols: usize, rows: usize) -> Result<(f64, f64), String> {
    let mut grid = Grid::new(cols, rows);
    let mut c = ready(entry, data, &mut grid)?;
    let env = Env::plain();
    let mut ms = Vec::with_capacity(FRAMES);
    for k in 0..FRAMES {
        let t = 1.0 + k as f64 / 60.0;
        let x = 0.5 + (cols - 1) as f64 * k as f64 / (FRAMES - 1) as f64;
        let event = Input::Move { t, x, y: rows as f64 * 0.5, pointer: PointerType::Mouse, buttons: 0, mods: Mods::NONE };
        let at = Instant::now();
        let changes = c.input(&event);
        if changes.overlay || changes.base {
            c.frame(t, &env, &mut grid);
        }
        ms.push(at.elapsed().as_secs_f64() * 1e3);
        std::hint::black_box((&changes, &grid));
    }
    Ok(stats(&ms))
}

/// One row of the table the test and the bench print.
#[derive(Clone, Debug)]
pub struct Row {
    /// The chart type.
    pub chart: &'static str,
    /// `frame` or `move`.
    pub play: &'static str,
    /// The fixture's name.
    pub fixture: &'static str,
    /// Points in the fixture.
    pub points: usize,
    /// The grid, columns and rows.
    pub grid: (usize, usize),
    /// What it took, or why it could not be played.
    pub timing: Result<Timing, String>,
    /// Its budget.
    pub limit: Limit,
    /// The requirement it checks.
    pub req: &'static str,
}

impl Row {
    /// Whether the row failed: over its budget, or not played.
    pub fn over(&self) -> bool {
        self.timing.as_ref().map_or(true, |t| t.over(self.limit))
    }
}

/// The rows as a table, one line each, with a header.
pub fn table(rows: &[Row]) -> String {
    let mut out = format!(
        "{:<9} {:<6} {:<26} {:>9} {:>9} {:>9} {:>9} {:>13} {:>4}  {:<4} {}\n",
        "chart", "play", "fixture", "points", "grid", "avg ms", "worst ms", "budget ms", "runs", "", "req"
    );
    for r in rows {
        let grid = format!("{}×{}", r.grid.0, r.grid.1);
        let budget = format!("{} / {}", r.limit.avg, r.limit.worst);
        let (avg, worst, runs) = match &r.timing {
            Ok(t) => (format!("{:.4}", t.avg), format!("{:.4}", t.worst), t.runs.to_string()),
            Err(e) => (String::new(), String::new(), format!("error: {e}")),
        };
        let mark = if r.over() { "OVER" } else { "ok" };
        out += &format!(
            "{:<9} {:<6} {:<26} {:>9} {:>9} {:>9} {:>9} {:>13} {:>4}  {:<4} {}\n",
            r.chart, r.play, r.fixture, r.points, grid, avg, worst, budget, runs, mark, r.req
        );
    }
    out
}
