//! The data contract's JSON form, deserialised by serde in the shape of
//! `spec/schemas/data.schema.json` (REQ-DATA-010): the full form of named
//! tables and matrices, the columns shorthand (`{"t": […], "close": […]}`)
//! and the rows shorthand (`[{…}, …]`).
//!
//! Integers are read through serde_json's `i64` and `u64`, never through
//! `f64`, so a nanosecond timestamp keeps every digit (REQ-DATA-020). Tables,
//! matrices and columns keep the order the JSON gives them. A descriptor
//! without `values`, as the binary API sends it, reserves its columns as
//! missing for buffers to fill.
//!
//! The element parses JSON in the shell with `JSON.parse` (REQ-DATA-120), and
//! no chart module may hold a JSON data parser (REQ-DATA-050): reading
//! values from JSON ([`Data::from_json`], a descriptor's `values`) is behind
//! the `text` feature, which the CLI, native users and the tests turn on and
//! the wasm chart modules leave off. Without it a descriptor is read without
//! numeric values ([`Data::from_descriptor`]), its columns filled from typed
//! arrays moved in whole ([`Data::from_typed`], [`Buffer`]); only a `string`
//! column's labels travel in the descriptor, as a category column's do.

#[cfg(feature = "text")]
use std::collections::BTreeMap;
use std::fmt;
use std::marker::PhantomData;

#[cfg(feature = "text")]
use serde::de::SeqAccess;
use serde::de::{self, Deserializer, MapAccess, Visitor};
use serde::Deserialize;

#[cfg(feature = "text")]
use super::{rfc3339, Stamp, SHORTHAND_TABLE};
use super::{Column, ColumnType, Data, DataError, Frame, Matrix, Role, Table, TimeUnit, Values, MAX_CATEGORIES, MAX_COLUMNS, MISSING_CODE};

/// The most characters in a `string` value (data.schema.json).
const MAX_STRING: usize = 256;

/// The most characters in a `unit` (data.schema.json).
const MAX_UNIT: usize = 32;

/// One JSON value of a column, as JSON wrote it: integers stay integers.
#[cfg(feature = "text")]
#[derive(Clone, Debug, PartialEq)]
pub enum Cell {
    /// `null`: missing.
    Null,
    /// `true` or `false`.
    Bool(bool),
    /// A negative integer, or any integer when read as signed.
    Int(i64),
    /// A non-negative integer.
    UInt(u64),
    /// A number with a fraction or an exponent.
    Float(f64),
    /// A string.
    Str(String),
}

#[cfg(feature = "text")]
impl<'de> Deserialize<'de> for Cell {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Cell, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Cell;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a number, string, boolean or null")
            }
            fn visit_bool<E>(self, v: bool) -> Result<Cell, E> {
                Ok(Cell::Bool(v))
            }
            fn visit_i64<E>(self, v: i64) -> Result<Cell, E> {
                Ok(Cell::Int(v))
            }
            fn visit_u64<E>(self, v: u64) -> Result<Cell, E> {
                Ok(Cell::UInt(v))
            }
            fn visit_f64<E>(self, v: f64) -> Result<Cell, E> {
                Ok(Cell::Float(v))
            }
            fn visit_str<E>(self, v: &str) -> Result<Cell, E> {
                Ok(Cell::Str(v.to_string()))
            }
            fn visit_string<E>(self, v: String) -> Result<Cell, E> {
                Ok(Cell::Str(v))
            }
            fn visit_unit<E>(self) -> Result<Cell, E> {
                Ok(Cell::Null)
            }
            fn visit_none<E>(self) -> Result<Cell, E> {
                Ok(Cell::Null)
            }
            fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Cell, D::Error> {
                Cell::deserialize(d)
            }
        }
        d.deserialize_any(V)
    }
}

/// Values in a descriptor where the build reads none: a descriptor naming
/// `values` is refused, as column values cross the boundary as typed arrays
/// (REQ-DATA-050).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoValues;

impl<'de> Deserialize<'de> for NoValues {
    fn deserialize<D: Deserializer<'de>>(_: D) -> Result<NoValues, D::Error> {
        Err(de::Error::custom("this build reads no `values` from JSON: column values cross as typed arrays (REQ-DATA-050)"))
    }
}

/// A column's values as a descriptor carries them: JSON cells with the
/// `text` feature.
#[cfg(feature = "text")]
pub type Cells = Vec<Cell>;
/// A column's values as a descriptor carries them: a `string` column's
/// labels only, without the `text` feature.
#[cfg(not(feature = "text"))]
pub type Cells = Labels;

/// A `string` column's labels in a descriptor where the build reads no JSON
/// values: strings and nulls (missing), as the shell sends them beside the
/// typed arrays. Any other value is refused, as column values cross as typed
/// arrays (REQ-DATA-050).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Labels(pub Vec<Option<String>>);

impl<'de> Deserialize<'de> for Labels {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Labels, D::Error> {
        Vec::<Option<String>>::deserialize(d).map(Labels).map_err(|_: D::Error| {
            de::Error::custom("this build reads only a `string` column's labels from JSON: column values cross as typed arrays (REQ-DATA-050)")
        })
    }
}

/// One column's values as the wasm boundary hands them over, moved into the
/// column without a copy (REQ-DATA-050, REQ-MEM-020).
#[derive(Clone, Debug, PartialEq)]
pub enum Buffer {
    /// Numbers, booleans (1, 0), timestamps and durations in the column's
    /// `time_unit` (default ms), or category codes as numbers; NaN is
    /// missing.
    F64(Vec<f64>),
    /// Category codes into the column's `categories`; [`MISSING_CODE`] is
    /// missing.
    U32(Vec<u32>),
}

impl Buffer {
    /// The number of values.
    pub fn len(&self) -> usize {
        match self {
            Buffer::F64(v) => v.len(),
            Buffer::U32(v) => v.len(),
        }
    }

    /// Whether it holds no values.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// A matrix's values as a descriptor carries them: JSON numbers and nulls
/// with the `text` feature.
#[cfg(feature = "text")]
pub type MatrixCells = Vec<Option<f64>>;
/// A matrix's values as a descriptor carries them: none without the `text`
/// feature.
#[cfg(not(feature = "text"))]
pub type MatrixCells = NoValues;

/// A JSON object read as its entries in document order.
struct Ordered<T>(Vec<(String, T)>);

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Ordered<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Ordered<T>, D::Error> {
        struct V<T>(PhantomData<T>);
        impl<'de, T: Deserialize<'de>> Visitor<'de> for V<T> {
            type Value = Ordered<T>;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an object")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Ordered<T>, A::Error> {
                let mut out = Vec::new();
                while let Some((k, v)) = map.next_entry::<String, T>()? {
                    out.push((k, v));
                }
                Ok(Ordered(out))
            }
        }
        d.deserialize_map(V(PhantomData))
    }
}

