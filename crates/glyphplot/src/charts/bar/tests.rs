//! The bar chart's own tests: eighths, the band hit test against brute
//! force, signed and one-ink fills, labels, row snapping, the readout, zoom,
//! stacking, orientation, options and the overlay. The contract
//! (`tests/contract.rs`) checks the rules every chart keeps.

use super::*;
use crate::data::{Column, Matrix};
use crate::fixtures::generate;
use crate::input::{Mods, PointerType};
use crate::mono;
use crate::palette::{DOWN, UP};

fn chart(data: Data, opts: &str) -> BarChart {
    let mut c = BarChart::new(&Options::parse(opts).unwrap()).unwrap();
    c.set_data(data).unwrap();
    c
}

fn draw_in(c: &mut BarChart, cols: usize, rows: usize, env: &Env) -> Grid {
    let mut g = Grid::new(cols, rows);
    c.frame(0.0, env, &mut g);
    g
}

fn draw(c: &mut BarChart, cols: usize, rows: usize) -> Grid {
    draw_in(c, cols, rows, &Env::plain())
}

fn counts(cats: &[&str], series: &[(&str, &[f64])]) -> Data {
    let mut cols = vec![Column::strings("k", cats.iter().map(|s| s.to_string()).collect())];
    cols.extend(series.iter().map(|(n, v)| Column::f64(n, v.to_vec())));
    Data::from_table(DataTable::from_columns(cols).unwrap())
}

fn geom(c: &BarChart) -> &Geom {
    &c.geom.as_ref().unwrap().1
}

fn mv(t: f64, x: f64, y: f64) -> Input {
    Input::Move { t, x, y, pointer: PointerType::Mouse, buttons: 0, mods: Mods::NONE }
}

fn ch(g: &Grid, col: usize, row: usize) -> char {
    char::from_u32(g.glyph(col, row)).unwrap()
}

