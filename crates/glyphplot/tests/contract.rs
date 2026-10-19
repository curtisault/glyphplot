//! The chart contract: every registered chart against rules 1–9 and 29 of the
//! plan's contract rules (`docs/IMPLEMENTATION_PLAN.md`, "Contract rules";
//! REQ-CONTRACT-010), with no test of the chart's own.
//!
//! - `every_chart_keeps_the_contract`: rules 1–9 over every fixture the chart
//!   accepts × every mode × plain, mono and paper × size set S × t = 0, 1,
//!   2.5, 6.
//! - `every_chart_matches_its_snapshots`: rule 29 (REQ-SNAP-010 to 030);
//!   `SNAP=write` rewrites the files instead (`mise run snap [type …]`).
//! - `every_size_draws_without_panic`: REQ-LAYOUT-040.
//! - `input_is_deterministic`: rule 17 in part (REQ-VIEW-010,
//!   REQ-INPUT-010).
//!
//! Each test spreads its work over threads, reports every failure of every
//! chart in one run, prints a per-chart table with `--nocapture`, and takes
//! `CHART=<type>[,<type>…]` (REQ-CONTRACT-020). Run it with
//! `cargo test -p glyphplot --profile contract --test contract`.

mod common;

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::Instant;

use glyphplot::chart::{CellPos, Chart, Env};
use glyphplot::data::Data;
use glyphplot::fixtures::{Fixture, SplitMix64, FIXTURES};
use glyphplot::grid::{Grid, BLANK, NO_BACK, WIDE_TAIL};
use glyphplot::input::{Input, Key, Mods, PointerType};
use glyphplot::options::Options;
use glyphplot::palette::PALETTE_LEN;
use glyphplot::registry::{self, Entry, RESERVED};
use glyphplot::view::ViewPatch;
use unicode_width::UnicodeWidthChar;

use common::{accepted, envs, guarded, mode_options, parallel, report, sizes, snap, Font, Tally, TIMES};

/// A fresh chart of `entry` for `options`, given `data`; the data's refusal
/// is not a failure here (rule 28 is v0.4.0's).
fn fresh(entry: &Entry, options: &Options, data: &Data) -> Result<Box<dyn Chart>, String> {
    guarded(|| {
        let (_, mut c) = registry::make_with(entry.meta.slug, options).map_err(|e| format!("make: {e}"))?;
        let _ = c.set_data(data.clone());
        Ok(c)
    })
    .and_then(|r| r)
}

/// A frame of `chart` at `t` in `env`, `cols × rows`.
fn draw(chart: &mut dyn Chart, t: f64, env: &Env, cols: usize, rows: usize) -> Grid {
    let mut g = Grid::new(cols, rows);
    chart.frame(t, env, &mut g);
    g
}

/// A pointer event of `kind` at a cell.
fn pointer(kind: &str, t: f64, x: f64, y: f64, buttons: u8, mods: Mods) -> Input {
    let pointer = PointerType::Mouse;
    match kind {
        "down" => Input::Down { t, x, y, pointer, buttons, mods },
        "up" => Input::Up { t, x, y, pointer, buttons, mods },
        _ => Input::Move { t, x, y, pointer, buttons, mods },
    }
}

/// A scripted input list on a `cols × rows` chart: hover, a drag, a Shift
/// brush, a ctrl+wheel zoom and its end, keys, a double tap that resets,
/// and a leave. Positions are seeded so every chart gets the same script.
fn script(cols: usize, rows: usize) -> Vec<Input> {
    let (w, h) = (cols as f64, rows as f64);
    let (cx, cy) = (w * 0.5, h * 0.5);
    let none = Mods::NONE;
    vec![
        pointer("move", 0.0, cx, cy, 0, none),
        pointer("move", 0.1, w * 0.3, h * 0.4, 0, none),
        pointer("down", 0.2, w * 0.3, h * 0.4, 1, none),
        pointer("move", 0.3, w * 0.45, h * 0.45, 1, none),
        pointer("up", 0.4, w * 0.45, h * 0.45, 0, none),
        pointer("down", 0.5, w * 0.35, h * 0.5, 1, Mods::SHIFT),
        pointer("move", 0.6, w * 0.6, h * 0.5, 1, Mods::SHIFT),
        pointer("up", 0.7, w * 0.6, h * 0.5, 0, Mods::SHIFT),
        Input::Wheel { t: 0.8, x: cx, y: cy, dx: 0.0, dy: -3.0, mods: Mods::CTRL },
        Input::Wheel { t: 0.9, x: cx, y: cy, dx: 0.0, dy: 0.0, mods: Mods::CTRL },
        Input::Key { t: 1.0, key: Key::Plus, mods: none },
        Input::Key { t: 1.1, key: Key::Right, mods: none },
        Input::Key { t: 1.2, key: Key::Minus, mods: none },
        Input::Key { t: 1.3, key: Key::Home, mods: none },
        Input::Key { t: 1.4, key: Key::End, mods: none },
        Input::Key { t: 1.5, key: Key::Escape, mods: none },
        Input::Tap { t: 1.6, x: cx, y: cy, count: 2 },
        pointer("move", 1.7, w * 0.7, h * 0.6, 0, none),
        Input::Leave { t: 1.8 },
    ]
}

