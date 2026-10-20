//! The real datasets of `tests/data/` (REQ-FIX-020): each is small, listed
//! with its source, licence, retrieval date and transform, read by the data
//! contract, and drawn by every chart that takes its shape.

mod datasets;

use std::collections::BTreeSet;

use glyphplot::chart::Env;
use glyphplot::grid::{Grid, BLANK};
use glyphplot::options::Options;
use glyphplot::registry;

use datasets::{dir, load, text, DATASETS};

/// The largest file the requirement allows: under 100 KB.
const LIMIT: u64 = 100_000;

/// REQ-FIX-020: every file under `tests/data/` but the README is a dataset
/// under 100 KB that the README lists with a source URL, a licence that is
/// public domain or MIT-compatible, a retrieval date and a transform, and
/// that the data contract reads; the loader lists the same files.
#[test]
fn every_dataset_is_small_and_listed() {
    let readme = text("README.md");
    let files: BTreeSet<String> =
        std::fs::read_dir(dir()).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).filter(|n| n != "README.md").collect();
    let loaded: BTreeSet<String> = DATASETS.iter().map(|d| d.file.to_string()).collect();
    assert_eq!(files, loaded, "the directory and the loader list the same datasets");
    for f in &files {
        let size = std::fs::metadata(dir().join(f)).unwrap().len();
        assert!(size < LIMIT, "tests/data/{f} is {size} B, not under 100 KB");
        let row = readme.lines().find(|l| l.starts_with(&format!("| `{f}` |"))).unwrap_or_else(|| panic!("README.md has no row for {f}"));
        let cells: Vec<&str> = row.split(" | ").collect();
        assert_eq!(cells.len(), 5, "{f}: the row has a file, source, licence, date and transform: {row}");
        assert!(cells[1].contains("<https://"), "{f}: the source names its URL");
        assert!(cells[2].contains("Public domain") || cells[2].contains("MIT"), "{f}: the licence is public domain or MIT-compatible");
        assert!(cells[3].len() == 10 && cells[3].chars().filter(|&c| c == '-').count() == 2, "{f}: the retrieval date is YYYY-MM-DD: {}", cells[3]);
        assert!(cells[4].trim_end_matches(" |").len() > 20, "{f}: the transform is written out");
        let data = load(f);
        let table = data.first_table().unwrap();
        assert!(table.len() > 100, "{f} holds {} rows", table.len());
    }
}

/// Every chart that takes a dataset's shape draws it at every size of the
/// contract's set and in colour, one ink and on paper without panicking,
/// keeps the grid's size, inks its plot, and hits a datum there; the line
/// chart's WTI frame reads its price range.
#[test]
fn real_datasets_draw_in_every_chart() {
    let envs = [Env::plain(), Env { mono: true, ..Env::plain() }, Env { paper: true, ..Env::plain() }];
    let mut drawn = 0;
    for d in &DATASETS {
        let data = load(d.file);
        for entry in registry::entries().filter(|e| e.meta.family != "selftest") {
            let options = Options::parse(if entry.meta.slug == "line" { d.line_options } else { "{}" }).unwrap();
            let Ok(mut chart) = (entry.make)(&options.for_meta(entry.meta)) else { continue };
            if chart.set_data(data.clone()).is_err() {
                continue;
            }
            for env in &envs {
                for (cols, rows) in [(1, 1), (usize::from(entry.meta.min.cols), usize::from(entry.meta.min.rows)), (40, 12), (80, 24), (320, 120)] {
                    let mut g = Grid::new(cols, rows);
                    chart.frame(0.0, env, &mut g);
                    drawn += 1;
                    assert_eq!((g.cols(), g.rows()), (cols, rows), "{} · {}", entry.meta.slug, d.file);
                    if (cols, rows) == (80, 24) {
                        let inked = g.glyphs().iter().filter(|&&c| c != BLANK).count();
                        assert!(inked > 200, "{} · {} · 80 × 24: {inked} cells inked", entry.meta.slug, d.file);
                        let hit = (20..60).find_map(|x| chart.hit(glyphplot::chart::CellPos { x: f64::from(x) + 0.5, y: 12.5 }));
                        assert!(hit.is_some(), "{} · {}: no datum under the plot's middle row", entry.meta.slug, d.file);
                    }
                }
            }
        }
    }
    assert!(drawn >= 3 * 3 * 5, "every dataset was drawn by a chart: {drawn} frames");
    // WTI's prices, from the line chart's accessible summary.
    let (_, mut line) = registry::make("line", "").unwrap();
    line.set_data(load("wti-daily.json")).unwrap();
    let mut g = Grid::new(80, 24);
    line.frame(0.0, &Env::plain(), &mut g);
    let summary = line.table(10).summary;
    assert!(summary.contains("1 series (price)") && summary.contains("250 points"), "{summary}");
}