/// A table as data.schema.json describes it.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TableDesc {
    /// Rows; needed when no column carries values.
    pub length: Option<u64>,
    /// A column the rows are said to be in non-decreasing order of.
    pub sorted: Option<String>,
    /// The coordinate frame of angle or position columns.
    pub frame: Option<Frame>,
    /// The equinox or epoch of a celestial frame.
    pub epoch: Option<String>,
    /// The columns, 1 to 256.
    pub columns: Vec<ColumnDesc>,
}

/// A column as data.schema.json describes it.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColumnDesc {
    /// The column's name.
    pub name: String,
    /// The column's type.
    #[serde(rename = "type")]
    pub kind: ColumnType,
    /// What the column is to a chart.
    pub role: Option<Role>,
    /// Display text for the unit.
    pub unit: Option<String>,
    /// Display scale.
    pub scale: Option<f64>,
    /// Display offset.
    pub offset: Option<f64>,
    /// The unit of a timestamp or duration.
    pub time_unit: Option<TimeUnit>,
    /// The zone ticks are drawn in.
    pub zone: Option<String>,
    /// A category column's labels.
    pub categories: Option<Vec<String>>,
    /// The `i64` origin, in `time_unit`, that a `us` or `ns` timestamp
    /// column's buffer holds offsets from, so instants past 2^53 units cross
    /// a buffer of `f64` exactly (REQ-DATA-020); without it the least value
    /// is the origin. Read only with a buffer ([`Data::from_typed`]) and only
    /// for `us` and `ns` timestamps.
    pub origin: Option<i64>,
    /// The values, absent when they arrive as a buffer.
    pub values: Option<Cells>,
}

/// A matrix as data.schema.json describes it.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MatrixDesc {
    /// Rows.
    pub rows: u64,
    /// Columns.
    pub cols: u64,
    /// `rows × cols` values row by row, null missing; absent when they arrive as a buffer.
    pub values: Option<MatrixCells>,
    /// A label a row.
    pub row_labels: Option<Vec<String>>,
    /// A label a column.
    pub col_labels: Option<Vec<String>>,
    /// Display text for the unit.
    pub unit: Option<String>,
    /// Only the upper triangle is read.
    pub symmetric: Option<bool>,
}

/// The options' `encode`: which columns play each channel (REQ-DATA-080).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Encode(Vec<(String, Vec<String>)>);

impl Encode {
    /// `encode` with `channel` played by `columns`, replacing what it had.
    pub fn with(mut self, channel: &str, columns: &[&str]) -> Encode {
        let columns: Vec<String> = columns.iter().map(|c| c.to_string()).collect();
        match self.0.iter_mut().find(|(ch, _)| ch == channel) {
            Some(entry) => entry.1 = columns,
            None => self.0.push((channel.to_string(), columns)),
        }
        self
    }

    /// The columns `encode` names for `channel`.
    pub fn get(&self, channel: &str) -> Option<&[String]> {
        self.0.iter().find(|(ch, _)| ch == channel).map(|(_, c)| c.as_slice())
    }
}

/// A column name or a list of them.
#[derive(Deserialize)]
#[serde(untagged)]
enum OneOrMany {
    One(String),
    Many(Vec<String>),
}

impl<'de> Deserialize<'de> for Encode {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Encode, D::Error> {
        let Ordered(entries) = Ordered::<OneOrMany>::deserialize(d)?;
        Ok(Encode(
            entries
                .into_iter()
                .map(|(k, v)| match v {
                    OneOrMany::One(s) => (k, vec![s]),
                    OneOrMany::Many(v) => (k, v),
                })
                .collect(),
        ))
    }
}

/// The three forms the JSON may take.
#[cfg(feature = "text")]
enum Top {
    Full { tables: Option<Ordered<TableDesc>>, matrices: Option<Ordered<MatrixDesc>>, extra: Option<String> },
    Columns(Vec<(String, Vec<Cell>)>),
    Rows(Vec<Ordered<Cell>>),
}

#[cfg(feature = "text")]
impl<'de> Deserialize<'de> for Top {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Top, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Top;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("the data object, an object of columns or an array of rows")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Top, A::Error> {
                let mut rows = Vec::new();
                while let Some(row) = seq.next_element::<Ordered<Cell>>()? {
                    rows.push(row);
                }
                Ok(Top::Rows(rows))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Top, A::Error> {
                let (mut tables, mut matrices, mut full) = (None, None, false);
                let mut columns = Vec::new();
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "tables" => tables = Some(map.next_value()?),
                        "matrices" => matrices = Some(map.next_value()?),
                        "version" => {
                            let v: u64 = map.next_value()?;
                            if v != 1 {
                                return Err(de::Error::custom(format!("version {v}: this reader takes version 1")));
                            }
                        }
                        _ => {
                            columns.push((key, map.next_value::<Vec<Cell>>()?));
                            continue;
                        }
                    }
                    full = true;
                }
                if full {
                    let extra = columns.into_iter().next().map(|(k, _)| k);
                    Ok(Top::Full { tables, matrices, extra })
                } else {
                    Ok(Top::Columns(columns))
                }
            }
        }
        d.deserialize_any(V)
    }
}

