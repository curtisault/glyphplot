//! The chart contract: every registered chart against rules 1–13 and 15–29
//! of the plan's contract rules (`docs/IMPLEMENTATION_PLAN.md`, "Contract
//! rules"; REQ-CONTRACT-010, REQ-CONTRACT-030), with no test of the chart's
//! own. Rule 14 (streaming) is v0.6.0's and rule 30 (budgets) is
//! `tests/budgets.rs`'s (ADR-126).
//!
//! - `every_chart_keeps_the_contract`: rules 1–13, 21–26 and 28 over every
//!   fixture the chart accepts × every mode × plain, mono and paper × size
//!   set S × t = 0, 1, 2.5, 6.
//! - `replace_is_whole`: rule 15.
//! - `view_round_trips`: rule 16.
//! - `input_is_deterministic`: rules 17 and 18.
//! - `hits_name_what_was_drawn`: rules 19 and 20.
//! - `downsampling_is_exact`: rule 27.
//! - `every_chart_matches_its_snapshots`: rule 29 (REQ-SNAP-010 to 030);
//!   `SNAP=write` rewrites the files instead (`mise run snap [type …]`).
//! - `every_size_draws_without_panic`: REQ-LAYOUT-040.
//! - `selftest_rejects_broken_charts` (feature `contract-selftest`): the
//!   contract rejects the three broken charts (REQ-CONTRACT-060).
//! - `rules_as_worded` (ignored): rules 10, 19 and 24 as the plan first
//!   worded them, which fail on right frames; the plan took the wordings the
//!   tests above check in v0.4.0, and this one stays as the record of why.
//!
//! Each test spreads its work over threads, reports every failure of every
//! chart in one run, prints a per-chart table with `--nocapture`, and takes
//! `CHART=<type>[,<type>…]` (REQ-CONTRACT-020). Run it with
//! `cargo test -p glyphplot --profile contract --test contract`.
//!
//! Where the plan's words leave a check open, the choice is written at the
//! check: "line-like" is a chart whose `META.options` has `spanGaps` (rule
//! 23) or `sample` (rule 27); a log scale is asked for through a `scales`
//! option (rule 25).

mod common;

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::Instant;

use glyphplot::cells::{self, Axis};
use glyphplot::chart::{CellPos, Chart, Env};
use glyphplot::data::{Data, Encode, Role, Table};
use glyphplot::fixtures::{self, Fixture, SplitMix64, FIXTURES};
use glyphplot::grid::{Grid, BLANK, NO_BACK, WIDE_TAIL};
use glyphplot::input::{Input, Key, Mods, PointerType};
use glyphplot::options::Options;
use glyphplot::palette::PALETTE_LEN;
use glyphplot::registry::{self, Entry, RESERVED};
use glyphplot::view::ViewPatch;
use unicode_width::UnicodeWidthChar;

use common::probe::{self, Held, Plot};
use common::schema::Schemas;
use common::{accepted, envs, guarded, mode_options, parallel, report, sizes, snap, Font, Tally, TIMES};

/// A fresh chart of `entry` for `options`, given `data`; the data's refusal
/// is not a failure here (rule 28 checks refusals).
fn fresh(entry: &Entry, options: &Options, data: &Data) -> Result<Box<dyn Chart>, String> {
    guarded(|| {
        let (_, mut c) = registry::make_with(entry.meta.slug, options).map_err(|e| format!("make: {e}"))?;
        let _ = c.set_data(data.clone());
        Ok(c)
    })
    .and_then(|r| r)
}

/// As [`fresh`], with `patch` set on the view.
fn fresh_with(entry: &Entry, options: &Options, data: &Data, patch: Option<&ViewPatch>) -> Result<Box<dyn Chart>, String> {
    let mut c = fresh(entry, options, data)?;
    if let Some(p) = patch {
        c.set_view(p);
    }
    Ok(c)
}

/// A frame of `chart` at `t` in `env`, `cols × rows`.
fn draw(chart: &mut dyn Chart, t: f64, env: &Env, cols: usize, rows: usize) -> Grid {
    let mut g = Grid::new(cols, rows);
    chart.frame(t, env, &mut g);
    g
}

/// Whether `entry`'s `META.options` has a default for `key`.
fn has_option(entry: &Entry, key: &str) -> bool {
    entry.meta.options.and_then(|o| serde_json::from_str::<serde_json::Value>(o).ok()).is_some_and(|v| v.get(key).is_some())
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
        Input::Tap { t: 1.6, x: cx, y: cy, count: 2, pointer: PointerType::Touch },
        pointer("move", 1.7, w * 0.7, h * 0.6, 0, none),
        Input::Leave { t: 1.8 },
    ]
}

/// A touch script: a tap that selects, a long press and a drag that brush,
/// a pinch, a two-finger pan with a fling, a click on the legend's row and a
/// double tap that resets.
fn touch_script(cols: usize, rows: usize) -> Vec<Input> {
    let (w, h) = (cols as f64, rows as f64);
    let touch = |kind: &str, t: f64, x: f64, y: f64, buttons: u8| {
        let pointer = PointerType::Touch;
        let mods = Mods::NONE;
        match kind {
            "down" => Input::Down { t, x, y, pointer, buttons, mods },
            "up" => Input::Up { t, x, y, pointer, buttons, mods },
            _ => Input::Move { t, x, y, pointer, buttons, mods },
        }
    };
    vec![
        Input::Tap { t: 0.0, x: w * 0.4, y: h * 0.5, count: 1, pointer: PointerType::Touch },
        Input::Press { t: 0.2, x: w * 0.3, y: h * 0.5 },
        touch("move", 0.3, w * 0.55, h * 0.5, 1),
        touch("up", 0.4, w * 0.55, h * 0.5, 0),
        Input::Pinch { t: 0.5, x: w * 0.5, y: h * 0.5, scale: 1.6, end: false },
        Input::Pinch { t: 0.6, x: w * 0.5, y: h * 0.5, scale: 1.1, end: true },
        Input::Pan { t: 0.7, dx: -6.0, dy: 0.0, end: false, velocity: 0.0 },
        Input::Pan { t: 0.8, dx: -2.0, dy: 0.0, end: true, velocity: 9.0 },
        pointer("down", 0.9, 2.5, 0.5, 1, Mods::NONE),
        pointer("up", 0.95, 2.5, 0.5, 0, Mods::NONE),
        Input::Tap { t: 1.0, x: w * 0.5, y: h * 0.5, count: 2, pointer: PointerType::Touch },
        Input::Leave { t: 1.1 },
    ]
}

