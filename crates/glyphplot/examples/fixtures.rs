//! The fixture datasets for Node (REQ-FIX-010, ADR-128).
//!
//! ```sh
//! cargo run -q -p glyphplot --example fixtures -- list           # one fixture a line: name, shape, series, rows, seed
//! cargo run -q -p glyphplot --example fixtures -- write <dir>    # every fixture as below
//! ```
//!
//! `write` generates one fixture at a time and writes:
//!
//! - `<dir>/<fixture>/<table>.<column>.f64`: a numeric or category column's
//!   values as little-endian `f64`, 8 bytes a row, nothing else. Timestamps
//!   are epoch milliseconds (or, for a `us` or `ns` column, offsets from the
//!   index's `origin`); missing is NaN; a category's codes are written as
//!   numbers, a missing code as 4294967295.
//! - `<dir>/<fixture>/<matrix>.f64`: a matrix's values row by row, the same way.
//! - `<dir>/index.json`: `{"version": 1, "fixtures": [...]}`, a fixture a
//!   value of `name`, `shape`, `series`, `rows`, `seed` and `digest` (both
//!   hexadecimal strings, as they exceed 2^53), `tables` and `matrices`. A
//!   table is `name`, `length` and `columns`; a column is `name`, `type`,
//!   `role`, `time_unit`, `origin`, `zone`, `unit` and `categories` where they
//!   apply, and `file`, the path from `<dir>`. A string column has `values`
//!   in the index instead of a file. A matrix is `name`, `rows`, `cols`,
//!   `row_labels`, `col_labels` and `file`.

use std::fs;
use std::path::Path;
use std::process::ExitCode;

use glyphplot::data::{Data, Values, MISSING_CODE};
use glyphplot::fixtures::{digest, FIXTURES};
use serde_json::{json, Map, Value};

/// `values` as little-endian `f64` bytes.
fn le_bytes(values: impl Iterator<Item = f64>) -> Vec<u8> {
    values.flat_map(f64::to_le_bytes).collect()
}

/// Writes one fixture's files under `dir/name/` and returns its tables and matrices for the index.
fn write_fixture(dir: &Path, name: &str, data: &Data) -> std::io::Result<(Vec<Value>, Vec<Value>)> {
    fs::create_dir_all(dir.join(name))?;
    let mut tables = Vec::new();
    for (tname, table) in data.tables() {
        let mut columns = Vec::new();
        for c in table.columns() {
            let mut col = Map::new();
            col.insert("name".into(), json!(c.name));
            col.insert("type".into(), json!(c.kind.name()));
            if let Some(role) = c.role {
                col.insert("role".into(), json!(role.name()));
            }
            if let Some(unit) = c.time_unit {
                col.insert("time_unit".into(), json!(unit.name()));
                if unit.has_origin() {
                    col.insert("origin".into(), json!(c.origin.to_string()));
                }
            }
            if let Some(zone) = &c.zone {
                col.insert("zone".into(), json!(zone));
            }
            if let Some(unit) = &c.unit {
                col.insert("unit".into(), json!(unit));
            }
            let file = format!("{name}/{tname}.{}.f64", c.name);
            match &c.values {
                Values::Num(v) => {
                    fs::write(dir.join(&file), le_bytes(v.iter().copied()))?;
                    col.insert("file".into(), json!(file));
                }
                Values::Codes(v) => {
                    fs::write(dir.join(&file), le_bytes(v.iter().map(|&k| if k == MISSING_CODE { MISSING_CODE as f64 } else { k as f64 })))?;
                    col.insert("categories".into(), json!(c.categories));
                    col.insert("file".into(), json!(file));
                }
                Values::Text(v) => {
                    col.insert("values".into(), json!(v));
                }
            }
            columns.push(Value::Object(col));
        }
        tables.push(json!({ "name": tname, "length": table.len(), "columns": columns }));
    }
    let mut matrices = Vec::new();
    for (mname, m) in data.matrices() {
        let file = format!("{name}/{mname}.f64");
        fs::write(dir.join(&file), le_bytes(m.values().iter().copied()))?;
        matrices.push(json!({
            "name": mname, "rows": m.rows(), "cols": m.cols(), "row_labels": m.row_labels, "col_labels": m.col_labels, "file": file
        }));
    }
    Ok((tables, matrices))
}

fn write(dir: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    let mut index = Vec::new();
    for f in &FIXTURES {
        // One at a time: the million-row fixtures never live together.
        let data = f.generate();
        let (tables, matrices) = write_fixture(dir, f.name, &data)?;
        index.push(json!({
            "name": f.name,
            "shape": f.shape.name(),
            "series": f.series,
            "rows": f.rows,
            "seed": format!("{:#018x}", f.seed),
            "digest": format!("{:#018x}", digest(&data)),
            "tables": tables,
            "matrices": matrices,
        }));
    }
    let text = serde_json::to_string_pretty(&json!({ "version": 1, "fixtures": index })).expect("JSON of strings and numbers");
    fs::write(dir.join("index.json"), text + "\n")
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["list"] => {
            for f in &FIXTURES {
                println!("{}\t{}\t{}\t{}\t{:#018x}", f.name, f.shape.name(), f.series, f.rows, f.seed);
            }
            ExitCode::SUCCESS
        }
        ["write", dir] => match write(Path::new(dir)) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("fixtures: {dir}: {e}");
                ExitCode::FAILURE
            }
        },
        _ => {
            eprintln!("usage: fixtures list | fixtures write <dir>");
            ExitCode::from(2)
        }
    }
}