#[cfg(feature = "text")]
impl Data {
    /// Data from its JSON: the full form, the columns shorthand or the rows
    /// shorthand of data.schema.json, each table named [`SHORTHAND_TABLE`] in
    /// a shorthand (REQ-DATA-010). JSON that does not fit the schema is an
    /// error, as are columns of unequal length.
    pub fn from_json(text: &str) -> Result<Data, DataError> {
        let top: Top = serde_json::from_str(text).map_err(|e| DataError::Json(e.to_string()))?;
        match top {
            Top::Full { extra: Some(k), .. } => {
                Err(DataError::Json(format!("unknown field `{k}` beside `tables`; a columns shorthand cannot use `tables`, `matrices` or `version`")))
            }
            Top::Full { tables: None, .. } => Err(DataError::Json("missing field `tables`".into())),
            Top::Full { tables: Some(tables), matrices, extra: None } => {
                if tables.0.is_empty() {
                    return Err(DataError::Json("`tables` needs at least one table".into()));
                }
                let mut data = Data::new();
                for (name, desc) in &tables.0 {
                    data.push_table(name, Table::from_desc(name, desc)?)?;
                }
                for (name, desc) in matrices.map(|m| m.0).unwrap_or_default() {
                    data.push_matrix(&name, Matrix::from_desc(&desc)?)?;
                }
                Ok(data)
            }
            Top::Columns(columns) => {
                if columns.is_empty() {
                    return Err(DataError::Json("a columns shorthand needs at least one column".into()));
                }
                shorthand(columns)
            }
            Top::Rows(rows) => {
                let mut names: Vec<(String, Vec<Cell>)> = Vec::new();
                let mut index: BTreeMap<String, usize> = BTreeMap::new();
                for (r, Ordered(row)) in rows.iter().enumerate() {
                    for (k, v) in row {
                        let i = *index.entry(k.clone()).or_insert_with(|| {
                            names.push((k.clone(), Vec::new()));
                            names.len() - 1
                        });
                        let col = &mut names[i].1;
                        if col.len() > r {
                            col[r] = v.clone();
                        } else {
                            col.resize(r, Cell::Null);
                            col.push(v.clone());
                        }
                    }
                }
                for (_, col) in &mut names {
                    col.resize(rows.len(), Cell::Null);
                }
                shorthand(names)
            }
        }
    }
}

#[cfg(feature = "text")]
/// One table from shorthand columns, types inferred as data.schema.json
/// says: numbers f64, RFC 3339 strings timestamps (ms, UTC), other strings
/// categories, booleans bool; all null is f64.
fn shorthand(columns: Vec<(String, Vec<Cell>)>) -> Result<Data, DataError> {
    let mut out = Vec::with_capacity(columns.len());
    for (name, cells) in columns {
        let present = || cells.iter().filter(|c| **c != Cell::Null);
        let column = if present().all(|c| matches!(c, Cell::Int(_) | Cell::UInt(_) | Cell::Float(_))) {
            Column::f64(&name, nums(&name, &cells)?)
        } else if present().all(|c| matches!(c, Cell::Bool(_))) {
            Column::numbers(&name, ColumnType::Bool, bools(&name, &cells)?)
        } else if present().all(|c| matches!(c, Cell::Str(s) if rfc3339::parse(s).is_some())) {
            let stamps = stamps(&name, TimeUnit::Ms, &cells)?;
            let (_, held) = super::hold_stamps(TimeUnit::Ms, &stamps);
            Column::timestamps_ms(&name, held)
        } else if present().all(|c| matches!(c, Cell::Str(_))) {
            let mut labels: Vec<String> = Vec::new();
            let mut index: BTreeMap<&str, u32> = BTreeMap::new();
            let mut codes = Vec::with_capacity(cells.len());
            for c in &cells {
                codes.push(match c {
                    Cell::Str(s) => *index.entry(s.as_str()).or_insert_with(|| {
                        labels.push(s.clone());
                        labels.len() as u32 - 1
                    }),
                    _ => MISSING_CODE,
                });
            }
            if labels.len() > MAX_CATEGORIES {
                Column::strings(&name, strings(&name, &cells)?)
            } else {
                Column::categories(&name, labels, codes)
            }
        } else {
            return Err(DataError::Value { column: name, row: 0, expected: "values of one type: numbers, booleans or strings" });
        };
        out.push(column);
    }
    if out.len() > MAX_COLUMNS {
        return Err(DataError::Columns { table: SHORTHAND_TABLE.to_string(), count: out.len() });
    }
    Ok(Data::from_table(Table::from_columns(out)?))
}

/// `n` copies of `v`, or an error when that many cannot be held.
fn filled<T: Clone>(n: usize, v: T) -> Result<Vec<T>, DataError> {
    let mut out = Vec::new();
    out.try_reserve_exact(n).map_err(|_| DataError::TooLarge { rows: n })?;
    out.resize(n, v);
    Ok(out)
}

#[cfg(feature = "text")]
fn nums(column: &str, cells: &[Cell]) -> Result<Vec<f64>, DataError> {
    cells
        .iter()
        .enumerate()
        .map(|(row, c)| match *c {
            Cell::Null => Ok(f64::NAN),
            Cell::Int(v) => Ok(v as f64),
            Cell::UInt(v) => Ok(v as f64),
            Cell::Float(v) => Ok(v),
            _ => Err(DataError::Value { column: column.to_string(), row, expected: "a number or null" }),
        })
        .collect()
}

#[cfg(feature = "text")]
fn bools(column: &str, cells: &[Cell]) -> Result<Vec<f64>, DataError> {
    cells
        .iter()
        .enumerate()
        .map(|(row, c)| match *c {
            Cell::Null => Ok(f64::NAN),
            Cell::Bool(b) => Ok(if b { 1.0 } else { 0.0 }),
            _ => Err(DataError::Value { column: column.to_string(), row, expected: "a boolean or null" }),
        })
        .collect()
}

#[cfg(feature = "text")]
fn strings(column: &str, cells: &[Cell]) -> Result<Vec<String>, DataError> {
    cells
        .iter()
        .enumerate()
        .map(|(row, c)| match c {
            Cell::Null => Ok(String::new()),
            Cell::Str(s) if s.chars().count() <= MAX_STRING => Ok(s.clone()),
            Cell::Str(_) => Err(DataError::TooLong { column: column.to_string(), what: "a string", max: MAX_STRING }),
            _ => Err(DataError::Value { column: column.to_string(), row, expected: "a string or null" }),
        })
        .collect()
}

#[cfg(feature = "text")]
fn codes(column: &str, cells: &[Cell], n: usize) -> Result<Vec<u32>, DataError> {
    let code = |row: usize, v: u64| {
        if v < n as u64 {
            Ok(v as u32)
        } else {
            Err(DataError::Code { column: column.to_string(), row, code: v, categories: n })
        }
    };
    let bad = |row: usize| DataError::Value { column: column.to_string(), row, expected: "a category index or null" };
    cells
        .iter()
        .enumerate()
        .map(|(row, c)| match *c {
            Cell::Null => Ok(MISSING_CODE),
            Cell::UInt(v) => code(row, v),
            Cell::Int(v) if v >= 0 => code(row, v as u64),
            Cell::Float(v) if (0.0..1.8e19).contains(&v) && v == (v as u64) as f64 => code(row, v as u64),
            _ => Err(bad(row)),
        })
        .collect()
}