/// Rules 3, 5 and 6 on one frame drawn `cols × rows`; `labels` is the label
/// text from the data and options.
fn check_frame(tally: &mut Tally, g: &Grid, cols: usize, rows: usize, labels: &BTreeSet<char>, case: &str) {
    let font = Font::get();
    // Rule 3: the grid keeps its size; its text has `rows` lines of `cols` columns.
    let text = g.text();
    let lines: Vec<&str> = text.split('\n').collect();
    let widths: Vec<usize> = lines.iter().map(|l| l.chars().map(|c| c.width().unwrap_or(0)).sum()).collect();
    let shaped = g.cols() == cols && g.rows() == rows && lines.len() == rows && widths.iter().all(|&w| w == cols);
    tally.check(3, shaped, || {
        let bad = widths.iter().position(|&w| w != cols).map_or(String::new(), |r| format!(", line {r} is {} columns", widths[r]));
        format!("{case}: the grid is {}×{} with {} lines{bad}", g.cols(), g.rows(), lines.len())
    });
    if g.cols() != cols || g.rows() != rows {
        return;
    }
    // Rule 5: glyphs in the font outside label text; wide glyphs paired.
    let mut bad: Option<String> = None;
    let mut count = 0usize;
    for r in 0..rows {
        for c in 0..cols {
            let gl = g.glyph(c, r);
            let problem = if gl == WIDE_TAIL {
                let head = (c > 0).then(|| g.glyph(c - 1, r)).and_then(char::from_u32);
                (head.and_then(|h| h.width()) != Some(2) || c == 0 || g.glyph(c - 1, r) == WIDE_TAIL).then(|| "a WIDE_TAIL after no wide glyph".to_string())
            } else {
                match char::from_u32(gl) {
                    None => Some(format!("U+{gl:04X} is no code point")),
                    Some(ch) => match ch.width() {
                        Some(2) if c + 1 >= cols || g.glyph(c + 1, r) != WIDE_TAIL => Some(format!("wide U+{gl:04X} {ch:?} without its WIDE_TAIL")),
                        Some(1 | 2) if !font.contains(gl) && !labels.contains(&ch) => Some(format!("U+{gl:04X} {ch:?} is outside the font and the label text")),
                        Some(0) | None => Some(format!("U+{gl:04X} takes no column")),
                        _ => None,
                    },
                }
            };
            if let Some(p) = problem {
                count += 1;
                bad.get_or_insert_with(|| format!("col {c} row {r}: {p}"));
            }
        }
    }
    tally.check(5, bad.is_none(), || format!("{case}: {count} cell(s), first at {}", bad.clone().unwrap_or_default()));
    // Rule 6: inks below the palette's length; backs too, or NO_BACK.
    let ink = g.inks().iter().position(|&i| usize::from(i) >= PALETTE_LEN);
    let back = g.backs().iter().position(|&b| usize::from(b) >= PALETTE_LEN && b != NO_BACK);
    tally.check(6, ink.is_none() && back.is_none(), || {
        let at = ink.or(back).unwrap_or(0);
        format!("{case}: col {} row {}: ink {} back {}", at % cols, at / cols, g.inks()[at], g.backs()[at])
    });
}