/// A keyboard script: steps, a lock, a key brush, pages, zooms, a series
/// toggled off and on, the ends and Escape.
fn key_script() -> Vec<Input> {
    let key = |t: f64, key: Key, mods: Mods| Input::Key { t, key, mods };
    let none = Mods::NONE;
    vec![
        key(0.0, Key::Right, none),
        key(0.1, Key::Right, none),
        key(0.2, Key::Enter, none),
        key(0.3, Key::Left, none),
        key(0.4, Key::Right, Mods::SHIFT),
        key(0.5, Key::Right, Mods::SHIFT),
        key(0.6, Key::Enter, Mods::SHIFT),
        key(0.7, Key::PageUp, none),
        key(0.8, Key::PageDown, none),
        key(0.9, Key::Plus, none),
        key(1.0, Key::Digit(1), none),
        key(1.1, Key::Up, none),
        key(1.2, Key::Digit(1), none),
        key(1.3, Key::Zero, none),
        key(1.4, Key::End, none),
        key(1.5, Key::Home, none),
        key(1.6, Key::Escape, none),
        key(1.7, Key::Question, none),
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
/// a datum (`hits`): `hit` is `None` off the plot.
fn check_first_frame(tally: &mut Tally, hits: &[bool], g: &Grid, one: bool, case: &str) {
    let (mut plot, mut inked, mut plot_cols, mut inked_cols) = (0usize, 0usize, 0usize, 0usize);
    for c in 0..g.cols() {
        let (mut in_plot, mut ink) = (false, false);
        for r in 0..g.rows() {
            if hits[r * g.cols() + c] {
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
    let need = if one { 1 } else { (plot * 2).div_ceil(100).max(1) };
    let wide = !one && inked_cols * 2 >= plot_cols;
    tally.check(7, plot > 0 && (inked >= need || wide), || {
        format!("{case}: {inked} of {plot} plot cells inked, {need} needed; {inked_cols} of {plot_cols} plot columns")
    });
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
    // Rule 25: values ≤ 0 on a log scale. A chart asks for a log y scale
    // through its `scales` option (options.schema.json); one without it has
    // no log scale to check.
    if !has_option(entry, "scales") {
        t.skip(25, "META.options has no `scales`, so the chart offers no log scale (log scales are REQ-SCALE-040, v0.5.0)");
    }
    t
}

/// Rule 28: data of a shape the chart does not accept is an error naming the
/// shape it expected, never a panic, and the chart keeps what it had. "Does
/// not accept" is the data contract's (`Data::shape`, REQ-DATA-110): a table
/// is of every table shape's kind, so a table lacking a channel is accepted
/// with a warning (REQ-DATA-080) and must only draw without a panic. The
/// cases are every fixture of a shape not in `META.shapes`, and data of no
/// table at all.
fn check_shapes(entry: &Entry) -> Tally {
    let mut t = Tally::default();
    let m = entry.meta;
    let mut cases: Vec<(&str, Data)> = FIXTURES.iter().filter(|f| !m.shapes.contains(&f.shape)).map(|f| (f.name, f.generate())).collect();
    cases.push(("no table", Data::new()));
    let start = fixtures::generate("sine-1k").filter(|_| m.shapes.contains(&glyphplot::data::Shape::Series));
    for (name, data) in cases {
        let refused = data.shape(m.shapes, &Encode::default()).is_err();
        let run = guarded(|| -> Result<(), String> {
            let (_, mut c) = registry::make(m.slug, "").map_err(|e| e.to_string())?;
            if let Some(d) = &start {
                let _ = c.set_data(d.clone());
            }
            let before = draw(c.as_mut(), 0.0, &Env::plain(), 80, 24);
            let given = c.set_data(data);
            let after = draw(c.as_mut(), 0.0, &Env::plain(), 80, 24);
            match (refused, given) {
                (true, Ok(_)) => Err("accepted".to_string()),
                (true, Err(e)) => {
                    let text = e.to_string();
                    let names = m.shapes.iter().all(|s| text.contains(s.name()));
                    if !names {
                        Err(format!("the error {text:?} does not name {:?}", m.shapes.iter().map(|s| s.name()).collect::<Vec<_>>()))
                    } else if before != after {
                        Err(format!("refused, but the frame changed: {}", common::first_difference(&before, &after)))
                    } else {
                        Ok(())
                    }
                }
                (false, _) => Ok(()),
            }
        });
        let what = if refused { "of a shape it does not take" } else { "a table it takes by kind, drawn" };
        match run {
            Err(p) => t.check(28, false, || format!("{name} ({what}) panicked: {p}")),
            Ok(r) => t.check(28, r.is_ok(), || format!("{name} ({what}): {}", r.err().unwrap_or_default())),
        }
    }
    t
}

/// One unit of the matrix rules: a chart, a fixture and a mode.
struct Job {
    entry: &'static Entry,
    fixture: &'static Fixture,
    mode: &'static str,
}

/// The x domain a chart draws, as its table reports it (REQ-A11Y-030).
fn domain_of(chart: &dyn Chart) -> Option<[f64; 2]> {
    chart.table(1).domain.filter(|[a, b]| a.is_finite() && b.is_finite() && a < b)
}

/// A view patch zooming the chart's x domain to the fractions `from..to` of
/// it (`to` above 1 or `from` below 0 zooms out).
fn zoom_patch(chart: &dyn Chart, from: f64, to: f64) -> Option<ViewPatch> {
    let [a, b] = domain_of(chart)?;
    let w = b - a;
    let json = serde_json::json!({"x": {"domain": [a + w * from, a + w * to]}}).to_string();
    ViewPatch::from_json(&json).ok()
}

/// Rule 10's one-ink checks on the 80 × 24 one-ink frame. With `as_worded`,
/// the plan's "ink 1–90 %" of the cells. Otherwise the wording the contract
/// keeps (see `rules_as_worded`): ink in at least 1 % of the cells, and the
/// plot not a slab of one inked glyph in every cell, since a filled area or a
/// dense envelope inks more than 90 % rightly (`constant` as an area: 91.8 %).
/// Either way no slab across the grid: no row across it, or column down it,
/// of one inked glyph. Glyphs in the font are rule 5's, which the one-ink
/// frames keep (counted in rule 10 above).
fn check_one_ink(t: &mut Tally, g: &Grid, plot: Option<Plot>, case: &str, as_worded: bool) {
    let (cols, rows) = (g.cols(), g.rows());
    let inked = g.glyphs().iter().filter(|&&c| probe::inked(c)).count();
    let share = inked as f64 / (cols * rows) as f64;
    if as_worded {
        t.check(10, (0.01..=0.90).contains(&share), || format!("{case}: ink in {:.1} % of the cells, not 1–90 %", share * 100.0));
    } else {
        t.check(10, share >= 0.01, || format!("{case}: ink in {:.1} % of the cells, under 1 %", share * 100.0));
        if let Some(p) = plot {
            let first = g.glyph(p.x, p.y);
            let slab = probe::inked(first) && (p.y..p.y + p.h).all(|r| (p.x..p.x + p.w).all(|c| g.glyph(c, r) == first));
            t.check(10, !slab, || format!("{case}: a slab: every plot cell is U+{first:04X}"));
        }
    }
    let row_slab = (0..rows).find(|&r| probe::inked(g.glyph(0, r)) && (0..cols).all(|c| g.glyph(c, r) == g.glyph(0, r)));
    let col_slab = (0..cols).find(|&c| probe::inked(g.glyph(c, 0)) && (0..rows).all(|r| g.glyph(c, r) == g.glyph(c, 0)));
    t.check(10, row_slab.is_none() && col_slab.is_none(), || format!("{case}: a slab: row {row_slab:?} or column {col_slab:?} is one glyph across the grid"));
}

/// Rule 11: in `multi-4`, each series draws a glyph or pattern the others do
/// not. So that the data cannot tell them apart, each series is given the
/// first series' values; each is then drawn alone in one ink (the others
/// hidden by the view) at 80 × 24, and no two of those frames, nor any and
/// the frame with every series hidden, may have the same glyphs.
fn check_series_apart(t: &mut Tally, entry: &Entry, options: &Options, data: &Data, mode: &str) {
    let Some(table) = data.first_table() else { return };
    let ys: Vec<usize> = table.columns().iter().enumerate().filter(|(_, c)| c.role == Some(Role::Y)).map(|(i, _)| i).collect();
    if ys.len() < 2 {
        return;
    }
    let first = table.columns()[ys[0]].values.clone();
    let columns = table
        .columns()
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let mut n = c.clone();
            if ys.contains(&i) {
                n.values = first.clone();
            }
            n
        })
        .collect();
    let Ok(same) = Table::from_columns(columns) else { return };
    let same = Data::from_table(same);
    let names: Vec<String> = ys.iter().map(|&i| table.columns()[i].name.clone()).collect();
    let mono = Env { mono: true, ..Env::plain() };
    let alone = |shown: Option<usize>| -> Result<Vec<u32>, String> {
        let hidden: Vec<&String> = names.iter().enumerate().filter(|(i, _)| Some(*i) != shown).map(|(_, n)| n).collect();
        let patch = ViewPatch::from_json(&serde_json::json!({ "hidden": hidden }).to_string()).map_err(|e| e.to_string())?;
        let mut c = fresh_with(entry, options, &same, Some(&patch))?;
        guarded(|| draw(c.as_mut(), 0.0, &mono, 80, 24).glyphs().to_vec())
    };
    let case = format!("multi-4, every series given the first's values · mode {mode} · mono · 80×24");
    let none = alone(None);
    let each: Vec<Result<Vec<u32>, String>> = (0..names.len()).map(|s| alone(Some(s))).collect();
    t.frames += 1 + names.len() as u64;
    for (s, g) in each.iter().enumerate() {
        match (g, &none) {
            (Ok(g), Ok(n)) => t.check(11, g != n, || format!("{case}: `{}` alone draws what no series draws", names[s])),
            (Err(e), _) | (_, Err(e)) => t.check(4, false, || format!("{case}: {e}")),
        }
        for (o, h) in each.iter().enumerate().skip(s + 1) {
            if let (Ok(g), Ok(h)) = (g, h) {
                t.check(11, g != h, || format!("{case}: `{}` and `{}` draw the same glyphs in one ink", names[s], names[o]));
            }
        }
    }
}

/// Rule 13 for one view: a chart drawn at A, then B, then A draws its first A
/// frame again, its B frame equals a fresh chart's (`want`, one a size), and
/// its view is unchanged, over every ordered pair of sizes.
fn check_resize(t: &mut Tally, chart: &mut dyn Chart, sizes: &[(usize, usize)], want: &[Option<Grid>], case: &str) {
    let env = Env::plain();
    let view = chart.view().clone();
    for (i, &a) in sizes.iter().enumerate() {
        for (j, &b) in sizes.iter().enumerate().filter(|&(j, _)| j != i) {
            let drawn = guarded(|| (draw(chart, 0.0, &env, a.0, a.1), draw(chart, 0.0, &env, b.0, b.1), draw(chart, 0.0, &env, a.0, a.1)));
            t.frames += 3;
            let pair = format!("{case} · {}×{} → {}×{} → {}×{}", a.0, a.1, b.0, b.1, a.0, a.1);
            let Ok((fa, fb, fa2)) = drawn else {
                t.check(4, false, || format!("{pair}: panicked"));
                return;
            };
            t.check(13, fa2 == fa, || format!("{pair}: the second A frame: {}", common::first_difference(&fa, &fa2)));
            if let Some(w) = &want[j] {
                t.check(13, &fb == w, || format!("{pair}: B against a fresh chart: {}", common::first_difference(w, &fb)));
            }
            if let Some(w) = &want[i] {
                t.check(13, &fa == w, || format!("{pair}: A against a fresh chart: {}", common::first_difference(w, &fa)));
            }
            t.check(13, chart.view() == &view, || format!("{pair}: the view changed"));
        }
    }
}

/// Rule 23 on one frame of `nan-gaps`: no dot column whose x range lies
/// inside a run of missing y is inked in the plot. Dot columns come from the
/// cell mapping over the x domain the chart reports, two a cell; a braille
/// cell's halves are read by its dots, any other glyph inks both.
fn check_gaps(t: &mut Tally, g: &Grid, plot: Plot, domain: [f64; 2], data: &Data, case: &str) {
    let Some(table) = data.first_table() else { return };
    let (Some(xi), Some(yi)) = (probe::x_column(table), table.columns().iter().position(|c| c.role == Some(Role::Y))) else { return };
    let (xc, yc) = (&table.columns()[xi], &table.columns()[yi]);
    let rows: Vec<(f64, f64)> = (0..table.len()).map(|r| (xc.get(r), yc.get(r))).filter(|(x, _)| x.is_finite()).collect();
    // The open x ranges between the last row with a y before a run of
    // missing y and the first after it.
    let mut runs: Vec<(f64, f64)> = Vec::new();
    let mut last: Option<f64> = None;
    let mut in_run = false;
    for &(x, y) in &rows {
        if y.is_nan() {
            in_run = true;
        } else {
            if let (true, Some(l)) = (in_run, last) {
                runs.push((l, x));
            }
            in_run = false;
            last = Some(x);
        }
    }
    let axis = Axis::x(domain[0], domain[1], plot.w, 2);
    let mut checked = 0;
    for k in 0..axis.dots() as i64 {
        let (lo, hi) = (axis.dot_value(k), axis.dot_value(k + 1));
        if !runs.iter().any(|&(a, b)| lo > a && hi < b) {
            continue;
        }
        checked += 1;
        let (c, right) = (plot.x + (k / 2) as usize, k % 2 == 1);
        let lit = (plot.y..plot.y + plot.h).find(|&r| {
            let (l, rt) = probe::lit_halves(g.glyph(c, r));
            if right {
                rt
            } else {
                l
            }
        });
        t.check(23, lit.is_none(), || format!("{case}: dot column {k} (x {lo}..{hi}) lies in a gap but is inked at row {lit:?}"));
    }
    t.check(23, checked > 0, || format!("{case}: no dot column lies inside a gap; the check checked nothing"));
}

/// Rule 24 on one frame: the y axis's labels are the text left of the y
/// axis's column on the plot's rows and the x axis's row; the x axis's
/// labels are the rows below the x axis, split at two or more spaces. With
/// `as_worded`, no two labels on an axis are equal (the plan's words).
/// Otherwise the wording the contract keeps (see `rules_as_worded`): no two
/// labels on a numeric axis are equal, and no two neighbours on any axis,
/// since a time axis of hourly ticks over more than a day repeats `01 AM` on
/// each day as d3's format writes it. Either way a y label leaves the axis's
/// column a box-drawing glyph (it does not run into it), and where every
/// label of an axis is a number they run in order (down for y, rightward for
/// x), which a label cut or run into another breaks.
fn check_ticks(t: &mut Tally, g: &Grid, plot: Plot, case: &str, as_worded: bool) {
    let axis_col = plot.x.checked_sub(1);
    let axis_row = plot.y + plot.h;
    let mut ys: Vec<String> = Vec::new();
    if let Some(ac) = axis_col {
        for r in plot.y..(axis_row + 1).min(g.rows()) {
            let text = probe::row_text(g, r, 0, ac);
            let label = text.trim();
            if !label.is_empty() {
                ys.push(label.to_string());
                let gl = g.glyph(ac, r);
                t.check(24, (0x2500..=0x257F).contains(&gl), || format!("{case}: y label {label:?} runs into the axis's column (U+{gl:04X})"));
            }
        }
    }
    let mut xs: Vec<String> = Vec::new();
    for r in axis_row + 1..g.rows() {
        let text = probe::row_text(g, r, 0, g.cols());
        xs.extend(text.split("  ").map(str::trim).filter(|s| !s.is_empty()).map(str::to_string));
    }
    for (axis, labels, down) in [("y", &ys, true), ("x", &xs, false)] {
        let values: Vec<f64> = labels.iter().filter_map(|l| probe::parse_number(l)).collect();
        let numeric = values.len() == labels.len();
        if as_worded || numeric {
            let mut seen = BTreeSet::new();
            for l in labels.iter() {
                t.check(24, seen.insert(l.clone()), || format!("{case}: the {axis} axis has two labels {l:?}"));
            }
        } else {
            for w in labels.windows(2) {
                t.check(24, w[0] != w[1], || format!("{case}: the {axis} axis has two neighbouring labels {:?}", w[0]));
            }
        }
        if numeric && values.len() >= 2 {
            let ordered = values.windows(2).all(|w| if down { w[0] > w[1] } else { w[0] < w[1] });
            t.check(24, ordered, || format!("{case}: the {axis} axis's labels {labels:?} are out of order"));
        }
    }
}

/// Rules 2–13 and 21–26 for one chart, fixture and mode.
///
/// Chart A draws every env × size × t in order, each frame checked by rules
/// 3–6, and at t = 0 rule 7 in colour and one ink; a one-ink frame passing
/// them counts for rule 10, a paper frame for rule 12. Chart B, fresh, draws
/// the same and must match A bitwise (rule 8; for `duplicate-x` rule 26 too).
/// Chart C, fresh, takes sizes and envs in reverse and, for each, t = 6, 1,
/// 2.5, 0 with `view()`, `hit()` and the frame drawn again between; each
/// frame must match A's (rule 9). 1 × 1 is checked for no panic only. From
/// A's colour frames at t = 0 come rules 21 (`empty`), 22, 23 (`nan-gaps`)
/// and 24, from its 80 × 24 one-ink frame rule 10's ink checks; then rule 13
/// resizes a chart over S at the default view and zoomed, rule 11 draws
/// `multi-4`'s series alone, and rule 26 draws `unsorted-x`'s sorted copy.
/// A then plays the input script, and every option key read must have a
/// default (rule 2).
fn run_job(job: &Job) -> Tally {
    let start = Instant::now();
    let mut t = Tally::default();
    let (entry, f, mode) = (job.entry, job.fixture, job.mode);
    let data = f.generate();
    let json = mode_options(entry, mode);
    let labels = common::label_chars(&data, &json);
    let label_texts = common::label_strings(&data, &json);
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
    // Chart A: rules 3–7, 10 and 12.
    let mut frames: Vec<Option<Grid>> = Vec::new();
    let mut probes: Vec<Option<Vec<bool>>> = vec![None; sizes.len()];
    for (env_name, env) in &envs {
        for (s, &(cols, rows)) in sizes.iter().enumerate() {
            for &time in &TIMES {
                let case = what(env_name, (cols, rows), time);
                let drawn = guarded(|| draw(a.as_mut(), time, env, cols, rows));
                t.frames += 1;
                t.check(4, drawn.is_ok(), || format!("{case}: {}", drawn.as_ref().err().cloned().unwrap_or_default()));
                match drawn {
                    Ok(g) => {
                        if (cols, rows) != (1, 1) {
                            let before = t.failures();
                            check_frame(&mut t, &g, cols, rows, &labels, &case);
                            if time == 0.0 && *env_name != "paper" {
                                if probes[s].is_none() {
                                    match guarded(|| probe::hit_map(a.as_ref(), cols, rows)) {
                                        Ok(h) => probes[s] = Some(h),
                                        Err(p) => t.check(4, false, || format!("{case}: hit panicked: {p}")),
                                    }
                                }
                                if let (Some(hits), true) = (&probes[s], f.rows > 0) {
                                    check_first_frame(&mut t, hits, &g, f.rows == 1, &case);
                                }
                            }
                            let ok = t.failures() == before;
                            match *env_name {
                                "mono" => t.check(10, ok, || format!("{case}: the one-ink frame fails rules 3–7 (see those rules)")),
                                "paper" => t.check(12, ok, || format!("{case}: the paper frame fails rules 3–6 (see those rules)")),
                                _ => {}
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
    let env_index = |name: &str| envs.iter().position(|(n, _)| *n == name).unwrap_or(0);
    // Rules 21–24 on the colour frames at t = 0, before any input.
    let checked = guarded(|| {
        let domain = domain_of(a.as_ref());
        for (s, &(cols, rows)) in sizes.iter().enumerate() {
            let (Some(g), Some(hits)) = (&frames[index(0, s, 0)], &probes[s]) else { continue };
            let case = what("plain", (cols, rows), 0.0);
            let plot = Plot::from_hits(hits, cols, rows);
            if f.rows == 0 {
                // Rule 21: axes or "no data", no hit, null domains.
                let text = g.text();
                let axes = g.glyphs().iter().any(|&c| (0x2500..=0x257F).contains(&c));
                t.check(21, axes || text.contains("no data"), || format!("{case}: neither axes nor \"no data\""));
                let hit = hits.iter().position(|&h| h);
                t.check(21, hit.is_none(), || format!("{case}: hit answers at col {} row {}", hit.unwrap_or(0) % cols, hit.unwrap_or(0) / cols));
            }
            // Rule 22: no label reads NaN, inf or -0 (REQ-FMT-030), on every
            // fixture; label text from the data is exempt.
            for line in g.text().lines() {
                for w in probe::words(line) {
                    let from_data = label_texts.iter().any(|l| l.contains(w.as_str()));
                    t.check(22, from_data || !probe::bad_number(&w), || format!("{case}: a label reads {w:?}"));
                }
            }
            let Some((plot, _)) = plot else { continue };
            // Rule 23: gaps break lines, in line-like modes (`spanGaps` off).
            if f.name == "nan-gaps" && has_option(entry, "spanGaps") && matches!((cols, rows), (80, 24) | (320, 120)) {
                match domain {
                    Some(d) => check_gaps(&mut t, g, plot, d, &data, &case),
                    None => t.check(23, false, || format!("{case}: the chart reports no x domain")),
                }
            }
            // Rule 24: tick labels distinct and fitting.
            check_ticks(&mut t, g, plot, &case, false);
        }
        if f.rows == 0 {
            let v = a.view();
            let null = v.x.domain.is_none() && v.y.domain.is_none() && v.axes.values().all(|ax| ax.domain.is_none());
            t.check(21, null, || format!("{} · mode {mode}: the view's domains are not null: {}", f.name, v.to_json()));
        }
        if f.name == "one" {
            // Rule 22: one point in a padded domain.
            let x = data.first_table().and_then(|tb| probe::x_column(tb).map(|i| tb.columns()[i].get(0)));
            let padded = matches!((domain, x), (Some([lo, hi]), Some(x)) if lo < x && x < hi);
            t.check(22, padded, || format!("one · mode {mode}: the x domain {domain:?} does not pad the point at {x:?}"));
        }
    });
    t.check(4, checked.is_ok(), || format!("{} · mode {mode} · rules 21–24: {}", f.name, checked.as_ref().err().cloned().unwrap_or_default()));
    // Rule 10's ink checks at 80 × 24.
    if let Some(s) = sizes.iter().position(|&s| s == (80, 24)) {
        if let Some(g) = &frames[index(env_index("mono"), s, 0)] {
            let plot = probes[s].as_ref().and_then(|h| Plot::from_hits(h, 80, 24)).map(|(p, _)| p);
            check_one_ink(&mut t, g, plot, &what("mono", (80, 24), 0.0), false);
        }
    }
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
    // Chart B: rule 8 (and 26 for duplicate x).
    if let Ok(mut b) = fresh(entry, &options, &data) {
        for (e, (env_name, env)) in envs.iter().enumerate() {
            for (s, &(cols, rows)) in sizes.iter().enumerate() {
                for (ti, &time) in TIMES.iter().enumerate() {
                    let drawn = guarded(|| draw(b.as_mut(), time, env, cols, rows));
                    t.frames += 1;
                    let (Ok(g), Some(want)) = (drawn, &frames[index(e, s, ti)]) else { continue };
                    if (cols, rows) != (1, 1) {
                        let case = what(env_name, (cols, rows), time);
                        t.check(8, &g == want, || format!("{case}: {}", common::first_difference(want, &g)));
                        if f.name == "duplicate-x" {
                            t.check(26, &g == want, || format!("{case}: duplicate x drawn twice differs: {}", common::first_difference(want, &g)));
                        }
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
    // Rule 13: resize, at the default view and zoomed to a quarter.
    let resized: Vec<(usize, (usize, usize))> = sizes.iter().copied().enumerate().filter(|&(_, s)| s != (1, 1)).collect();
    let both: Vec<(usize, usize)> = resized.iter().map(|&(_, s)| s).collect();
    let defaults: Vec<Option<Grid>> = resized.iter().map(|&(s, _)| frames[index(0, s, 0)].clone()).collect();
    if let Ok(mut r) = fresh(entry, &options, &data) {
        check_resize(&mut t, r.as_mut(), &both, &defaults, &format!("{} · mode {mode} · default view", f.name));
    }
    let zoom = fresh(entry, &options, &data).ok().and_then(|c| guarded(|| zoom_patch(c.as_ref(), 0.3, 0.55)).ok().flatten()).filter(|_| f.rows > 0);
    if let Some(z) = &zoom {
        let want: Vec<Option<Grid>> = match fresh_with(entry, &options, &data, Some(z)) {
            Ok(mut c) => both.iter().map(|&(cols, rows)| guarded(|| draw(c.as_mut(), 0.0, &Env::plain(), cols, rows)).ok()).collect(),
            Err(_) => vec![None; both.len()],
        };
        t.frames += both.len() as u64;
        if let Ok(mut r) = fresh_with(entry, &options, &data, Some(z)) {
            check_resize(&mut t, r.as_mut(), &both, &want, &format!("{} · mode {mode} · zoomed", f.name));
        }
    }
    // Rule 11: one ink tells series apart.
    if f.name == "multi-4" {
        check_series_apart(&mut t, entry, &options, &data, mode);
    }
    // Rule 26: unsorted x draws its sorted copy's frame.
    if f.name == "unsorted-x" {
        if let Ok(mut s) = fresh(entry, &options, &probe::sorted_copy(&data)) {
            for (e, (env_name, env)) in envs.iter().enumerate() {
                for (si, &(cols, rows)) in sizes.iter().enumerate().filter(|&(_, &s)| s != (1, 1)) {
                    let Ok(g) = guarded(|| draw(s.as_mut(), 0.0, env, cols, rows)) else { continue };
                    t.frames += 1;
                    if let Some(want) = &frames[index(e, si, 0)] {
                        let case = what(env_name, (cols, rows), 0.0);
                        t.check(26, &g == want, || format!("{case}: the sorted copy draws {}", common::first_difference(&g, want)));
                    }
                }
            }
        }
    }
    t.time = start.elapsed();
    t
}

/// Rules 1–13, 21–26 and 28 for every registered chart (REQ-CONTRACT-010,
/// REQ-CONTRACT-020, REQ-CONTRACT-030, REQ-CHAR-010, REQ-CHAR-020,
/// REQ-CHART-010, REQ-CHART-030, REQ-VIEW-010, REQ-MONO-010, REQ-DATA-040,
/// REQ-DATA-110, REQ-LINE-030, REQ-TEXT-010, REQ-FMT-030).
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
    let mut tallies: Vec<(&'static str, Tally)> = charts
        .iter()
        .map(|e| {
            let mut t = check_meta(e, &slugs);
            t.merge(check_shapes(e));
            (e.meta.slug, t)
        })
        .collect();
    for (job, r) in jobs.iter().zip(results) {
        if let Some((_, t)) = tallies.iter_mut().find(|(s, _)| *s == job.entry.meta.slug) {
            t.merge(r);
        }
    }
    report("contract, rules 1–13, 21–26, 28", &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 21, 22, 23, 24, 25, 26, 28], &tallies, start.elapsed());
}

/// Merges per-job tallies into one a chart, in `charts`' order.
fn by_chart<J>(charts: &[&'static Entry], jobs: &[J], entry: impl Fn(&J) -> &'static Entry, results: Vec<Tally>) -> Vec<(&'static str, Tally)> {
    let mut tallies: Vec<(&'static str, Tally)> = charts.iter().map(|e| (e.meta.slug, Tally::default())).collect();
    for (job, r) in jobs.iter().zip(results) {
        if let Some((_, t)) = tallies.iter_mut().find(|(s, _)| *s == entry(job).meta.slug) {
            t.merge(r);
        }
    }
    tallies
}

/// One unit of `replace_is_whole`: a chart, a mode and the fixture replaced.
struct ReplaceJob {
    entry: &'static Entry,
    mode: &'static str,
    from: &'static Fixture,
}

/// Rule 15 for one chart, mode and first fixture: replacing it with each
/// other accepted fixture draws what that fixture alone draws, at the
/// default view and with a cursor set by a hover before the replace (the
/// fresh chart then given the view the replaced one holds).
fn replace_job(job: &ReplaceJob, onto: &[&'static Fixture], datas: &[Data]) -> Tally {
    let began = Instant::now();
    let mut t = Tally::default();
    let (entry, mode, a) = (job.entry, job.mode, job.from);
    let options = Options::parse(&mode_options(entry, mode)).expect("options");
    let env = Env::plain();
    let Some(ai) = onto.iter().position(|f| f.name == a.name) else { return t };
    for (bi, b) in onto.iter().enumerate().filter(|&(_, b)| b.name != a.name) {
        let case = format!("{} then {} · mode {mode}", a.name, b.name);
        let run = guarded(|| -> Result<(), String> {
            // At the default view.
            let mut x = fresh(entry, &options, &datas[ai])?;
            let _ = draw(x.as_mut(), 0.0, &env, 80, 24);
            let _ = x.set_data(datas[bi].clone());
            let got = draw(x.as_mut(), 0.0, &env, 80, 24);
            let mut alone = fresh(entry, &options, &datas[bi])?;
            let want = draw(alone.as_mut(), 0.0, &env, 80, 24);
            t.check(15, got == want, || format!("{case}: {}", common::first_difference(&want, &got)));
            // With a cursor.
            let mut y = fresh(entry, &options, &datas[ai])?;
            let _ = draw(y.as_mut(), 0.0, &env, 80, 24);
            let _ = y.input(&pointer("move", 0.0, 40.5, 12.5, 0, Mods::NONE));
            let _ = draw(y.as_mut(), 0.0, &env, 80, 24);
            let _ = y.set_data(datas[bi].clone());
            let got = draw(y.as_mut(), 0.0, &env, 80, 24);
            let mut alone = fresh_with(entry, &options, &datas[bi], Some(&ViewPatch::whole(y.view())))?;
            let want = draw(alone.as_mut(), 0.0, &env, 80, 24);
            t.check(15, got == want, || format!("{case} with a cursor: {}", common::first_difference(&want, &got)));
            t.frames += 8;
            Ok(())
        });
        match run {
            Err(p) => t.check(4, false, || format!("{case}: panicked: {p}")),
            Ok(Err(e)) => t.check(4, false, || format!("{case}: {e}")),
            Ok(Ok(())) => {}
        }
    }
    t.time = began.elapsed();
    t
}

/// Rule 15: replacing with a, then b, draws what b alone draws (REQ-DATA-130,
/// REQ-STREAM-060), for every ordered pair of fixtures a chart accepts, in
/// every mode, at 80 × 24.
#[test]
fn replace_is_whole() {
    let start = Instant::now();
    let charts = common::charts();
    let fixtures: Vec<&'static Fixture> = FIXTURES.iter().collect();
    let datas: Vec<Data> = fixtures.iter().map(|f| f.generate()).collect();
    let mut jobs = Vec::new();
    for &entry in &charts {
        for f in accepted(entry) {
            for &mode in entry.meta.modes {
                jobs.push(ReplaceJob { entry, mode, from: f });
            }
        }
    }
    let results = parallel(
        &jobs,
        |j| j.from.rows,
        |j| {
            let onto: Vec<&'static Fixture> = accepted(j.entry);
            let ds: Vec<Data> = onto.iter().map(|f| datas[fixtures.iter().position(|g| g.name == f.name).unwrap_or(0)].clone()).collect();
            replace_job(j, &onto, &ds)
        },
    );
    let tallies = by_chart(&charts, &jobs, |j| j.entry, results);
    report("replace, rule 15", &[4, 15], &tallies, start.elapsed());
}

/// The fixtures the interaction rules (16, 19, 20) play on: every accepted
/// fixture of at most 100,000 rows a series, and the 1M one in the default
/// mode only.
fn interaction_jobs(charts: &[&'static Entry]) -> Vec<Job> {
    let mut jobs = Vec::new();
    for &entry in charts {
        for f in accepted(entry) {
            for &mode in entry.meta.modes {
                if f.rows <= 100_000 || mode == entry.meta.modes[0] {
                    jobs.push(Job { entry, fixture: f, mode });
                }
            }
        }
    }
    jobs
}

/// Rule 16 for one chart, fixture and mode: after each of a zoom, a pan, a
/// hover, a brush, a tap and a series hidden, at 80 × 24, the view validates
/// against view.schema.json, `set_view(view())` changes neither the view
/// nor the frame, and the view's JSON set on a fresh chart gives the same
/// view and frame.
fn view_job(job: &Job) -> Tally {
    let began = Instant::now();
    let mut t = Tally::default();
    let (entry, f, mode) = (job.entry, job.fixture, job.mode);
    let data = f.generate();
    let options = Options::parse(&mode_options(entry, mode)).expect("options");
    let env = Env::plain();
    let schemas = Schemas::get();
    let mut r = SplitMix64::new(f.seed ^ 0x7669_6577);
    let mut cell = || {
        let mut at = |n: f64| (n * (0.2 + 0.6 * r.next_f64())).floor() + 0.5;
        (at(80.0), at(24.0))
    };
    let (zx, zy) = cell();
    let (px, py) = cell();
    let (hx, hy) = cell();
    let (bx, by) = cell();
    let (tx, ty) = cell();
    let steps: Vec<(&str, Vec<Input>)> = vec![
        (
            "zoom",
            vec![
                Input::Wheel { t: 0.0, x: zx, y: zy, dx: 0.0, dy: -3.0, mods: Mods::CTRL },
                Input::Wheel { t: 0.1, x: zx, y: zy, dx: 0.0, dy: 0.0, mods: Mods::CTRL },
            ],
        ),
        (
            "pan",
            vec![
                pointer("down", 0.2, px, py, 1, Mods::NONE),
                pointer("move", 0.3, px - 9.0, py, 1, Mods::NONE),
                pointer("up", 0.4, px - 9.0, py, 0, Mods::NONE),
            ],
        ),
        ("hover", vec![pointer("move", 0.5, hx, hy, 0, Mods::NONE)]),
        (
            "brush",
            vec![
                pointer("down", 0.6, bx, by, 1, Mods::SHIFT),
                pointer("move", 0.7, bx + 12.0, by, 1, Mods::SHIFT),
                pointer("up", 0.8, bx + 12.0, by, 0, Mods::SHIFT),
            ],
        ),
        ("tap", vec![Input::Tap { t: 0.9, x: tx, y: ty, count: 1, pointer: PointerType::Mouse }]),
        ("hidden", vec![]),
    ];
    let run = guarded(|| -> Result<(), String> {
        let mut a = fresh(entry, &options, &data)?;
        let _ = draw(a.as_mut(), 0.0, &env, 80, 24);
        for (name, inputs) in steps {
            let case = format!("{} · mode {mode} · after the {name}", f.name);
            for i in &inputs {
                let _ = a.input(i);
            }
            if name == "hidden" {
                // Hide the series a hit names at the plot's centre.
                if let Some(h) = a.hit(CellPos::center(40, 12)) {
                    a.set_view(&ViewPatch::from_json(&serde_json::json!({ "hidden": [h.series] }).to_string()).map_err(|e| e.to_string())?);
                }
            }
            let before = draw(a.as_mut(), 2.0, &env, 80, 24);
            let view = a.view().clone();
            let json = view.to_json();
            let value: serde_json::Value = serde_json::from_str(&json).map_err(|e| e.to_string())?;
            let errors = schemas.validate("view.schema.json", &value);
            t.check(16, errors.is_empty(), || format!("{case}: the view breaks view.schema.json: {}", errors.join("; ")));
            a.set_view(&ViewPatch::whole(&view));
            let again = draw(a.as_mut(), 2.0, &env, 80, 24);
            t.check(16, a.view() == &view, || format!("{case}: set_view(view()) changed the view to {}", a.view().to_json()));
            t.check(16, again == before, || format!("{case}: set_view(view()) changed the frame: {}", common::first_difference(&before, &again)));
            a.set_view(&ViewPatch::default());
            t.check(16, a.view() == &view, || format!("{case}: an empty patch changed the view"));
            let patch = ViewPatch::from_json(&json).map_err(|e| e.to_string())?;
            let mut b = fresh_with(entry, &options, &data, Some(&patch))?;
            let other = draw(b.as_mut(), 2.0, &env, 80, 24);
            // The fresh chart holds the view the JSON reads as. (Not `view`
            // bit for bit: serde_json's default number parsing is not
            // correctly rounded, so a number may come back an ulp off; the
            // frame must not change for it.)
            let read = glyphplot::view::View::from_json(&json).map_err(|e| e.to_string())?;
            t.check(16, b.view() == &read, || format!("{case}: through JSON the view {json} became {}", b.view().to_json()));
            t.check(16, other == before, || format!("{case}: through JSON onto a fresh chart: {}", common::first_difference(&before, &other)));
            t.frames += 3;
        }
        Ok(())
    });
    match run {
        Err(p) => t.check(4, false, || format!("{} · mode {mode}: panicked: {p}", f.name)),
        Ok(Err(e)) => t.check(4, false, || format!("{} · mode {mode}: {e}", f.name)),
        Ok(Ok(())) => {}
    }
    t.time = began.elapsed();
    t
}

/// Rule 16: the view round-trips (REQ-VIEW-020, REQ-VIEW-030).
#[test]
fn view_round_trips() {
    let start = Instant::now();
    let charts = common::charts();
    let jobs = interaction_jobs(&charts);
    let results = parallel(&jobs, |j| j.fixture.rows * j.fixture.series, view_job);
    let tallies = by_chart(&charts, &jobs, |j| j.entry, results);
    report("view round trip, rule 16", &[4, 16], &tallies, start.elapsed());
}

/// Rules 17 and 18 (REQ-VIEW-010, REQ-INPUT-010, REQ-EVENT-010): each input
/// script (mouse, touch, keys) played on two fresh charts gives the same
/// changes, events, views and frames after every event, and the view it ends
/// on, set on a third chart, draws the same frame; every event serialises to
/// JSON that events.schema.json accepts. On `sine-1k` and `multi-4`, or every
/// accepted fixture of more than two rows up to 10,000, in every mode at
/// 80 × 24.
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
    let scripts: [(&str, Vec<Input>); 3] = [("mouse", script(80, 24)), ("touch", touch_script(80, 24)), ("keys", key_script())];
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
            let schemas = Schemas::get();
            for (name, inputs) in &scripts {
                let case = |what: &str| format!("{} · mode {mode} · {name} script · {what}", f.name);
                let played = guarded(|| -> Result<(), String> {
                    let mut a = fresh(entry, &options, &data)?;
                    let mut b = fresh(entry, &options, &data)?;
                    let _ = draw(a.as_mut(), 0.0, &env, 80, 24);
                    let _ = draw(b.as_mut(), 0.0, &env, 80, 24);
                    for (i, event) in inputs.iter().enumerate() {
                        let (ca, cb) = (a.input(event), b.input(event));
                        t.check(17, ca == cb, || case(&format!("event {i}: changes {ca:?} against {cb:?}")));
                        t.check(17, a.view() == b.view(), || case(&format!("event {i}: the views differ")));
                        for e in &ca.events {
                            let value = serde_json::to_value(e).map_err(|e| e.to_string())?;
                            let errors = schemas.validate("events.schema.json", &value);
                            t.check(18, errors.is_empty(), || case(&format!("event {i}: {} breaks events.schema.json: {}", e.name(), errors.join("; "))));
                        }
                        let (ga, gb) = (draw(a.as_mut(), event.t(), &env, 80, 24), draw(b.as_mut(), event.t(), &env, 80, 24));
                        t.frames += 2;
                        t.check(17, ga == gb, || case(&format!("event {i}: {}", common::first_difference(&ga, &gb))));
                    }
                    let end = 2.0;
                    let ga = draw(a.as_mut(), end, &env, 80, 24);
                    let mut c = fresh_with(entry, &options, &data, Some(&ViewPatch::whole(a.view())))?;
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
            }
            t.time = began.elapsed();
            t
        },
    );
    let tallies = by_chart(&charts, &jobs, |j| j.entry, results);
    report("input, rules 17 and 18", &[4, 17, 18], &tallies, start.elapsed());
}

/// The probes of `hits_name_what_was_drawn`: size and view.
const HIT_CASES: [(usize, usize, bool); 3] = [(80, 24, false), (40, 12, false), (80, 24, true)];

/// What one hit walk found: the chart's plot and x mapping at one case, and
/// the hits at up to 512 seeded cells.
struct Walk {
    case: String,
    frame: Grid,
    plot: Plot,
    axis: Axis,
    held: Held,
    hits: Vec<((usize, usize), glyphplot::chart::Hit)>,
    patch: Option<ViewPatch>,
}

/// Draws `entry` on `f` in `mode` at one hit case and walks up to 512 seeded
/// cells with `hit`; `None` when the chart has no plot there (no data).
fn hit_walk(t: &mut Tally, entry: &Entry, f: &Fixture, mode: &str, (cols, rows, zoomed): (usize, usize, bool)) -> Option<Walk> {
    let data = f.generate();
    let options = Options::parse(&mode_options(entry, mode)).expect("options");
    let case = format!("{} · mode {mode} · {cols}×{rows} · {}", f.name, if zoomed { "zoomed" } else { "default view" });
    let found = guarded(|| -> Result<Option<Walk>, String> {
        let patch = if zoomed {
            let c = fresh(entry, &options, &data)?;
            match zoom_patch(c.as_ref(), 0.4, 0.47) {
                Some(p) => Some(p),
                None => return Ok(None),
            }
        } else {
            None
        };
        let mut c = fresh_with(entry, &options, &data, patch.as_ref())?;
        let frame = draw(c.as_mut(), 0.0, &Env::plain(), cols, rows);
        let Some((plot, solid)) = Plot::find(c.as_ref(), cols, rows) else { return Ok(None) };
        t.check(19, solid, || format!("{case}: the cells hit answers at are not a rectangle"));
        let Some(domain) = domain_of(c.as_ref()) else {
            t.check(19, false, || format!("{case}: hit answers but the chart reports no x domain"));
            return Ok(None);
        };
        let axis = Axis::x(domain[0], domain[1], plot.w, 2);
        let mut r = SplitMix64::new(f.seed ^ 0x6869_7473 ^ (cols * 1000 + rows) as u64 ^ u64::from(zoomed));
        let all = cols * rows;
        let cells: Vec<usize> = if all <= 512 { (0..all).collect() } else { (0..512).map(|_| r.below(all as u64) as usize).collect() };
        let hits = cells.into_iter().filter_map(|i| c.hit(CellPos::center(i % cols, i / cols)).map(|h| ((i % cols, i / cols), h))).collect();
        t.frames += 1;
        Ok(Some(Walk { case, frame, plot, axis, held: Held::new(&data), hits, patch }))
    });
    match found {
        Err(p) => {
            t.check(4, false, || format!("{}: panicked: {p}", f.name));
            None
        }
        Ok(Err(e)) => {
            t.check(4, false, || format!("{}: {e}", f.name));
            None
        }
        Ok(Ok(w)) => w,
    }
}

/// The dot column under the centre of cell `c` of a plot starting at
/// column `x0`, by the cell mapping (two dots a cell).
fn probe_dot(c: usize, x0: usize) -> i64 {
    cells::nearest(c as f64 + 0.5 - x0 as f64, 2).map_or(0, |(_, d)| d)
}

/// Rules 19 and 20 for one chart, fixture and mode.
///
/// Rule 19, at 80 × 24 and 40 × 12 at the default view and at 80 × 24
/// zoomed: for up to 512 seeded cells, `hit` names a datum of the data as
/// held or nothing; the datum's x maps by the cell mapping into the column
/// `hit` says it is drawn in, and the frame inked that cell when its y is
/// finite; and the datum is the one a snap to the nearest x may name: in the
/// dot column under the probe when that column holds a datum, else the
/// nearest datum on its left or its right. (The plan's first wording,
/// "maps into that cell's column", is in `rules_as_worded`, ignored: it
/// fails on sparse data.)
///
/// Rule 20, for up to 24 of those cells: a hover there sets the view's
/// cursor to the datum `hit` names, and the frame's readout names its series
/// and writes its y rounded to the digits shown.
fn hits_job(job: &Job) -> Tally {
    let began = Instant::now();
    let mut t = Tally::default();
    let (entry, f, mode) = (job.entry, job.fixture, job.mode);
    let options = Options::parse(&mode_options(entry, mode)).expect("options");
    for hc in HIT_CASES {
        let Some(w) = hit_walk(&mut t, entry, f, mode, hc) else { continue };
        let Walk { case, frame, plot, axis, held, hits, patch } = w;
        // Which dot columns hold a datum's x.
        let mut full = vec![false; axis.dots()];
        for &x in &held.xs {
            if let Some(k) = axis.dot_in(x) {
                full[k] = true;
            }
        }
        for &((c, r), ref h) in &hits {
            let at = format!("{case} · cell {c},{r}");
            let named = held.datum(&h.series, h.index);
            let real = named.is_some_and(|(x, y)| probe::same(x, h.x) && probe::same(y, h.y));
            t.check(19, real, || format!("{at}: hit names {} row {} at ({}, {}), but the data holds {named:?}", h.series, h.index, h.x, h.y));
            if let Some(cell) = axis.cell_in(h.x) {
                t.check(19, plot.x + cell == h.col, || format!("{at}: x {} maps to column {}, but hit says {}", h.x, plot.x + cell, h.col));
                if h.y.is_finite() && plot.contains(h.col, h.row) {
                    let g = frame.glyph(h.col, h.row);
                    t.check(19, probe::inked(g), || format!("{at}: hit says {} row {} is drawn at {},{}, which is blank", h.series, h.index, h.col, h.row));
                }
            }
            let k = probe_dot(c, plot.x);
            let ok = if full.get(k as usize).copied().unwrap_or(false) {
                axis.dot_in(h.x) == Some(k as usize)
            } else {
                let (lo, hi) = (axis.dot_value(k), axis.dot_value(k + 1));
                let left = held.xs.iter().rev().find(|&&x| x < lo);
                let right = held.xs.iter().find(|&&x| x >= hi);
                left.is_some_and(|&l| l == h.x) || right.is_some_and(|&rt| rt == h.x)
            };
            t.check(19, ok, || format!("{at}: hit names x {} for the dot column {k}, which is not the nearest datum's", h.x));
        }
        // Rule 20.
        let data = f.generate();
        for &((c, r), ref h) in hits.iter().take(24) {
            let at = format!("{case} · cell {c},{r}");
            let run = guarded(|| -> Result<(), String> {
                let mut ch = fresh_with(entry, &options, &data, patch.as_ref())?;
                let base = draw(ch.as_mut(), 0.0, &Env::plain(), frame.cols(), frame.rows());
                let _ = ch.input(&pointer("move", 0.0, c as f64 + 0.5, r as f64 + 0.5, 0, Mods::NONE));
                let g = draw(ch.as_mut(), 0.0, &Env::plain(), frame.cols(), frame.rows());
                t.frames += 2;
                let cursor = ch.view().cursor.clone().unwrap_or_default();
                let agree = cursor.series.as_deref() == Some(h.series.as_str())
                    && cursor.index == Some(h.index as u64)
                    && cursor.x.is_some_and(|x| probe::same(x, h.x))
                    && cursor.y.map_or(!h.y.is_finite(), |y| probe::same(y, h.y));
                t.check(20, agree, || format!("{at}: the cursor {cursor:?} is not hit's {} row {} ({}, {})", h.series, h.index, h.x, h.y));
                t.check(20, g != base, || format!("{at}: the hover drew no readout"));
                let name = probe::as_shown(&h.series);
                let text = g.text();
                t.check(20, text.contains(&name), || format!("{at}: the readout does not name the series {name:?}"));
                if h.y.is_finite() {
                    let key = format!("{name} ");
                    let mut values: Vec<String> = Vec::new();
                    for l in text.lines() {
                        for (i, _) in l.match_indices(&key) {
                            values.push(l[i + key.len()..].split(' ').next().unwrap_or("").to_string());
                        }
                    }
                    let rounds = |w: &String| {
                        let Some(v) = probe::parse_number(w) else { return false };
                        let digits = w.split_once('.').map_or(0, |(_, d)| d.chars().take_while(char::is_ascii_digit).count());
                        let exp = w.contains(['e', 'E']);
                        let tol = if exp { v.abs() * 1e-11 } else { 0.5 * 10f64.powi(-(digits as i32)) * (1.0 + 1e-9) + h.y.abs() * 1e-15 };
                        (v - h.y).abs() <= tol
                    };
                    t.check(20, values.iter().any(rounds), || format!("{at}: the readout writes {values:?} after {name:?}, not {} rounded", h.y));
                }
                Ok(())
            });
            match run {
                Err(p) => t.check(4, false, || format!("{at}: panicked: {p}")),
                Ok(Err(e)) => t.check(4, false, || format!("{at}: {e}")),
                Ok(Ok(())) => {}
            }
        }
    }
    t.time = began.elapsed();
    t
}

/// Rules 19 and 20 (REQ-HIT-010, REQ-CANVAS-020, REQ-LINE-040).
#[test]
fn hits_name_what_was_drawn() {
    let start = Instant::now();
    let charts = common::charts();
    let jobs = interaction_jobs(&charts);
    let results = parallel(&jobs, |j| j.fixture.rows * j.fixture.series, hits_job);
    let tallies = by_chart(&charts, &jobs, |j| j.entry, results);
    // A mark chart (scatter, candle, bar) must also have drawn its mark in
    // the probed cell; no registered chart is one yet.
    report("hits, rules 19 and 20", &[4, 19, 20], &tallies, start.elapsed());
}

/// Rules 10, 19 and 24 as the plan first worded them, before v0.4.0 took the
/// wordings the contract checks. Ignored: each fails on a frame that is
/// right; run it with `--ignored` to see why the wording changed.
///
/// - Rule 10, "ink 1–90 %": `constant` as an area in one ink at 80 × 24 inks
///   91.8 % of the cells, the area filled to its baseline 0.
/// - Rule 19, "a named datum maps into that cell's column": a hit snaps to
///   the nearest datum (REQ-HIT-010), which on sparse data (`two` at
///   80 × 24, two points at the plot's ends) lies in another column.
/// - Rule 24, "no two labels on an axis are equal": `random-walk-100k` at
///   320 × 120 has hourly ticks over 28 hours, and d3's time format writes
///   `01 AM` on each day.
#[test]
#[ignore = "rules 10, 19 and 24 as the plan first worded them fail on right frames: see the doc comment"]
fn rules_as_worded() {
    let start = Instant::now();
    let charts = common::charts();
    let jobs = interaction_jobs(&charts);
    let results = parallel(
        &jobs,
        |j| j.fixture.rows,
        |job| {
            let mut t = Tally::default();
            let (entry, f, mode) = (job.entry, job.fixture, job.mode);
            for hc in HIT_CASES {
                let Some(w) = hit_walk(&mut t, entry, f, mode, hc) else { continue };
                for ((c, r), h) in &w.hits {
                    let cell = w.axis.cell_in(h.x).map(|k| w.plot.x + k);
                    t.check(19, cell == Some(*c), || format!("{} · cell {c},{r}: hit names x {}, in column {cell:?}", w.case, h.x));
                }
            }
            let data = f.generate();
            let options = Options::parse(&mode_options(entry, mode)).expect("options");
            let drawn = guarded(|| -> Result<(), String> {
                for (env_name, env) in envs().iter().take(2) {
                    for &(cols, rows) in sizes(entry).iter().filter(|&&s| s != (1, 1)) {
                        let mut c = fresh(entry, &options, &data)?;
                        let g = draw(c.as_mut(), 0.0, env, cols, rows);
                        let case = format!("{} · mode {mode} · {env_name} · {cols}×{rows}", f.name);
                        let plot = Plot::find(c.as_ref(), cols, rows).map(|(p, _)| p);
                        match (*env_name, plot) {
                            ("mono", _) if (cols, rows) == (80, 24) => check_one_ink(&mut t, &g, plot, &case, true),
                            ("plain", Some(p)) => check_ticks(&mut t, &g, p, &case, true),
                            _ => {}
                        }
                    }
                }
                Ok(())
            });
            if let Ok(Err(e)) | Err(e) = drawn {
                t.check(4, false, || format!("{} · mode {mode}: {e}", f.name));
            }
            t
        },
    );
    let tallies = by_chart(&charts, &jobs, |j| j.entry, results);
    report("rules 10, 19 and 24 as worded", &[4, 10, 19, 24], &tallies, start.elapsed());
}

/// One unit of `downsampling_is_exact`: a chart, a fixture, a mode and a view.
struct SampleJob {
    entry: &'static Entry,
    fixture: &'static Fixture,
    mode: &'static str,
    view: &'static str,
}

/// The views rule 27 draws at: the fitted one, zoomed into 30 % of it, and
/// zoomed out to three times it.
const SAMPLE_VIEWS: [(&str, f64, f64); 3] = [("default view", 0.0, 1.0), ("zoomed in", 0.35, 0.65), ("zoomed out", -1.0, 2.0)];

/// Rule 27 (REQ-SAMPLE-010, REQ-PERF-010): for a line-like chart (one whose
/// `META.options` has `sample`) and every accepted fixture with x already
/// sorted, each mode's frame equals the frame with `{"sample":"none"}`,
/// which draws every segment, at every size of S but 1 × 1, at the fitted
/// view and zoomed in and out; with `spanGaps` on too for the fixtures with
/// missing or infinite y.
#[test]
fn downsampling_is_exact() {
    let start = Instant::now();
    let charts = common::charts();
    let mut jobs = Vec::new();
    let mut tallies: Vec<(&'static str, Tally)> = charts.iter().map(|e| (e.meta.slug, Tally::default())).collect();
    for (i, &entry) in charts.iter().enumerate() {
        if !has_option(entry, "sample") {
            tallies[i].1.skip(27, "not line-like: META.options has no `sample`");
            continue;
        }
        for f in accepted(entry).into_iter().filter(|f| f.name != "unsorted-x") {
            for &mode in entry.meta.modes {
                for (view, _, _) in SAMPLE_VIEWS {
                    jobs.push(SampleJob { entry, fixture: f, mode, view });
                }
            }
        }
    }
    let results = parallel(
        &jobs,
        |j| j.fixture.rows * j.fixture.series,
        |job| {
            let began = Instant::now();
            let mut t = Tally::default();
            let (entry, f, mode) = (job.entry, job.fixture, job.mode);
            let data = f.generate();
            let (_, from, to) = SAMPLE_VIEWS.iter().copied().find(|v| v.0 == job.view).unwrap_or(SAMPLE_VIEWS[0]);
            let mut variants = vec![serde_json::json!({ "mode": mode })];
            if matches!(f.name, "nan-gaps" | "inf") && has_option(entry, "spanGaps") {
                variants.push(serde_json::json!({ "mode": mode, "spanGaps": true }));
            }
            for v in variants {
                let mut none = v.clone();
                none["sample"] = "none".into();
                let case = format!("{} · {v} · {}", f.name, job.view);
                let run = guarded(|| -> Result<(), String> {
                    let (sampled, every) =
                        (Options::parse(&v.to_string()).map_err(|e| e.to_string())?, Options::parse(&none.to_string()).map_err(|e| e.to_string())?);
                    let mut a = fresh(entry, &sampled, &data)?;
                    let patch = if job.view == "default view" { None } else { zoom_patch(a.as_ref(), from, to) };
                    if let Some(p) = &patch {
                        a.set_view(p);
                    }
                    let mut b = fresh_with(entry, &every, &data, patch.as_ref())?;
                    for &(cols, rows) in sizes(entry).iter().filter(|&&s| s != (1, 1)) {
                        let (ga, gb) = (draw(a.as_mut(), 0.0, &Env::plain(), cols, rows), draw(b.as_mut(), 0.0, &Env::plain(), cols, rows));
                        t.frames += 2;
                        t.check(27, ga == gb, || format!("{case} · {cols}×{rows}: sampled against every segment: {}", common::first_difference(&gb, &ga)));
                    }
                    Ok(())
                });
                match run {
                    Err(p) => t.check(4, false, || format!("{case}: panicked: {p}")),
                    Ok(Err(e)) => t.check(4, false, || format!("{case}: {e}")),
                    Ok(Ok(())) => {}
                }
            }
            t.time = began.elapsed();
            t
        },
    );
    for (job, r) in jobs.iter().zip(results) {
        if let Some((_, t)) = tallies.iter_mut().find(|(s, _)| *s == job.entry.meta.slug) {
            t.merge(r);
        }
    }
    report("downsampling, rule 27", &[4, 27], &tallies, start.elapsed());
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
    let tallies = by_chart(&charts, &jobs, |(e, _, _)| e, results);
    report("every size, REQ-LAYOUT-040 (rule 4)", &[4], &tallies, start.elapsed());
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

/// The rows `range` of `data`'s first table, as data of their own.
#[cfg(feature = "contract-selftest")]
fn rows_of(data: &Data, range: std::ops::Range<usize>) -> Data {
    let t = data.first_table().expect("a table");
    let columns = t
        .columns()
        .iter()
        .map(|c| {
            let mut n = c.clone();
            n.values = match &c.values {
                glyphplot::data::Values::Num(v) => glyphplot::data::Values::Num(v[range.clone()].to_vec()),
                glyphplot::data::Values::Codes(v) => glyphplot::data::Values::Codes(v[range.clone()].to_vec()),
                glyphplot::data::Values::Text(v) => glyphplot::data::Values::Text(v[range.clone()].to_vec()),
            };
            n
        })
        .collect();
    Data::from_table(Table::from_columns(columns).expect("rows of a valid table"))
}

/// REQ-CONTRACT-060: the contract rejects each broken chart the
/// `contract-selftest` feature registers, with the matching rule's message,
/// and passes the `line` chart they are built on under the same checks:
///
/// - `selftest-random` draws a different frame on each fresh chart: rule 8;
/// - `selftest-charset` draws U+0416 Ж, outside the font: rule 5;
/// - `selftest-stateful` keeps a count of the data it was given across
///   replaces and appends. Appending in batches ends on a frame other than a
///   replace's, which is rule 14's case (v0.6.0, not checked yet); the same
///   state survives a replace, so rule 15 rejects it now.
#[cfg(feature = "contract-selftest")]
#[test]
fn selftest_rejects_broken_charts() {
    let entry = |slug: &str| registry::entries().find(|e| e.meta.slug == slug).unwrap_or_else(|| panic!("{slug} is registered"));
    let sine = fixtures::fixture("sine-1k").expect("sine-1k");
    let rejects = |t: &Tally, rule: u32, says: &str| {
        let prefix = format!("rule {rule}: ");
        t.fails.get(&rule).copied().unwrap_or(0) > 0 && t.examples.iter().any(|e| e.starts_with(&prefix) && e.contains(says))
    };
    let line = entry("line");
    let control = run_job(&Job { entry: line, fixture: sine, mode: "line" });
    assert!(control.ok(), "the line chart passes: {:?}", control.examples);
    for (slug, rule, says) in [("selftest-random", 8, "cell(s) differ"), ("selftest-charset", 5, "is outside the font and the label text")] {
        let t = run_job(&Job { entry: entry(slug), fixture: sine, mode: "line" });
        assert!(rejects(&t, rule, says), "{slug} is not rejected by rule {rule} ({says:?}): fails {:?}, {:?}", t.fails, t.examples);
    }
    let onto: Vec<&'static Fixture> = ["sine-1k", "two"].iter().filter_map(|n| fixtures::fixture(n)).collect();
    let datas: Vec<Data> = onto.iter().map(|f| f.generate()).collect();
    let control = replace_job(&ReplaceJob { entry: line, mode: "line", from: sine }, &onto, &datas);
    assert!(control.ok(), "the line chart passes rule 15: {:?}", control.examples);
    let stateful = entry("selftest-stateful");
    let t = replace_job(&ReplaceJob { entry: stateful, mode: "line", from: sine }, &onto, &datas);
    assert!(rejects(&t, 15, "then two"), "selftest-stateful is not rejected by rule 15: fails {:?}, {:?}", t.fails, t.examples);
    // Rule 14's case, shown directly: batches of 1, 7, ⌊n/3⌋ and the rest
    // against a replace.
    let data = sine.generate();
    let n = sine.rows;
    let cuts = [0, 1, 8, 8 + n / 3, n];
    let frame_after = |slug: &str, batches: bool| {
        let (_, mut c) = registry::make(slug, "").expect("made");
        if batches {
            for w in cuts.windows(2) {
                c.append(rows_of(&data, w[0]..w[1])).expect("appended");
                let _ = draw(c.as_mut(), 0.0, &Env::plain(), 80, 24);
            }
        } else {
            c.set_data(data.clone()).expect("replaced");
        }
        draw(c.as_mut(), 0.0, &Env::plain(), 80, 24)
    };
    assert_eq!(frame_after("line", true), frame_after("line", false), "the line chart's appends end on the replace's frame");
    assert_ne!(frame_after("selftest-stateful", true), frame_after("selftest-stateful", false), "selftest-stateful keeps state across appends");
}