#[cfg(feature = "text")]
/// Timestamps before they are held: integers exact, strings parsed exactly
/// into the unit (milliseconds directly for `s` and `ms`).
fn stamps(column: &str, unit: TimeUnit, cells: &[Cell]) -> Result<Vec<Stamp>, DataError> {
    cells
        .iter()
        .enumerate()
        .map(|(row, c)| match c {
            Cell::Null => Ok(Stamp::Missing),
            Cell::Int(v) => Ok(Stamp::Exact(*v)),
            Cell::UInt(v) => Ok(i64::try_from(*v).map_or(Stamp::Approx(*v as f64), Stamp::Exact)),
            Cell::Float(v) => Ok(Stamp::Approx(*v)),
            Cell::Str(s) => {
                let bad = || DataError::Timestamp { column: column.to_string(), row, text: s.clone() };
                let (secs, nanos) = rfc3339::parse(s).ok_or_else(bad)?;
                match unit {
                    TimeUnit::S | TimeUnit::Ms => Ok(Stamp::Ms(secs as f64 * 1000.0 + nanos as f64 / 1_000_000.0)),
                    TimeUnit::Us => secs.checked_mul(1_000_000).and_then(|v| v.checked_add(nanos as i64 / 1000)).map(Stamp::Exact).ok_or_else(bad),
                    TimeUnit::Ns => secs.checked_mul(1_000_000_000).and_then(|v| v.checked_add(nanos as i64)).map(Stamp::Exact).ok_or_else(bad),
                }
            }
            Cell::Bool(_) => Err(DataError::Value { column: column.to_string(), row, expected: "a number, an RFC 3339 string or null" }),
        })
        .collect()
}

/// The full form's top level, as a descriptor without values reads it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Descriptor {
    version: Option<u64>,
    tables: Ordered<TableDesc>,
    matrices: Option<Ordered<MatrixDesc>>,
}

impl Data {
    /// Data from a descriptor in data.schema.json's full form whose columns
    /// carry no `values`: each column `length` missing values, for typed
    /// arrays to fill (REQ-DATA-050). In every build; without the `text`
    /// feature a descriptor that carries `values` is refused.
    pub fn from_descriptor(json: &str) -> Result<Data, DataError> {
        Data::from_typed(json, Vec::new())
    }

    /// As [`Data::from_typed`], every buffer of `f64`.
    pub fn from_buffers(descriptor: &str, buffers: Vec<Option<Vec<f64>>>) -> Result<Data, DataError> {
        Data::from_typed(descriptor, buffers.into_iter().map(|b| b.map(Buffer::F64)).collect())
    }

    /// Data from a descriptor without numeric values
    /// ([`Data::from_descriptor`]) and one typed array a column, numbered
    /// across the descriptor: every table's columns in order, then each
    /// matrix's values as one more column. This is what the wasm boundary's
    /// `reserve_column`, `set_column` and `commit` deliver (REQ-DATA-050).
    /// Each buffer is moved into its column and converted in place (only a
    /// category column given `f64` codes is copied, into `u32`), and a
    /// column given a buffer is never filled first, so the data holds its
    /// buffers' bytes and no more (REQ-MEM-010, REQ-MEM-020).
    ///
    /// A `None`, or a buffer past the end of `buffers`, leaves its column
    /// missing. Values are held as [`Buffer::F64`] says: a timestamp in its
    /// `time_unit` becomes epoch ms (`s`, `ms`) or an offset from its
    /// `origin` (`us`, `ns`; the least value without one), a duration
    /// becomes ms, an `f32` is rounded, a category code past its labels or
    /// not whole is missing. A `string` column takes no buffer, a buffer of
    /// `u32` fits only a category column, a buffer of the wrong length is an
    /// error, and so is a buffer for a column the descriptor does not have.
    pub fn from_typed(descriptor: &str, buffers: Vec<Option<Buffer>>) -> Result<Data, DataError> {
        let d: Descriptor = serde_json::from_str(descriptor).map_err(|e| DataError::Json(e.to_string()))?;
        if d.version.is_some_and(|v| v != 1) {
            return Err(DataError::Json("this reader takes version 1".into()));
        }
        if d.tables.0.is_empty() {
            return Err(DataError::Json("`tables` needs at least one table".into()));
        }
        let matrices = d.matrices.map(|m| m.0).unwrap_or_default();
        let columns = d.tables.0.iter().map(|(_, t)| t.columns.len()).sum::<usize>() + matrices.len();
        if let Some(extra) = buffers.iter().skip(columns).position(Option::is_some) {
            return Err(DataError::Buffer { index: columns + extra, columns });
        }
        let mut buffers = buffers.into_iter();
        let mut data = Data::new();
        for (name, desc) in &d.tables.0 {
            let own: Vec<Option<Buffer>> = desc.columns.iter().map(|_| buffers.next().flatten()).collect();
            data.push_table(name, Table::from_desc_with(name, desc, own)?)?;
        }
        for (name, desc) in matrices {
            let m = match buffers.next().flatten() {
                None => Matrix::from_desc(&desc)?,
                Some(Buffer::U32(_)) => return Err(DataError::Kind { table: name, column: "values".into(), kind: ColumnType::F64 }),
                Some(Buffer::F64(values)) => Matrix::described(&desc, values)?,
            };
            data.push_matrix(&name, m)?;
        }
        Ok(data)
    }
}