/// Rule 7: the frame inks the plot area at the default view: at least one
/// cell for `one`; otherwise 2 % of plot cells, or a cell in at least half of
/// the plot's columns, since a flat line covers the width of a large plot but
/// not 2 % of its area. The plot area is the cells whose centre `hit` maps to
/// a datum: `hit` is `None` off the plot.
fn check_first_frame(tally: &mut Tally, chart: &dyn Chart, g: &Grid, one: bool, case: &str) {
    let probe = guarded(|| {
        let (mut plot, mut inked, mut plot_cols, mut inked_cols) = (0usize, 0usize, 0usize, 0usize);
        for c in 0..g.cols() {
            let (mut in_plot, mut ink) = (false, false);
            for r in 0..g.rows() {
                if chart.hit(CellPos::center(c, r)).is_some() {
                    plot += 1;
                    in_plot = true;
                    let lit = g.glyph(c, r) != BLANK;
                    inked += usize::from(lit);
                    ink |= lit;
                }
            }
            plot_cols += usize::from(in_plot);
            inked_cols += usize::from(ink);
        }
        (plot, inked, plot_cols, inked_cols)
    });
    match probe {
        Err(p) => tally.check(4, false, || format!("{case}: hit panicked: {p}")),
        Ok((plot, inked, plot_cols, inked_cols)) => {
            let need = if one { 1 } else { (plot * 2).div_ceil(100).max(1) };
            let wide = !one && inked_cols * 2 >= plot_cols;
            tally.check(7, plot > 0 && (inked >= need || wide), || {
                format!("{case}: {inked} of {plot} plot cells inked, {need} needed; {inked_cols} of {plot_cols} plot columns")
            });
        }
    }
}

/// Rule 1 and the unknown-mode half of rule 2, for one chart.
fn check_meta(entry: &Entry, slugs: &[&str]) -> Tally {
    let mut t = Tally::default();
    let m = entry.meta;
    let kebab = !m.slug.is_empty() && m.slug.split('-').all(|w| !w.is_empty() && w.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()));
    t.check(1, kebab, || format!("slug {:?} is not kebab-case", m.slug));
    t.check(1, slugs.iter().filter(|s| **s == m.slug).count() == 1, || format!("slug {:?} is registered more than once", m.slug));
    t.check(1, !RESERVED.contains(&m.slug), || format!("slug {:?} is reserved", m.slug));
    t.check(1, !m.name.trim().is_empty(), || "no name".into());
    let note = m.note.chars().count();
    t.check(1, (1..=72).contains(&note), || format!("note is {note} characters, not 1–72"));
    let modes: BTreeSet<&str> = m.modes.iter().copied().collect();
    t.check(1, !m.modes.is_empty() && modes.len() == m.modes.len() && m.modes.iter().all(|s| !s.is_empty()), || format!("modes {:?}", m.modes));
    t.check(1, !m.shapes.is_empty(), || "no shapes".into());
    t.check(1, (1..=320).contains(&m.min.cols) && (1..=120).contains(&m.min.rows), || format!("min {}×{}", m.min.cols, m.min.rows));
    let b = m.budget;
    let budget = b.gzip > 0
        && b.frame_ms.is_finite()
        && b.frame_ms > 0.0
        && b.worst_ms.is_finite()
        && b.worst_ms >= b.frame_ms
        && b.input_ms.is_finite()
        && b.input_ms > 0.0;
    t.check(1, budget, || format!("budget {b:?}"));
    t.check(1, registry::FAMILIES.iter().any(|f| f.name == m.family && f.charts.iter().any(|e| e.meta.slug == m.slug)), || {
        format!("family {:?} does not list it", m.family)
    });
    let defaults = m.options.map(serde_json::from_str::<serde_json::Value>);
    let object = match &defaults {
        None => true,
        Some(Ok(serde_json::Value::Object(o))) => o.get("mode").is_none_or(|d| d.as_str() == Some(m.modes[0])),
        Some(_) => false,
    };
    t.check(1, object, || format!("options {:?} are not a JSON object whose mode is the first listed", m.options));
    // Rule 2: an unknown mode is an error naming every valid one, from make and set_options.
    let named = |e: &str| m.modes.iter().all(|md| e.contains(md));
    let bad = r#"{"mode":"no-such-mode"}"#;
    let made = guarded(|| registry::make(m.slug, bad).err().map(|e| e.to_string()));
    t.check(2, matches!(&made, Ok(Some(e)) if named(e)), || format!("make with an unknown mode gave {made:?}"));
    let set =
        guarded(|| registry::make(m.slug, "").ok().and_then(|(_, mut c)| c.set_options(&Options::parse(bad).unwrap_or_default()).err()).map(|e| e.to_string()));
    t.check(2, matches!(&set, Ok(Some(e)) if named(e)), || format!("set_options with an unknown mode gave {set:?}"));
    t
}

