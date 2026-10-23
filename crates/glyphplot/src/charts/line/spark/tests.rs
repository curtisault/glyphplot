//! The sparkline's own tests: its meta, its bands and text, its modes, its
//! ring, gaps and the display transform. The contract
//! (`tests/contract.rs`) checks the rules every chart keeps.

use super::*;
use crate::data::{Column, Table as DataTable};
use crate::fixtures::generate;
use crate::input::{Key, Mods, PointerType};

fn chart(fixture: &str, opts: &str) -> SparkChart {
    let mut c = SparkChart::new(&Options::parse(opts).unwrap()).unwrap();
    c.set_data(generate(fixture).unwrap()).unwrap();
    c
}

fn draw(c: &mut SparkChart, cols: usize, rows: usize, t: f64) -> Grid {
    let mut g = Grid::new(cols, rows);
    c.frame(t, &Env::plain(), &mut g);
    g
}

fn xy(x: Vec<f64>, y: Vec<f64>) -> Data {
    Data::from_table(DataTable::from_columns(vec![Column::f64("x", x), Column::f64("y", y)]).unwrap())
}

fn geom(c: &SparkChart) -> &Geom {
    &c.geom.as_ref().unwrap().1
}

/// Whether a glyph draws a mark: braille with a dot, or a block element.
fn mark(g: u32) -> bool {
    (0x2801..=0x28FF).contains(&g) || (0x2580..=0x259F).contains(&g)
}