impl Column {
    /// A column from its description; a column without values is `len`
    /// missing values, for a buffer to fill.
    pub fn from_desc(d: &ColumnDesc, len: usize) -> Result<Column, DataError> {
        let name = d.name.as_str();
        if d.time_unit.is_some() && !matches!(d.kind, ColumnType::Timestamp | ColumnType::Duration) {
            return Err(DataError::TimeUnit { column: name.to_string(), kind: d.kind });
        }
        if d.unit.as_ref().is_some_and(|u| u.chars().count() > MAX_UNIT) {
            return Err(DataError::TooLong { column: name.to_string(), what: "unit", max: MAX_UNIT });
        }
        let categories = match (d.kind, &d.categories) {
            (ColumnType::Category, None) => return Err(DataError::Categories { column: name.to_string() }),
            (ColumnType::Category, Some(c)) if c.len() > MAX_CATEGORIES => {
                return Err(DataError::TooLong { column: name.to_string(), what: "categories", max: MAX_CATEGORIES })
            }
            (ColumnType::Category, Some(c)) => c.clone(),
            _ => Vec::new(),
        };
        let unit = d.time_unit.unwrap_or_default();
        #[cfg_attr(not(feature = "text"), allow(unused_mut))]
        let mut origin = 0;
        let values = match (&d.values, d.kind) {
            (None, ColumnType::Category) => Values::Codes(filled(len, MISSING_CODE)?),
            (None, ColumnType::String) => Values::Text(filled(len, String::new())?),
            (None, _) => Values::Num(filled(len, f64::NAN)?),
            #[cfg(feature = "text")]
            (Some(cells), ColumnType::F64 | ColumnType::I64 | ColumnType::U32) => Values::Num(nums(name, cells)?),
            #[cfg(feature = "text")]
            (Some(cells), ColumnType::F32) => Values::Num(nums(name, cells)?.into_iter().map(|v| v as f32 as f64).collect()),
            #[cfg(feature = "text")]
            (Some(cells), ColumnType::Bool) => Values::Num(bools(name, cells)?),
            #[cfg(feature = "text")]
            (Some(cells), ColumnType::Duration) => Values::Num(nums(name, cells)?.into_iter().map(|v| unit.to_ms(v)).collect()),
            #[cfg(feature = "text")]
            (Some(cells), ColumnType::Timestamp) => {
                let (o, held) = super::hold_stamps(unit, &stamps(name, unit, cells)?);
                origin = o;
                Values::Num(held)
            }
            #[cfg(feature = "text")]
            (Some(cells), ColumnType::Category) => Values::Codes(codes(name, cells, categories.len())?),
            #[cfg(feature = "text")]
            (Some(cells), ColumnType::String) => Values::Text(strings(name, cells)?),
            #[cfg(not(feature = "text"))]
            (Some(Labels(labels)), ColumnType::String) => Values::Text(
                labels
                    .iter()
                    .map(|l| match l {
                        Some(s) if s.chars().count() > MAX_STRING => Err(DataError::TooLong { column: name.to_string(), what: "a string", max: MAX_STRING }),
                        Some(s) => Ok(s.clone()),
                        None => Ok(String::new()),
                    })
                    .collect::<Result<_, _>>()?,
            ),
            #[cfg(not(feature = "text"))]
            (Some(_), _) => return Err(DataError::Json("column values cross as typed arrays (REQ-DATA-050)".into())),
        };
        Ok(Column {
            name: name.to_string(),
            kind: d.kind,
            role: d.role,
            unit: d.unit.clone(),
            scale: d.scale,
            offset: d.offset,
            time_unit: matches!(d.kind, ColumnType::Timestamp | ColumnType::Duration).then_some(unit),
            origin,
            zone: d.zone.clone(),
            categories,
            values,
        })
    }
}

/// How many values a column's description carries.
#[cfg(feature = "text")]
fn values_len(c: &ColumnDesc) -> Option<usize> {
    c.values.as_ref().map(Vec::len)
}

/// How many values a column's description carries: a `string` column's
/// labels only without `text`.
#[cfg(not(feature = "text"))]
fn values_len(c: &ColumnDesc) -> Option<usize> {
    c.values.as_ref().map(|l| l.0.len())
}

impl Column {
    /// Moves `buffer` in as this column's values, converting in place as
    /// [`Data::from_typed`] says; `table` names it in errors.
    fn take(&mut self, table: &str, buffer: Buffer, origin: Option<i64>) -> Result<(), DataError> {
        let unit = self.time_unit.unwrap_or_default();
        self.values = match (self.kind, buffer) {
            (ColumnType::String, _) => return Err(DataError::Kind { table: table.to_string(), column: self.name.clone(), kind: self.kind }),
            (ColumnType::Category, Buffer::U32(mut codes)) => {
                let n = self.categories.len();
                codes.iter_mut().filter(|c| **c as usize >= n).for_each(|c| *c = MISSING_CODE);
                Values::Codes(codes)
            }
            (_, Buffer::U32(_)) => return Err(DataError::Kind { table: table.to_string(), column: self.name.clone(), kind: self.kind }),
            (ColumnType::Category, Buffer::F64(v)) => {
                let n = self.categories.len() as f64;
                Values::Codes(v.iter().map(|&v| if v >= 0.0 && v < n && v == v.trunc() { v as u32 } else { MISSING_CODE }).collect())
            }
            (ColumnType::F32, Buffer::F64(mut v)) => {
                v.iter_mut().for_each(|x| *x = *x as f32 as f64);
                Values::Num(v)
            }
            (ColumnType::Duration, Buffer::F64(mut v)) => {
                if unit != TimeUnit::Ms {
                    v.iter_mut().for_each(|x| *x = unit.to_ms(*x));
                }
                Values::Num(v)
            }
            (ColumnType::Timestamp, Buffer::F64(mut v)) => {
                match (unit, origin) {
                    (TimeUnit::S, _) => v.iter_mut().for_each(|x| *x *= 1000.0),
                    (TimeUnit::Ms, _) => {}
                    (_, Some(origin)) => self.origin = origin,
                    (_, None) => {
                        let least = v.iter().copied().filter(|x| x.is_finite()).fold(f64::INFINITY, f64::min);
                        if least.is_finite() {
                            // `as` saturates; an origin of a whole number of units, so offsets stay exact.
                            let origin = least.floor() as i64;
                            let shift = origin as f64;
                            v.iter_mut().for_each(|x| *x -= shift);
                            self.origin = origin;
                        }
                    }
                }
                Values::Num(v)
            }
            (_, Buffer::F64(v)) => Values::Num(v),
        };
        Ok(())
    }
}

impl Table {
    /// A table of no rows with this one's columns, types, roles, units,
    /// origins and labels: what a staging buffer's rows are read by
    /// ([`Table::staged`]).
    pub fn empty_like(&self) -> Table {
        let columns = self
            .columns
            .iter()
            .map(|c| {
                let values = match &c.values {
                    Values::Num(_) => Values::Num(Vec::new()),
                    Values::Codes(_) => Values::Codes(Vec::new()),
                    Values::Text(_) => Values::Text(Vec::new()),
                };
                c.with_values(values)
            })
            .collect();
        Table { len: 0, columns, sorted: self.sorted.clone(), frame: self.frame, epoch: self.epoch.clone() }
    }