/// One unit of rules 2–9: a chart, a fixture and a mode.
struct Job {
    entry: &'static Entry,
    fixture: &'static Fixture,
    mode: &'static str,
}

/// Rules 2–9 for one chart, fixture and mode.
///
/// Chart A draws every env × size × t in order, each frame checked by rules
/// 3–7. Chart B, fresh, draws the same and must match A bitwise (rule 8).
/// Chart C, fresh, takes sizes and envs in reverse and, for each, t = 6, 1,
/// 2.5, 0 with `view()`, `hit()` and the frame drawn again between; each
/// frame must match A's (rule 9). 1 × 1 is checked for no panic only. A
/// then plays the input script, and every option key read must have a
/// default (rule 2).
fn run_job(job: &Job) -> Tally {
    let start = Instant::now();
    let mut t = Tally::default();
    let (entry, f, mode) = (job.entry, job.fixture, job.mode);
    let data = f.generate();
    let json = mode_options(entry, mode);
    let labels = common::label_chars(&data, &json);
    let options = Options::parse(&json).expect("options");
    let what = |env: &str, (c, r): (usize, usize), time: f64| format!("{} · mode {mode} · {env} · {c}×{r} · t={time}", f.name);
    let envs = envs();
    let sizes = sizes(entry);
    let mut a = match fresh(entry, &options, &data) {
        Ok(c) => c,
        Err(e) => {
            t.check(4, false, || format!("{} · mode {mode}: {e}", f.name));
            return t;
        }
    };
    // Chart A: rules 3–7.
    let mut frames: Vec<Option<Grid>> = Vec::new();
    for (env_name, env) in &envs {
        for &(cols, rows) in &sizes {
            for &time in &TIMES {
                let case = what(env_name, (cols, rows), time);
                let drawn = guarded(|| draw(a.as_mut(), time, env, cols, rows));
                t.frames += 1;
                t.check(4, drawn.is_ok(), || format!("{case}: {}", drawn.as_ref().err().cloned().unwrap_or_default()));
                match drawn {
                    Ok(g) => {
                        if (cols, rows) != (1, 1) {
                            check_frame(&mut t, &g, cols, rows, &labels, &case);
                            if *env_name == "plain" && time == 0.0 && f.rows > 0 {
                                check_first_frame(&mut t, a.as_ref(), &g, f.rows == 1, &case);
                            }
                        }
                        frames.push(Some(g));
                    }
                    Err(_) => {
                        frames.push(None);
                        match fresh(entry, &options, &data) {
                            Ok(c) => a = c,
                            Err(_) => return t,
                        }
                    }
                }
            }
        }
    }
    let index = |e: usize, s: usize, ti: usize| (e * sizes.len() + s) * TIMES.len() + ti;
    // Input: no panic, and the keys it reads count for rule 2.
    let played = guarded(|| {
        let _ = draw(a.as_mut(), 0.0, &envs[0].1, 80, 24);
        for event in script(80, 24) {
            let _ = a.input(&event);
        }
        let _ = draw(a.as_mut(), 2.0, &envs[0].1, 80, 24);
        let _ = a.table(50);
    });
    t.check(4, played.is_ok(), || format!("{} · mode {mode} · the input script: {}", f.name, played.as_ref().err().cloned().unwrap_or_default()));
    // Rule 2: every key read has a default.
    let defaults: serde_json::Value = entry.meta.options.and_then(|o| serde_json::from_str(o).ok()).unwrap_or(serde_json::Value::Null);
    let missing: Vec<String> = options.read().into_iter().filter(|k| defaults.get(k).is_none()).collect();
    t.check(2, missing.is_empty(), || format!("{} · mode {mode}: read without a default: {}", f.name, missing.join(", ")));
    // Chart B: rule 8.
    if let Ok(mut b) = fresh(entry, &options, &data) {
        for (e, (env_name, env)) in envs.iter().enumerate() {
            for (s, &(cols, rows)) in sizes.iter().enumerate() {
                for (ti, &time) in TIMES.iter().enumerate() {
                    let drawn = guarded(|| draw(b.as_mut(), time, env, cols, rows));
                    t.frames += 1;
                    let (Ok(g), Some(want)) = (drawn, &frames[index(e, s, ti)]) else { continue };
                    if (cols, rows) != (1, 1) {
                        t.check(8, &g == want, || format!("{}: {}", what(env_name, (cols, rows), time), common::first_difference(want, &g)));
                    }
                }
            }
        }
    }
    // Chart C: rule 9.
    if let Ok(mut c) = fresh(entry, &options, &data) {
        for (e, (env_name, env)) in envs.iter().enumerate().rev() {
            for (s, &(cols, rows)) in sizes.iter().enumerate().rev() {
                if (cols, rows) == (1, 1) {
                    continue;
                }
                for &time in &[6.0, 1.0, 2.5, 0.0] {
                    let ti = TIMES.iter().position(|&x| x == time).unwrap_or(0);
                    let drawn = guarded(|| {
                        let first = draw(c.as_mut(), time, env, cols, rows);
                        let _ = c.view().clone();
                        let _ = c.hit(CellPos::center(cols / 2, rows / 2));
                        let again = draw(c.as_mut(), time, env, cols, rows);
                        (first, again)
                    });
                    t.frames += 2;
                    let (Ok((first, again)), Some(want)) = (drawn, &frames[index(e, s, ti)]) else { continue };
                    let case = what(env_name, (cols, rows), time);
                    t.check(9, &first == want, || format!("{case} out of order: {}", common::first_difference(want, &first)));
                    t.check(9, &again == want, || format!("{case} drawn again after view() and hit(): {}", common::first_difference(want, &again)));
                }
            }
        }
    }
    t.time = start.elapsed();
    t
}

