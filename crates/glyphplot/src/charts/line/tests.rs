//! The line chart's own tests: modes, gaps, M4 against every segment, the
//! caches, hits, the reducer's defaults through the chart, the accessible
//! text and the frame times. The contract (`tests/contract.rs`) checks the
//! rules every chart keeps.

use std::time::Instant;

use super::*;
use crate::data::{Column, Table as DataTable};
use crate::events::{Cause, Event, Source};
use crate::fixtures::{generate, FIXTURES};
use crate::grid::WIDE_TAIL;
use crate::input::{Key, Mods, PointerType};
use crate::palette;

fn chart(fixture: &str, opts: &str) -> LineChart {
    let mut c = LineChart::new(&Options::parse(opts).unwrap()).unwrap();
    c.set_data(generate(fixture).unwrap()).unwrap();
    c
}

fn draw(c: &mut LineChart, cols: usize, rows: usize, t: f64) -> Grid {
    let mut g = Grid::new(cols, rows);
    c.frame(t, &Env::plain(), &mut g);
    g
}

fn xy(x: Vec<f64>, y: Vec<f64>) -> Data {
    Data::from_table(DataTable::from_columns(vec![Column::f64("x", x), Column::f64("y", y)]).unwrap())
}

fn mv(t: f64, x: f64, y: f64) -> Input {
    Input::Move { t, x, y, pointer: PointerType::Mouse, buttons: 0, mods: Mods::NONE }
}

fn key(t: f64, key: Key, mods: Mods) -> Input {
    Input::Key { t, key, mods }
}

fn names(ch: &Changes) -> Vec<&'static str> {
    ch.events.iter().map(Event::name).collect()
}

/// The plot of the chart's last frame.
fn plot(c: &LineChart) -> Rect {
    c.geom.as_ref().unwrap().1.plot
}

/// Whether a glyph is data: braille.
fn is_dots(g: u32) -> bool {
    (0x2801..=0x28FF).contains(&g)
}

