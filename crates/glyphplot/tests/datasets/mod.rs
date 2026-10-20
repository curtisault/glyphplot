//! The real datasets of `tests/data/` (REQ-FIX-020), for any test to load:
//! `mod datasets;` then [`load`] or [`DATASETS`].
#![allow(dead_code)]

use std::path::PathBuf;

use glyphplot::data::Data;

/// One dataset: its file under `tests/data/` and the options that draw it
/// as a line chart.
pub struct Dataset {
    /// The file's name.
    pub file: &'static str,
    /// Options for a line chart of it: which columns to draw.
    pub line_options: &'static str,
}

/// Every dataset, as `tests/data/README.md` lists them.
pub const DATASETS: [Dataset; 3] = [
    Dataset { file: "wti-daily.json", line_options: "{}" },
    Dataset { file: "treasury-par-yield-2026.json", line_options: "{}" },
    Dataset { file: "usgs-earthquakes-day.json", line_options: r#"{"encode":{"y":"mag"}}"# },
];

/// The directory the datasets are in.
pub fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data")
}

/// A dataset's JSON text.
pub fn text(file: &str) -> String {
    let path = dir().join(file);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// A dataset parsed as the data contract reads it.
pub fn load(file: &str) -> Data {
    Data::from_json(&text(file)).unwrap_or_else(|e| panic!("tests/data/{file}: {e}"))
}