/// Rules 1–9 for every registered chart (REQ-CONTRACT-010,
/// REQ-CONTRACT-020, REQ-CHAR-010, REQ-CHAR-020, REQ-CHART-010,
/// REQ-CHART-030, REQ-VIEW-010).
#[test]
fn every_chart_keeps_the_contract() {
    let start = Instant::now();
    let charts = common::charts();
    let slugs: Vec<&str> = registry::metas().map(|m| m.slug).collect();
    let mut jobs = Vec::new();
    for &entry in &charts {
        for f in accepted(entry) {
            for &mode in entry.meta.modes {
                jobs.push(Job { entry, fixture: f, mode });
            }
        }
    }
    let results = parallel(&jobs, |j| j.fixture.rows * j.fixture.series, run_job);
    let mut tallies: Vec<(&'static str, Tally)> = charts.iter().map(|e| (e.meta.slug, check_meta(e, &slugs))).collect();
    for (job, r) in jobs.iter().zip(results) {
        if let Some((_, t)) = tallies.iter_mut().find(|(s, _)| *s == job.entry.meta.slug) {
            t.merge(r);
        }
    }
    report("contract, rules 1–9", &[1, 2, 3, 4, 5, 6, 7, 8, 9], &tallies, start.elapsed());
}

/// The fixture a chart's non-default modes are snapshotted on: `sine-1k`
/// when it accepts it, else its first accepted fixture of more than two rows.
fn mode_fixture(entry: &Entry) -> Option<&'static Fixture> {
    let fs = accepted(entry);
    fs.iter().find(|f| f.name == "sine-1k").or_else(|| fs.iter().find(|f| f.rows > 2)).copied()
}

/// The snapshot files a chart must have: one a fixture it accepts in its
/// default mode, and `<mode>-<fixture>.snap` for each other mode
/// (REQ-SNAP-010).
fn snapshot_jobs(entry: &'static Entry) -> Vec<Job> {
    let mut jobs: Vec<Job> = accepted(entry).into_iter().map(|f| Job { entry, fixture: f, mode: entry.meta.modes[0] }).collect();
    if let Some(f) = mode_fixture(entry) {
        jobs.extend(entry.meta.modes[1..].iter().map(|&mode| Job { entry, fixture: f, mode }));
    }
    jobs
}

/// The file of a snapshot job.
fn snapshot_name(job: &Job) -> String {
    if job.mode == job.entry.meta.modes[0] {
        format!("{}.snap", job.fixture.name)
    } else {
        format!("{}-{}.snap", job.mode, job.fixture.name)
    }
}

/// The directory of a chart's snapshots.
fn snapshot_dir(entry: &Entry) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots").join(entry.meta.slug)
}

/// A seeded cell in the middle 40 % of a `cols × rows` grid.
fn seeded_cell(r: &mut SplitMix64, cols: usize, rows: usize) -> (f64, f64) {
    let at = |r: &mut SplitMix64, n: usize| ((n as f64 * (0.3 + 0.4 * r.next_f64())).floor() + 0.5).min(n as f64 - 0.5);
    (at(r, cols), at(r, rows))
}

