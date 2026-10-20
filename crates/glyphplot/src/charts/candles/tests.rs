//! The candlestick chart's own tests: glyphs by spacing and their
//! snapshots, one ink, the crosshair and readout, up and down, the volume
//! pane, the index scale, row snapping, display transforms, shapes and the
//! caches. The contract (`tests/contract.rs`) checks the rules every chart
//! keeps.

use std::fmt::Write as _;
use std::path::PathBuf;

use super::*;
use crate::data::{Column, Role, Table as DataTable};
use crate::fixtures::generate;
use crate::input::{Mods, PointerType};
use crate::palette::{DOWN, UP};
use crate::view::{Patch, XPatch};

fn chart(fixture: &str, opts: &str) -> CandlesChart {
    let mut c = CandlesChart::new(&Options::parse(opts).unwrap()).unwrap();
    c.set_data(generate(fixture).unwrap()).unwrap();
    c
}

fn draw(c: &mut CandlesChart, cols: usize, rows: usize, env: &Env) -> Grid {
    let mut g = Grid::new(cols, rows);
    c.frame(0.0, env, &mut g);
    g
}

fn geom(c: &CandlesChart) -> &Geom {
    &c.geom.as_ref().expect("a frame was drawn").1
}

fn mv(x: f64, y: f64) -> Input {
    Input::Move { t: 0.0, x, y, pointer: PointerType::Mouse, buttons: 0, mods: Mods::NONE }
}

/// The view showing rows `a..a + n` of `c`'s data whole: from the midpoint
/// before row `a` to the midpoint after row `a + n − 1`, positions
/// `a − ½` to `a + n − ½` on the index scale.
fn rows_view(c: &CandlesChart, a: usize, n: usize) -> ViewPatch {
    let xs = c.data.xs();
    let d = [(xs[a - 1] + xs[a]) / 2.0, (xs[a + n - 1] + xs[a + n]) / 2.0];
    ViewPatch { x: Patch::Set(XPatch { domain: Patch::Set(d), follow: None }), ..ViewPatch::default() }
}

