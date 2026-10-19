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
//! values ([`Data::from_descriptor`]), its columns filled from typed arrays
//! ([`Data::from_buffers`]).

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
#[cfg(feature = "text")]
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
/// A column's values as a descriptor carries them: none without the `text`
/// feature.
#[cfg(not(feature = "text"))]
pub type Cells = NoValues;

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
        let d: Descriptor = serde_json::from_str(json).map_err(|e| DataError::Json(e.to_string()))?;
        if d.version.is_some_and(|v| v != 1) {
            return Err(DataError::Json("this reader takes version 1".into()));
        }
        if d.tables.0.is_empty() {
            return Err(DataError::Json("`tables` needs at least one table".into()));
        }
        let mut data = Data::new();
        for (name, desc) in &d.tables.0 {
            data.push_table(name, Table::from_desc(name, desc)?)?;
        }
        for (name, desc) in d.matrices.map(|m| m.0).unwrap_or_default() {
            data.push_matrix(&name, Matrix::from_desc(&desc)?)?;
        }
        Ok(data)
    }

    /// Data from a descriptor without values ([`Data::from_descriptor`]) and
    /// one typed array a column, in the descriptor's order across its tables
    /// then its matrices, as the wasm boundary's `reserve_column`, `set_column`
    /// and `commit` deliver them (REQ-DATA-050). A `None` leaves its column
    /// missing. Numbers are held as given: a timestamp in epoch ms for `s`
    /// and `ms` columns and in its unit from 0 for `us` and `ns`, a duration
    /// in ms, a category as its code (NaN or past its labels is missing, as
    /// a fraction is), a boolean as 0 or 1. A string column takes no array,
    /// and an array of the wrong length is an error.
    pub fn from_buffers(descriptor: &str, buffers: Vec<Option<Vec<f64>>>) -> Result<Data, DataError> {
        let data = Data::from_descriptor(descriptor)?;
        let mut buffers = buffers.into_iter();
        let mut out = Data::new();
        for (name, table) in data.tables {
            let len = table.len();
            let mut columns = Vec::with_capacity(table.columns.len());
            for mut c in table.columns {
                if let Some(values) = buffers.next().flatten() {
                    if values.len() != len {
                        return Err(DataError::Length { table: name.clone(), column: c.name.clone(), expected: len, found: values.len() });
                    }
                    c.values = match c.kind {
                        ColumnType::String => return Err(DataError::Kind { table: name.clone(), column: c.name.clone(), kind: c.kind }),
                        ColumnType::Category => {
                            let n = c.categories.len() as f64;
                            Values::Codes(values.iter().map(|&v| if v >= 0.0 && v < n && v == v.trunc() { v as u32 } else { MISSING_CODE }).collect())
                        }
                        ColumnType::F32 => Values::Num(values.into_iter().map(|v| v as f32 as f64).collect()),
                        _ => Values::Num(values),
                    };
                }
                columns.push(c);
            }
            let mut t = Table::named(&name, columns, Some(len))?;
            t.sorted = table.sorted;
            t.frame = table.frame;
            t.epoch = table.epoch;
            out.push_table(&name, t)?;
        }
        for (name, m) in data.matrices {
            let m = match buffers.next().flatten() {
                Some(values) => {
                    let mut f = Matrix::new(m.rows(), m.cols(), values)?.labelled(m.row_labels.clone(), m.col_labels.clone())?;
                    f.unit = m.unit.clone();
                    f.symmetric = m.symmetric;
                    f
                }
                None => m,
            };
            out.push_matrix(&name, m)?;
        }
        Ok(out)
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
            (Some(NoValues), _) => return Err(DataError::Json("column values cross as typed arrays (REQ-DATA-050)".into())),
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

/// How many values a column's description carries: none without `text`.
#[cfg(not(feature = "text"))]
fn values_len(_: &ColumnDesc) -> Option<usize> {
    None
}

impl Table {
    /// A table from its description, `name` naming it in errors: 1 to 256
    /// columns of one length, `length` when no column carries values.
    pub fn from_desc(name: &str, d: &TableDesc) -> Result<Table, DataError> {
        if d.columns.is_empty() || d.columns.len() > MAX_COLUMNS {
            return Err(DataError::Columns { table: name.to_string(), count: d.columns.len() });
        }
        let declared = d.length.map(|n| usize::try_from(n).map_err(|_| DataError::TooLarge { rows: usize::MAX })).transpose()?;
        let len = declared.or_else(|| d.columns.iter().find_map(values_len)).ok_or_else(|| DataError::NoLength { table: name.to_string() })?;
        let columns = d.columns.iter().map(|c| Column::from_desc(c, len)).collect::<Result<Vec<_>, _>>()?;
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
        let (rows, cols) = (size(d.rows)?, size(d.cols)?);
        let n = rows.checked_mul(cols).ok_or(DataError::TooLarge { rows: usize::MAX })?;
        let values = match &d.values {
            #[cfg(feature = "text")]
            Some(v) => v.iter().map(|x| x.unwrap_or(f64::NAN)).collect(),
            #[cfg(not(feature = "text"))]
            Some(NoValues) => return Err(DataError::Json("matrix values cross as typed arrays (REQ-DATA-050)".into())),
            None => filled(n, f64::NAN)?,
        };
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
}