/// The snapshot text of a job, as `common/snap.rs` lays it out.
fn snapshot_text(job: &Job) -> Result<String, String> {
    let (entry, f, mode) = (job.entry, job.fixture, job.mode);
    let data = f.generate();
    let json = mode_options(entry, mode);
    let options = Options::parse(&json).map_err(|e| e.to_string())?;
    let [(_, plain), (_, mono), (_, paper)] = envs();
    let mut out = format!("# glyphplot snapshot {} · {} · {} · mode {mode} · options {json} · default view\n", snap::VERSION, entry.meta.slug, f.name);
    let mut c = fresh(entry, &options, &data)?;
    guarded(|| {
        snap::whole(&mut out, &draw(c.as_mut(), 0.0, &plain, 80, 24), 0.0, "plain", None);
        for time in [1.0, 2.5, 6.0] {
            snap::hash(&mut out, &draw(c.as_mut(), time, &plain, 80, 24), time, "plain", None);
        }
        snap::hash(&mut out, &draw(c.as_mut(), 0.0, &mono, 80, 24), 0.0, "mono", None);
        snap::hash(&mut out, &draw(c.as_mut(), 0.0, &paper, 80, 24), 0.0, "paper", None);
        let (mc, mr) = (usize::from(entry.meta.min.cols), usize::from(entry.meta.min.rows));
        snap::whole(&mut out, &draw(c.as_mut(), 0.0, &plain, mc, mr), 0.0, "plain", None);
        snap::hash(&mut out, &draw(c.as_mut(), 0.0, &plain, 320, 120), 0.0, "plain", None);
    })?;
    let mut z = fresh(entry, &options, &data)?;
    guarded(|| {
        let mut r = SplitMix64::new(f.seed ^ 0x7a6f_6f6d);
        let (wx, wy) = seeded_cell(&mut r, 80, 24);
        let (mx, my) = seeded_cell(&mut r, 80, 24);
        let _ = draw(z.as_mut(), 0.0, &plain, 80, 24);
        z.input(&Input::Wheel { t: 0.0, x: wx, y: wy, dx: 0.0, dy: -3.0, mods: Mods::CTRL });
        z.input(&Input::Wheel { t: 0.0, x: wx, y: wy, dx: 0.0, dy: 0.0, mods: Mods::CTRL });
        z.input(&pointer("move", 0.0, mx, my, 0, Mods::NONE));
        let what = format!("zoom: ctrl+wheel dy -3 at {wx},{wy}, wheel end, move to {mx},{my}");
        snap::whole(&mut out, &draw(z.as_mut(), 0.0, &plain, 80, 24), 0.0, "plain", Some(&what));
    })?;
    Ok(out)
}

/// Rule 29: every chart's frames match its snapshot files, and a chart
/// without them fails (REQ-SNAP-010, REQ-SNAP-020, REQ-SNAP-030). With
/// `SNAP=write` it writes them instead and removes files no fixture or mode
/// names.
#[test]
fn every_chart_matches_its_snapshots() {
    let start = Instant::now();
    let write = std::env::var("SNAP").is_ok_and(|v| v == "write");
    let charts = common::charts();
    let jobs: Vec<Job> = charts.iter().flat_map(|&e| snapshot_jobs(e)).collect();
    let results = parallel(
        &jobs,
        |j| j.fixture.rows * j.fixture.series,
        |job| {
            let began = Instant::now();
            let mut t = Tally { frames: 9, ..Tally::default() };
            let path = snapshot_dir(job.entry).join(snapshot_name(job));
            let shown = path.strip_prefix(env!("CARGO_MANIFEST_DIR")).unwrap_or(&path).display().to_string();
            match snapshot_text(job) {
                Err(e) => t.check(29, false, || format!("{shown}: {e}")),
                Ok(text) if write => {
                    let done = std::fs::create_dir_all(snapshot_dir(job.entry)).and_then(|()| std::fs::write(&path, &text));
                    t.check(29, done.is_ok(), || format!("{shown}: {done:?}"));
                }
                Ok(text) => match std::fs::read_to_string(&path) {
                    Err(_) => t.check(29, false, || format!("{shown} is missing: run `mise run snap {}`", job.entry.meta.slug)),
                    Ok(want) => t.check(29, want == text, || format!("{shown} differs at {}", snap::first_difference(&want, &text))),
                },
            }
            t.time = began.elapsed();
            t
        },
    );
    let mut tallies: Vec<(&'static str, Tally)> = charts.iter().map(|e| (e.meta.slug, Tally::default())).collect();
    for (&entry, (_, tally)) in charts.iter().zip(tallies.iter_mut()) {
        let names: BTreeSet<String> = snapshot_jobs(entry).iter().map(snapshot_name).collect();
        let dir = snapshot_dir(entry);
        let present: Vec<String> = std::fs::read_dir(&dir)
            .map(|d| d.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.ends_with(".snap")).collect())
            .unwrap_or_default();
        for stray in present.iter().filter(|n| !names.contains(*n)) {
            if write {
                let _ = std::fs::remove_file(dir.join(stray));
            } else {
                tally.check(29, false, || {
                    format!("tests/snapshots/{}/{stray} names no fixture or mode: run `mise run snap {}`", entry.meta.slug, entry.meta.slug)
                });
            }
        }
    }
    for (job, r) in jobs.iter().zip(results) {
        if let Some((_, t)) = tallies.iter_mut().find(|(s, _)| *s == job.entry.meta.slug) {
            t.merge(r);
        }
    }
    let title = if write { "snapshots written" } else { "snapshots, rule 29" };
    report(title, &[29], &tallies, start.elapsed());
}