/// `ohlcv-days` at 80 × 24 in colour with the view holding as many daily
/// rows from row 200 as give `spacing` cells a candle, the plot's width
/// over the spacing (REQ-CANDLE-020).
fn at_spacing(spacing: usize, mode: &str) -> (CandlesChart, Grid) {
    let mut c = chart("ohlcv-days", &format!(r#"{{"mode":"{mode}"}}"#));
    let mut g = draw(&mut c, 80, 24, &Env::plain());
    for _ in 0..3 {
        let n = geom(&c).price.w / spacing;
        c.set_view(&rows_view(&c, 200, n));
        g = draw(&mut c, 80, 24, &Env::plain());
    }
    (c, g)
}

/// The glyphs drawn in the price plot.
fn plot_glyphs(g: &Grid, plot: Rect) -> Vec<(usize, usize, u32, u8)> {
    let mut out = Vec::new();
    for y in plot.y..plot.y + plot.h {
        for x in plot.x..plot.x + plot.w {
            if g.glyph(x, y) != crate::grid::BLANK {
                out.push((x, y, g.glyph(x, y), g.ink(x, y)));
            }
        }
    }
    out
}

fn is_braille(c: u32) -> bool {
    (0x2800..=0x28FF).contains(&c)
}

fn is_box(c: u32) -> bool {
    (0x2500..=0x259F).contains(&c)
}

/// REQ-CANDLE-020: box drawing at 4 cells a candle, braille with a blank
/// cell between at 2, braille without the gap at 1, in every mode.
#[test]
fn glyphs_by_spacing() {
    for mode in ["candle", "hollow", "ohlc"] {
        for (s, want) in [(4, Glyphs::Box), (2, Glyphs::Gap), (1, Glyphs::Dense)] {
            let (c, g) = at_spacing(s, mode);
            let (spacing, glyphs, interval) = c.spacing().unwrap();
            assert_eq!((glyphs, interval), (want, None), "{mode} at {s}: the rows themselves, spacing {spacing}");
            assert!(spacing >= s as f64 && spacing < s as f64 + 1.0, "{mode}: {spacing}");
            let cells = plot_glyphs(&g, geom(&c).price);
            assert!(!cells.is_empty());
            let cols: std::collections::BTreeSet<usize> = cells.iter().map(|c| c.0).collect();
            match want {
                Glyphs::Box => assert!(cells.iter().all(|c| is_box(c.2)), "{mode}: box drawing only"),
                _ => assert!(cells.iter().all(|c| is_braille(c.2)), "{mode}: braille only"),
            }
            let adjacent = cols.iter().any(|&x| cols.contains(&(x + 1)));
            match (want, mode) {
                (Glyphs::Gap, _) => assert!(!adjacent, "{mode}: a blank column between candles"),
                (Glyphs::Dense, _) => assert!(adjacent, "{mode}: no gap"),
                _ => {}
            }
        }
    }
}

/// The snapshot text of a frame, laid out as the contract's snapshot
/// writer lays out a whole block (`tests/common/snap.rs`).
fn snapshot(head: &str, g: &Grid) -> String {
    let runs = |row: &[u8], back: bool| {
        let mut out = String::new();
        let mut i = 0;
        while i < row.len() {
            let v = row[i];
            let n = row[i..].iter().take_while(|&&w| w == v).count();
            if !out.is_empty() {
                out.push(' ');
            }
            if back && v == NO_BACK {
                out.push_str("..");
            } else {
                let _ = write!(out, "{v:02x}");
            }
            if n > 1 {
                let _ = write!(out, "{n}");
            }
            i += n;
        }
        out
    };
    let mut out = format!("{head}\n@ {}x{} · t=0 · plain\n", g.cols(), g.rows());
    for row in g.text().split('\n') {
        let _ = writeln!(out, "|{row}|");
    }
    out.push_str("ink\n");
    for row in g.inks().chunks_exact(g.cols()) {
        let _ = writeln!(out, "{}", runs(row, false));
    }
    out.push_str("back\n");
    for row in g.backs().chunks_exact(g.cols()) {
        let _ = writeln!(out, "{}", runs(row, true));
    }
    out
}

/// REQ-CANDLE-020, REQ-SNAP-010: whole frames at spacings 4, 2 and 1, which
/// the contract's frames reach only in part: there `ohlcv-days` at 80 × 24
/// is drawn at 3.2 cells (1 mo, braille with a gap), at `META.min` at 2.75
/// (3 mo, gap), zoomed at 1.0 (1 w, no gap), and in one ink at 9.5 (3 mo,
/// box drawing, hashed); `ohlcv-ticks-1m` at 1.1 (4 h), 1.8 (1 d) and 1.4
/// (4 h), all without the gap, and in one ink at 6.3 (1 d, box, hashed).
/// Here `ohlcv-days` at 80 × 24 shows the plot's width over 4, 2 and 1 of
/// its daily rows from row 200, in each mode, with a hover on the middle
/// candle; `SNAP=write` rewrites `tests/snapshots/candles/spacings/`.
#[test]
fn snapshots_at_spacings_4_2_1() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots/candles/spacings");
    let write = std::env::var("SNAP").is_ok_and(|v| v == "write");
    for mode in ["candle", "hollow", "ohlc"] {
        for s in [4, 2, 1] {
            let (mut c, _) = at_spacing(s, mode);
            let g = geom(&c);
            let mid = (g.shown.start + g.shown.end) / 2;
            let col = g.column(mid).unwrap();
            let row = g.y.dot(g.level.ohlc[3][mid]).unwrap() as usize / 4 + g.price.y;
            c.input(&mv(col as f64 + 0.5, row as f64 + 0.5));
            let frame = draw(&mut c, 80, 24, &Env::plain());
            let (spacing, glyphs, _) = c.spacing().unwrap();
            let head = format!("# glyphplot spacing snapshot · candles · ohlcv-days · mode {mode} · rows from 200 at {spacing} cells a candle ({glyphs:?})");
            let text = snapshot(&head, &frame);
            let path = dir.join(format!("{mode}-spacing-{s}.snap"));
            if write {
                std::fs::create_dir_all(&dir).unwrap();
                std::fs::write(&path, &text).unwrap();
            } else {
                let want = std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("{} is missing: run with SNAP=write", path.display()));
                assert_eq!(want, text, "{} differs", path.display());
            }
        }
    }
}