    /// `n` rows of this table's columns from a staging buffer (REQ-STREAM-010,
    /// the boundary's `staging(n)` and `append(n)`): column `j`'s values are
    /// `values[j·n .. (j+1)·n]`, in the columns' order, each as a reserved
    /// buffer of kind 0 holds it ([`Data::from_typed`]): numbers; booleans 1
    /// and 0; durations in the column's time unit; timestamps in theirs, a
    /// `us` or `ns` column's as offsets from the column's `origin`; a
    /// category column's codes into its labels; NaN missing. A string
    /// column's rows are missing (its labels do not cross as numbers).
    pub fn staged(&self, values: &[f64], n: usize) -> Result<Table, DataError> {
        let k = self.columns.len();
        let need = k.checked_mul(n).ok_or(DataError::TooLarge { rows: n })?;
        if values.len() < need {
            return Err(DataError::Append(format!("{n} rows of {k} columns need {need} staged values; {} are staged", values.len())));
        }
        let mut columns = Vec::with_capacity(k);
        for (j, c) in self.columns.iter().enumerate() {
            let mut column = c.with_values(Values::Num(Vec::new()));
            if c.kind == ColumnType::String {
                column.values = Values::Text(vec![String::new(); n]);
            } else {
                column.take(super::SHORTHAND_TABLE, Buffer::F64(values[j * n..(j + 1) * n].to_vec()), Some(c.origin))?;
            }
            columns.push(column);
        }
        Ok(Table { len: n, columns, sorted: self.sorted.clone(), frame: self.frame, epoch: self.epoch.clone() })
    }

    /// A table from its description, `name` naming it in errors: 1 to 256
    /// columns of one length, `length` when no column carries values.
    pub fn from_desc(name: &str, d: &TableDesc) -> Result<Table, DataError> {
        Table::from_desc_with(name, d, Vec::new())
    }

    /// As [`Table::from_desc`], with a buffer a column, moved in
    /// ([`Data::from_typed`]); a column given one is never filled first. The
    /// length is `length`, else a column's values', else the first buffer's.
    fn from_desc_with(name: &str, d: &TableDesc, mut buffers: Vec<Option<Buffer>>) -> Result<Table, DataError> {
        if d.columns.is_empty() || d.columns.len() > MAX_COLUMNS {
            return Err(DataError::Columns { table: name.to_string(), count: d.columns.len() });
        }
        buffers.resize_with(d.columns.len(), || None);
        let declared = d.length.map(|n| usize::try_from(n).map_err(|_| DataError::TooLarge { rows: usize::MAX })).transpose()?;
        let len = declared
            .or_else(|| d.columns.iter().find_map(values_len))
            .or_else(|| buffers.iter().flatten().map(Buffer::len).next())
            .ok_or_else(|| DataError::NoLength { table: name.to_string() })?;
        let mut columns = Vec::with_capacity(d.columns.len());
        for (c, buffer) in d.columns.iter().zip(buffers) {
            let column = match buffer {
                None => Column::from_desc(c, len)?,
                Some(b) if b.len() != len => return Err(DataError::Length { table: name.to_string(), column: c.name.clone(), expected: len, found: b.len() }),
                Some(b) => {
                    let mut column = Column::from_desc(&ColumnDesc { values: None, ..c.clone() }, 0)?;
                    column.take(name, b, c.origin)?;
                    column
                }
            };
            columns.push(column);
        }
        let mut table = Table::named(name, columns, Some(len))?;
        if let Some(s) = &d.sorted {
            if table.find(s).is_none() {
                return Err(DataError::Sorted { table: name.to_string(), column: s.clone() });
            }
        }
        table.sorted = d.sorted.clone();
        table.frame = d.frame;
        table.epoch = d.epoch.clone();
        Ok(table)
    }
}

impl Matrix {
    /// A matrix from its description; without values, `rows × cols` missing
    /// values for a buffer to fill.
    pub fn from_desc(d: &MatrixDesc) -> Result<Matrix, DataError> {
        let size = |n: u64| usize::try_from(n).map_err(|_| DataError::TooLarge { rows: usize::MAX });
        let n = size(d.rows)?.checked_mul(size(d.cols)?).ok_or(DataError::TooLarge { rows: usize::MAX })?;
        let values = match &d.values {
            #[cfg(feature = "text")]
            Some(v) => v.iter().map(|x| x.unwrap_or(f64::NAN)).collect(),
            #[cfg(not(feature = "text"))]
            Some(NoValues) => return Err(DataError::Json("matrix values cross as typed arrays (REQ-DATA-050)".into())),
            None => filled(n, f64::NAN)?,
        };
        Matrix::described(d, values)
    }

    /// The matrix `d` describes with `values` moved in, row by row
    /// ([`Data::from_typed`]); `d`'s own values are not read.
    fn described(d: &MatrixDesc, values: Vec<f64>) -> Result<Matrix, DataError> {
        let size = |n: u64| usize::try_from(n).map_err(|_| DataError::TooLarge { rows: usize::MAX });
        let (rows, cols) = (size(d.rows)?, size(d.cols)?);
        if d.unit.as_ref().is_some_and(|u| u.chars().count() > MAX_UNIT) {
            return Err(DataError::TooLong { column: "matrix".into(), what: "unit", max: MAX_UNIT });
        }
        let mut m = Matrix::new(rows, cols, values)?.labelled(d.row_labels.clone().unwrap_or_default(), d.col_labels.clone().unwrap_or_default())?;
        m.unit = d.unit.clone();
        m.symmetric = d.symmetric.unwrap_or(false);
        Ok(m)
    }
}

#[cfg(all(test, feature = "text"))]
mod tests {
    use super::*;

    fn err(json: &str) -> DataError {
        Data::from_json(json).unwrap_err()
    }