/// The sizes `every_size_draws_without_panic` draws a fixture at: every
/// size from 1 × 1 to 320 × 120 for fixtures of at most 10,000 rows a
/// series; for the 100k and 1M fixtures, the four corners, 1 × n and n × 1
/// edges, and 256 sizes from SplitMix64 seeded by the fixture's seed.
fn layout_sizes(f: &Fixture) -> Vec<(usize, usize)> {
    if f.rows <= 10_000 {
        return (1..=120).flat_map(|r| (1..=320).map(move |c| (c, r))).collect();
    }
    let mut out = vec![(1, 1), (320, 1), (1, 120), (320, 120), (2, 1), (1, 2), (2, 2), (319, 119)];
    let mut r = SplitMix64::new(f.seed ^ 0x7369_7a65);
    while out.len() < 8 + 256 {
        let s = (1 + r.below(320) as usize, 1 + r.below(120) as usize);
        if !out.contains(&s) {
            out.push(s);
        }
    }
    out
}

/// One unit of `every_size_draws_without_panic`: a chart, a fixture and a
/// band of sizes.
type SizeJob = (&'static Entry, &'static Fixture, Vec<(usize, usize)>);

/// REQ-LAYOUT-040: every chart draws without a panic at every size from
/// 1 × 1 to 320 × 120 with every fixture it accepts, in its default mode
/// and colour at t = 0, one chart a fixture drawn size after size (the 100k
/// and 1M fixtures at a seeded sample of sizes, as `layout_sizes` says).
/// The small fixtures' sizes are split into rows of the grid, one job a
/// band of 8 rows, so the work spreads over threads.
#[test]
fn every_size_draws_without_panic() {
    let start = Instant::now();
    let charts = common::charts();
    let mut jobs: Vec<SizeJob> = Vec::new();
    for &entry in &charts {
        for f in accepted(entry) {
            let all = layout_sizes(f);
            for band in all.chunks(320 * 8) {
                jobs.push((entry, f, band.to_vec()));
            }
        }
    }
    let results = parallel(
        &jobs,
        |(_, f, s)| s.len() * (f.rows * f.series).max(1000),
        |(entry, f, band)| {
            let began = Instant::now();
            let mut t = Tally::default();
            let data = f.generate();
            let options = Options::new();
            let env = Env::plain();
            let mut c = match fresh(entry, &options, &data) {
                Ok(c) => c,
                Err(e) => {
                    t.check(4, false, || format!("{}: {e}", f.name));
                    return t;
                }
            };
            for &(cols, rows) in band {
                let drawn = guarded(|| draw(c.as_mut(), 0.0, &env, cols, rows));
                t.frames += 1;
                t.check(4, drawn.is_ok(), || format!("{} · {cols}×{rows}: {}", f.name, drawn.as_ref().err().cloned().unwrap_or_default()));
                if drawn.is_err() {
                    match fresh(entry, &options, &data) {
                        Ok(n) => c = n,
                        Err(_) => break,
                    }
                }
            }
            t.time = began.elapsed();
            t
        },
    );
    let mut tallies: Vec<(&'static str, Tally)> = charts.iter().map(|e| (e.meta.slug, Tally::default())).collect();
    for ((entry, _, _), r) in jobs.iter().zip(results) {
        if let Some((_, t)) = tallies.iter_mut().find(|(s, _)| *s == entry.meta.slug) {
            t.merge(r);
        }
    }
    report("every size, REQ-LAYOUT-040 (rule 4)", &[4], &tallies, start.elapsed());
}

/// Rule 17 in part (REQ-VIEW-010, REQ-INPUT-010): the input script played on
/// two fresh charts gives the same changes, views and frames after every
/// event, and the view it ends on, set on a third chart, draws the same
/// frame. On `sine-1k` and `multi-4`, or every accepted fixture of more than
/// two rows up to 10,000, in every mode at 80 × 24.
#[test]
fn input_is_deterministic() {
    let start = Instant::now();
    let charts = common::charts();
    let mut jobs = Vec::new();
    for &entry in &charts {
        let fs = accepted(entry);
        let picked: Vec<&'static Fixture> = {
            let named: Vec<_> = fs.iter().copied().filter(|f| f.name == "sine-1k" || f.name == "multi-4").collect();
            if named.is_empty() {
                fs.into_iter().filter(|f| f.rows > 2 && f.rows <= 10_000).collect()
            } else {
                named
            }
        };
        for f in picked {
            for &mode in entry.meta.modes {
                jobs.push(Job { entry, fixture: f, mode });
            }
        }
    }
    let results = parallel(
        &jobs,
        |j| j.fixture.rows,
        |job| {
            let began = Instant::now();
            let mut t = Tally::default();
            let (entry, f, mode) = (job.entry, job.fixture, job.mode);
            let data = f.generate();
            let options = Options::parse(&mode_options(entry, mode)).expect("options");
            let env = Env::plain();
            let case = |what: &str| format!("{} · mode {mode} · {what}", f.name);
            let played = guarded(|| -> Result<(), String> {
                let mut a = fresh(entry, &options, &data)?;
                let mut b = fresh(entry, &options, &data)?;
                let _ = draw(a.as_mut(), 0.0, &env, 80, 24);
                let _ = draw(b.as_mut(), 0.0, &env, 80, 24);
                for (i, event) in script(80, 24).iter().enumerate() {
                    let (ca, cb) = (a.input(event), b.input(event));
                    t.check(17, ca == cb, || case(&format!("event {i}: changes {ca:?} against {cb:?}")));
                    t.check(17, a.view() == b.view(), || case(&format!("event {i}: the views differ")));
                    let (ga, gb) = (draw(a.as_mut(), event.t(), &env, 80, 24), draw(b.as_mut(), event.t(), &env, 80, 24));
                    t.frames += 2;
                    t.check(17, ga == gb, || case(&format!("event {i}: {}", common::first_difference(&ga, &gb))));
                }
                let end = 2.0;
                let ga = draw(a.as_mut(), end, &env, 80, 24);
                let mut c = fresh(entry, &options, &data)?;
                c.set_view(&ViewPatch::whole(a.view()));
                let gc = draw(c.as_mut(), end, &env, 80, 24);
                t.frames += 2;
                t.check(17, ga == gc, || case(&format!("the script's view on a fresh chart: {}", common::first_difference(&ga, &gc))));
                Ok(())
            });
            match played {
                Err(p) => t.check(4, false, || case(&format!("panicked: {p}"))),
                Ok(Err(e)) => t.check(4, false, || case(&e)),
                Ok(Ok(())) => {}
            }
            t.time = began.elapsed();
            t
        },
    );
    let mut tallies: Vec<(&'static str, Tally)> = charts.iter().map(|e| (e.meta.slug, Tally::default())).collect();
    for (job, r) in jobs.iter().zip(results) {
        if let Some((_, t)) = tallies.iter_mut().find(|(s, _)| *s == job.entry.meta.slug) {
            t.merge(r);
        }
    }
    report("input, rule 17 in part", &[4, 17], &tallies, start.elapsed());
}

/// The fixtures are the plan's 23, so the contract's matrix is what the plan
/// says it is.
#[test]
fn the_matrix_is_the_plans() {
    assert_eq!(FIXTURES.len(), 23);
    assert_eq!(TIMES, [0.0, 1.0, 2.5, 6.0]);
    for e in registry::entries() {
        let s = sizes(e);
        assert!(s.contains(&(1, 1)) && s.contains(&(40, 12)) && s.contains(&(80, 24)) && s.contains(&(320, 120)));
        assert!(s.contains(&(usize::from(e.meta.min.cols), usize::from(e.meta.min.rows))));
    }
}