/// REQ-SPARK-010, REQ-CHART-020: the meta is the schema example's, the
/// options aside (the example gives none; the chart's defaults are the keys
/// it reads, REQ-OPT-010), and it is the `line` family's second type.
#[test]
fn meta_is_the_schema_example() {
    let example: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../spec/schemas/examples/meta.sparkline.json")).unwrap())
            .unwrap();
    let mut ours: serde_json::Value = serde_json::from_str(&META.to_json()).unwrap();
    assert!(ours["options"].is_object());
    ours["options"] = serde_json::Value::Null;
    // Numbers as numbers: the example writes 4 where serde writes 4.0.
    let mut example = example;
    for k in ["gzip", "frameMs", "worstMs", "inputMs"] {
        assert_eq!(ours["budget"][k].as_f64(), example["budget"][k].as_f64(), "{k}");
    }
    ours["budget"] = serde_json::Value::Null;
    example["budget"] = serde_json::Value::Null;
    assert_eq!(ours, example);
    assert_eq!(crate::registry::meta("sparkline").map(|m| m.family), Some("line"));
    assert!(SparkChart::new(&Options::parse(r#"{"mode":"step"}"#).unwrap()).is_err_and(|e| e.to_string().contains("column")));
}

/// REQ-SPARK-010: no axes; a band of rows a series, in order, each ending in
/// its label and latest value on its middle row; a hit in a band names that
/// band's series.
#[test]
fn one_series_a_band_and_no_axes() {
    let mut c = chart("multi-4", "");
    let g = draw(&mut c, 80, 24, 0.0);
    assert!(!g.glyphs().iter().any(|&ch| (0x2500..=0x257F).contains(&ch)), "no axis, tick or rule:\n{}", g.text());
    let names = ["alpha", "beta", "gamma", "delta"];
    let lines: Vec<String> = g.text().lines().map(str::to_string).collect();
    for (s, name) in names.iter().enumerate() {
        let b = geom(&c).bands[s].clone().unwrap();
        assert_eq!((b.rect.y, b.rect.h), (6 * s, 6), "{name}'s band");
        let row = &lines[b.rect.y + 2];
        assert!(row.contains(&format!("{name} ")), "{name} on its band's middle row: {row:?}");
        let ys = c.data.ys(s);
        let latest = ys.iter().rev().find(|v| v.is_finite()).unwrap();
        assert!(row.contains(&compact(*latest, &Display::default(), &Locale::default())), "{row:?}");
        let inked = (b.rect.y..b.rect.y + b.rect.h).flat_map(|r| (0..geom(&c).plot.w).map(move |x| (x, r))).filter(|&(x, r)| mark(g.glyph(x, r))).count();
        assert!(inked > 40, "{name} draws in its band");
        let h = c.hit(CellPos::center(20, b.rect.y + 1)).unwrap();
        assert_eq!(h.series, *name);
        assert!(h.row >= b.rect.y && h.row < b.rect.y + b.rect.h);
    }
    // Fewer rows than series: a row each for the first ones.
    let g = draw(&mut c, 40, 3, 0.0);
    assert_eq!(geom(&c).bands.iter().map(|b| b.as_ref().map(|b| b.rect.y)).collect::<Vec<_>>(), [Some(0), Some(1), Some(2), None]);
    assert!(g.text().lines().nth(2).unwrap().contains("gamma"));
    // A hidden series keeps its band, empty, its name muted.
    c.set_view(&ViewPatch::from_json(r#"{"hidden":["beta"]}"#).unwrap());
    let g = draw(&mut c, 80, 24, 0.0);
    assert!((6..12).all(|r| (0..geom(&c).plot.w).all(|x| !mark(g.glyph(x, r)))));
    let tx = geom(&c).text.as_ref().unwrap().0;
    assert_eq!(g.ink(tx, 8), MUTED);
    assert!(c.hit(CellPos::center(20, 7)).is_none());
}

/// REQ-SPARK-010: the label, the latest value and the extent are each an
/// option, and parts drop, the extent first, until the plot keeps half the
/// columns; at the type's minimum, 8 × 1, there is no text.
#[test]
fn text_parts_by_option_and_room() {
    let text = |opts: &str, cols: usize| {
        let mut c = chart("sine-1k", opts);
        let g = draw(&mut c, cols, 1, 0.0);
        (g.text(), geom(&c).plot.w)
    };
    let (all, w) = text("", 80);
    assert!(all.contains(" y ") && all.contains('↓') && all.contains('↑'), "{all:?}");
    assert!(w >= 40);
    let (no_label, _) = text(r#"{"label":false}"#, 80);
    assert!(!no_label.contains(" y ") && no_label.contains('↓'), "{no_label:?}");
    let (none, w) = text(r#"{"label":false,"latest":false,"extent":false}"#, 80);
    assert_eq!(w, 80, "no text: the plot is every column: {none:?}");
    let (narrow, w) = text("", 24);
    assert!(!narrow.contains('↓') && 2 * w >= 24, "the extent drops first at 24 columns: {narrow:?}");
    let (min, w) = text("", 8);
    assert_eq!(w, 8);
    assert!(min.chars().all(|c| c == ' ' || (0x2800..=0x28FF).contains(&(c as u32))), "{min:?}");
    // The extent is of the rows held: sine-1k runs from -1 to 1.
    assert!(all.contains("↓-1 ↑1") || all.contains("↓-0.9999") || all.contains("↓-1 "), "{all:?}");
}

/// REQ-SPARK-010: the modes line, area and column draw each its own frame;
/// an area fills under its line; columns are block elements, rising from 0
/// in eighths and hanging below it.
#[test]
fn modes_draw_lines_areas_and_columns() {
    let frames: Vec<Grid> = ["line", "area", "column"]
        .iter()
        .map(|m| {
            let mut c = chart("sine-1k", &format!(r#"{{"mode":"{m}","label":false,"latest":false,"extent":false}}"#));
            draw(&mut c, 40, 8, 0.0)
        })
        .collect();
    assert!(frames[0] != frames[1] && frames[1] != frames[2] && frames[0] != frames[2]);
    let inked = |g: &Grid| g.glyphs().iter().filter(|&&c| mark(c)).count();
    assert!(inked(&frames[1]) > inked(&frames[0]) * 3 / 2, "an area fills under its line");
    let blocks = |g: &Grid| g.glyphs().iter().filter(|&&c| (0x2580..=0x259F).contains(&c)).count();
    assert_eq!(blocks(&frames[0]) + blocks(&frames[1]), 0);
    assert!(blocks(&frames[2]) > 40);
    assert!(frames[2].glyphs().iter().any(|&c| c == '▀' as u32 || c == '▔' as u32), "negative values hang below 0");
    // A bar's top in eighths: 0 to 4 over two rows rises 3 eighths for 0.75.
    let mut c = SparkChart::new(&Options::parse(r#"{"mode":"column","label":false,"latest":false,"extent":false}"#).unwrap()).unwrap();
    c.set_data(xy(vec![0.0, 1.0, 2.0, 3.0], vec![0.0, 0.75, 2.0, 4.0])).unwrap();
    let g = draw(&mut c, 8, 2, 0.0);
    let b = geom(&c).bands[0].clone().unwrap();
    let mut cells = Vec::new();
    bar(&b, 0.75, 0.75, |r, ch| cells.push((r, ch)));
    // The domain is −0.08 to 4.08 (padded 2 %), 16 eighths high: 0 is at
    // 15.7, snapped to 16, and 0.75 at 12.8, so the bar's top cell holds
    // 3.2 eighths, drawn `▃`.
    assert_eq!(cells, [(1, '▃')]);
    assert!(g.text().lines().last().unwrap().chars().filter(|&ch| ch == '█').count() >= 2, "{}", g.text());
}

/// REQ-SPARK-010, REQ-STREAM-020: the ring keeps the last `retain.rows` rows: a replace of more
/// draws what a chart given only those rows draws, and appends past the
/// capacity drop the oldest; rows keep their numbers while held. The ring is
/// the core's (`ring.rs`), shared with the line chart.
#[test]
fn the_ring_keeps_the_last_capacity_rows() {
    let n = 300;
    let x: Vec<f64> = (0..n).map(f64::from).collect();
    let y: Vec<f64> = x.iter().map(|v| (v / 17.0).sin()).collect();
    let mut c = SparkChart::new(&Options::parse(r#"{"retain":{"rows":100}}"#).unwrap()).unwrap();
    let a = c.set_data(xy(x.clone(), y.clone())).unwrap();
    assert_eq!((a.rows, c.held()), (100, 100));
    let mut last = SparkChart::new(&Options::parse(r#"{"retain":{"rows":100}}"#).unwrap()).unwrap();
    last.set_data(xy(x[200..].to_vec(), y[200..].to_vec())).unwrap();
    assert_eq!(draw(&mut c, 60, 4, 0.0), draw(&mut last, 60, 4, 0.0));
    let first = |c: &SparkChart| Scene::edge(c, c.view(), c.domain_now(), false).unwrap();
    let h = first(&c);
    assert_eq!((h.index, h.x), (200, 200.0), "the first held row is row 200 of the data");
    assert_eq!(c.datum(&h).get("x"), Some(&Value::Number(200.0)));
    // Appends: 30 rows, then 40, one late and one replacing the last.
    let mut r = SparkChart::new(&Options::parse(r#"{"retain":{"rows":50}}"#).unwrap()).unwrap();
    r.set_data(xy(x[..30].to_vec(), y[..30].to_vec())).unwrap();
    let mut bx = x[30..70].to_vec();
    let mut by = y[30..70].to_vec();
    bx.insert(0, 29.0);
    by.insert(0, 9.0);
    bx.insert(1, 5.0);
    by.insert(1, 9.0);
    let app = r.append(xy(bx, by)).unwrap();
    assert_eq!((app.rows, app.dropped, app.late), (50, 20, 1));
    let mut want = SparkChart::new(&Options::parse(r#"{"retain":{"rows":50}}"#).unwrap()).unwrap();
    let mut wy = y[20..70].to_vec();
    wy[9] = 9.0; // x 29, replaced by the append's row of equal x
    want.set_data(xy(x[20..70].to_vec(), wy)).unwrap();
    assert_eq!(draw(&mut r, 60, 4, 0.0), draw(&mut want, 60, 4, 0.0), "the ring ends on a replace of its last 50 rows");
    let h = first(&r);
    assert_eq!((h.index, h.x), (20, 20.0));
    // A smaller capacity set on a live chart drops the oldest held.
    r.set_options(&Options::parse(r#"{"retain":{"rows":10}}"#).unwrap()).unwrap();
    assert_eq!(r.held(), 10);
    assert_eq!(first(&r).index, 60);
}

/// REQ-DATA-030, REQ-LINE-030: a missing y breaks the line and the area;
/// a column chart draws no bar across it.
#[test]
fn gaps_break_the_marks() {
    let x: Vec<f64> = (0..40).map(f64::from).collect();
    let y: Vec<f64> = x.iter().map(|&v| if (15.0..25.0).contains(&v) { f64::NAN } else { (v / 5.0).sin() }).collect();
    for mode in ["line", "area", "column"] {
        let mut c = SparkChart::new(&Options::parse(&format!(r#"{{"mode":"{mode}","label":false,"latest":false,"extent":false}}"#)).unwrap()).unwrap();
        c.set_data(xy(x.clone(), y.clone())).unwrap();
        let g = draw(&mut c, 39, 4, 0.0);
        // x 15 to 24 is missing: cells 16 to 22 lie inside the gap.
        for col in 16..=22 {
            assert!((0..4).all(|r| !mark(g.glyph(col, r))), "{mode}: column {col} is inked:\n{}", g.text());
        }
    }
}

/// REQ-DATA-100: the text and the readout write the column's display
/// transform: stored cents at `scale: 0.01` in USD.
#[test]
fn text_and_readout_take_the_display_transform() {
    let mut price = Column::f64("price", vec![10_000.0, 12_000.0, 18_240.0]).unit("USD");
    price.scale = Some(0.01);
    let data = Data::from_table(DataTable::from_columns(vec![Column::f64("x", vec![0.0, 1.0, 2.0]), price]).unwrap());
    let mut c = SparkChart::new(&Options::new()).unwrap();
    c.set_data(data).unwrap();
    let g = draw(&mut c, 100, 3, 0.0);
    assert!(g.text().contains("price 182.40 USD ↓100.00 USD ↑182.40 USD"), "{}", g.text());
    c.input(&Input::Key { t: 0.0, key: Key::End, mods: Mods::NONE });
    let g = draw(&mut c, 100, 3, 0.0);
    assert!(g.text().lines().next().unwrap().contains("price 182.40 USD"), "{}", g.text());
    assert_eq!(c.table(10).rows[2], ["2", "182.40 USD"]);
}

/// REQ-HIT-010, REQ-OVERLAY-020: a hover snaps to the band's datum and
/// draws the crosshair in that band and the readout; Escape hides it.
#[test]
fn hover_reads_out_its_band() {
    let mut c = chart("multi-4", "");
    let base = draw(&mut c, 80, 24, 0.0);
    let ch = c.input(&Input::Move { t: 0.0, x: 30.5, y: 14.5, pointer: PointerType::Mouse, buttons: 0, mods: Mods::NONE });
    assert!(ch.overlay && !ch.base);
    let cursor = c.view().cursor.clone().unwrap();
    assert_eq!(cursor.series.as_deref(), Some("gamma"));
    let g = draw(&mut c, 80, 24, 0.0);
    assert!(g.text().lines().next().unwrap().contains("gamma "), "{}", g.text());
    let differ: Vec<usize> = (0..24).filter(|&r| (0..80).any(|x| g.glyph(x, r) != base.glyph(x, r))).collect();
    assert!(differ.iter().all(|&r| r == 0 || (12..18).contains(&r)), "the crosshair stays in gamma's band: rows {differ:?}");
    c.input(&Input::Key { t: 0.0, key: Key::Escape, mods: Mods::NONE });
    assert_eq!(draw(&mut c, 80, 24, 0.0), base);
}

/// REQ-LAYOUT-040, contract rule 21: no data draws `no data` over an empty
/// track, at every size without a panic.
#[test]
fn no_data_draws_an_empty_track() {
    let mut c = chart("empty", "");
    let g = draw(&mut c, 40, 5, 0.0);
    let middle = g.text().lines().nth(2).unwrap().to_string();
    assert!(middle.contains("no data") && middle.contains('┄'), "{middle:?}");
    assert!(c.hit(CellPos::center(10, 2)).is_none());
    for (cols, rows) in [(1, 1), (8, 1), (3, 7), (320, 120)] {
        draw(&mut c, cols, rows, 0.0);
    }
}