    #[test]
    fn refuses_what_the_schema_refuses() {
        assert!(matches!(err("{\"tables\":{}}"), DataError::Json(_)));
        assert!(matches!(err("{\"version\":2,\"tables\":{\"a\":{\"columns\":[{\"name\":\"x\",\"type\":\"f64\",\"values\":[]}]}}}"), DataError::Json(_)));
        assert!(matches!(err("{\"tables\":{\"a\":{\"columns\":[]}}}"), DataError::Columns { .. }));
        assert!(matches!(err("{\"tables\":{\"a\":{\"columns\":[{\"name\":\"x\",\"type\":\"f64\"}]}}}"), DataError::NoLength { .. }));
        assert!(matches!(err("{\"tables\":{\"a\":{\"columns\":[{\"name\":\"x\",\"type\":\"f64\",\"values\":[\"1\"]}]}}}"), DataError::Value { .. }));
        assert!(matches!(err("{\"tables\":{\"a\":{\"columns\":[{\"name\":\"x\",\"type\":\"f128\",\"values\":[]}]}}}"), DataError::Json(_)));
        assert!(matches!(err("{\"tables\":{\"a\":{\"columns\":[{\"name\":\"x\",\"type\":\"f64\",\"colour\":1}]}}}"), DataError::Json(_)));
        assert!(matches!(err("{\"tables\":{\"a\":{\"length\":2,\"columns\":[{\"name\":\"x\",\"type\":\"f64\",\"values\":[1]}]}}}"), DataError::Length { .. }));
        assert!(matches!(
            err("{\"tables\":{\"a\":{\"sorted\":\"t\",\"columns\":[{\"name\":\"x\",\"type\":\"f64\",\"values\":[1]}]}}}"),
            DataError::Sorted { .. }
        ));
        assert!(matches!(err("{\"tables\":{\"a b\":{\"columns\":[{\"name\":\"x\",\"type\":\"f64\",\"values\":[1]}]}}}"), DataError::Name { .. }));
        assert!(matches!(
            err("{\"tables\":{\"a\":{\"columns\":[{\"name\":\"c\",\"type\":\"category\",\"categories\":[\"a\"],\"values\":[1]}]}}}"),
            DataError::Code { .. }
        ));
        assert!(matches!(
            err("{\"tables\":{\"a\":{\"columns\":[{\"name\":\"c\",\"type\":\"category\",\"categories\":[\"a\"],\"values\":[-1]}]}}}"),
            DataError::Value { .. }
        ));
        assert!(matches!(err("{\"tables\":{\"a\":{\"columns\":[{\"name\":\"b\",\"type\":\"bool\",\"values\":[1]}]}}}"), DataError::Value { .. }));
        let long = format!("{{\"tables\":{{\"a\":{{\"columns\":[{{\"name\":\"s\",\"type\":\"string\",\"values\":[\"{}\"]}}]}}}}}}", "é".repeat(257));
        assert!(matches!(err(&long), DataError::TooLong { .. }));
        assert!(matches!(err("{\"tables\":{\"a\":{\"columns\":[{\"name\":\"x\",\"type\":\"f64\",\"values\":[1]}]}},\"matrices\":{\"m\":{\"rows\":2,\"cols\":2,\"values\":[1]}}}"), DataError::Matrix { .. }));
        assert!(matches!(err("{\"x\":[1,\"a\"]}"), DataError::Value { .. }));
        assert!(matches!(err("{}"), DataError::Json(_)));
        assert!(matches!(err("[1]"), DataError::Json(_)));
        assert!(matches!(err("{\"tables\":{\"a\":{\"columns\":[{\"name\":\"x\",\"type\":\"f64\",\"values\":[1]}]}},\"x\":[1]}"), DataError::Json(_)));
        assert!(matches!(err("3"), DataError::Json(_)));
    }

    #[test]
    fn shorthands_infer_types_and_keep_order() {
        let d = Data::from_json(r#"[{"z":1,"s":"b"},{"s":"a","on":true},{"z":2.5,"s":"b","s2":"2026-10-18T00:00:00Z"}]"#).unwrap();
        let t = d.first_table().unwrap();
        let names: Vec<&str> = t.columns().iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["z", "s", "on", "s2"]);
        assert_eq!(t.len(), 3);
        assert!(t.by_name("z").unwrap().is_missing(1));
        let s = t.by_name("s").unwrap();
        assert_eq!((s.kind, s.codes().unwrap(), s.categories.as_slice()), (ColumnType::Category, &[0u32, 1, 0][..], &["b".to_string(), "a".to_string()][..]));
        assert_eq!(t.by_name("on").unwrap().kind, ColumnType::Bool);
        assert_eq!(t.by_name("s2").unwrap().kind, ColumnType::Timestamp);
        let empty = Data::from_json("[]").unwrap();
        assert!(empty.first_table().unwrap().is_empty());
        let big = Data::from_json(r#"{"n":[9007199254740993]}"#).unwrap();
        assert_eq!(big.first_table().unwrap().column(0).unwrap().get(0), 9_007_199_254_740_992.0);
    }

    #[test]
    fn encode_takes_a_name_or_a_list() {
        let e: Encode = serde_json::from_str(r#"{"y":["a","b"],"x":"t"}"#).unwrap();
        assert_eq!(e.get("x"), Some(&["t".to_string()][..]));
        assert_eq!(e.get("y").map(<[String]>::len), Some(2));
        assert_eq!(e.get("open"), None);
        assert!(serde_json::from_str::<Encode>(r#"{"x":1}"#).is_err());
    }
}

#[cfg(test)]
mod buffer_tests {
    use super::*;

    /// REQ-DATA-050: a descriptor without values and typed arrays make the
    /// data, in every build; a descriptor with values is refused without
    /// `text`.
    #[test]
    fn descriptor_and_buffers_make_data() {
        let desc = r#"{"tables":{"p":{"length":3,"sorted":"t","columns":[
            {"name":"t","type":"timestamp","role":"x"},
            {"name":"close","type":"f64","unit":"USD"},
            {"name":"sym","type":"category","categories":["A","B"]},
            {"name":"note","type":"string"}]}},
            "matrices":{"m":{"rows":1,"cols":2}}}"#;
        let empty = Data::from_descriptor(desc).unwrap();
        let t = empty.table("p").unwrap();
        assert_eq!(t.len(), 3);
        assert!(t.by_name("close").unwrap().is_missing(2));
        let d =
            Data::from_buffers(desc, vec![Some(vec![1.0, 2.0, 3.0]), Some(vec![9.5, f64::NAN, 7.0]), Some(vec![1.0, 0.0, 5.0]), None, Some(vec![4.0, 2.0])])
                .unwrap();
        let t = d.table("p").unwrap();
        assert_eq!(t.by_name("t").unwrap().nums().unwrap(), &[1.0, 2.0, 3.0]);
        assert_eq!(t.by_name("close").unwrap().get(0), 9.5);
        assert_eq!((t.by_name("sym").unwrap().label(0), t.by_name("sym").unwrap().label(2)), (Some("B"), None), "past its labels is missing");
        assert_eq!(t.sorted.as_deref(), Some("t"));
        assert_eq!(d.matrix("m").unwrap().get(0, 1), 2.0);
        assert!(Data::from_buffers(desc, vec![Some(vec![1.0])]).is_err(), "the wrong length");
        assert!(Data::from_buffers(desc, vec![None, None, None, Some(vec![0.0, 0.0, 0.0])]).is_err(), "a string column takes no array");
        let with_values = r#"{"tables":{"p":{"columns":[{"name":"x","type":"f64","values":[1,2]}]}}}"#;
        assert_eq!(Data::from_descriptor(with_values).is_ok(), cfg!(feature = "text"));
    }

