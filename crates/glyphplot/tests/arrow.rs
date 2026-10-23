//! The core's Arrow IPC stream reader against streams pyarrow wrote
//! (REQ-DATA-140, ADR-105). `scripts/arrow-fixtures.py` writes the streams
//! under `tests/fixtures/arrow/` and records in `expected.json` what pyarrow
//! reads back from each; every value the reader gives is checked against it,
//! floats bit for bit.

use std::path::PathBuf;

use glyphplot::data::arrow::ArrowError;
use glyphplot::data::{ColumnType, Data, Table, TimeUnit, MISSING_CODE};
use glyphplot::registry;
use proptest::prelude::*;
use serde_json::Value;

/// A file of `tests/fixtures/arrow/`.
fn fixture(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/arrow").join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// What `scripts/arrow-fixtures.py` recorded.
fn expected() -> Value {
    serde_json::from_slice(&fixture("expected.json")).unwrap()
}

/// The stream `name`.arrows, read.
fn read(name: &str) -> Table {
    Table::from_arrow_ipc(&fixture(&format!("{name}.arrows"))).unwrap_or_else(|e| panic!("{name}.arrows: {e}"))
}

/// The error the stream `bytes` gives.
fn error(bytes: &[u8]) -> String {
    match Table::from_arrow_ipc(bytes) {
        Ok(t) => panic!("read {} rows where an error was expected", t.len()),
        Err(e) => e.to_string(),
    }
}

/// The column type and time unit a pyarrow type string reads as.
fn kind(arrow: &str) -> (ColumnType, Option<TimeUnit>) {
    let unit = |u: &str| match u {
        "s" => TimeUnit::S,
        "ms" => TimeUnit::Ms,
        "us" => TimeUnit::Us,
        _ => TimeUnit::Ns,
    };
    match arrow {
        "double" => (ColumnType::F64, None),
        "float" => (ColumnType::F32, None),
        "int8" | "int16" | "int32" | "int64" => (ColumnType::I64, None),
        "uint8" | "uint16" | "uint32" => (ColumnType::U32, None),
        "bool" => (ColumnType::Bool, None),
        "string" | "large_string" => (ColumnType::String, None),
        t if t.starts_with("dictionary<") => (ColumnType::Category, None),
        t if t.starts_with("timestamp[") => (ColumnType::Timestamp, Some(unit(t[10..].split([']', ',']).next().unwrap()))),
        t => panic!("expected.json has a type the test does not know: {t}"),
    }
}

/// REQ-DATA-140, REQ-DATA-020, REQ-DATA-030: every stream pyarrow wrote
/// (each type, nulls, several batches with an empty and a sliced one,
/// delta and replaced dictionaries, ns past 2^53, an empty table, the
/// legacy framing) reads as pyarrow reads it: every column's name, type,
/// unit, zone, and every value exactly, a null as missing.
#[test]
fn every_stream_reads_as_pyarrow_reads_it() {
    let all = expected();
    let read_all = all["read"].as_object().unwrap();
    assert_eq!(read_all.len(), 6, "types, batches, replaced, ns, empty, legacy");
    for (name, want) in read_all {
        let table = read(name);
        let rows = want["rows"].as_u64().unwrap() as usize;
        assert_eq!(table.len(), rows, "{name}: rows");
        let cols = want["columns"].as_array().unwrap();
        assert_eq!(table.columns().len(), cols.len(), "{name}: columns");
        for (c, w) in table.columns().iter().zip(cols) {
            let at = format!("{name}.{}", c.name);
            assert_eq!(c.name, w["name"].as_str().unwrap(), "{at}: name");
            let (ty, unit) = kind(w["type"].as_str().unwrap());
            assert_eq!(c.kind, ty, "{at}: type");
            assert_eq!(c.time_unit, unit, "{at}: time unit");
            assert_eq!(c.zone.as_deref(), w.get("zone").and_then(Value::as_str), "{at}: zone");
            let values = w["values"].as_array().unwrap();
            assert_eq!(c.len(), rows, "{at}: length");
            for (row, v) in values.iter().enumerate() {
                let at = format!("{at}[{row}]");
                match (c.kind, v) {
                    (ColumnType::Category, Value::Null) => assert_eq!(c.codes().unwrap()[row], MISSING_CODE, "{at}"),
                    (ColumnType::String, Value::Null) => assert_eq!(c.texts().unwrap()[row], "", "{at}"),
                    (_, Value::Null) => assert!(c.is_missing(row), "{at}: missing"),
                    (ColumnType::Category, Value::String(s)) => assert_eq!(c.label(row), Some(s.as_str()), "{at}"),
                    (ColumnType::String, Value::String(s)) => assert_eq!(&c.texts().unwrap()[row], s, "{at}"),
                    (ColumnType::Timestamp, v) => assert_eq!(c.exact(row), v.as_i64(), "{at}: exact"),
                    (ColumnType::Bool, Value::Bool(b)) => assert_eq!(c.get(row), *b as u8 as f64, "{at}"),
                    // Floats are recorded as text, which `parse` rounds correctly.
                    (ColumnType::F64 | ColumnType::F32, Value::String(s)) => assert_eq!(c.get(row).to_bits(), s.parse::<f64>().unwrap().to_bits(), "{at}"),
                    (_, v) => assert_eq!(Some(c.get(row)), v.as_i64().map(|v| v as f64), "{at}"),
                }
            }
        }
    }
}

/// REQ-DATA-020: ns instants past 2^53 read back exactly through the `i64`
/// origin, and int64 values at ±2^53 exactly as `f64`.
#[test]
fn ns_past_2_53_is_exact() {
    let t = read("ns");
    let ns = t.by_name("t").unwrap();
    assert_eq!((ns.time_unit, ns.origin), (Some(TimeUnit::Ns), 1792281600000000001));
    let back: Vec<Option<i64>> = (0..4).map(|r| ns.exact(r)).collect();
    assert_eq!(back, [Some(1792281600000000001), Some(1792281600000000002), None, Some(1792281600000000004)]);
    let n = t.by_name("n").unwrap();
    assert_eq!(n.nums().unwrap()[..2], [-(2f64.powi(53)), 2f64.powi(53)]);
    assert_eq!(n.get(3), 2f64.powi(53) - 1.0);
}

/// Several dictionaries: deltas extend a column's labels, a replacement
/// adds its new labels after the old and remaps its indices.
#[test]
fn dictionary_deltas_and_replacements_keep_one_label_list() {
    let deltas = read("batches");
    let series = deltas.by_name("series").unwrap();
    assert_eq!(series.categories, ["a", "b", "c", "d", "e"]);
    let replaced = read("replaced");
    let kind = replaced.by_name("kind").unwrap();
    assert_eq!(kind.categories, ["red", "green", "blue"]);
    assert_eq!(kind.codes().unwrap(), [0, 1, 0, 2, 0, MISSING_CODE]);
}

/// An empty table keeps its columns and their types.
#[test]
fn an_empty_stream_is_a_table_of_no_rows() {
    let t = read("empty");
    let kinds: Vec<ColumnType> = t.columns().iter().map(|c| c.kind).collect();
    assert_eq!((t.len(), kinds), (0, vec![ColumnType::Timestamp, ColumnType::F64, ColumnType::Category]));
}

/// The streams draw: a line chart takes the batches' x and y.
#[test]
fn a_line_chart_takes_a_stream() {
    let (_, mut chart) = registry::make("line", "{}").unwrap();
    let accepted = chart.set_data(Data::from_table(read("batches"))).unwrap();
    assert_eq!(accepted.rows, 2 + 3 + 11 + 1);
}

/// Bad input is an error naming what is wrong: compression, an unsupported
/// type and its column, the file format, a wrong continuation, a truncated
/// stream, no stream at all.
#[test]
fn bad_streams_are_errors_naming_what_is_wrong() {
    let refused = expected()["refused"].clone();
    assert_eq!(refused, serde_json::json!(["compressed", "unsupported"]));
    assert!(error(&fixture("compressed.arrows")).contains("compressed with ZSTD; write the stream uncompressed"));
    assert!(error(&fixture("unsupported.arrows")).contains("column `day` is Date, which is not read"));
    assert!(error(b"ARROW1\0\0rest").contains("Arrow file format"));
    assert!(error(b"").contains("no schema"));
    let types = fixture("types.arrows");
    let mut wrong = types.clone();
    wrong[..4].copy_from_slice(&[0xFF, 0xFF, 0xFF, 0x7F]);
    assert!(error(&wrong).contains("the message at byte 0: its metadata runs past the end"), "{}", error(&wrong));
    assert!(error(&types[..types.len() / 2]).contains("runs past the end: the stream is truncated"));
    assert!(error(&types[..6]).contains("the message at byte 0: the stream ends inside it"));
    // A schema with no batch is a table; a batch cut short is not.
    let schema_len = 8 + u32::from_le_bytes(types[4..8].try_into().unwrap()) as usize;
    assert_eq!(Table::from_arrow_ipc(&types[..schema_len]).map(|t| t.len()), Ok(0));
    assert!(matches!(Table::from_arrow_ipc(&types[..schema_len + 9]), Err(ArrowError(_))));
}

/// Every prefix of every fixture reads or errs, and never panics.
#[test]
fn every_truncation_of_every_fixture_is_handled() {
    for name in ["types", "batches", "replaced", "ns", "empty", "legacy", "compressed", "unsupported"] {
        let bytes = fixture(&format!("{name}.arrows"));
        for n in 0..bytes.len() {
            let _ = Table::from_arrow_ipc(&bytes[..n]);
        }
    }
}

proptest! {
    /// REQ-DATA-140: random bytes never panic the reader.
    #[test]
    fn random_bytes_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..1024)) {
        let _ = Table::from_arrow_ipc(&bytes);
        let mut framed = vec![0xFF, 0xFF, 0xFF, 0xFF, bytes.len() as u8, 0, 0, 0];
        framed.extend(&bytes);
        let _ = Table::from_arrow_ipc(&framed);
    }

    /// REQ-DATA-140: a valid stream cut anywhere, with bytes changed, never
    /// panics the reader.
    #[test]
    fn damaged_streams_never_panic(
        which in 0usize..6,
        cut in any::<prop::sample::Index>(),
        changes in proptest::collection::vec((any::<prop::sample::Index>(), any::<u8>()), 0..8),
    ) {
        let name = ["types", "batches", "replaced", "ns", "empty", "legacy"][which];
        let mut bytes = fixture(&format!("{name}.arrows"));
        for (at, b) in changes {
            let i = at.index(bytes.len());
            bytes[i] = b;
        }
        let n = cut.index(bytes.len() + 1);
        let _ = Table::from_arrow_ipc(&bytes[..n]);
    }
}