/// REQ-CANDLE-030, REQ-THEME-060: in one ink no candle is narrower than four
/// cells, at any size, fixture or mode, the data resampled instead; rising
/// candles are hollow and falling ones solid.
#[test]
fn one_ink_resamples_below_four_cells() {
    let mono = Env { mono: true, ..Env::plain() };
    for fixture in ["ohlcv-days", "ohlcv-ticks-1m"] {
        for mode in ["candle", "hollow", "ohlc"] {
            let mut c = chart(fixture, &format!(r#"{{"mode":"{mode}"}}"#));
            for (cols, rows) in [(26, 10), (40, 12), (80, 24), (120, 30), (320, 120)] {
                let g = draw(&mut c, cols, rows, &mono);
                let (spacing, glyphs, interval) = c.spacing().unwrap();
                assert!(spacing >= 4.0 && glyphs == Glyphs::Box, "{fixture} {mode} {cols}×{rows}: {spacing} cells");
                assert!(interval.is_some(), "{fixture} {mode} {cols}×{rows}: resampled");
                if mode == "candle" {
                    for (_, _, glyph, ink) in plot_glyphs(&g, geom(&c).price) {
                        let solid = [0x2588, 0x2580, 0x2584].contains(&glyph);
                        let outline = [0x250C, 0x2510, 0x2514, 0x2518, 0x251C].contains(&glyph);
                        assert!(!(ink == UP && solid), "{fixture} {cols}×{rows}: a rising candle is hollow");
                        assert!(!(ink == DOWN && outline), "{fixture} {cols}×{rows}: a falling candle is solid");
                    }
                }
            }
        }
    }
}

/// REQ-THEME-060: up and down are the palette's tokens and told apart by
/// fill: in `candle` a rising candle is hollow in `UP`, a falling one solid
/// in `DOWN`; in `hollow` the fill follows close against open and the
/// colour close against the previous close.
#[test]
fn up_and_down_by_fill_and_colour() {
    let (c, g) = at_spacing(4, "candle");
    let gm = geom(&c);
    let lv = &gm.level;
    let (mut up, mut down) = (0, 0);
    for k in gm.shown.clone() {
        let col = gm.column(k).unwrap();
        let rising = lv.ohlc[3][k] >= lv.ohlc[0][k];
        let cells: Vec<(u32, u8)> = (gm.price.y..gm.price.y + gm.price.h).map(|y| (g.glyph(col + 1, y), g.ink(col + 1, y))).filter(|c| c.0 != 0x20).collect();
        assert!(cells.iter().all(|&(_, ink)| ink == if rising { UP } else { DOWN }), "candle {k}");
        let solid = cells.iter().any(|&(gl, _)| [0x2588, 0x2580, 0x2584].contains(&gl));
        assert_eq!(solid, !rising, "candle {k}: solid when falling");
        if rising {
            up += 1;
        } else {
            down += 1;
        }
    }
    assert!(up > 0 && down > 0);
    let mut prev = f64::NAN;
    for k in 0..lv.len() {
        let (ink, hollow) = draw::style(Mode::Hollow, lv, k, prev);
        assert_eq!(hollow, lv.ohlc[3][k] >= lv.ohlc[0][k]);
        if prev.is_finite() {
            assert_eq!(ink == UP, lv.ohlc[3][k] >= prev);
        }
        prev = lv.ohlc[3][k];
    }
}

/// REQ-CANDLE-050, REQ-HIT-010: a hover snaps to the candle under the
/// pointer and to the nearest of its open, high, low and close, naming the
/// row the value came from; the readout writes `o h l c`, the volume and
/// the signed change.
#[test]
fn crosshair_snaps_to_ohlc() {
    let (mut c, _) = at_spacing(4, "candle");
    let gm = geom(&c).clone();
    let lv = gm.level.clone();
    let mut named = std::collections::BTreeSet::new();
    for k in gm.shown.clone() {
        let col = gm.column(k).unwrap() as f64 + 0.5;
        for j in 0..4 {
            let v = lv.ohlc[j][k];
            let row = (gm.price.y + gm.y.dot(v).unwrap() as usize / 4) as f64 + 0.5;
            let h = c.hit(CellPos::new(col, row)).unwrap();
            // The value at the pointer's row, or one as near.
            let near = |x: f64| gm.y.dot(x).map_or(f64::INFINITY, |d| (d as f64 + 0.5 - (row - gm.price.y as f64) * 4.0).abs());
            assert!(near(h.y) <= near(v), "candle {k}: {} named for {}", h.y, v);
            assert_eq!(h.y, c.data.price(h.series_index)[h.index], "the row holds the value named");
            assert_eq!(h.series, c.data.names[h.series_index]);
            assert_eq!(h.x, c.data.xs()[h.index]);
            named.insert(h.series_index);
        }
    }
    assert_eq!(named.len(), 4, "every price is snapped to somewhere");
    let k = gm.shown.start + 3;
    let col = gm.column(k).unwrap() as f64 + 0.5;
    c.input(&mv(col, gm.price.y as f64 + 0.5));
    let g = draw(&mut c, 80, 24, &Env::plain());
    let text = g.text();
    for name in ["open ", "high ", "low ", "close ", "volume "] {
        assert!(text.contains(name), "the readout names {name:?}:\n{text}");
    }
    let change = lv.ohlc[3][k] - lv.ohlc[3][k - 1];
    let sign = if change >= 0.0 { '+' } else { '-' };
    assert!(text.contains(&format!("{sign}{}", readout::price_format(2, &Locale::default()).format(change.abs()))), "the signed change:\n{text}");
    assert!(text.contains(&format!("close {} USD", readout::price_format(2, &Locale::default()).format(lv.ohlc[3][k]))));
}

/// REQ-SCALE-060: on the index scale a weekend takes no column, so the rows
/// at one cell each ink every column; on a time scale the weekends leave
/// columns empty.
#[test]
fn index_scale_leaves_no_empty_columns() {
    let (c, g) = at_spacing(1, "candle");
    let plot = geom(&c).price;
    let cols: std::collections::BTreeSet<usize> = plot_glyphs(&g, plot).into_iter().map(|c| c.0).collect();
    assert_eq!(cols.len(), plot.w, "every column inked");
    let mut t = chart("ohlcv-days", r#"{"xScale":"time"}"#);
    t.set_view(&rows_view(&t, 200, 40));
    let g = draw(&mut t, 80, 24, &Env::plain());
    assert_eq!(t.spacing().map(|s| s.2), Some(None), "the rows themselves");
    let plot = geom(&t).price;
    let cols: std::collections::BTreeSet<usize> = plot_glyphs(&g, plot).into_iter().map(|c| c.0).collect();
    assert_eq!(cols.len(), 40, "one column a day");
    let gaps = cols.iter().zip(cols.iter().skip(1)).filter(|(a, b)| **b - **a > 2).count();
    assert!(gaps >= 7, "weekends leave columns: {gaps}");
}

/// REQ-CANDLE-040: a `volume` column draws a second pane under the prices
/// whose bars stand under their candles; without one, or with
/// `volume: false`, there is one pane.
#[test]
fn volume_pane_shares_x() {
    let (c, g) = at_spacing(2, "candle");
    let gm = geom(&c);
    let vp = gm.volume.expect("a volume pane");
    assert_eq!((vp.x, vp.w), (gm.price.x, gm.price.w));
    assert!(vp.y > gm.price.y + gm.price.h);
    let candles: std::collections::BTreeSet<usize> = gm.shown.clone().filter_map(|k| gm.column(k)).collect();
    let bars: std::collections::BTreeSet<usize> = plot_glyphs(&g, vp).into_iter().map(|c| c.0).collect();
    assert!(!bars.is_empty() && bars.is_subset(&candles), "each bar under its candle");
    let most = gm.shown.clone().max_by(|&a, &b| gm.level.volume[a].total_cmp(&gm.level.volume[b])).unwrap();
    assert!(bars.contains(&gm.column(most).unwrap()));
    let mut no = chart("ohlcv-days", r#"{"volume":false}"#);
    draw(&mut no, 80, 24, &Env::plain());
    assert!(geom(&no).volume.is_none());
    let t = DataTable::from_columns(vec![
        Column::timestamps_ms("t", vec![0.0, 86_400_000.0]).role(Role::X),
        Column::f64("open", vec![1.0, 2.0]),
        Column::f64("high", vec![2.0, 3.0]),
        Column::f64("low", vec![0.5, 1.0]),
        Column::f64("close", vec![1.5, 2.5]),
    ])
    .unwrap();
    let mut c = CandlesChart::new(&Options::new()).unwrap();
    c.set_data(Data::from_table(t)).unwrap();
    draw(&mut c, 80, 24, &Env::plain());
    assert!(geom(&c).volume.is_none());
    assert_eq!(geom(&c).shown.len(), 2);
}

/// REQ-TICK-050: under 20 rows the price axis's ticks sit on rows with a
/// nice step a row; from 20 rows they are fitted unless `snap: "rows"`.
#[test]
fn ticks_snap_to_rows_under_20_rows() {
    let on_rows = |c: &CandlesChart| {
        let p = &geom(c).layout.panes[0];
        let a = p.left.as_ref().unwrap();
        let axis = Axis::y(a.domain[0], a.domain[1], p.plot.h, 4);
        a.ticks.iter().all(|t| axis.position(t.value) == t.cell as f64 + 0.5)
    };
    let mut c = chart("ohlcv-days", "{}");
    draw(&mut c, 80, 24, &Env::plain());
    assert!(geom(&c).price.h < 20 && on_rows(&c));
    draw(&mut c, 80, 40, &Env::plain());
    assert!(geom(&c).price.h >= 20 && !on_rows(&c));
    let mut c = chart("ohlcv-days", r#"{"snap":"rows"}"#);
    draw(&mut c, 80, 40, &Env::plain());
    assert!(on_rows(&c));
}

/// REQ-DATA-100: a column's scale and unit apply to the ticks and the
/// readout, never to the values held.
#[test]
fn display_transforms_apply_to_ticks_and_readout() {
    let mut data = generate("ohlcv-days").unwrap().into_tables().remove(0).1;
    let cols: Vec<Column> = data
        .columns()
        .iter()
        .map(|c| {
            let mut c = c.clone();
            if ["open", "high", "low", "close"].contains(&c.name.as_str()) {
                if let crate::data::Values::Num(v) = &mut c.values {
                    v.iter_mut().for_each(|x| *x *= 100.0);
                }
                c.scale = Some(0.01);
            }
            c
        })
        .collect();
    data = DataTable::from_columns(cols).unwrap();
    let mut cents = CandlesChart::new(&Options::new()).unwrap();
    cents.set_data(Data::from_table(data)).unwrap();
    let mut plain = chart("ohlcv-days", "{}");
    let a = draw(&mut cents, 80, 24, &Env::plain());
    let b = draw(&mut plain, 80, 24, &Env::plain());
    assert_eq!(a.text(), b.text(), "ticks in displayed units");
    let gm = geom(&cents).clone();
    let col = gm.column(gm.shown.start).unwrap() as f64 + 0.5;
    cents.input(&mv(col, gm.price.y as f64 + 0.5));
    let text = draw(&mut cents, 80, 24, &Env::plain()).text();
    let close = gm.level.ohlc[3][gm.shown.start];
    assert!(close > 1000.0);
    assert!(text.contains(&format!("close {} USD", readout::price_format(2, &Locale::default()).format(close / 100.0))), "{text}");
}

/// REQ-DATA-110, REQ-DATA-080: data with no table is an error naming
/// `ohlcv`; a table with no value to give the prices draws "no data" and
/// warns naming `close`.
#[test]
fn other_shapes_name_ohlcv() {
    let mut c = CandlesChart::new(&Options::new()).unwrap();
    let e = c.set_data(generate("matrix-64").unwrap()).unwrap_err();
    assert!(e.to_string().contains("ohlcv"), "{e}");
    let t = DataTable::from_columns(vec![Column::timestamps_ms("t", vec![0.0, 1.0]), Column::strings("note", vec!["a".into(), "b".into()])]).unwrap();
    let a = c.set_data(Data::from_table(t)).unwrap();
    assert_eq!(a.warnings, vec![crate::data::Warning::NoChannel { channel: "close".into() }]);
    let g = draw(&mut c, 80, 24, &Env::plain());
    assert!(g.text().contains("no data"));
    assert!(c.hit(CellPos::new(40.5, 10.5)).is_none());
}

/// REQ-SAMPLE-040, the plan's catalogue ("Oil in weekly candles"): a year
/// of EIA WTI daily prices, one value a row, resamples at 80 × 24 into
/// weekly candles whose open is the week's first price, high its greatest,
/// low its least and close its last.
#[cfg(feature = "text")]
#[test]
fn daily_closes_resample_into_weekly_candles() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/wti-daily.json");
    let data = Data::from_json(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut c = CandlesChart::new(&Options::new()).unwrap();
    assert!(c.set_data(data).unwrap().warnings.is_empty());
    draw(&mut c, 80, 24, &Env::plain());
    assert_eq!(c.spacing().unwrap().2.as_deref(), Some("1 w"));
    let gm = geom(&c);
    let (lv, price) = (&gm.level, c.data.price(3));
    assert!(gm.shown.len() >= 50);
    for k in 0..lv.len() {
        let rows = lv.rows[k][0]..=lv.rows[k][1];
        let week = &price[rows.clone()];
        assert_eq!((lv.ohlc[0][k], lv.ohlc[3][k]), (week[0], week[week.len() - 1]));
        assert_eq!(lv.ohlc[1][k], week.iter().copied().fold(f64::MIN, f64::max));
        assert_eq!(lv.ohlc[2][k], week.iter().copied().fold(f64::MAX, f64::min));
    }
    let t = c.table(5);
    assert_eq!(t.columns, ["date", "price", "price", "price", "price"]);
}

/// REQ-CANDLE-010: the three modes draw three frames.
#[test]
fn modes_draw_each_way() {
    let frames: Vec<Grid> = ["candle", "hollow", "ohlc"].iter().map(|m| at_spacing(4, m).1).collect();
    assert!(frames[0] != frames[1] && frames[1] != frames[2] && frames[0] != frames[2]);
    let mut c = CandlesChart::new(&Options::new()).unwrap();
    let e = c.set_options(&Options::parse(r#"{"mode":"renko"}"#).unwrap()).unwrap_err();
    assert_eq!(e.to_string(), "there is no mode \"renko\": the modes are candle, hollow, ohlc");
}

/// REQ-SAMPLE-040, ADR-109, REQ-OVERLAY-010: a pan at one interval builds no
/// level (the candles are calendar buckets, the same wherever the view
/// is), and a hover draws no base.
#[test]
fn pans_read_no_rows_and_hovers_draw_no_base() {
    let mut c = chart("ohlcv-ticks-1m", "{}");
    draw(&mut c, 80, 24, &Env::plain());
    let [lo, hi] = c.table(0).domain.unwrap();
    let (builds, stamps) = (c.builds(), geom(&c).level.stamp.clone());
    for k in 1..20 {
        let off = (hi - lo) * 0.01 * k as f64;
        c.set_view(&ViewPatch { x: Patch::Set(XPatch { domain: Patch::Set([lo + off, hi + off]), follow: None }), ..ViewPatch::default() });
        draw(&mut c, 80, 24, &Env::plain());
        assert_eq!(geom(&c).level.stamp, stamps, "the same buckets");
    }
    assert_eq!(c.builds(), builds);
    let draws = c.base_draws();
    for x in 10..60 {
        let ch = c.input(&mv(x as f64 + 0.5, 8.5));
        assert!(!ch.base);
        draw(&mut c, 80, 24, &Env::plain());
    }
    assert_eq!(c.base_draws(), draws);
}

/// REQ-A11Y-020, REQ-A11Y-030: the table holds the candles in view at the
/// interval drawn, and the value text is the readout's.
#[test]
fn table_holds_the_candles_in_view() {
    let mut c = chart("ohlcv-days", "{}");
    draw(&mut c, 80, 24, &Env::plain());
    let t = c.table(100);
    assert_eq!(t.columns, ["t", "open", "high", "low", "close", "volume"]);
    assert_eq!((t.rows.len(), t.total), (24, 24));
    assert_eq!(t.caption, "candlestick chart, candles of 1 mo");
    assert_eq!(t.rows[0][0], "2024-10");
    assert!(t.summary.contains("24 candles of 1 mo in view"), "{}", t.summary);
    assert!(t.value.is_none());
    c.input(&mv(30.5, 8.5));
    assert!(c.table(10).value.unwrap().contains("open "));
    assert_eq!(c.table(10).rows.len(), 10);
}