    /// REQ-DATA-050, REQ-DATA-020: typed buffers as the shell fills them:
    /// u32 category codes, times in their `time_unit`, a `string` column's
    /// labels in the descriptor, and an `origin` that carries nanoseconds
    /// past 2^53 exactly.
    #[test]
    fn typed_buffers_as_the_shell_sends_them() {
        let desc = r#"{"version":1,"tables":{"t":{"length":4,"columns":[
            {"name":"s","type":"timestamp","time_unit":"s"},
            {"name":"ns","type":"timestamp","time_unit":"ns","origin":1792281600000000000},
            {"name":"us","type":"timestamp","time_unit":"us"},
            {"name":"d","type":"duration","time_unit":"s"},
            {"name":"c","type":"category","categories":["a","b"]},
            {"name":"note","type":"string","values":["x",null,"z","w"]}]}}}"#;
        let d = Data::from_typed(
            desc,
            vec![
                Some(Buffer::F64(vec![1.0, 2.0, f64::NAN, 4.5])),
                Some(Buffer::F64(vec![1.0, 2.0, 3.0, 4.0])),
                Some(Buffer::F64(vec![1_792_281_600_000_002.0, 1_792_281_600_000_001.0, f64::NAN, 1_792_281_600_000_003.0])),
                Some(Buffer::F64(vec![1.5, 0.0, f64::NAN, 2.0])),
                Some(Buffer::U32(vec![1, 0, MISSING_CODE, 7])),
            ],
        )
        .unwrap();
        let t = d.table("t").unwrap();
        assert_eq!(t.by_name("s").unwrap().nums().unwrap()[..2], [1000.0, 2000.0]);
        assert_eq!(t.by_name("s").unwrap().get(3), 4500.0);
        let ns = t.by_name("ns").unwrap();
        assert_eq!(
            (0..4).map(|r| ns.exact(r).unwrap()).collect::<Vec<_>>(),
            [1_792_281_600_000_000_001, 1_792_281_600_000_000_002, 1_792_281_600_000_000_003, 1_792_281_600_000_000_004]
        );
        let us = t.by_name("us").unwrap();
        assert_eq!(
            (us.origin, us.exact(1), us.exact(3), us.is_missing(2)),
            (1_792_281_600_000_001, Some(1_792_281_600_000_001), Some(1_792_281_600_000_003), true)
        );
        assert_eq!(t.by_name("d").unwrap().get(0), 1500.0);
        let c = t.by_name("c").unwrap();
        assert_eq!((c.label(0), c.label(1), c.label(2), c.label(3)), (Some("b"), Some("a"), None, None), "a code past the labels is missing");
        let note = t.by_name("note").unwrap();
        assert_eq!((note.label(0), note.label(1), note.label(3)), (Some("x"), None, Some("w")));

        let xy = r#"{"tables":{"t":{"length":2,"columns":[{"name":"x","type":"f64"},{"name":"note","type":"string"}]}}}"#;
        assert!(matches!(Data::from_typed(xy, vec![Some(Buffer::U32(vec![1, 2]))]), Err(DataError::Kind { .. })), "u32 fits only a category");
        assert!(matches!(Data::from_typed(xy, vec![None, Some(Buffer::F64(vec![1.0, 2.0]))]), Err(DataError::Kind { .. })), "a string takes none");
        assert!(matches!(Data::from_typed(xy, vec![None, None, Some(Buffer::F64(vec![]))]), Err(DataError::Buffer { index: 2, columns: 2 })));
        assert!(matches!(Data::from_typed(xy, vec![Some(Buffer::F64(vec![1.0]))]), Err(DataError::Length { .. })));
        let no_length = r#"{"tables":{"t":{"columns":[{"name":"x","type":"f64"}]}}}"#;
        assert_eq!(Data::from_typed(no_length, vec![Some(Buffer::F64(vec![1.0, 2.0, 3.0]))]).unwrap().first_table().unwrap().len(), 3);
        let numbers_in_json = r#"{"tables":{"t":{"columns":[{"name":"x","type":"string","values":[1]}]}}}"#;
        assert!(Data::from_descriptor(numbers_in_json).is_err());

        // Staged rows read as these buffers do, a column after another, a
        // `us` column's as offsets from the origin the commit gave it
        // (REQ-STREAM-010).
        let template = t.empty_like();
        assert_eq!((template.len(), template.columns().len()), (0, 6));
        let staged = [
            [3.0, 5.5],      // s
            [5.0, 6.0],      // ns, past the origin
            [4.0, f64::NAN], // us, past the commit's origin
            [1.0, 0.5],      // d, seconds
            [0.0, 2.0],      // c: a, then past the labels
            [0.0, 0.0],      // note: not read
        ]
        .concat();
        let b = template.staged(&staged, 2).unwrap();
        assert_eq!(b.by_name("s").unwrap().nums().unwrap(), [3000.0, 5500.0]);
        assert_eq!(b.by_name("ns").unwrap().exact(0), Some(1_792_281_600_000_000_005));
        assert_eq!((b.by_name("us").unwrap().exact(0), b.by_name("us").unwrap().is_missing(1)), (Some(1_792_281_600_000_005), true));
        assert_eq!(b.by_name("d").unwrap().nums().unwrap(), [1000.0, 500.0]);
        assert_eq!((b.by_name("c").unwrap().label(0), b.by_name("c").unwrap().is_missing(1)), (Some("a"), true));
        assert!(b.by_name("note").unwrap().is_missing(0));
        assert!(matches!(template.staged(&staged[..11], 2), Err(DataError::Append(_))));
    }
}