/// REQ-BAR-010: each bar's free end at an eighth of a cell, `▁`–`▇` on a
/// vertical bar and `▏`–`▉` on a horizontal one, full cells below it.
#[test]
fn free_end_at_an_eighth() {
    let cats = ["a", "b", "c", "d", "e", "f", "g", "h"];
    // Vertical: 4 rows over [0, 4], a row a unit, an eighth 0.125.
    let values: Vec<f64> = (1..=8).map(|k| 2.0 + f64::from(k) * 0.125).collect();
    let mut c = chart(counts(&cats, &[("v", &values)]), "");
    c.set_view(&ViewPatch::from_json(r#"{"y":{"domain":[0,4],"fit":"fixed"}}"#).unwrap());
    let g = draw(&mut c, 40, 6);
    let gm = geom(&c);
    let (p, cats_) = (gm.plot, gm.cats.unwrap());
    assert_eq!((p.h, gm.value.domain), (4, [0.0, 4.0]));
    let ends: String = (0..8).map(|k| ch(&g, p.x + cats_.middle(k), p.y + 1)).collect();
    assert_eq!(ends, "▁▂▃▄▅▆▇█", "the third row from the bottom holds each free end");
    for k in 0..8 {
        let col = p.x + cats_.middle(k);
        assert_eq!((ch(&g, col, p.y + 2), ch(&g, col, p.y + 3), ch(&g, col, p.y)), ('█', '█', ' '), "bar {k}: full below its end, blank above");
    }
    // Horizontal: 32 columns over [0, 32], a cell a unit.
    let values: Vec<f64> = (1..=8).map(|k| 5.0 + f64::from(k) * 0.125).collect();
    let mut c = chart(counts(&cats, &[("v", &values)]), r#"{"orient":"horizontal"}"#);
    c.set_view(&ViewPatch::from_json(r#"{"y":{"domain":[0,32],"fit":"fixed"}}"#).unwrap());
    let g = draw(&mut c, 34, 20);
    let gm = geom(&c);
    let (p, cats_) = (gm.plot, gm.cats.unwrap());
    assert_eq!((p.x, p.w), (2, 32));
    let ends: String = (0..8).map(|k| ch(&g, p.x + 5, p.y + cats_.middle(k))).collect();
    assert_eq!(ends, "▏▎▍▌▋▊▉█");
    assert!((0..8).all(|k| (0..5).all(|x| ch(&g, p.x + x, p.y + cats_.middle(k)) == '█')));
}

/// REQ-BAR-020, REQ-HIT-010: the band hit test names, for every plot cell,
/// the category a brute-force scan of every drawn band finds (the band
/// holding the cell, else the nearest), and grouped, the series whose bar is
/// nearest within it; every plot cell answers and no cell off it does.
#[test]
fn hit_by_band() {
    let three =
        || counts(&["a", "b", "c", "d", "e"], &[("p", &[1.0, 2.0, 3.0, 4.0, 5.0]), ("q", &[2.0, 1.0, 2.0, 1.0, 2.0]), ("r", &[5.0, 4.0, 3.0, 2.0, 1.0])]);
    let cases: Vec<(Data, &str)> = vec![
        (generate("categorical-12").unwrap(), ""),
        (generate("categorical-12").unwrap(), r#"{"orient":"horizontal"}"#),
        (three(), ""),
        (three(), r#"{"orient":"horizontal"}"#),
        (three(), r#"{"stack":true}"#),
    ];
    for (data, opts) in cases {
        for (cols, rows) in [(30, 6), (40, 12), (80, 24), (123, 37)] {
            let mut c = chart(data.clone(), opts);
            draw(&mut c, cols, rows);
            let g = geom(&c).clone();
            let Some(cats) = g.cats else { continue };
            let p = g.plot;
            for row in 0..rows {
                for col in 0..cols {
                    let hit = c.hit(CellPos::center(col, row));
                    let inside = col >= p.x && col < p.x + p.w && row >= p.y && row < p.y + p.h;
                    assert_eq!(hit.is_some(), inside, "{opts} {cols}×{rows} cell {col},{row}");
                    let Some(hit) = hit else { continue };
                    let cross = if g.horizontal { row - p.y } else { col - p.x };
                    // Brute force: the drawn band nearest the cell.
                    let dist = |k: usize| {
                        let s = cats.band.slot(k).unwrap();
                        if cross < s.start {
                            s.start - cross
                        } else if cross >= s.end {
                            cross + 1 - s.end
                        } else {
                            0
                        }
                    };
                    let k = (0..cats.count()).min_by_key(|&k| dist(k)).unwrap();
                    assert_eq!(hit.index, cats.first + k, "{opts} {cols}×{rows} cell {col},{row}");
                    assert_eq!(hit.x, hit.index as f64);
                    if cats.groups > 1 {
                        let bar = |j: usize| {
                            let s = cats.cross(k, j);
                            if cross < s.start {
                                s.start - cross
                            } else if cross >= s.end {
                                cross + 1 - s.end
                            } else {
                                0
                            }
                        };
                        let j = (0..cats.groups).min_by_key(|&j| (bar(j), j)).unwrap();
                        assert_eq!(hit.series_index, g.shown[j], "{opts} {cols}×{rows} cell {col},{row}");
                    }
                    assert_eq!(hit.y.to_bits(), c.data.stored(hit.series_index, hit.index).to_bits());
                }
            }
        }
    }
}

/// REQ-BAR-020: the hit test is O(1) in the categories: over a million
/// categories it reads one band (timed loosely: 10,000 hits well under a
/// second in a debug build).
#[test]
fn hit_does_not_scan_the_categories() {
    let n = 1_000_000;
    let t = DataTable::from_columns(vec![Column::f64("value", (0..n).map(|i| f64::from(i % 97)).collect())]).unwrap();
    let mut c = chart(Data::from_table(t), "");
    draw(&mut c, 80, 24);
    let start = std::time::Instant::now();
    let mut found = 0;
    for i in 0..10_000 {
        found += usize::from(c.hit(CellPos::new(10.0 + (i % 60) as f64, 10.5)).is_some());
    }
    assert_eq!(found, 10_000);
    assert!(start.elapsed().as_secs_f64() < 1.0, "{:?}", start.elapsed());
}

/// REQ-THEME-060: a single series with values below its baseline draws in
/// the up and down tokens, told apart by fill as well: `█` up, `▒` down;
/// `signed: "off"` keeps the series' colour.
#[test]
fn signed_values_take_up_and_down_by_fill() {
    let data = || counts(&["a", "b", "c"], &[("v", &[4.0, -4.0, 2.0])]);
    let mut c = chart(data(), "");
    let g = draw(&mut c, 40, 24);
    let fills: Vec<(char, u8)> =
        g.glyphs().iter().zip(g.inks()).filter(|(&gl, _)| gl == '█' as u32 || gl == '▒' as u32).map(|(&gl, &i)| (char::from_u32(gl).unwrap(), i)).collect();
    assert!(fills.contains(&('█', UP)) && fills.contains(&('▒', DOWN)), "{fills:?}");
    assert!(fills.iter().all(|&(f, i)| (f == '█') == (i == UP)), "fill and colour agree");
    let mut c = chart(data(), r#"{"signed":"off"}"#);
    let g = draw(&mut c, 40, 24);
    assert!(g.inks().iter().all(|&i| i != UP && i != DOWN));
    let mut c = chart(counts(&["a", "b"], &[("v", &[1.0, 2.0])]), "");
    let g = draw(&mut c, 40, 24);
    assert!(g.inks().iter().all(|&i| i != UP && i != DOWN), "no value below the baseline: the series' colour");
}

/// REQ-MONO-030: in one ink, series fill with `█ ▓ ▒ ░` by series, grouped
/// or stacked.
#[test]
fn one_ink_fills_by_series() {
    let data = || counts(&["a", "b"], &[("p", &[4.0, 3.0]), ("q", &[4.0, 3.0]), ("r", &[4.0, 3.0]), ("s", &[4.0, 3.0])]);
    for opts in ["", r#"{"stack":true}"#] {
        let mut c = chart(data(), opts);
        let mono = Env { mono: true, ..Env::plain() };
        let g = draw_in(&mut c, 60, 24, &mono);
        let gm = geom(&c).clone();
        for seg in &gm.segs {
            let full = seg.cells().find(|&a| seg.lo <= 8 * a && 8 * a + 8 <= seg.hi).unwrap();
            let (col, row) = gm.cell(seg.cross.start, full).unwrap();
            assert_eq!(ch(&g, col, row), mono::fill(seg.series), "{opts} series {}", seg.series);
        }
        let plain = draw(&mut c, 60, 24);
        assert!(plain.glyphs().iter().all(|&gl| gl != '▓' as u32 && gl != '░' as u32), "in colour, solid");
    }
}

/// `categorical-12`'s labels are longer than their bands: every other one
/// is thinned out, and the rest are cut with `…` to their cells, never
/// rotated, two cells apart.
#[test]
fn long_labels_thin_and_truncate() {
    let mut c = chart(generate("categorical-12").unwrap(), "");
    let g = draw(&mut c, 80, 24);
    let gm = geom(&c);
    let axis = gm.layout.x_axis.as_ref().unwrap();
    let labels: Vec<&TickLabelRef> = axis.ticks.iter().filter_map(|t| t.label.as_ref()).collect();
    assert!(labels.len() >= 2 && labels.len() < 12, "{}", labels.len());
    assert!(labels.iter().all(|l| l.text.ends_with('…')));
    assert!(labels.windows(2).all(|w| w[0].start + w[0].width + 2 <= w[1].start), "two cells apart");
    let row = probe_row(&g, axis.rect.y + 1);
    assert!(row.contains("North At…  Central E…  Western m…"), "{row}");
    let mut c = chart(generate("categorical-12").unwrap(), r#"{"orient":"horizontal"}"#);
    let g = draw(&mut c, 80, 24);
    let left = geom(&c).layout.panes[0].left.clone().unwrap();
    assert!(left.rect.w <= 80 / 3, "the gutter takes at most a third");
    let text = g.text();
    assert!(text.contains("North Atlantic distribut…"), "{text}");
}

type TickLabelRef = crate::layout::TickLabel;

fn probe_row(g: &Grid, r: usize) -> String {
    (0..g.cols()).filter_map(|c| char::from_u32(g.glyph(c, r))).collect()
}

/// REQ-TICK-050: under 20 rows the value axis is snapped: every tick on a
/// row, the row's centre holding the tick's value, a nice step a row; from
/// 20 rows it is d3's.
#[test]
fn value_axis_snaps_to_rows_under_20_rows() {
    for rows in [6, 8, 12, 15, 21] {
        let mut c = chart(generate("categorical-12").unwrap(), "");
        draw(&mut c, 60, rows);
        let gm = geom(&c);
        let left = gm.layout.panes[0].left.as_ref().unwrap();
        let h = gm.plot.h;
        let [lo, hi] = gm.value.domain;
        let step = (hi - lo) / h as f64;
        for t in &left.ticks {
            let centre = hi - (t.cell as f64 + 0.5) * step;
            if h < 20 {
                assert!((centre - t.value).abs() < 1e-9 * step, "{rows} rows: tick {} on row {} whose centre is {centre}", t.value, t.cell);
                let m = step / 10f64.powf(step.log10().floor());
                assert!([1.0, 2.0, 5.0].iter().any(|d| (m - d).abs() < 1e-9), "{rows} rows: a step of {step} a row");
            }
        }
        assert!(left.ticks.iter().all(|t| t.label.is_some()) && !left.ticks.is_empty(), "{rows} rows");
        if h >= 20 {
            assert_eq!(gm.value.domain, [0.0, 1000.0], "d3's nice domain");
        }
    }
}

/// REQ-DATA-100: the readout names the category, the series and its value
/// through the column's display: stored 18240 at scale 0.01 reads 182.40
/// USD, and the table shows the same; the stored value is untouched.
#[test]
fn readout_names_category_series_and_value() {
    let mut price = Column::f64("price", vec![18240.0, 10000.0]).unit("USD");
    price.scale = Some(0.01);
    let data = Data::from_table(DataTable::from_columns(vec![Column::strings("item", vec!["tea".into(), "cake".into()]), price]).unwrap());
    let mut c = chart(data, "");
    let base = draw(&mut c, 40, 12);
    let gm = geom(&c).clone();
    let col = gm.plot.x + gm.cats.unwrap().middle(0);
    c.input(&mv(0.0, col as f64 + 0.5, gm.plot.y as f64 + gm.plot.h as f64 - 0.5));
    let g = draw(&mut c, 40, 12);
    assert_ne!(g, base);
    assert!(g.text().contains("tea  price 182.40 USD"), "{}", g.text());
    assert_eq!(c.view().cursor.as_ref().and_then(|c| c.y), Some(18240.0), "the cursor holds the stored value");
    let t = c.table(10);
    assert_eq!(t.rows[0], vec!["tea".to_string(), "182.40".to_string()]);
    assert_eq!(t.columns, vec!["item".to_string(), "price".to_string()]);
    assert!(t.value.unwrap().contains("price 182.40 USD"));
}

/// The view's x domain, in category indices, picks the categories drawn;
/// the value axis fits them.
#[test]
fn zoom_picks_categories() {
    let mut c = chart(generate("categorical-12").unwrap(), "");
    draw(&mut c, 80, 24);
    c.set_view(&ViewPatch::from_json(r#"{"x":{"domain":[3.1,6.1]}}"#).unwrap());
    draw(&mut c, 80, 24);
    let cats = geom(&c).cats.unwrap();
    assert_eq!((cats.first, cats.count()), (4, 3));
    let t = c.table(100);
    assert_eq!(t.total, 3);
    let [a, b] = t.domain.unwrap();
    assert!(a < 4.0 && b > 6.0 && a > 3.0 && b < 7.0, "the drawn domain [{a}, {b}]");
    let before = c.view().clone();
    c.input(&Input::Key { t: 0.0, key: crate::input::Key::Zero, mods: Mods::NONE });
    assert_ne!(c.view().x.domain, before.x.domain, "0 resets");
}

/// REQ-BAR-010: stacked bars pile the shown series, each category's height
/// their sum; horizontal bars run left to right; hiding a series removes it
/// from the stack and the extent.
#[test]
fn stacked_and_horizontal() {
    let data = || counts(&["a", "b"], &[("p", &[3.0, 1.0]), ("q", &[2.0, 4.0])]);
    let mut c = chart(data(), r#"{"stack":true}"#);
    c.set_view(&ViewPatch::from_json(r#"{"y":{"domain":[0,5],"fit":"fixed"}}"#).unwrap());
    draw(&mut c, 40, 7);
    let gm = geom(&c).clone();
    assert_eq!(gm.plot.h, 5);
    let tops: Vec<i64> = (0..2).map(|k| gm.seg(k, 1).unwrap().hi).collect();
    assert_eq!(tops, vec![40, 40], "5 of 5 rows");
    assert_eq!((gm.seg(0, 0).unwrap().hi, gm.seg(0, 1).unwrap().lo), (24, 24), "segments meet on a whole cell");
    c.set_view(&ViewPatch::from_json(r#"{"hidden":["p"]}"#).unwrap());
    draw(&mut c, 40, 7);
    assert_eq!(geom(&c).seg(1, 0).unwrap().hi, 32);
    let mut c = chart(data(), r#"{"orient":"horizontal"}"#);
    let g = draw(&mut c, 40, 12);
    let gm = geom(&c).clone();
    assert!(gm.horizontal && gm.cats.unwrap().groups == 2);
    let seg = gm.seg(1, 1).unwrap();
    let (col, row) = gm.cell(seg.cross.start, 0).unwrap();
    assert_eq!((col, ch(&g, col, row)), (gm.plot.x, '█'), "bars start at the left");
}

/// REQ-CHART-030, REQ-DATA-110, REQ-OPT-010: an unknown mode names the
/// mode; data of no table names the shapes taken; every option read has a
/// default.
#[test]
fn options_and_shapes() {
    let e = BarChart::new(&Options::parse(r#"{"mode":"pie"}"#).unwrap()).err().unwrap().to_string();
    assert_eq!(e, "there is no mode \"pie\": the modes are bar");
    let mut c = chart(generate("categorical-12").unwrap(), r#"{"orient":"horizontal","stack":true}"#);
    let before = draw(&mut c, 40, 12);
    let mut m = Data::new();
    m.push_matrix("m", Matrix::new(2, 2, vec![1.0; 4]).unwrap()).unwrap();
    let e = c.set_data(m).err().unwrap().to_string();
    assert!(e.contains("counts") && e.contains("values"), "{e}");
    assert_eq!(draw(&mut c, 40, 12), before, "the chart keeps its data");
    c.input(&mv(0.0, 20.5, 6.5));
    c.table(10);
    assert!(c.options().read_without_default().is_empty(), "{:?}", c.options().read_without_default());
}

/// REQ-OVERLAY-010: hovers redraw the overlay only, the hovered bar in the
/// accent ink with the readout; the frame equals a fresh chart's with the
/// same view.
#[test]
fn hover_redraws_only_the_overlay() {
    let mut c = chart(generate("categorical-12").unwrap(), "");
    let mut g = Grid::new(80, 24);
    c.frame(0.0, &Env::plain(), &mut g);
    for i in 0..40 {
        let ch = c.input(&mv(0.1 * f64::from(i), 8.0 + f64::from(i) * 1.7, 12.5));
        assert!(!ch.base);
        c.frame(0.1 * f64::from(i), &Env::plain(), &mut g);
    }
    assert_eq!(c.base_draws(), 1);
    assert!(g.inks().contains(&crate::palette::ACCENT), "the hovered bar");
    let mut fresh = chart(generate("categorical-12").unwrap(), "");
    fresh.set_view(&ViewPatch::whole(c.view()));
    let mut h = Grid::new(80, 24);
    fresh.frame(3.9, &Env::plain(), &mut h);
    assert_eq!(g, h);
}