/// REQ-LINE-010: the modes line, step (before, after, middle) and area draw,
/// each its own frame.
#[test]
fn modes_draw_each_way() {
    let opts = [r#"{"mode":"line"}"#, r#"{"mode":"step"}"#, r#"{"mode":"step","step":"before"}"#, r#"{"mode":"step","step":"middle"}"#, r#"{"mode":"area"}"#];
    let sparse = || xy((0..30).map(f64::from).collect(), (0..30).map(|i| f64::from((i * 7) % 11)).collect());
    let frames: Vec<Grid> = opts
        .iter()
        .map(|o| {
            let mut c = LineChart::new(&Options::parse(o).unwrap()).unwrap();
            c.set_data(sparse()).unwrap();
            draw(&mut c, 80, 24, 0.0)
        })
        .collect();
    for (i, a) in frames.iter().enumerate() {
        for b in &frames[i + 1..] {
            assert_ne!(a, b);
        }
    }
    let inked = |g: &Grid| g.glyphs().iter().filter(|&&c| is_dots(c)).count();
    assert!(inked(&frames[4]) > 2 * inked(&frames[0]), "an area fills under its line");
    // Steps of two data: after rises at the later x, before at the earlier.
    let step = |at: &str| {
        let mut c = LineChart::new(&Options::parse(&format!(r#"{{"mode":"step","step":"{at}"}}"#)).unwrap()).unwrap();
        c.set_data(xy(vec![0.0, 10.0], vec![0.0, 10.0])).unwrap();
        let g = draw(&mut c, 40, 12, 0.0);
        let p = plot(&c);
        // The top right cell holds the step's top for `after`, the top left
        // for `before`.
        (is_dots(g.glyph(p.x, p.y)), is_dots(g.glyph(p.x + p.w - 1, p.y + p.h - 1)))
    };
    assert_eq!(step("before"), (true, false));
    assert_eq!(step("after"), (false, true));
}

/// REQ-LINE-030, contract rule 23: a missing y breaks the line, no dot column
/// inside the gap inked; `spanGaps` joins across it.
#[test]
fn gaps_break_the_line() {
    let x: Vec<f64> = (0..100).map(f64::from).collect();
    let y: Vec<f64> = (0..100).map(|i| if (40..60).contains(&i) { f64::NAN } else { (i % 7) as f64 }).collect();
    for mode in ["line", "step", "area"] {
        for span in [false, true] {
            let mut c = LineChart::new(&Options::parse(&format!(r#"{{"mode":"{mode}","spanGaps":{span}}}"#)).unwrap()).unwrap();
            c.set_data(xy(x.clone(), y.clone())).unwrap();
            let g = draw(&mut c, 80, 24, 0.0);
            let geom = &c.geom.as_ref().unwrap().1;
            let p = geom.plot;
            let inside: Vec<usize> = (0..p.w).filter(|&col| geom.x.cell_value(col as i64) > 39.0 && geom.x.cell_value(col as i64 + 1) < 60.0).collect();
            assert!(inside.len() > 10);
            let inked = inside.iter().any(|&col| (p.y..p.y + p.h).any(|r| is_dots(g.glyph(p.x + col, r))));
            assert_eq!(inked, span, "{mode}, spanGaps {span}");
        }
    }
}

/// REQ-SAMPLE-010, REQ-PERF-010, contract rule 27: drawn from M4, a frame
/// equals the frame drawn from every segment (`sample: "none"`), at every
/// mode and size, zoomed in and out.
#[test]
fn m4_frame_equals_every_segment() {
    let views = ["", r#"{"x":{"domain":[100.5,700.25]}}"#, r#"{"x":{"domain":[-50,5000]}}"#, r#"{"x":{"domain":[1792281700000,1792290000000]}}"#];
    for fixture in
        ["sine-1k", "random-walk-100k", "nan-gaps", "steps-10k", "multi-4", "duplicate-x", "unsorted-x", "labels-wide", "timestamps-dst", "inf", "two"]
    {
        for mode in [r#""mode":"line""#, r#""mode":"step","step":"middle""#, r#""mode":"area""#, r#""mode":"line","spanGaps":true"#] {
            for view in views {
                for (cols, rows) in [(80, 24), (320, 120), (23, 6)] {
                    let mut a = chart(fixture, &format!("{{{mode}}}"));
                    let mut b = chart(fixture, &format!(r#"{{{mode},"sample":"none"}}"#));
                    for c in [&mut a, &mut b] {
                        c.set_view(&ViewPatch::from_json(view).unwrap());
                    }
                    assert_eq!(draw(&mut a, cols, rows, 0.0), draw(&mut b, cols, rows, 0.0), "{fixture} {mode} {view} {cols}×{rows}");
                }
            }
        }
    }
    let mut a = chart("random-walk-1m", "");
    let mut b = chart("random-walk-1m", r#"{"sample":"none"}"#);
    assert_eq!(draw(&mut a, 160, 40, 0.0), draw(&mut b, 160, 40, 0.0), "a million rows");
}

/// REQ-SAMPLE-020, REQ-PERF-020: frames with nothing changed read no data and
/// draw no base; a change of data, view domain, options or size does.
#[test]
fn unchanged_frames_read_no_data_and_draw_no_base() {
    let mut c = chart("random-walk-100k", "");
    let first = draw(&mut c, 80, 24, 0.0);
    let (scans, draws) = (c.scans(), c.base_draws());
    assert_eq!((scans, draws), (1, 1));
    for t in [0.5, 1.0, 2.5, 6.0] {
        assert_eq!(draw(&mut c, 80, 24, t), first);
    }
    c.input(&mv(0.0, 30.0, 10.0));
    draw(&mut c, 80, 24, 0.0);
    assert_eq!((c.scans(), c.base_draws()), (1, 1), "a hover");
    c.set_view(&ViewPatch::from_json(r#"{"x":{"domain":[1792281700000,1792300000000]}}"#).unwrap());
    draw(&mut c, 80, 24, 0.0);
    assert_eq!((c.scans(), c.base_draws()), (2, 2), "a new x range");
    draw(&mut c, 81, 24, 0.0);
    assert_eq!((c.scans(), c.base_draws()), (3, 3), "a new width");
    draw(&mut c, 81, 30, 0.0);
    assert_eq!((c.scans(), c.base_draws()), (3, 4), "a new height reuses the M4");
    c.set_options(&Options::parse(r#"{"mode":"step"}"#).unwrap()).unwrap();
    draw(&mut c, 81, 30, 0.0);
    assert_eq!((c.scans(), c.base_draws()), (3, 5), "new options");
    c.set_data(generate("random-walk-100k").unwrap()).unwrap();
    draw(&mut c, 81, 30, 0.0);
    assert_eq!((c.scans(), c.base_draws()), (4, 6), "new data");
    let mut g = Grid::new(81, 30);
    c.frame(0.0, &Env { mono: true, ..Env::plain() }, &mut g);
    assert_eq!((c.scans(), c.base_draws()), (4, 7), "one ink");
}

/// REQ-CHART-060, REQ-CANVAS-040: where series meet, the later series' ink
/// wins, unless a selected series is raised.
#[test]
fn later_series_wins_a_shared_cell() {
    let x: Vec<f64> = (0..50).map(f64::from).collect();
    let y: Vec<f64> = x.iter().map(|v| v % 5.0).collect();
    let data =
        || Data::from_table(DataTable::from_columns(vec![Column::f64("x", x.clone()), Column::f64("a", y.clone()), Column::f64("b", y.clone())]).unwrap());
    let mut c = LineChart::new(&Options::new()).unwrap();
    c.set_data(data()).unwrap();
    let g = draw(&mut c, 60, 16, 0.0);
    let p = plot(&c);
    let inks: Vec<u8> = (p.y..p.y + p.h)
        .flat_map(|r| (p.x..p.x + p.w).map(move |col| (col, r)))
        .filter(|&(col, r)| is_dots(g.glyph(col, r)))
        .map(|(col, r)| g.ink(col, r))
        .collect();
    assert!(!inks.is_empty() && inks.iter().all(|&i| i == palette::series(1)), "b is drawn over a");
    c.set_view(&ViewPatch::from_json(r#"{"selection":{"points":[{"series":"a","index":3}]}}"#).unwrap());
    let g = draw(&mut c, 60, 16, 0.0);
    let raised = (p.y..p.y + p.h)
        .flat_map(|r| (p.x..p.x + p.w).map(move |col| (col, r)))
        .filter(|&(col, r)| is_dots(g.glyph(col, r)))
        .all(|(col, r)| g.ink(col, r) == palette::series(0));
    assert!(raised, "a selected series is raised");
}

/// The brute-force snap: the first row of least distance to the dot
/// column's centre among the shown series.
fn brute(c: &LineChart, target: f64) -> Option<f64> {
    let mut best: Option<f64> = None;
    for s in 0..c.data.series.len() {
        for &x in c.data.xs(s).iter().filter(|x| x.is_finite()) {
            if best.is_none_or(|b| (x - target).abs() < (b - target).abs()) {
                best = Some(x);
            }
        }
    }
    best
}

/// REQ-HIT-010, REQ-HIT-020: every plot cell snaps to the datum a linear
/// scan finds nearest its dot column, that datum lies in the cell's column
/// when the column holds data, and a column of several data carries their
/// aggregate.
#[test]
fn hits_match_brute_force() {
    for fixture in ["sine-1k", "two", "one", "multi-4", "duplicate-x", "nan-gaps", "labels-wide", "timestamps-dst"] {
        let mut c = chart(fixture, "");
        draw(&mut c, 80, 24, 0.0);
        let geom = c.geom.as_ref().unwrap().1.clone();
        let p = geom.plot;
        for col in p.x..p.x + p.w {
            for frac in [0.25, 0.75] {
                let at = CellPos::new(col as f64 + frac, (p.y + p.h / 2) as f64 + 0.5);
                let k = ((at.x - p.x as f64) * 2.0).floor() as i64;
                let hit = c.hit(at).unwrap();
                assert_eq!(Some(hit.x), brute(&c, geom.x.dot_center(k)), "{fixture} col {col}");
                if let Some(dot) = Edges::new(&geom.x).column(hit.x) {
                    if dot as i64 == k {
                        assert_eq!(hit.col, col, "{fixture}: the datum is drawn in the cell");
                    }
                }
            }
        }
    }
    let mut c = chart("random-walk-1m", "");
    draw(&mut c, 80, 24, 0.0);
    let hit = c.hit(CellPos::new(40.5, 10.5)).unwrap();
    let agg = hit.aggregate.unwrap();
    assert!(agg.count > 5000 && agg.min <= hit.y && hit.y <= agg.max, "{agg:?}");
}

/// REQ-LINE-040: the readout names the datum whose x is nearest the cursor's
/// x, not the dot column's least or greatest.
#[test]
fn readout_names_the_nearest_x() {
    // Ten data a dot column; a spike at row 503, next to the column's middle.
    let x: Vec<f64> = (0..1000).map(f64::from).collect();
    let y: Vec<f64> = (0..1000).map(|i| if i == 503 { 900.0 } else { (i % 10) as f64 }).collect();
    let mut c = LineChart::new(&Options::new()).unwrap();
    c.set_data(xy(x, y.clone())).unwrap();
    draw(&mut c, 60, 20, 0.0);
    let geom = c.geom.as_ref().unwrap().1.clone();
    let k = Edges::new(&geom.x).column(503.0).unwrap();
    let at = CellPos::new(geom.plot.x as f64 + (k as f64 + 0.5) / 2.0, geom.plot.y as f64 + 10.5);
    let ch = c.input(&mv(0.0, at.x, at.y));
    let cur = c.view().cursor.clone().unwrap();
    let target = geom.x.dot_center(k as i64);
    let nearest = (0..1000).min_by(|&a, &b| (a as f64 - target).abs().partial_cmp(&(b as f64 - target).abs()).unwrap()).unwrap();
    assert_eq!(cur.index, Some(nearest as u64));
    assert_eq!(cur.y, Some(y[nearest]));
    let Some(Event::Hover(Some(h))) = ch.events.first() else { panic!("{:?}", ch.events) };
    assert_eq!((h.index, h.aggregate.unwrap().max), (nearest as u64, 900.0));
    let g = draw(&mut c, 60, 20, 0.0);
    let text = g.text();
    let row = text.lines().nth(geom.plot.y).unwrap();
    assert!(row.contains(&format!("y {}", y[nearest])), "{row}");
}

/// REQ-LINE-040, REQ-FMT-030: the readout names x by its column and the
/// value by its series, and writes each in its axis's tick format with the
/// digits that tell the datum from its neighbours: never d3's twelve
/// significant digits, and grouped as the axis is.
#[test]
fn readout_names_its_values_in_the_axis_format() {
    for fixture in ["sine-1k", "tiny-range", "nan-gaps", "steps-10k", "multi-4", "random-walk-100k"] {
        let mut c = chart(fixture, "");
        draw(&mut c, 80, 24, 0.0);
        let p = plot(&c);
        c.input(&mv(0.0, (p.x + p.w / 2) as f64 + 0.3, (p.y + p.h / 2) as f64 + 0.5));
        let cur = c.view().cursor.clone().unwrap();
        let g = draw(&mut c, 80, 24, 0.0);
        let row = g.text().lines().nth(p.y).unwrap().trim_end().to_string();
        let (series, y) = (cur.series.clone().unwrap(), cur.y.unwrap());
        let tail = format!("  {series} ");
        let (head, y_text) = row.rsplit_once(&tail).unwrap_or_else(|| panic!("{fixture}: {row}"));
        let x_text = head.rsplit_once(&format!("{} ", c.data.x_name)).unwrap_or_else(|| panic!("{fixture}: no x name in {row}")).1;
        let s = c.data.find(&series).unwrap();
        let i = c.data.pos(s, cur.index.unwrap() as usize).unwrap();
        let ys = c.data.ys(s);
        let back: f64 = y_text.parse().unwrap_or_else(|_| panic!("{fixture}: {y_text}"));
        for n in [i.checked_sub(1), Some(i + 1)].into_iter().flatten().filter_map(|j| ys.get(j)).filter(|&&n| n != y) {
            assert!((back - y).abs() < (back - n).abs(), "{fixture}: {y_text} for {y} beside {n}");
        }
        // tiny-range's data lie 10⁻¹⁵ apart near 1, which takes 16 digits.
        let significant = y_text.trim_start_matches(['-', '0', '.']).chars().filter(char::is_ascii_digit).count();
        assert!(significant < 12 || fixture == "tiny-range", "{fixture}: {y_text}");
        if c.data.kind == Kind::Linear {
            assert_eq!(x_text.replace(',', "").parse::<f64>().ok(), Some(cur.x.unwrap()), "{fixture}: {row}");
            assert_eq!(x_text.contains(','), cur.x.unwrap().abs() >= 1000.0, "{fixture}: grouped as the axis");
        } else {
            assert!(x_text.starts_with("2026-10-1"), "{fixture}: {x_text}");
        }
    }
}

/// REQ-LAYOUT-040, contract rule 5: a nice y domain the data fills less than
/// `NICE_FILL` of gives way to the data's, so at `META.min` and other small
/// sizes the data fills the plot's height and the axis still labels ticks;
/// at 80 × 24 the nice domain stays.
#[test]
fn small_plots_fill_their_height() {
    // The dot rows a braille cell lights, top to bottom.
    let rows_of = |g: u32| -> Vec<usize> {
        let bits = if is_dots(g) { g - 0x2800 } else { 0 };
        [(0, 3), (1, 4), (2, 5), (6, 7)].iter().enumerate().filter(|(_, (a, b))| bits & (1 << a | 1 << b) != 0).map(|(r, _)| r).collect()
    };
    for fixture in ["multi-4", "two", "labels-wide", "random-walk-100k", "timestamps-dst", "nan-gaps", "unsorted-x", "inf", "zero-crossing"] {
        for (cols, rows) in [(23, 6), (30, 8), (40, 12), (80, 24), (160, 48)] {
            let mut c = chart(fixture, "");
            let g = draw(&mut c, cols, rows, 0.0);
            let geom = &c.geom.as_ref().unwrap().1;
            let [lo, hi] = c.data.y_extent(|_| true, None).unwrap();
            let span = geom.y.hi() - geom.y.lo();
            assert!(hi - lo >= NICE_FILL * span, "{fixture} at {cols}×{rows}: data {lo}..{hi} in {}..{}", geom.y.lo(), geom.y.hi());
            let p = geom.plot;
            let mut lit = Vec::new();
            for r in p.y..p.y + p.h {
                for x in p.x..p.x + p.w {
                    lit.extend(rows_of(g.glyph(x, r)).into_iter().map(|d| (r - p.y) * 4 + d));
                }
            }
            let (top, bottom) = (lit.iter().min().unwrap(), lit.iter().max().unwrap());
            assert!((bottom - top + 1) as f64 >= NICE_FILL * (4 * p.h) as f64 - 1.0, "{fixture} at {cols}×{rows}: dots {top}..{bottom} of {}", 4 * p.h);
        }
    }
    let labels = |c: &LineChart| c.geom.as_ref().unwrap().1.layout.panes[0].left.as_ref().unwrap().ticks.iter().filter(|t| t.label.is_some()).count();
    let mut c = chart("multi-4", "");
    draw(&mut c, 23, 6, 0.0);
    assert!(labels(&c) >= 2, "two y labels at META.min");
    draw(&mut c, 80, 24, 0.0);
    let y = c.geom.as_ref().unwrap().1.y;
    assert_eq!((y.lo(), y.hi()), (-10.0, 12.0), "niced at 80 × 24");
}

/// REQ-LAYOUT-040: every accepted fixture draws at sizes from 1 × 1 to
/// 320 × 120 in every mode and env without a panic, with `rows` lines of
/// `cols` cells.
#[test]
fn every_size_draws_without_panic() {
    let widths = [1, 2, 3, 5, 8, 15, 16, 17, 22, 23, 24, 40, 79, 80, 160, 320];
    let heights = [1, 2, 3, 4, 5, 6, 9, 10, 11, 12, 24, 60, 120];
    let envs = [Env::plain(), Env { mono: true, ..Env::plain() }, Env { paper: true, ..Env::plain() }];
    for f in FIXTURES.iter().filter(|f| f.rows < 1_000_000) {
        let data = f.generate();
        for mode in ["line", "step", "area"] {
            let mut c = LineChart::new(&Options::parse(&format!(r#"{{"mode":"{mode}","title":"A title"}}"#)).unwrap()).unwrap();
            if c.set_data(data.clone()).is_err() {
                assert!(matches!(f.name, "matrix-64"), "{} is refused", f.name);
                continue;
            }
            for (i, &w) in widths.iter().enumerate() {
                for &h in &heights {
                    let mut g = Grid::new(w, h);
                    c.frame(0.0, &envs[i % 3], &mut g);
                    let text = g.text();
                    assert_eq!(text.lines().count(), h, "{} {w}×{h}", f.name);
                    c.input(&mv(0.0, w as f64 / 2.0, h as f64 / 2.0));
                    let _ = c.hit(CellPos::new(w as f64 / 2.0, h as f64 / 2.0));
                }
            }
        }
    }
    for name in ["random-walk-1m", "ohlcv-ticks-1m"] {
        let mut c = chart(name, "");
        for (w, h) in [(1, 1), (23, 6), (320, 120)] {
            draw(&mut c, w, h, 0.0);
        }
    }
}

/// Whether a code point is in the bundled font's ranges (REQ-FONT-010).
fn in_font(c: u32) -> bool {
    matches!(c, 0x20..=0x7E | 0xA0..=0xBF | 0xD7 | 0xF7 | 0x2007..=0x200A | 0x2010..=0x2015 | 0x2018..=0x201F | 0x2026 | 0x202F | 0x2030 | 0x2032..=0x2033
        | 0x2070..=0x209F | 0x20A0..=0x20C0 | 0x2116 | 0x2122 | 0x2190..=0x21FF | 0x2212 | 0x221E | 0x2248 | 0x2260 | 0x2264..=0x2265 | 0x2500..=0x25FF
        | 0x2800..=0x28FF | 0x1CD00..=0x1CDE5 | 0x1CEA0..=0x1CEAF | 0x1FB00..=0x1FBFF)
}

/// REQ-CHAR-010, REQ-CHAR-020: every glyph outside label text from the data
/// is in the font; a wide tail follows only a wide glyph.
#[test]
fn glyphs_stay_in_the_font() {
    for f in FIXTURES.iter().filter(|f| f.rows < 1_000_000 && f.name != "matrix-64") {
        let mut c = chart(f.name, r#"{"title":"Title"}"#);
        let labels: String = c.data.series.iter().map(|s| s.name.clone()).collect::<Vec<_>>().join("") + &c.data.x_name;
        for env in [Env::plain(), Env { mono: true, ..Env::plain() }] {
            for (w, h) in [(80, 24), (320, 120), (23, 6)] {
                c.input(&mv(0.0, w as f64 / 2.0, h as f64 / 2.0));
                let mut g = Grid::new(w, h);
                c.frame(0.0, &env, &mut g);
                for row in 0..h {
                    for col in 0..w {
                        let gl = g.glyph(col, row);
                        if gl == WIDE_TAIL {
                            assert!(col > 0 && unicode_width::UnicodeWidthChar::width(char::from_u32(g.glyph(col - 1, row)).unwrap()) == Some(2));
                            continue;
                        }
                        let ch = char::from_u32(gl).unwrap();
                        assert!(in_font(gl) || labels.contains(ch), "{}: U+{gl:04X} at {col},{row}", f.name);
                    }
                }
            }
        }
    }
}

/// REQ-LINE-020: a frame over 1,000 visible points of 1,000,000 held takes
/// under a tenth of a frame over all of them (medians of 7 frames, each
/// drawing its base, natively).
#[test]
fn visible_slice_frames_are_fast() {
    let ms = |c: &mut LineChart, views: [&str; 2], cols: usize, rows: usize| {
        // The fastest of 15 frames after a warm-up: the work a frame does, not
        // the noise of a machine loaded by the other tests running beside it.
        let mut fastest = f64::INFINITY;
        let mut g = Grid::new(cols, rows);
        for i in 0..16 {
            c.set_view(&ViewPatch::from_json(views[i % 2]).unwrap());
            let start = Instant::now();
            c.frame(0.0, &Env::plain(), &mut g);
            let t = start.elapsed().as_secs_f64() * 1000.0;
            if i > 0 && t < fastest {
                fastest = t;
            }
        }
        fastest
    };
    let mut c = chart("random-walk-1m", "");
    let t0 = crate::fixtures::T0_MS;
    // 1,000 rows a second apart, nudged so every frame scans again.
    let slice = [
        format!(r#"{{"x":{{"domain":[{},{}]}}}}"#, t0 + 400_000_000.0, t0 + 400_999_000.0),
        format!(r#"{{"x":{{"domain":[{},{}]}}}}"#, t0 + 400_000_001.0, t0 + 400_999_001.0),
    ];
    let whole = [format!(r#"{{"x":{{"domain":[{},{}]}}}}"#, t0 - 1.0, t0 + 1e9), format!(r#"{{"x":{{"domain":[{},{}]}}}}"#, t0 - 2.0, t0 + 1e9)];
    for (cols, rows) in [(80, 24), (320, 120)] {
        // Over the bound, timed twice more and the best counts, as the time
        // budgets are (REQ-CONTRACT-050).
        let (mut part, mut all) = (f64::INFINITY, 0.0_f64);
        for _ in 0..3 {
            let p = ms(&mut c, [&slice[0], &slice[1]], cols, rows);
            let a = ms(&mut c, [&whole[0], &whole[1]], cols, rows);
            if p / a < part / all.max(f64::MIN_POSITIVE) {
                (part, all) = (p, a);
            }
            if part < all / 10.0 {
                break;
            }
        }
        println!("line {cols}×{rows}: 1,000 of 1,000,000 visible {part:.3} ms, all visible {all:.3} ms (fastest of 15)");
        assert!(part < all / 10.0, "{cols}×{rows}: {part} ms against {all} ms");
    }
}

/// REQ-STREAM-060, contract rule 15: data replaced draws what a fresh chart
/// given the same data, options and view draws.
#[test]
fn replacing_data_equals_a_fresh_chart() {
    let mut c = chart("multi-4", r#"{"mode":"area"}"#);
    draw(&mut c, 80, 24, 0.0);
    c.set_view(&ViewPatch::from_json(r#"{"x":{"domain":[100,600]}}"#).unwrap());
    draw(&mut c, 80, 24, 0.0);
    c.set_data(generate("sine-1k").unwrap()).unwrap();
    let mut fresh = chart("sine-1k", r#"{"mode":"area"}"#);
    fresh.set_view(&ViewPatch::from_json(r#"{"x":{"domain":[100,600]}}"#).unwrap());
    assert_eq!(draw(&mut c, 80, 24, 1.0), draw(&mut fresh, 80, 24, 1.0));
    assert_eq!(c.view(), fresh.view());
}

/// Contract rules 8 and 9: two charts draw the same; one asked out of order,
/// with view, hit and repeated frames between, draws what a fresh one does.
#[test]
fn frames_are_pure_in_any_order() {
    let opts = r#"{"motion":true}"#;
    let mut a = chart("sine-1k", opts);
    let mut b = chart("sine-1k", opts);
    for c in [&mut a, &mut b] {
        draw(c, 80, 24, 0.0);
        c.input(&key(0.5, Key::Plus, Mods::NONE));
        c.input(&key(1.0, Key::Zero, Mods::NONE));
    }
    assert!(a.view().motion.is_some(), "a reset eases with motion on");
    let order = [6.0, 1.0, 2.5, 0.0, 1.1];
    let mut frames = Vec::new();
    for &t in &order {
        frames.push(draw(&mut a, 80, 24, t));
        let _ = a.view();
        let _ = a.hit(CellPos::new(40.5, 10.5));
        assert_eq!(draw(&mut a, 80, 24, t), frames[frames.len() - 1]);
    }
    for (i, &t) in order.iter().enumerate() {
        assert_eq!(draw(&mut b, 80, 24, t), frames[i], "t = {t}");
    }
    assert_ne!(frames[3], frames[4], "the motion moves the view between t = 1 and 1.25");
    assert_eq!(frames[0], frames[2], "after the motion, the reset view");
}

/// REQ-DATA-080: `{"t": […], "close": […]}` draws with no configuration, x a
/// time scale.
#[test]
#[cfg(feature = "text")]
fn columns_draw_with_no_configuration() {
    let data = Data::from_json(r#"{"t":["2026-10-18T00:00:00Z","2026-10-18T01:00:00Z","2026-10-18T02:00:00Z"],"close":[10,12,11]}"#).unwrap();
    let mut c = LineChart::new(&Options::new()).unwrap();
    let ok = c.set_data(data).unwrap();
    assert!(ok.warnings.is_empty() && ok.rows == 3 && ok.version == 1);
    let g = draw(&mut c, 60, 12, 0.0);
    assert!(g.glyphs().iter().any(|&c| is_dots(c)));
    assert!(g.text().contains("01 AM") || g.text().contains("01:00") || g.text().contains("Oct"), "{}", g.text());
    let mut bad = LineChart::new(&Options::new()).unwrap();
    let err = bad.set_data(generate("matrix-64").unwrap()).unwrap_err();
    assert!(err.to_string().contains("series"), "{err}");
}

/// REQ-MONO-010, REQ-MONO-040: in one ink the four series draw four dash
/// patterns and the legend draws each pattern's key beside its name; in
/// colour the lines are solid and so are the keys, each in its series' ink,
/// unless `patterns: "on"` dashes both.
#[test]
fn one_ink_tells_series_apart() {
    let names = ["alpha", "beta", "gamma", "delta"];
    let legend_of = |g: &Grid| g.text().lines().next().unwrap().to_string();
    let mut c = chart("multi-4", "");
    let mut g = Grid::new(100, 30);
    c.frame(0.0, &Env { mono: true, ..Env::plain() }, &mut g);
    let legend = legend_of(&g);
    for (k, name) in crate::mono::KEYS.iter().zip(names) {
        assert!(legend.contains(&format!("{k} {name}")), "{legend}");
    }
    let colour = draw(&mut c, 100, 30, 0.0);
    assert_eq!(legend_of(&colour), "─ alpha  ─ beta  ─ gamma  ─ delta".to_string() + &" ".repeat(67), "solid keys for solid lines");
    for (s, (_, span)) in c.geom.as_ref().unwrap().1.legend.iter().enumerate() {
        assert_eq!(colour.ink(span.x, span.y), palette::series(s), "the key takes its series' ink");
    }
    assert_ne!(colour, g, "colour draws solid lines");
    let mut on = chart("multi-4", r#"{"patterns":"on"}"#);
    let dashed = draw(&mut on, 100, 30, 0.0);
    assert_eq!(legend_of(&dashed), legend, "patterned lines in colour show their patterns' keys");
    let p = plot(&c);
    let col_key =
        |s: u8| (p.y..p.y + p.h).flat_map(|r| (p.x..p.x + p.w).map(move |x| (x, r))).filter(|&(x, r)| g.ink(x, r) == palette::series(s.into())).count();
    assert!((0..4).all(|s| col_key(s) > 0), "each series still has its ink index");
}

/// REQ-INPUT-040, REQ-EVENT-020: a hover fires `plot-hover` only when the
/// datum changes, and `null` when the pointer leaves the plot.
#[test]
fn hover_fires_only_on_a_new_datum() {
    let mut c = chart("two", "");
    draw(&mut c, 40, 12, 0.0);
    let p = plot(&c);
    let y = (p.y + p.h / 2) as f64;
    assert_eq!(names(&c.input(&mv(0.0, p.x as f64 + 0.2, y))), ["plot-hover"]);
    assert!(c.input(&mv(0.1, p.x as f64 + 1.2, y)).events.is_empty(), "the same datum");
    assert_eq!(names(&c.input(&mv(0.2, (p.x + p.w) as f64 - 0.5, y))), ["plot-hover"]);
    let leave = c.input(&mv(0.3, 0.0, 0.0));
    assert_eq!(leave.events, vec![Event::Hover(None)]);
    assert!(leave.overlay && !leave.base && leave.bits() == 5);
    assert!(c.input(&Input::Leave { t: 0.4 }).is_empty());
}

/// REQ-INPUT-040, REQ-EVENT-030: a drag pans with `final: false`, then once
/// `final: true` on release; Shift+drag brushes; the option `drag: "zoom"`
/// zooms to the brushed range.
#[test]
fn drags_pan_brush_and_zoom() {
    let down = |x: f64, y: f64, mods: Mods| Input::Down { t: 0.0, x, y, pointer: PointerType::Mouse, buttons: 1, mods };
    let held = |x: f64, y: f64| Input::Move { t: 0.0, x, y, pointer: PointerType::Mouse, buttons: 1, mods: Mods::NONE };
    let up = |x: f64, y: f64| Input::Up { t: 0.0, x, y, pointer: PointerType::Mouse, buttons: 0, mods: Mods::NONE };
    let mut c = chart("sine-1k", "");
    draw(&mut c, 80, 24, 0.0);
    let before = c.x_domain(c.view());
    c.input(&down(40.0, 10.0, Mods::NONE));
    let ch = c.input(&held(30.0, 10.0));
    assert!(ch.base);
    let Some(Event::View(v)) = ch.events.last() else { panic!() };
    assert_eq!((v.cause, v.done, v.source), (Cause::Pan, false, Source::Pointer));
    let after = c.view().x.domain.unwrap();
    assert!(after[0] > before[0] && (after[1] - after[0] - (before[1] - before[0])).abs() < 1e-9, "dragging left moves the view right");
    let ch = c.input(&up(30.0, 10.0));
    let finals: Vec<bool> = ch.events.iter().filter_map(|e| if let Event::View(v) = e { Some(v.done) } else { None }).collect();
    assert_eq!(finals, [true]);

    let mut c = chart("sine-1k", "");
    draw(&mut c, 80, 24, 0.0);
    c.input(&down(20.0, 10.0, Mods::SHIFT));
    assert_eq!(names(&c.input(&held(50.0, 10.0))), ["plot-brush"]);
    let ch = c.input(&up(50.0, 10.0));
    let Some(Event::Brush(b)) = ch.events.last() else { panic!() };
    assert!(b.done && b.count > 300 && b.x.is_some());

    let mut c = chart("sine-1k", r#"{"drag":"zoom"}"#);
    draw(&mut c, 80, 24, 0.0);
    c.input(&down(20.0, 10.0, Mods::NONE));
    c.input(&held(50.0, 10.0));
    let ch = c.input(&up(50.0, 10.0));
    let Some(Event::View(v)) = ch.events.last() else { panic!() };
    assert_eq!((v.cause, v.done), (Cause::BrushZoom, true));
    let d = c.view().x.domain.unwrap();
    assert!(d[1] - d[0] < 500.0);

    // A click selects and locks.
    let mut c = chart("sine-1k", "");
    draw(&mut c, 80, 24, 0.0);
    c.input(&down(40.0, 10.0, Mods::NONE));
    let ch = c.input(&up(40.2, 10.0));
    assert!(names(&ch).contains(&"plot-select") && names(&ch).contains(&"plot-cursor"));
    assert!(c.view().cursor.as_ref().unwrap().locked);
    assert!(c.input(&mv(0.0, 10.0, 10.0)).events.is_empty(), "a locked cursor stays");
}

/// REQ-INPUT-040, REQ-EVENT-030: the plain wheel scrolls the page; Ctrl or ⌘
/// with it zooms about the pointer, ending with `final: true` at a wheel of
/// no delta or 150 ms later.
#[test]
fn wheel_zooms_with_ctrl_and_ends_final() {
    let wheel = |t: f64, dy: f64, mods: Mods| Input::Wheel { t, x: 40.0, y: 10.0, dx: 0.0, dy, mods };
    let mut c = chart("sine-1k", "");
    draw(&mut c, 80, 24, 0.0);
    assert!(c.input(&wheel(0.0, -3.0, Mods::NONE)).is_empty(), "the page scrolls");
    let ch = c.input(&wheel(0.0, -3.0, Mods::CTRL));
    let Some(Event::View(v)) = ch.events.last() else { panic!() };
    assert_eq!((v.cause, v.done, v.source), (Cause::Zoom, false, Source::Wheel));
    assert!(c.input(&wheel(0.05, -3.0, Mods::META)).events.len() == 1);
    let ch = c.input(&wheel(0.1, 0.0, Mods::NONE));
    let Some(Event::View(v)) = ch.events.first() else { panic!() };
    assert!(v.done);
    c.input(&wheel(1.0, 2.0, Mods::CTRL));
    let ch = c.input(&mv(2.0, 40.0, 10.0));
    assert!(matches!(ch.events.first(), Some(Event::View(v)) if v.done), "a later input ends it");
    let mut z = chart("sine-1k", r#"{"wheel":"zoom"}"#);
    draw(&mut z, 80, 24, 0.0);
    assert!(!z.input(&wheel(0.0, -3.0, Mods::NONE)).is_empty());
}

/// REQ-KEY-010, REQ-A11Y-030: the keys of the plan's table step, pan, zoom,
/// reset, select, brush, toggle and clear.
#[test]
fn keys_do_what_the_table_says() {
    let mut c = chart("multi-4", "");
    draw(&mut c, 80, 24, 0.0);
    let k = |c: &mut LineChart, key_: Key, mods: Mods| c.input(&key(0.0, key_, mods));
    assert_eq!(names(&k(&mut c, Key::Right, Mods::NONE)), ["plot-hover"]);
    assert_eq!(c.view().cursor.as_ref().unwrap().index, Some(0), "the first datum in view");
    k(&mut c, Key::Right, Mods::NONE);
    k(&mut c, Key::L, Mods::NONE);
    assert_eq!(c.view().cursor.as_ref().unwrap().index, Some(2));
    k(&mut c, Key::Left, Mods::NONE);
    assert_eq!(c.view().cursor.as_ref().unwrap().index, Some(1));
    k(&mut c, Key::PageDown, Mods::NONE);
    assert!(c.view().cursor.as_ref().unwrap().index.unwrap() > 50);
    k(&mut c, Key::End, Mods::NONE);
    assert_eq!(c.view().cursor.as_ref().unwrap().index, Some(999));
    assert!(c.view().x.follow, "End follows");
    let ch = k(&mut c, Key::Plus, Mods::NONE);
    assert!(ch.base && matches!(ch.events.last(), Some(Event::View(v)) if v.cause == Cause::Zoom && v.done));
    let zoomed = c.view().x.domain.unwrap();
    assert!(zoomed[1] - zoomed[0] < 999.0 && zoomed[1] >= 990.0, "zoomed about the cursor: {zoomed:?}");
    k(&mut c, Key::Left, Mods::SHIFT);
    let panned = c.view().x.domain.unwrap();
    assert!(panned[0] < zoomed[0]);
    k(&mut c, Key::Home, Mods::NONE);
    let home = c.view().cursor.as_ref().unwrap().x.unwrap();
    assert!(home >= panned[0] && home <= panned[0] + 1.0);
    // Stepping past the edge pans.
    k(&mut c, Key::Left, Mods::NONE);
    let ch = k(&mut c, Key::Left, Mods::NONE);
    assert!(names(&ch).contains(&"plot-view"), "{:?}", names(&ch));
    assert_eq!(names(&k(&mut c, Key::Zero, Mods::NONE)), ["plot-view"]);
    assert_eq!(c.view().x.domain, None);
    let ch = k(&mut c, Key::Enter, Mods::NONE);
    assert_eq!(names(&ch), ["plot-select", "plot-cursor"]);
    assert!(c.view().cursor.as_ref().unwrap().locked);
    assert_eq!(c.view().selection.as_ref().unwrap().points.len(), 1);
    assert_eq!(names(&k(&mut c, Key::Right, Mods::NONE)), ["plot-hover", "plot-cursor"], "a locked cursor steps");
    assert_eq!(names(&k(&mut c, Key::Enter, Mods::SHIFT)), ["plot-brush"]);
    k(&mut c, Key::Right, Mods::NONE);
    k(&mut c, Key::Right, Mods::NONE);
    let ch = k(&mut c, Key::Enter, Mods::SHIFT);
    let Some(Event::Brush(b)) = ch.events.last() else { panic!() };
    assert!(b.done && b.count >= 3);
    let ch = k(&mut c, Key::Digit(2), Mods::NONE);
    let Some(Event::Legend(l)) = ch.events.last() else { panic!() };
    assert_eq!((l.series.as_str(), l.visible, l.source), ("beta", false, Source::Keyboard));
    assert!(ch.base);
    assert_eq!(c.view().hidden, ["beta"]);
    let ch = k(&mut c, Key::Escape, Mods::NONE);
    assert_eq!(names(&ch), ["plot-brush", "plot-cursor", "plot-hover"]);
    assert!(c.view().cursor.is_none() && c.view().selection.as_ref().unwrap().x.is_none());
    assert!(k(&mut c, Key::Escape, Mods::NONE).is_empty(), "a second Escape is the shell's blur");
    assert!(k(&mut c, Key::Space, Mods::NONE).is_empty() && k(&mut c, Key::Question, Mods::NONE).is_empty());
}

/// REQ-TOUCH-010: one finger scrubs, two pan, a pinch zooms about its
/// centre, a double tap resets, a long press brushes, a tap selects.
#[test]
fn touch_gestures() {
    let touch = |kind: u32, x: f64, b: u32| Input::decode(kind, x, 10.0, 0.0, 0.0, b + 512, 0, 0.0).unwrap();
    let mut c = chart("sine-1k", "");
    draw(&mut c, 80, 24, 0.0);
    let ch = c.input(&touch(Input::DOWN, 20.0, 1));
    assert!(matches!(ch.events.as_slice(), [Event::Hover(Some(h))] if h.source == Source::Touch));
    assert_eq!(names(&c.input(&touch(Input::MOVE, 40.0, 1))), ["plot-hover"], "scrubbing moves the cursor");
    assert_eq!(c.view().x.domain, None, "and does not pan");
    c.input(&touch(Input::UP, 40.0, 0));
    let ch = c.input(&Input::Pan { t: 0.0, dx: -5.0, dy: 0.0, end: false, velocity: 0.0 });
    assert!(matches!(ch.events.last(), Some(Event::View(v)) if v.cause == Cause::Pan && !v.done && v.source == Source::Touch));
    let ch = c.input(&Input::Pan { t: 0.1, dx: -1.0, dy: 0.0, end: true, velocity: -20.0 });
    assert!(matches!(ch.events.last(), Some(Event::View(v)) if v.done));
    assert!(c.view().motion.is_none(), "no kinetic motion with motion off");
    let before = c.view().x.domain.unwrap();
    let ch = c.input(&Input::Pinch { t: 0.2, x: 40.0, y: 10.0, scale: 2.0, end: true });
    assert!(matches!(ch.events.last(), Some(Event::View(v)) if v.cause == Cause::Zoom && v.done));
    let after = c.view().x.domain.unwrap();
    assert!(((after[1] - after[0]) * 2.0 - (before[1] - before[0])).abs() < 1e-6);
    let ch = c.input(&Input::Tap { t: 0.3, x: 40.0, y: 10.0, count: 2, pointer: PointerType::Touch });
    assert!(matches!(ch.events.last(), Some(Event::View(v)) if v.cause == Cause::Reset && v.source == Source::Touch));
    assert_eq!(c.view().x.domain, None);
    // A mouse's double-click resets as the pointer (REQ-INPUT-020: the tap carries its pointer type).
    c.set_view(&ViewPatch::from_json(r#"{"x":{"domain":[100,400]}}"#).unwrap());
    let ch = c.input(&Input::decode(Input::TAP, 40.0, 10.0, 2.0, 0.0, 0, 0, 0.35).unwrap());
    assert!(matches!(ch.events.last(), Some(Event::View(v)) if v.cause == Cause::Reset && v.source == Source::Pointer));
    let ch = c.input(&Input::Press { t: 0.4, x: 20.0, y: 10.0 });
    assert_eq!(names(&ch), ["plot-brush"]);
    c.input(&touch(Input::MOVE, 50.0, 1));
    let ch = c.input(&touch(Input::UP, 50.0, 0));
    assert!(matches!(ch.events.last(), Some(Event::Brush(b)) if b.done && b.count > 0));
    let ch = c.input(&Input::Tap { t: 0.5, x: 30.0, y: 10.0, count: 1, pointer: PointerType::Touch });
    assert!(names(&ch).contains(&"plot-select"));
    let mut k = chart("sine-1k", r#"{"motion":true}"#);
    draw(&mut k, 80, 24, 0.0);
    k.input(&Input::Pan { t: 1.0, dx: -1.0, dy: 0.0, end: true, velocity: -20.0 });
    let m = k.view().motion.unwrap();
    assert_eq!((m.kind, m.t0), (crate::view::MotionKind::Kinetic, 1.0));
    assert_eq!(k.view().x.domain, Some(m.to), "the view holds where the motion ends");
}

/// REQ-OPT-010: every option the chart read through frames and inputs has a
/// default in its meta.
#[test]
fn every_option_read_has_a_default() {
    let mut c = chart("multi-4", r#"{"mode":"step","unknown":1}"#);
    draw(&mut c, 80, 24, 0.0);
    c.input(&mv(0.0, 30.0, 10.0));
    c.input(&key(0.0, Key::Zero, Mods::NONE));
    let _ = c.table(10);
    assert!(c.options().read().len() >= 10);
    assert_eq!(c.options().read_without_default(), Vec::<String>::new());
}

/// REQ-CHART-020, REQ-LAYOUT-070: the meta serialises with every field of
/// meta.schema.json and keeps its rules.
#[test]
fn meta_has_the_schema_fields() {
    let v: serde_json::Value = serde_json::from_str(&META.to_json()).unwrap();
    let keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
    for k in ["slug", "name", "note", "family", "modes", "shapes", "options", "min", "drop", "mono", "charset", "budget"] {
        assert!(keys.contains(&k), "{k}");
    }
    assert_eq!(keys.len(), 12);
    assert_eq!(v["options"]["mode"], "line");
    assert_eq!(v["min"], serde_json::json!({"cols": 23, "rows": 6}));
    assert_eq!(v["drop"], serde_json::json!(["title", "legend", "axes"]));
    assert_eq!(v["budget"]["frameMs"], 4.0);
    let note = META.note;
    assert!(!note.is_empty() && note.chars().count() <= 72 && !note.chars().any(char::is_uppercase));
}

/// REQ-A11Y-020, REQ-A11Y-030: the table holds the view's data within its
/// row limit, the summary names the series and ranges, and the value text
/// gives the cursor's x and each series' value, four then `and N more`.
#[test]
fn table_and_text() {
    let mut c = chart("random-walk-100k", "");
    let t = c.table(500);
    assert!(t.rows.len() <= 500 && t.total == 100_000);
    assert_eq!(t.columns, ["t", "y"]);
    // The shell adds the count; the caption holds none (REQ-A11Y-020).
    assert_eq!(t.caption, "line chart, each bucket's least and greatest kept");
    assert!(!t.caption.chars().any(|c| c.is_ascii_digit()), "{}", t.caption);
    assert_eq!(chart("sine-1k", r#"{"title":"Sine"}"#).table(5000).caption, "Sine");
    assert!(t.summary.starts_with("line chart of 1 series (y); t from 2026-10-18"), "{}", t.summary);
    assert_eq!(t.value, None);
    assert!(c.table(0).rows.is_empty());
    let five = Data::from_table(DataTable::from_columns((0..6).map(|i| Column::f64(&format!("c{i}"), vec![1.0, 2.0, 3.0])).collect()).unwrap());
    let mut f = LineChart::new(&Options::new()).unwrap();
    f.set_data(five).unwrap();
    draw(&mut f, 40, 12, 0.0);
    f.input(&key(0.0, Key::Right, Mods::NONE));
    let v = f.table(10).value.unwrap();
    assert_eq!(v, "1, c1 1, c2 1, c3 1, c4 1, and 1 more");
    assert_eq!(f.table(10).rows, vec![vec!["1", "1", "1", "1", "1", "1"], vec!["2", "2", "2", "2", "2", "2"], vec!["3", "3", "3", "3", "3", "3"]]);
    c.input(&key(0.0, Key::Right, Mods::NONE));
    let json: serde_json::Value = serde_json::from_str(&c.table(3).to_json()).unwrap();
    assert_eq!(json["rows"].as_array().unwrap().len(), 2);
}

/// Contract rule 14's start, v0.6.0's part: appending rows extends the data,
/// drops late rows and replaces an equal x.
#[test]
fn append_extends_the_data() {
    let mut c = LineChart::new(&Options::new()).unwrap();
    c.set_data(xy(vec![0.0, 1.0, 2.0], vec![5.0, 6.0, 7.0])).unwrap();
    let a = c.append(xy(vec![1.5, 2.0, 3.0], vec![0.0, 8.0, 9.0])).unwrap();
    assert_eq!((a.rows, a.late, a.version), (4, 1, 2));
    assert_eq!(c.data.ys(0), &[5.0, 6.0, 8.0, 9.0]);
    let mut whole = LineChart::new(&Options::new()).unwrap();
    whole.set_data(xy(vec![0.0, 1.0, 2.0, 3.0], vec![5.0, 6.0, 8.0, 9.0])).unwrap();
    assert_eq!(draw(&mut c, 40, 12, 0.0), draw(&mut whole, 40, 12, 0.0));
}

/// REQ-OVERLAY-010: a hover over 1,000,000 points at 320 × 120, its input
/// and its frame, costs under 0.1 ms (median of 301, natively; the budget
/// holds in an optimised build).
#[test]
fn hover_is_cheap() {
    let mut c = chart("random-walk-1m", "");
    let mut g = Grid::new(320, 120);
    c.frame(0.0, &Env::plain(), &mut g);
    let mut times = Vec::new();
    for i in 0..301 {
        let start = Instant::now();
        c.input(&mv(0.0, 10.0 + (i % 290) as f64 + 0.3, 60.5));
        c.frame(0.0, &Env::plain(), &mut g);
        times.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = times[150];
    println!("hover at 320×120 over 1,000,000 points: {median:.4} ms (median of 301)");
    assert_eq!(c.base_draws(), 1);
    if !cfg!(debug_assertions) {
        assert!(median < 0.1, "{median} ms");
    }
}

/// Writes the meta, views and events of a scripted session to
/// `$GLYPHPLOT_SAMPLES` for checking against the schemas by hand.
#[test]
#[ignore]
fn write_schema_samples() {
    let Ok(dir) = std::env::var("GLYPHPLOT_SAMPLES") else { return };
    let dir = std::path::Path::new(&dir);
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join("meta.json"), META.to_json()).unwrap();
    let mut c = chart("multi-4", "");
    draw(&mut c, 80, 24, 0.0);
    let script = [
        mv(0.0, 30.0, 10.0),
        Input::Down { t: 0.1, x: 30.0, y: 10.0, pointer: PointerType::Mouse, buttons: 1, mods: Mods::NONE },
        Input::Move { t: 0.2, x: 20.0, y: 10.0, pointer: PointerType::Mouse, buttons: 1, mods: Mods::NONE },
        Input::Up { t: 0.3, x: 20.0, y: 10.0, pointer: PointerType::Mouse, buttons: 0, mods: Mods::NONE },
        Input::Down { t: 0.4, x: 40.0, y: 10.0, pointer: PointerType::Mouse, buttons: 1, mods: Mods::NONE },
        Input::Up { t: 0.45, x: 40.0, y: 10.0, pointer: PointerType::Mouse, buttons: 0, mods: Mods::NONE },
        key(0.5, Key::Enter, Mods::SHIFT),
        key(0.6, Key::Right, Mods::NONE),
        key(0.7, Key::Enter, Mods::SHIFT),
        key(0.8, Key::Digit(3), Mods::NONE),
        Input::Wheel { t: 0.9, x: 40.0, y: 10.0, dx: 0.0, dy: -2.0, mods: Mods::CTRL },
        Input::Wheel { t: 1.2, x: 40.0, y: 10.0, dx: 0.0, dy: 0.0, mods: Mods::NONE },
        key(1.3, Key::Escape, Mods::NONE),
        key(1.4, Key::End, Mods::NONE),
        Input::Leave { t: 1.5 },
        key(1.6, Key::Zero, Mods::NONE),
    ];
    let mut events = Vec::new();
    for i in &script {
        events.extend(c.input(i).events);
    }
    std::fs::write(dir.join("events.json"), crate::events::to_json(&events)).unwrap();
    std::fs::write(dir.join("view.json"), c.view().to_json()).unwrap();
    std::fs::write(dir.join("input.json"), serde_json::to_string(&script).unwrap()).unwrap();
    std::fs::write(dir.join("locale.json"), crate::locale::LocalePack::default().to_json()).unwrap();
    std::fs::write(dir.join("zone.json"), serde_json::to_string(&crate::locale::ZoneTable::default()).unwrap()).unwrap();
    println!("{} events", events.len());
}

/// REQ-INPUT-040, REQ-TOUCH-010, REQ-EVENT-010 (`plot-legend`): a click or a
/// tap on a legend entry toggles its series once: a mouse's on its `up`, the
/// `tap` the shell sends after a still click toggling nothing more; a
/// finger's on its `tap`, its down and up toggling nothing.
#[test]
fn legend_clicks_toggle() {
    let mut c = chart("multi-4", "");
    draw(&mut c, 100, 30, 0.0);
    let (_, span) = c.geom.as_ref().unwrap().1.legend[1];
    let (x, y) = (span.x as f64 + 3.5, span.y as f64 + 0.5);
    c.input(&Input::Down { t: 0.0, x, y, pointer: PointerType::Mouse, buttons: 1, mods: Mods::NONE });
    let ch = c.input(&Input::Up { t: 0.0, x, y, pointer: PointerType::Mouse, buttons: 0, mods: Mods::NONE });
    assert!(matches!(ch.events.as_slice(), [Event::Legend(l)] if l.series == "beta" && !l.visible));
    for pointer in [PointerType::Mouse, PointerType::Pen] {
        let ch = c.input(&Input::Tap { t: 0.0, x, y, count: 1, pointer });
        assert!(ch.events.is_empty() && !ch.base, "the click's tap toggles nothing more ({pointer:?})");
    }
    assert_eq!(c.view().hidden, ["beta"]);
    let g = draw(&mut c, 100, 30, 0.0);
    assert_eq!(g.ink(span.x + 2, span.y), palette::MUTED, "a hidden series' name is muted");
    c.input(&Input::Down { t: 0.0, x, y, pointer: PointerType::Touch, buttons: 1, mods: Mods::NONE });
    assert!(c.input(&Input::Up { t: 0.0, x, y, pointer: PointerType::Touch, buttons: 0, mods: Mods::NONE }).events.is_empty());
    let ch = c.input(&Input::Tap { t: 0.0, x, y, count: 1, pointer: PointerType::Touch });
    assert!(matches!(ch.events.as_slice(), [Event::Legend(l)] if l.visible && l.source == Source::Touch));
    assert!(c.view().hidden.is_empty());
}

/// Draws `c` at `cols × rows` in one ink.
fn draw_mono(c: &mut LineChart, cols: usize, rows: usize) -> Grid {
    let mut g = Grid::new(cols, rows);
    c.frame(0.0, &Env { mono: true, ..Env::plain() }, &mut g);
    g
}

/// Eight series of one shape, offset so their lines stay apart.
fn eight() -> Data {
    let xs: Vec<f64> = (0..200).map(f64::from).collect();
    let mut columns = vec![Column::f64("x", xs.clone())];
    for s in 0..8 {
        let ys = xs.iter().map(|&x| f64::from(s) * 10.0 + crate::fixtures::sin_turns(x / 50.0)).collect();
        columns.push(Column::f64(&format!("s{}", s + 1), ys));
    }
    Data::from_table(DataTable::from_columns(columns).unwrap())
}

/// The plot's cells of `c`'s last frame.
fn plot_cells(c: &LineChart) -> Vec<(usize, usize)> {
    let p = plot(c);
    (p.y..p.y + p.h).flat_map(|r| (p.x..p.x + p.w).map(move |x| (x, r))).collect()
}

/// REQ-MONO-020, REQ-MONO-040: in one ink, series 5 to 8 repeat the dash
/// patterns and add their markers `● ○ ■ □` in their inks, along their
/// lines, at most one a cell; series 1 to 4 have none; each legend key is the
/// series' dash or marker. In colour there are no markers.
#[test]
fn series_five_to_eight_add_markers() {
    let mut c = LineChart::new(&Options::parse(r#"{"legend":{"position":"outside"}}"#).unwrap()).unwrap();
    c.set_data(eight()).unwrap();
    let g = draw_mono(&mut c, 160, 40);
    let cells = plot_cells(&c);
    let with = |glyph: char| cells.iter().copied().filter(|&(x, r)| g.glyph(x, r) == glyph as u32).collect::<Vec<_>>();
    for s in 0..8 {
        match crate::mono::marker(s) {
            None => assert!(s < 4),
            Some(m) => {
                let at = with(m);
                assert!(at.len() >= 3, "series {} draws its marker {m} along its line: {at:?}", s + 1);
                assert!(at.iter().all(|&(x, r)| g.ink(x, r) == palette::series(s)), "in its ink");
            }
        }
    }
    for m in &crate::mono::MARKERS[4..] {
        assert!(with(*m).is_empty(), "series 9 to 12's markers are not drawn");
    }
    let legend = g.text().lines().next().unwrap().to_string();
    for s in 0..8 {
        assert!(legend.contains(&format!("{} s{}", crate::mono::key(s), s + 1)), "{legend}");
    }
    let colour = draw(&mut c, 160, 40, 0.0);
    assert!(crate::mono::MARKERS.iter().all(|&m| !colour.glyphs().contains(&(m as u32))), "no markers in colour");
}

/// REQ-MONO-030: in one ink, area series fill with `█ ▓ ▒ ░` by series where
/// the fill covers a cell whole, the line drawn over it in dots; the legend
/// keys are the fills.
#[test]
fn area_fills_by_series_in_one_ink() {
    let names = ["alpha", "beta", "gamma", "delta"];
    for (s, name) in names.iter().enumerate() {
        let hidden: Vec<&str> = names.iter().copied().filter(|n| n != name).collect();
        let mut c = chart("multi-4", r#"{"mode":"area"}"#);
        c.set_view(&ViewPatch::from_json(&serde_json::json!({ "hidden": hidden }).to_string()).unwrap());
        let g = draw_mono(&mut c, 100, 30);
        let cells = plot_cells(&c);
        let fill = crate::mono::fill(s) as u32;
        let filled = cells.iter().filter(|&&(x, r)| g.glyph(x, r) == fill).count();
        assert!(filled > 20, "{name} fills {filled} cells with U+{fill:04X}");
        let others = cells.iter().filter(|&&(x, r)| crate::mono::FILLS.iter().any(|&f| f as u32 == g.glyph(x, r) && f as u32 != fill)).count();
        assert_eq!(others, 0, "{name} draws no other fill");
        assert!(cells.iter().any(|&(x, r)| is_dots(g.glyph(x, r))), "{name}'s line stays in dots");
        let legend = g.text().lines().next().unwrap().to_string();
        assert!(legend.contains(&format!("{} {name}", crate::mono::fill_key(s))), "{legend}");
    }
    // In colour the area fills in dots, as before.
    let mut c = chart("sine-1k", r#"{"mode":"area"}"#);
    let g = draw(&mut c, 80, 24, 0.0);
    assert!(crate::mono::FILLS.iter().all(|&f| !g.glyphs().contains(&(f as u32))));
}

/// REQ-ANNOT-010 through the line chart's options: an `hline` with a label
/// on the y axis hides the tick label it overlaps; a `vline`, a `band` and
/// a `marker` draw over the data inside the plot; annotations that do not
/// parse are an error naming the option.
#[test]
fn annotations_draw_from_the_options() {
    let mut plain = chart("sine-1k", "");
    let before = draw(&mut plain, 80, 24, 0.0);
    let geom = &plain.geom.as_ref().unwrap().1;
    let y_axis = geom.layout.panes[0].left.as_ref().unwrap();
    let tick = y_axis.ticks.iter().filter(|t| t.label.is_some()).nth(1).unwrap().clone();
    let (lo, hi) = (geom.layout.x_domain[0], geom.layout.x_domain[1]);
    let mid = lo + (hi - lo) / 2.0;
    let opts = serde_json::json!({ "annotations": [
        { "type": "hline", "scale": "y", "value": tick.value, "label": "limit" },
        { "type": "vline", "value": mid },
        { "type": "band", "scale": "x", "from": lo, "to": lo + (hi - lo) / 10.0 },
        { "type": "marker", "x": mid, "y": 0.0, "glyph": "\u{25C6}" }
    ]});
    let mut c = chart("sine-1k", &opts.to_string());
    let g = draw(&mut c, 80, 24, 0.0);
    assert_ne!(g, before);
    let lay = &c.geom.as_ref().unwrap().1.layout;
    let p = lay.panes[0].plot;
    let left = lay.panes[0].left.as_ref().unwrap();
    assert!(left.ticks.iter().find(|t| t.value == tick.value).unwrap().label.is_none(), "the tick label under `limit` is hidden");
    let row = p.y + tick.cell;
    let line = g.text().lines().nth(row).unwrap().to_string();
    assert!(line.starts_with(&format!("{:>w$}", "limit", w = left.rect.w - 1)), "{line}");
    assert_eq!(g.ink(left.rect.x + left.rect.w - 2, row), palette::ACCENT);
    assert!(g.glyphs().contains(&0x25C6), "the marker");
    let band = lay.marks.iter().find_map(|m| if let crate::annotate::Mark::Shade(r) = m { Some(*r) } else { None }).unwrap();
    assert_eq!((band.x, band.y, band.h), (p.x, p.y, p.h));
    assert_eq!(g.back(band.x, band.y + 1), palette::RULE, "the band shades the back");
    let mono = draw_mono(&mut c, 80, 24);
    assert!(mono.glyphs().contains(&0x2591), "in one ink the band is ░");
    for bad in [r#"{"annotations":[{"type":"hline","value":1}]}"#, r#"{"annotations":{"type":"vline","value":1}}"#] {
        let e = LineChart::new(&Options::parse(bad).unwrap()).err().unwrap();
        assert!(e.to_string().starts_with("option `annotations`"), "{e}");
    }
}

/// REQ-LAYOUT-060 through the options: `legend.position` forces the legend
/// inside, outside or off; `auto` leaves it out when it fits neither, and
/// the series stay in the table.
#[test]
fn legend_position_option() {
    let at = |opts: &str, cols, rows| {
        let mut c = chart("multi-4", opts);
        draw(&mut c, cols, rows, 0.0);
        let g = &c.geom.as_ref().unwrap().1;
        (g.layout.legend, g.layout.panes[0].plot, g.legend.len())
    };
    let (legend, plot, n) = at("", 100, 30);
    assert_eq!((legend.map(|r| r.y), n), (Some(0), 4), "outside at 30 rows");
    assert!(plot.y > 0);
    let (legend, plot, _) = at(r#"{"legend":{"position":"inside"}}"#, 100, 30);
    assert_eq!(legend.map(|r| (r.x, r.y)), Some((plot.x, plot.y)));
    let (legend, _, n) = at(r#"{"legend":{"position":"none"}}"#, 100, 30);
    assert_eq!((legend, n), (None, 0));
    let (legend, _, n) = at("", 60, 11);
    assert_eq!((legend, n), (None, 0), "at 60 × 11 it fits neither: left out");
    let mut c = chart("multi-4", "");
    draw(&mut c, 60, 11, 0.0);
    assert_eq!(c.table(10).columns.len(), 5, "a legend left out stays in the table");
}

/// Prices held in cents, 10,000 to 20,000, with 18,240 at row 50, shown by
/// `scale: 0.01` in USD.
fn cents() -> Data {
    let y: Vec<f64> = (0..101).map(|i| if i == 50 { 18_240.0 } else { 10_000.0 + 100.0 * f64::from(i) }).collect();
    let mut price = Column::f64("price", y).unit("USD");
    price.scale = Some(0.01);
    Data::from_table(DataTable::from_columns(vec![Column::f64("x", (0..101).map(f64::from).collect()), price]).unwrap())
}

/// REQ-DATA-100: a column's `scale` and `unit` apply to the y ticks, the
/// readout and the table, and the stored values do not change: a readout
/// of stored 18240 at `scale: 0.01` shows `182.40`.
#[test]
fn display_transform_applies_to_ticks_readout_and_table() {
    let mut c = LineChart::new(&Options::new()).unwrap();
    c.set_data(cents()).unwrap();
    let g = draw(&mut c, 80, 24, 0.0);
    let y_labels: Vec<String> =
        c.geom.as_ref().unwrap().1.layout.panes[0].left.as_ref().unwrap().ticks.iter().filter_map(|t| t.label.as_ref()).map(|l| l.text.clone()).collect();
    assert!(y_labels.iter().any(|l| l == "200") && y_labels.iter().all(|l| !l.contains(",0")), "ticks in dollars: {y_labels:?}");
    assert!(g.text().contains("200┤"), "{}", g.text());
    let p = plot(&c);
    let y = c.geom.as_ref().unwrap().1.y;
    assert!(y.lo() <= 10_000.0 && y.hi() >= 20_000.0, "the plot draws stored values: {:?}", (y.lo(), y.hi()));
    c.set_view(&ViewPatch::from_json(r#"{"cursor":{"x":50,"y":18240,"series":"price","index":50}}"#).unwrap());
    let g = draw(&mut c, 80, 24, 0.0);
    assert!(g.text().contains("price 182.40 USD"), "{}", g.text().lines().nth(p.y).unwrap());
    assert_eq!(c.table(500).rows[50], ["50", "182.40 USD"]);
    assert_eq!(c.table(500).value.as_deref(), Some("50, price 182.40 USD"));
    assert_eq!(c.data.ys(0)[50], 18_240.0, "the stored value is unchanged");
}

/// A zone table of one offset from the epoch on.
fn fixed_zone(name: &str, offset: i32) -> crate::locale::ZoneTable {
    crate::locale::ZoneTable { name: name.into(), transitions: vec![crate::locale::Transition { at: 0, offset }], covering: [0, 2_000_000_000_000] }
}

/// REQ-TZ-030, REQ-TICK-070: the time axis ticks and labels in the env's
/// zone, as do the readout and the table, and a span under two days dates
/// each local day in the axis's context row; UTC stays the default.
#[test]
fn time_ticks_and_labels_follow_the_zone() {
    let start = crate::time::days_from_civil(2026, 3, 7) as f64 * crate::time::DAY;
    let n = 48;
    let t: Vec<f64> = (0..n).map(|i| start + f64::from(i) * crate::time::HOUR).collect();
    let y: Vec<f64> = (0..n).map(|i| (f64::from(i) / 5.0).sin()).collect();
    let data = || Data::from_table(DataTable::from_columns(vec![Column::timestamps_ms("t", t.clone()), Column::f64("y", y.clone())]).unwrap());
    let frame = |zone: &crate::locale::ZoneTable| {
        let mut c = LineChart::new(&Options::new()).unwrap();
        c.set_data(data()).unwrap();
        let mut g = Grid::new(80, 24);
        c.frame(0.0, &Env { zone, ..Env::plain() }, &mut g);
        let ticks: Vec<f64> = c.geom.as_ref().unwrap().1.layout.x_axis.as_ref().unwrap().ticks.iter().map(|t| t.value).collect();
        (g, ticks, c)
    };
    let (utc, utc_ticks, mut u) = frame(&crate::locale::ZoneTable::default());
    let mut plain = LineChart::new(&Options::new()).unwrap();
    plain.set_data(data()).unwrap();
    assert_eq!(draw(&mut plain, 80, 24, 0.0), utc, "UTC is the default");
    let (india, india_ticks, mut i) = frame(&fixed_zone("+05:30", 330));
    assert_ne!(utc, india, "a +05:30 table draws other ticks");
    let hour = crate::time::HOUR;
    assert!(utc_ticks.iter().all(|v| v % hour == 0.0));
    assert!(india_ticks.iter().all(|v| (v + 330.0 * 60_000.0) % hour == 0.0), "ticks on local hours: {india_ticks:?}");
    let last = utc.text().lines().last().unwrap().to_string();
    assert!(last.contains("Mar 07") || last.contains("Mar 7"), "the context row dates the first day: {last:?}");
    assert!(last.contains("Mar 8"), "and the next: {last:?}");
    // The readout and the table write times in the zone.
    let patch = ViewPatch::from_json(&format!(r#"{{"cursor":{{"x":{},"series":"y","index":0}}}}"#, start)).unwrap();
    u.set_view(&patch);
    i.set_view(&patch);
    assert!(u.table(10).rows[0][0].starts_with("2026-03-07 00:00"));
    assert!(i.table(10).rows[0][0].starts_with("2026-03-07 05:30"), "{:?}", i.table(10).rows[0]);
    let mut g = Grid::new(80, 24);
    i.frame(0.0, &Env { zone: &fixed_zone("+05:30", 330), ..Env::plain() }, &mut g);
    assert!(g.text().contains("t 2026-03-07 05:30:00"), "{}", g.text());
}
