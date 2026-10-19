//! What a chart draws: tables of typed columns held as `f64`, and matrices
//! (REQ-DATA-010 to 130).
//!
//! A [`Data`] holds named [`Table`]s and named [`Matrix`]es in the order they
//! were given. Every number, time and duration is an `f64` (REQ-DATA-020);
//! categories are `u32` codes into their labels and strings stay strings.
//! Missing is NaN, or [`MISSING_CODE`] for a category (REQ-DATA-030).
//!
//! Timestamps are UTC instants (REQ-DATA-090): `s` and `ms` columns hold epoch
//! milliseconds; `us` and `ns` columns hold offsets in their unit from an
//! `i64` origin, so a span under 2^53 units reads back exactly (REQ-DATA-020).
//! A column's `zone` is kept as text for ticks and labels; nothing here reads
//! the host's zone or clock.
//!
//! Data applies at once (REQ-DATA-130, ADR-106): a `Data` is a value with no
//! history, and nothing in this module keeps an earlier version or eases
//! between two.
//!
//! The JSON form ([`Data::from_json`]) is in [`desc`]. The CSV reader and the
//! Arrow reader arrive in later phases (REQ-DATA-030's empty CSV field and
//! cleared validity bit, REQ-DATA-140).

pub mod desc;
#[cfg(feature = "text")]
pub(crate) mod rfc3339;

use std::fmt;

use serde::{Deserialize, Serialize};

pub use desc::{Buffer, ColumnDesc, Encode, MatrixDesc, TableDesc};

/// The code of a missing category (REQ-DATA-030).
pub const MISSING_CODE: u32 = u32::MAX;

/// The name a shorthand's one table is given.
pub const SHORTHAND_TABLE: &str = "data";

/// The most columns a table holds (data.schema.json).
pub const MAX_COLUMNS: usize = 256;

/// The most labels a category column holds (data.schema.json).
pub const MAX_CATEGORIES: usize = 65_535;

/// Whether a held number, time or duration is missing: NaN (REQ-DATA-030).
pub fn is_missing(v: f64) -> bool {
    v.is_nan()
}

/// Whether `name` is a table, matrix or column name: a letter or `_`, then up
/// to 63 letters, digits, `_`, `.` or `-` (data.schema.json).
pub fn is_name(name: &str) -> bool {
    let b = name.as_bytes();
    match b.first() {
        Some(c) if c.is_ascii_alphabetic() || *c == b'_' => {}
        _ => return false,
    }
    b.len() <= 64 && b[1..].iter().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'.' | b'-'))
}

/// A column's type as data.schema.json names it (REQ-DATA-010).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ColumnType {
    /// A 64-bit float.
    F64,
    /// A 32-bit float, widened to `f64` once.
    F32,
    /// A signed integer, held as `f64` (exact to 2^53).
    I64,
    /// An unsigned 32-bit integer, held as `f64`.
    U32,
    /// A boolean, held as 1.0 or 0.0, NaN missing.
    Bool,
    /// An instant since 1970-01-01T00:00:00Z in the column's time unit.
    Timestamp,
    /// A span, held in milliseconds.
    Duration,
    /// An index into the column's labels, held as `u32`.
    Category,
    /// A free label a row.
    String,
}

impl ColumnType {
    /// The type's name in data.schema.json.
    pub fn name(self) -> &'static str {
        match self {
            ColumnType::F64 => "f64",
            ColumnType::F32 => "f32",
            ColumnType::I64 => "i64",
            ColumnType::U32 => "u32",
            ColumnType::Bool => "bool",
            ColumnType::Timestamp => "timestamp",
            ColumnType::Duration => "duration",
            ColumnType::Category => "category",
            ColumnType::String => "string",
        }
    }

    /// Whether the type is held as numbers ([`Values::Num`]).
    pub fn is_numeric(self) -> bool {
        !matches!(self, ColumnType::Category | ColumnType::String)
    }
}

/// The unit of a timestamp or duration column; `ms` unless it says otherwise.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TimeUnit {
    /// Seconds.
    S,
    /// Milliseconds, the default.
    #[default]
    Ms,
    /// Microseconds.
    Us,
    /// Nanoseconds.
    Ns,
}

impl TimeUnit {
    /// The unit's name in data.schema.json.
    pub fn name(self) -> &'static str {
        match self {
            TimeUnit::S => "s",
            TimeUnit::Ms => "ms",
            TimeUnit::Us => "us",
            TimeUnit::Ns => "ns",
        }
    }

    /// Whether a timestamp column in this unit is held as offsets from an
    /// `i64` origin (REQ-DATA-020).
    pub fn has_origin(self) -> bool {
        matches!(self, TimeUnit::Us | TimeUnit::Ns)
    }

    /// Units in a millisecond, for `us` and `ns`; 1 otherwise.
    fn per_ms(self) -> i64 {
        match self {
            TimeUnit::Us => 1_000,
            TimeUnit::Ns => 1_000_000,
            _ => 1,
        }
    }

    /// A value in this unit in milliseconds.
    fn to_ms(self, v: f64) -> f64 {
        match self {
            TimeUnit::S => v * 1000.0,
            TimeUnit::Ms => v,
            TimeUnit::Us => v / 1000.0,
            TimeUnit::Ns => v / 1_000_000.0,
        }
    }
}

/// What a column is to a chart (data.schema.json, REQ-DATA-080).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// The horizontal position: time or a number.
    X,
    /// A vertical value; a chart may take several.
    Y,
    /// A value on the second vertical axis.
    Y2,
    /// The series a row belongs to, in long form.
    Series,
    /// A bar's opening price.
    Open,
    /// A bar's highest price.
    High,
    /// A bar's lowest price.
    Low,
    /// A bar's closing price.
    Close,
    /// A bar's traded volume.
    Volume,
    /// A band's or bin's lower bound.
    Lo,
    /// A band's or bin's upper bound.
    Hi,
    /// A bin's count.
    Count,
    /// A value to bin, or a category's value.
    Value,
    /// The category a value belongs to.
    Category,
    /// A point's size.
    Size,
    /// A point's colour value.
    Color,
    /// A row's text label.
    Label,
    /// A node's identifier.
    Id,
    /// An edge's source node.
    Source,
    /// An edge's target node.
    Target,
    /// An edge's or node's weight.
    Weight,
    /// A node's group.
    Group,
    /// Right ascension, degrees.
    Ra,
    /// Declination, degrees.
    Dec,
    /// Longitude, degrees.
    Lon,
    /// Latitude, degrees.
    Lat,
    /// Azimuth, degrees.
    Az,
    /// Altitude, degrees.
    Alt,
    /// Apparent magnitude.
    Mag,
    /// A third position coordinate.
    Z,
    /// An orbit's semi-major axis, au.
    A,
    /// An orbit's eccentricity.
    E,
    /// An orbit's inclination, degrees.
    I,
    /// An orbit's longitude of the ascending node, degrees.
    Node,
    /// An orbit's argument of perihelion, degrees.
    Peri,
    /// An orbit's mean anomaly, degrees.
    Anomaly,
    /// The epoch of an orbit's elements.
    Epoch,
}

impl Role {
    /// Every role, in the schema's order.
    pub const ALL: [Role; 37] = [
        Role::X,
        Role::Y,
        Role::Y2,
        Role::Series,
        Role::Open,
        Role::High,
        Role::Low,
        Role::Close,
        Role::Volume,
        Role::Lo,
        Role::Hi,
        Role::Count,
        Role::Value,
        Role::Category,
        Role::Size,
        Role::Color,
        Role::Label,
        Role::Id,
        Role::Source,
        Role::Target,
        Role::Weight,
        Role::Group,
        Role::Ra,
        Role::Dec,
        Role::Lon,
        Role::Lat,
        Role::Az,
        Role::Alt,
        Role::Mag,
        Role::Z,
        Role::A,
        Role::E,
        Role::I,
        Role::Node,
        Role::Peri,
        Role::Anomaly,
        Role::Epoch,
    ];

    /// The role's name in data.schema.json, which is also the channel's name.
    pub fn name(self) -> &'static str {
        const NAMES: [&str; 37] = [
            "x", "y", "y2", "series", "open", "high", "low", "close", "volume", "lo", "hi", "count", "value", "category", "size", "color", "label", "id",
            "source", "target", "weight", "group", "ra", "dec", "lon", "lat", "az", "alt", "mag", "z", "a", "e", "i", "node", "peri", "anomaly", "epoch",
        ];
        NAMES[self as usize]
    }

    /// The role of a name, if it is one.
    pub fn from_name(name: &str) -> Option<Role> {
        Role::ALL.into_iter().find(|r| r.name() == name)
    }
}

/// The coordinate frame of a table's angle or position columns.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Frame {
    /// The International Celestial Reference System.
    Icrs,
    /// Ecliptic coordinates.
    Ecliptic,
    /// Galactic coordinates.
    Galactic,
    /// Azimuth and altitude.
    Altaz,
    /// Longitude and latitude on WGS 84.
    Geographic,
    /// Heliocentric ecliptic positions.
    HeliocentricEcliptic,
}

/// A column's values as the chart holds them (REQ-DATA-020, REQ-DATA-030).
#[derive(Clone, Debug)]
pub enum Values {
    /// Numbers, booleans, times and durations; NaN is missing.
    Num(Vec<f64>),
    /// Category codes into the column's `categories`; [`MISSING_CODE`] is missing.
    Codes(Vec<u32>),
    /// Free labels; an empty label is missing.
    Text(Vec<String>),
}

impl Values {
    /// The number of rows.
    pub fn len(&self) -> usize {
        match self {
            Values::Num(v) => v.len(),
            Values::Codes(v) => v.len(),
            Values::Text(v) => v.len(),
        }
    }

    /// Whether there are no rows.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The same values in the row order `order` gives.
    fn permuted(&self, order: &[usize]) -> Values {
        match self {
            Values::Num(v) => Values::Num(order.iter().map(|&i| v[i]).collect()),
            Values::Codes(v) => Values::Codes(order.iter().map(|&i| v[i]).collect()),
            Values::Text(v) => Values::Text(order.iter().map(|&i| v[i].clone()).collect()),
        }
    }

    /// Whether two value lists are equal bit for bit (NaN equals NaN).
    fn bitwise_eq(&self, other: &Values) -> bool {
        match (self, other) {
            (Values::Num(a), Values::Num(b)) => a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits()),
            (Values::Codes(a), Values::Codes(b)) => a == b,
            (Values::Text(a), Values::Text(b)) => a == b,
            _ => false,
        }
    }
}

/// One typed column (REQ-DATA-010).
#[derive(Clone, Debug)]
pub struct Column {
    /// The column's name, unique in its table.
    pub name: String,
    /// The column's type.
    pub kind: ColumnType,
    /// What the column is to a chart.
    pub role: Option<Role>,
    /// Display text for the unit (`USD`); not a conversion.
    pub unit: Option<String>,
    /// Displayed value = stored × scale + offset; display only (REQ-DATA-100).
    pub scale: Option<f64>,
    /// Added after `scale` for display only.
    pub offset: Option<f64>,
    /// The declared unit of a timestamp or duration column; a duration is
    /// held in ms whatever it says.
    pub time_unit: Option<TimeUnit>,
    /// The origin of a `us` or `ns` timestamp column, in that unit; 0 otherwise (REQ-DATA-020).
    pub origin: i64,
    /// The zone ticks and labels are drawn in, as given; never read here (REQ-DATA-090).
    pub zone: Option<String>,
    /// The labels a category column's codes index, in legend order.
    pub categories: Vec<String>,
    /// The values.
    pub values: Values,
}

impl Column {
    fn with(name: &str, kind: ColumnType, values: Values) -> Column {
        Column {
            name: name.to_string(),
            kind,
            role: None,
            unit: None,
            scale: None,
            offset: None,
            time_unit: None,
            origin: 0,
            zone: None,
            categories: Vec::new(),
            values,
        }
    }

    /// A numeric column of `kind` (f64, f32, i64, u32, bool or duration in
    /// ms); NaN is missing. An `f32` column's values are rounded to `f32`.
    pub fn numbers(name: &str, kind: ColumnType, mut values: Vec<f64>) -> Column {
        if kind == ColumnType::F32 {
            values.iter_mut().for_each(|v| *v = *v as f32 as f64);
        }
        let mut c = Column::with(name, kind, Values::Num(values));
        if kind == ColumnType::Duration {
            c.time_unit = Some(TimeUnit::Ms);
        }
        c
    }

    /// An `f64` column.
    pub fn f64(name: &str, values: Vec<f64>) -> Column {
        Column::numbers(name, ColumnType::F64, values)
    }

    /// A timestamp column of epoch milliseconds; NaN is missing.
    pub fn timestamps_ms(name: &str, values: Vec<f64>) -> Column {
        let mut c = Column::with(name, ColumnType::Timestamp, Values::Num(values));
        c.time_unit = Some(TimeUnit::Ms);
        c
    }

    /// A timestamp column from exact integers in `unit` (`None` missing). A
    /// `us` or `ns` column is held as offsets from its least value, so values
    /// spanning under 2^53 units read back exactly through [`Column::exact`]
    /// (REQ-DATA-020).
    pub fn timestamps(name: &str, unit: TimeUnit, values: &[Option<i64>]) -> Column {
        let stamps: Vec<Stamp> = values.iter().map(|v| v.map_or(Stamp::Missing, Stamp::Exact)).collect();
        let (origin, held) = hold_stamps(unit, &stamps);
        let mut c = Column::with(name, ColumnType::Timestamp, Values::Num(held));
        c.time_unit = Some(unit);
        c.origin = origin;
        c
    }

    /// A category column: codes into `labels`, [`MISSING_CODE`] missing.
    pub fn categories(name: &str, labels: Vec<String>, codes: Vec<u32>) -> Column {
        let mut c = Column::with(name, ColumnType::Category, Values::Codes(codes));
        c.categories = labels;
        c
    }

    /// A string column; an empty label is missing.
    pub fn strings(name: &str, values: Vec<String>) -> Column {
        Column::with(name, ColumnType::String, Values::Text(values))
    }

    /// The column with `role`.
    pub fn role(mut self, role: Role) -> Column {
        self.role = Some(role);
        self
    }

    /// The column with display `unit`.
    pub fn unit(mut self, unit: &str) -> Column {
        self.unit = Some(unit.to_string());
        self
    }

    /// The column with the zone its ticks are drawn in (REQ-DATA-090).
    pub fn zone(mut self, zone: &str) -> Column {
        self.zone = Some(zone.to_string());
        self
    }

    /// The number of rows.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Whether there are no rows.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// The held numbers of a numeric column.
    pub fn nums(&self) -> Option<&[f64]> {
        match &self.values {
            Values::Num(v) => Some(v),
            _ => None,
        }
    }

    /// The codes of a category column.
    pub fn codes(&self) -> Option<&[u32]> {
        match &self.values {
            Values::Codes(v) => Some(v),
            _ => None,
        }
    }

    /// The labels of a string column.
    pub fn texts(&self) -> Option<&[String]> {
        match &self.values {
            Values::Text(v) => Some(v),
            _ => None,
        }
    }

    /// Row `row` as a number: the held value, a category's code (NaN when
    /// missing), or NaN for a string or a row past the end.
    pub fn get(&self, row: usize) -> f64 {
        match &self.values {
            Values::Num(v) => v.get(row).copied().unwrap_or(f64::NAN),
            Values::Codes(v) => v.get(row).filter(|&&c| c != MISSING_CODE).map_or(f64::NAN, |&c| c as f64),
            Values::Text(_) => f64::NAN,
        }
    }

    /// Whether row `row` is missing (REQ-DATA-030); a row past the end is.
    pub fn is_missing(&self, row: usize) -> bool {
        match &self.values {
            Values::Num(v) => v.get(row).is_none_or(|x| x.is_nan()),
            Values::Codes(v) => v.get(row).is_none_or(|&c| c == MISSING_CODE),
            Values::Text(v) => v.get(row).is_none_or(|s| s.is_empty()),
        }
    }

    /// The label of row `row` of a category or string column.
    pub fn label(&self, row: usize) -> Option<&str> {
        match &self.values {
            Values::Codes(v) => v.get(row).and_then(|&c| self.categories.get(c as usize)).map(String::as_str),
            Values::Text(v) => v.get(row).map(String::as_str).filter(|s| !s.is_empty()),
            Values::Num(_) => None,
        }
    }

    /// A held value of this column in epoch milliseconds: a timestamp's
    /// offset plus its origin, a duration as is, any other number as is.
    pub fn to_ms(&self, held: f64) -> f64 {
        match (self.kind, self.time_unit) {
            (ColumnType::Timestamp, Some(u)) if u.has_origin() => {
                let per = u.per_ms();
                self.origin.div_euclid(per) as f64 + (self.origin.rem_euclid(per) as f64 + held) / per as f64
            }
            _ => held,
        }
    }

    /// Row `row` of a timestamp column as an integer in its time unit
    /// (epoch ms for `ms` and `s` columns, which are held in ms), or `None`
    /// when it is missing, fractional or out of range (REQ-DATA-020).
    pub fn exact(&self, row: usize) -> Option<i64> {
        if self.kind != ColumnType::Timestamp {
            return None;
        }
        let v = self.nums()?.get(row).copied().filter(|v| v.is_finite())?;
        let unit = self.time_unit.unwrap_or_default();
        let v = if unit == TimeUnit::S { v / 1000.0 } else { v };
        if v != (v as i64) as f64 {
            return None;
        }
        (v as i64).checked_add(if unit.has_origin() { self.origin } else { 0 })
    }

    /// Whether two columns are equal, values bit for bit.
    pub fn bitwise_eq(&self, other: &Column) -> bool {
        self.name == other.name
            && self.kind == other.kind
            && self.role == other.role
            && self.unit == other.unit
            && self.scale.map(f64::to_bits) == other.scale.map(f64::to_bits)
            && self.offset.map(f64::to_bits) == other.offset.map(f64::to_bits)
            && self.time_unit == other.time_unit
            && self.origin == other.origin
            && self.zone == other.zone
            && self.categories == other.categories
            && self.values.bitwise_eq(&other.values)
    }

    /// Whether the values agree with the type.
    fn check(&self, table: &str) -> Result<(), DataError> {
        if !is_name(&self.name) {
            return Err(DataError::Name { name: self.name.clone() });
        }
        let agrees = match &self.values {
            Values::Num(_) => self.kind.is_numeric(),
            Values::Codes(_) => self.kind == ColumnType::Category,
            Values::Text(_) => self.kind == ColumnType::String,
        };
        if !agrees {
            return Err(DataError::Kind { table: table.to_string(), column: self.name.clone(), kind: self.kind });
        }
        if self.time_unit.is_some() && !matches!(self.kind, ColumnType::Timestamp | ColumnType::Duration) {
            return Err(DataError::TimeUnit { column: self.name.clone(), kind: self.kind });
        }
        if self.kind == ColumnType::Category && self.categories.len() > MAX_CATEGORIES {
            return Err(DataError::TooLong { column: self.name.clone(), what: "categories", max: MAX_CATEGORIES });
        }
        if let Values::Codes(codes) = &self.values {
            let n = self.categories.len();
            if let Some(row) = codes.iter().position(|&c| c != MISSING_CODE && c as usize >= n) {
                return Err(DataError::Code { column: self.name.clone(), row, code: codes[row] as u64, categories: n });
            }
        }
        Ok(())
    }
}

/// A timestamp before it is held: exact in its unit, approximate, or missing.
#[cfg_attr(not(feature = "text"), allow(dead_code))]
#[derive(Clone, Copy, Debug)]
enum Stamp {
    Exact(i64),
    Approx(f64),
    /// Already in epoch milliseconds (an RFC 3339 string in an `s` or `ms` column).
    Ms(f64),
    Missing,
}

/// The origin and held values of timestamps in `unit`: epoch ms for `s` and
/// `ms` (origin 0); offsets from the least value for `us` and `ns`.
fn hold_stamps(unit: TimeUnit, stamps: &[Stamp]) -> (i64, Vec<f64>) {
    if !unit.has_origin() {
        let held = stamps
            .iter()
            .map(|s| match *s {
                Stamp::Exact(v) => unit.to_ms(v as f64),
                Stamp::Approx(v) => unit.to_ms(v),
                Stamp::Ms(v) => v,
                Stamp::Missing => f64::NAN,
            })
            .collect();
        return (0, held);
    }
    let least = stamps
        .iter()
        .filter_map(|s| match *s {
            Stamp::Exact(v) => Some(v),
            // `as` saturates, so a huge or infinite value cannot overflow.
            Stamp::Approx(v) if v.is_finite() => Some(v as i64),
            Stamp::Ms(v) if v.is_finite() => Some((v * unit.per_ms() as f64) as i64),
            _ => None,
        })
        .min()
        .unwrap_or(0);
    let held = stamps
        .iter()
        .map(|s| match *s {
            Stamp::Exact(v) => (v as i128 - least as i128) as f64,
            Stamp::Approx(v) => v - least as f64,
            Stamp::Ms(v) => v * unit.per_ms() as f64 - least as f64,
            Stamp::Missing => f64::NAN,
        })
        .collect();
    (least, held)
}

/// A table of columns, all of one length (REQ-DATA-010).
#[derive(Clone, Debug)]
pub struct Table {
    len: usize,
    columns: Vec<Column>,
    /// A column the rows are said to be in non-decreasing order of; verified by [`Table::into_sorted`].
    pub sorted: Option<String>,
    /// The coordinate frame of angle or position columns.
    pub frame: Option<Frame>,
    /// The equinox or epoch of a celestial frame (`J2000`).
    pub epoch: Option<String>,
}

impl Table {
    /// A table of `columns`, which must have valid unique names, one length,
    /// values that agree with their types, and category codes within their
    /// labels. No columns is an empty table of no rows.
    pub fn from_columns(columns: Vec<Column>) -> Result<Table, DataError> {
        Table::named(SHORTHAND_TABLE, columns, None)
    }

    /// As [`Table::from_columns`], naming the table in errors and taking the
    /// declared `length`, if any.
    pub(crate) fn named(table: &str, columns: Vec<Column>, length: Option<usize>) -> Result<Table, DataError> {
        if columns.len() > MAX_COLUMNS {
            return Err(DataError::Columns { table: table.to_string(), count: columns.len() });
        }
        let len = length.or_else(|| columns.first().map(Column::len)).unwrap_or(0);
        for (i, c) in columns.iter().enumerate() {
            c.check(table)?;
            if columns[..i].iter().any(|d| d.name == c.name) {
                return Err(DataError::Duplicate { name: c.name.clone() });
            }
            if c.len() != len {
                return Err(DataError::Length { table: table.to_string(), column: c.name.clone(), expected: len, found: c.len() });
            }
        }
        Ok(Table { len, columns, sorted: None, frame: None, epoch: None })
    }

    /// The number of rows.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether there are no rows.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The columns, in order.
    pub fn columns(&self) -> &[Column] {
        &self.columns
    }

    /// The column at `index`.
    pub fn column(&self, index: usize) -> Option<&Column> {
        self.columns.get(index)
    }

    /// The index of the column named `name`.
    pub fn find(&self, name: &str) -> Option<usize> {
        self.columns.iter().position(|c| c.name == name)
    }

    /// The column named `name`.
    pub fn by_name(&self, name: &str) -> Option<&Column> {
        self.find(name).and_then(|i| self.column(i))
    }

    /// The column for `channel`: the one `encode` names, else the first with
    /// the channel's role, else the one of the channel's name. When none
    /// matches, a warning naming the channel (REQ-DATA-080).
    pub fn channel(&self, channel: &str, encode: &Encode) -> Result<usize, Warning> {
        self.channels(channel, encode).map(|v| v[0])
    }

    /// Every column for a channel that takes several (`y`): those `encode`
    /// names, else every column with the channel's role, else the one of the
    /// channel's name. Never empty when `Ok` (REQ-DATA-080).
    pub fn channels(&self, channel: &str, encode: &Encode) -> Result<Vec<usize>, Warning> {
        if let Some(names) = encode.get(channel) {
            let mut found = Vec::with_capacity(names.len());
            for name in names {
                match self.find(name) {
                    Some(i) => found.push(i),
                    None => return Err(Warning::UnknownColumn { channel: channel.to_string(), column: name.clone() }),
                }
            }
            if !found.is_empty() {
                return Ok(found);
            }
        }
        if let Some(role) = Role::from_name(channel) {
            let found: Vec<usize> = self.columns.iter().enumerate().filter(|(_, c)| c.role == Some(role)).map(|(i, _)| i).collect();
            if !found.is_empty() {
                return Ok(found);
            }
        }
        self.find(channel).map(|i| vec![i]).ok_or_else(|| Warning::NoChannel { channel: channel.to_string() })
    }

    /// Whether every channel of one of `alternatives` resolves.
    fn has_channels(&self, alternatives: &[&[&str]], encode: &Encode) -> bool {
        alternatives.iter().any(|set| set.iter().all(|ch| self.channel(ch, encode).is_ok()))
    }

    /// Whether column `x` is non-decreasing, missing values aside; true for a
    /// column that is not numeric or does not exist (REQ-DATA-040).
    pub fn is_sorted_by(&self, x: usize) -> bool {
        let Some(v) = self.column(x).and_then(Column::nums) else { return true };
        let mut last = f64::NEG_INFINITY;
        for &x in v.iter().filter(|x| !x.is_nan()) {
            if x < last {
                return false;
            }
            last = x;
        }
        true
    }

    /// When column `x` is out of order, a copy with every column's rows
    /// stably sorted by it (missing x last) and the warning to give once;
    /// `None` when it is in order (REQ-DATA-040).
    pub fn sorted_by(&self, x: usize) -> Option<(Table, Warning)> {
        if self.is_sorted_by(x) {
            return None;
        }
        let v = self.column(x)?.nums()?;
        let mut order: Vec<usize> = (0..self.len).collect();
        // Stable; NaN after every number, so missing x rows go last.
        order.sort_by(|&a, &b| match (v[a].is_nan(), v[b].is_nan()) {
            (false, false) => v[a].partial_cmp(&v[b]).unwrap_or(std::cmp::Ordering::Equal),
            (a, b) => a.cmp(&b),
        });
        let warning = Warning::Unsorted { column: self.columns[x].name.clone() };
        Some((self.permuted(&order), warning))
    }

    /// The table ordered by column `x`: itself when in order, else a stably
    /// sorted copy, with the warning added to `warnings` once (REQ-DATA-040).
    pub fn into_sorted(self, x: usize, warnings: &mut Warnings) -> Table {
        match self.sorted_by(x) {
            Some((sorted, w)) => {
                warnings.push(w);
                sorted
            }
            None => self,
        }
    }

    /// The table with its rows in the order `order` gives; `order` must be a
    /// permutation of the rows.
    pub(crate) fn permuted(&self, order: &[usize]) -> Table {
        let columns = self.columns.iter().map(|c| Column { values: c.values.permuted(order), ..c.clone() }).collect();
        Table { len: self.len, columns, sorted: self.sorted.clone(), frame: self.frame, epoch: self.epoch.clone() }
    }

    /// Whether two tables are equal, values bit for bit.
    pub fn bitwise_eq(&self, other: &Table) -> bool {
        self.len == other.len
            && self.sorted == other.sorted
            && self.frame == other.frame
            && self.epoch == other.epoch
            && self.columns.len() == other.columns.len()
            && self.columns.iter().zip(&other.columns).all(|(a, b)| a.bitwise_eq(b))
    }
}

/// A row-major matrix of `rows × cols` numbers; NaN is missing.
#[derive(Clone, Debug)]
pub struct Matrix {
    rows: usize,
    cols: usize,
    values: Vec<f64>,
    /// A label for each row, top to bottom, or none.
    pub row_labels: Vec<String>,
    /// A label for each column, left to right, or none.
    pub col_labels: Vec<String>,
    /// Display text for the values' unit.
    pub unit: Option<String>,
    /// An adjacency matrix of an undirected graph: only the upper triangle is read.
    pub symmetric: bool,
}

impl Matrix {
    /// A matrix of `rows × cols` values, row by row.
    pub fn new(rows: usize, cols: usize, values: Vec<f64>) -> Result<Matrix, DataError> {
        let n = rows.checked_mul(cols).ok_or(DataError::TooLarge { rows: usize::MAX })?;
        if values.len() != n {
            return Err(DataError::Matrix { expected: n, found: values.len() });
        }
        Ok(Matrix { rows, cols, values, row_labels: Vec::new(), col_labels: Vec::new(), unit: None, symmetric: false })
    }

    /// The matrix with row and column labels, each list empty or one a row (column).
    pub fn labelled(mut self, row_labels: Vec<String>, col_labels: Vec<String>) -> Result<Matrix, DataError> {
        if !row_labels.is_empty() && row_labels.len() != self.rows {
            return Err(DataError::Matrix { expected: self.rows, found: row_labels.len() });
        }
        if !col_labels.is_empty() && col_labels.len() != self.cols {
            return Err(DataError::Matrix { expected: self.cols, found: col_labels.len() });
        }
        self.row_labels = row_labels;
        self.col_labels = col_labels;
        Ok(self)
    }

    /// The number of rows.
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// The number of columns.
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// The values, row by row.
    pub fn values(&self) -> &[f64] {
        &self.values
    }

    /// The value at (`row`, `col`); NaN outside the matrix.
    pub fn get(&self, row: usize, col: usize) -> f64 {
        if row >= self.rows || col >= self.cols {
            return f64::NAN;
        }
        self.values[row * self.cols + col]
    }

    /// Whether two matrices are equal, values bit for bit.
    pub fn bitwise_eq(&self, other: &Matrix) -> bool {
        self.rows == other.rows
            && self.cols == other.cols
            && self.row_labels == other.row_labels
            && self.col_labels == other.col_labels
            && self.unit == other.unit
            && self.symmetric == other.symmetric
            && self.values.iter().zip(&other.values).all(|(a, b)| a.to_bits() == b.to_bits())
    }
}

/// What a chart draws: named tables and named matrices, in the order given
/// (REQ-DATA-010). A value with no history: replacing it replaces it whole
/// (REQ-DATA-130).
#[derive(Clone, Debug, Default)]
pub struct Data {
    tables: Vec<(String, Table)>,
    matrices: Vec<(String, Matrix)>,
}

impl Data {
    /// No tables and no matrices.
    pub fn new() -> Data {
        Data::default()
    }

    /// One table named [`SHORTHAND_TABLE`].
    pub fn from_table(table: Table) -> Data {
        Data { tables: vec![(SHORTHAND_TABLE.to_string(), table)], matrices: Vec::new() }
    }

    /// Adds a table; its name must be a name and new.
    pub fn push_table(&mut self, name: &str, table: Table) -> Result<(), DataError> {
        if !is_name(name) {
            return Err(DataError::Name { name: name.to_string() });
        }
        if self.table(name).is_some() {
            return Err(DataError::Duplicate { name: name.to_string() });
        }
        self.tables.push((name.to_string(), table));
        Ok(())
    }

    /// Adds a matrix; its name must be a name and new.
    pub fn push_matrix(&mut self, name: &str, matrix: Matrix) -> Result<(), DataError> {
        if !is_name(name) {
            return Err(DataError::Name { name: name.to_string() });
        }
        if self.matrix(name).is_some() {
            return Err(DataError::Duplicate { name: name.to_string() });
        }
        self.matrices.push((name.to_string(), matrix));
        Ok(())
    }

    /// The tables, in order.
    pub fn tables(&self) -> impl Iterator<Item = (&str, &Table)> {
        self.tables.iter().map(|(n, t)| (n.as_str(), t))
    }

    /// The matrices, in order.
    pub fn matrices(&self) -> impl Iterator<Item = (&str, &Matrix)> {
        self.matrices.iter().map(|(n, m)| (n.as_str(), m))
    }

    /// The table named `name`.
    pub fn table(&self, name: &str) -> Option<&Table> {
        self.tables.iter().find(|(n, _)| n == name).map(|(_, t)| t)
    }

    /// The matrix named `name`.
    pub fn matrix(&self, name: &str) -> Option<&Matrix> {
        self.matrices.iter().find(|(n, _)| n == name).map(|(_, m)| m)
    }

    /// The tables, by value, in order: how a chart takes its table without a
    /// copy (REQ-MEM-020).
    pub fn into_tables(self) -> Vec<(String, Table)> {
        self.tables
    }

    /// The first table, the one a chart of one table draws.
    pub fn first_table(&self) -> Option<&Table> {
        self.tables.first().map(|(_, t)| t)
    }

    /// The first matrix.
    pub fn first_matrix(&self) -> Option<&Matrix> {
        self.matrices.first().map(|(_, m)| m)
    }

    /// Whether the data is of `shape`'s kind: a matrix for `matrix`, tables
    /// `nodes` and `edges` for `graph`, a table for the rest.
    fn is_kind(&self, shape: Shape) -> bool {
        match shape {
            Shape::Matrix => !self.matrices.is_empty(),
            Shape::Graph => self.table("nodes").is_some() && self.table("edges").is_some(),
            _ => !self.tables.is_empty(),
        }
    }

    /// Whether the data is of `shape`'s kind and every channel it needs resolves.
    fn fits(&self, shape: Shape, encode: &Encode) -> bool {
        match shape {
            Shape::Matrix => self.is_kind(shape),
            Shape::Graph => self.table("edges").is_some_and(|t| t.has_channels(shape.channels(), encode)) && self.table("nodes").is_some(),
            _ => self.first_table().is_some_and(|t| t.has_channels(shape.channels(), encode)),
        }
    }

    /// The first of `accepted` the data fits with every channel, else the
    /// first whose kind it is (its missing channels then warn through
    /// [`Table::channel`]); otherwise an error naming the shapes expected
    /// (REQ-DATA-110). Never panics.
    pub fn shape(&self, accepted: &[Shape], encode: &Encode) -> Result<Shape, DataError> {
        accepted
            .iter()
            .copied()
            .find(|&s| self.fits(s, encode))
            .or_else(|| accepted.iter().copied().find(|&s| self.is_kind(s)))
            .ok_or_else(|| DataError::Shape { expected: accepted.to_vec() })
    }

    /// Whether two data are equal, values bit for bit.
    pub fn bitwise_eq(&self, other: &Data) -> bool {
        self.tables.len() == other.tables.len()
            && self.matrices.len() == other.matrices.len()
            && self.tables.iter().zip(&other.tables).all(|((a, x), (b, y))| a == b && x.bitwise_eq(y))
            && self.matrices.iter().zip(&other.matrices).all(|((a, x), (b, y))| a == b && x.bitwise_eq(y))
    }
}

/// A data shape a chart accepts (meta.schema.json `shapes`; discovery/data.md,
/// "Domain shapes").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Shape {
    /// `x` and one or more `y`, optionally a `series` category.
    Series,
    /// `x`, `open`, `high`, `low`, `close`, optionally `volume`.
    Ohlcv,
    /// One `value` column, unbinned.
    Values,
    /// `lo`, `hi`, `count`.
    Bins,
    /// `category` and one or more `value`.
    Counts,
    /// `x`, `y`, optionally `size`, `color`, `label`, `series`.
    Points,
    /// A matrix.
    Matrix,
    /// Tables `nodes` and `edges` (`source`, `target`).
    Graph,
    /// `lon`, `lat`.
    Geo,
    /// `ra`, `dec` or `az`, `alt`.
    Sky,
    /// `a`, `e`, `i`, `node`, `peri`, `anomaly`, `epoch`.
    Orbits,
    /// `x` with `ra`, `dec` or with `x`, `y`, `z`.
    Ephemeris,
}

impl Shape {
    /// Every shape, in meta.schema.json's order.
    pub const ALL: [Shape; 12] = [
        Shape::Series,
        Shape::Ohlcv,
        Shape::Values,
        Shape::Bins,
        Shape::Counts,
        Shape::Points,
        Shape::Matrix,
        Shape::Graph,
        Shape::Geo,
        Shape::Sky,
        Shape::Orbits,
        Shape::Ephemeris,
    ];

    /// The shape's name in meta.schema.json.
    pub fn name(self) -> &'static str {
        match self {
            Shape::Series => "series",
            Shape::Ohlcv => "ohlcv",
            Shape::Values => "values",
            Shape::Bins => "bins",
            Shape::Counts => "counts",
            Shape::Points => "points",
            Shape::Matrix => "matrix",
            Shape::Graph => "graph",
            Shape::Geo => "geo",
            Shape::Sky => "sky",
            Shape::Orbits => "orbits",
            Shape::Ephemeris => "ephemeris",
        }
    }

    /// The shape of a name, if it is one.
    pub fn from_name(name: &str) -> Option<Shape> {
        Shape::ALL.into_iter().find(|s| s.name() == name)
    }

    /// The channels the shape needs, as alternatives: the data fits when
    /// every channel of one alternative resolves. A graph's are its `edges`
    /// table's; a matrix needs none.
    pub fn channels(self) -> &'static [&'static [&'static str]] {
        match self {
            Shape::Series | Shape::Points => &[&["x", "y"]],
            Shape::Ohlcv => &[&["x", "open", "high", "low", "close"]],
            Shape::Values => &[&["value"]],
            Shape::Bins => &[&["lo", "hi", "count"]],
            Shape::Counts => &[&["category", "value"]],
            Shape::Matrix => &[&[]],
            Shape::Graph => &[&["source", "target"]],
            Shape::Geo => &[&["lon", "lat"]],
            Shape::Sky => &[&["ra", "dec"], &["az", "alt"]],
            Shape::Orbits => &[&["a", "e", "i", "node", "peri", "anomaly", "epoch"]],
            Shape::Ephemeris => &[&["x", "ra", "dec"], &["x", "y", "z"]],
        }
    }
}

impl fmt::Display for Shape {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Something about the data a chart draws anyway, said once.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Warning {
    /// The x column was out of order; a stably sorted copy is drawn (REQ-DATA-040).
    Unsorted {
        /// The x column.
        column: String,
    },
    /// No column plays the channel; the chart draws its empty-data frame (REQ-DATA-080).
    NoChannel {
        /// The channel.
        channel: String,
    },
    /// `encode` names a column the table does not have (REQ-DATA-080).
    UnknownColumn {
        /// The channel.
        channel: String,
        /// The column `encode` names.
        column: String,
    },
}

impl fmt::Display for Warning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Warning::Unsorted { column } => write!(f, "column `{column}` is not in non-decreasing order; drawing a sorted copy"),
            Warning::NoChannel { channel } => {
                write!(f, "no column for channel `{channel}`: set `encode.{channel}`, a column's role `{channel}`, or name a column `{channel}`")
            }
            Warning::UnknownColumn { channel, column } => write!(f, "`encode.{channel}` names column `{column}`, which the table does not have"),
        }
    }
}

/// Warnings, each kept once however often it is given.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Warnings(Vec<Warning>);

impl Warnings {
    /// No warnings.
    pub fn new() -> Warnings {
        Warnings::default()
    }

    /// Adds `w` unless it is already here; whether it was new.
    pub fn push(&mut self, w: Warning) -> bool {
        if self.0.contains(&w) {
            return false;
        }
        self.0.push(w);
        true
    }

    /// The warnings, in the order first given.
    pub fn as_slice(&self) -> &[Warning] {
        &self.0
    }

    /// The number of distinct warnings.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether there are none.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Why data could not be read, or is not of a shape the chart takes.
#[derive(Clone, Debug, PartialEq)]
pub enum DataError {
    /// The JSON is malformed or not in the shape of data.schema.json.
    Json(String),
    /// A name that is not a name.
    Name {
        /// The name.
        name: String,
    },
    /// A name given twice.
    Duplicate {
        /// The name.
        name: String,
    },
    /// A table of no columns or more than 256.
    Columns {
        /// The table.
        table: String,
        /// Its column count.
        count: usize,
    },
    /// A column whose length differs from its table's.
    Length {
        /// The table.
        table: String,
        /// The column.
        column: String,
        /// The table's length.
        expected: usize,
        /// The column's length.
        found: usize,
    },
    /// A table with no `length` and no column carrying values.
    NoLength {
        /// The table.
        table: String,
    },
    /// Too many rows to hold.
    TooLarge {
        /// The rows asked for.
        rows: usize,
    },
    /// A column whose values do not agree with its type.
    Kind {
        /// The table.
        table: String,
        /// The column.
        column: String,
        /// Its type.
        kind: ColumnType,
    },
    /// A value of the wrong JSON type for its column.
    Value {
        /// The column.
        column: String,
        /// The row.
        row: usize,
        /// What the column takes.
        expected: &'static str,
    },
    /// A timestamp string that is not RFC 3339, or out of range.
    Timestamp {
        /// The column.
        column: String,
        /// The row.
        row: usize,
        /// The text.
        text: String,
    },
    /// A category column without `categories`.
    Categories {
        /// The column.
        column: String,
    },
    /// A category code past its labels.
    Code {
        /// The column.
        column: String,
        /// The row.
        row: usize,
        /// The code.
        code: u64,
        /// The number of labels.
        categories: usize,
    },
    /// A `time_unit` on a column that is not a timestamp or duration.
    TimeUnit {
        /// The column.
        column: String,
        /// Its type.
        kind: ColumnType,
    },
    /// A text or list longer than the schema allows.
    TooLong {
        /// The column.
        column: String,
        /// What is too long.
        what: &'static str,
        /// The most allowed.
        max: usize,
    },
    /// `sorted` names a column the table does not have.
    Sorted {
        /// The table.
        table: String,
        /// The name.
        column: String,
    },
    /// A matrix whose values or labels do not match its size.
    Matrix {
        /// The count expected.
        expected: usize,
        /// The count found.
        found: usize,
    },
    /// Appended rows that do not fit the data held: other columns, or a
    /// column type append does not take yet.
    Append(String),
    /// The data is not of a shape the chart takes (REQ-DATA-110).
    Shape {
        /// The shapes the chart takes.
        expected: Vec<Shape>,
    },
    /// A buffer given for a column the descriptor does not have
    /// ([`Data::from_typed`]).
    Buffer {
        /// The buffer's column number.
        index: usize,
        /// The columns the descriptor has, matrices counted as one each.
        columns: usize,
    },
}

impl fmt::Display for DataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DataError::Json(m) => write!(f, "data: {m}"),
            DataError::Name { name } => write!(f, "`{name}` is not a name: a letter or `_`, then up to 63 letters, digits, `_`, `.` or `-`"),
            DataError::Duplicate { name } => write!(f, "`{name}` is given twice"),
            DataError::Columns { table, count } => write!(f, "table `{table}` has {count} columns; it takes 1 to {MAX_COLUMNS}"),
            DataError::Length { table, column, expected, found } => {
                write!(f, "column `{column}` of table `{table}` has {found} values; the table has {expected} rows")
            }
            DataError::NoLength { table } => write!(f, "table `{table}` needs `length`: no column carries values"),
            DataError::TooLarge { rows } => write!(f, "{rows} rows are more than can be held"),
            DataError::Kind { table, column, kind } => write!(f, "column `{column}` of table `{table}` holds values that are not {}", kind.name()),
            DataError::Value { column, row, expected } => write!(f, "column `{column}`, row {row}: expected {expected}"),
            DataError::Timestamp { column, row, text } => write!(f, "column `{column}`, row {row}: `{text}` is not an RFC 3339 date-time"),
            DataError::Categories { column } => write!(f, "category column `{column}` needs `categories`"),
            DataError::Code { column, row, code, categories } => {
                write!(f, "column `{column}`, row {row}: category {code} is past its {categories} labels")
            }
            DataError::TimeUnit { column, kind } => write!(f, "column `{column}` is {}; only timestamp and duration columns take `time_unit`", kind.name()),
            DataError::TooLong { column, what, max } => write!(f, "column `{column}`: {what} longer than {max}"),
            DataError::Sorted { table, column } => write!(f, "table `{table}` is `sorted` by `{column}`, which it does not have"),
            DataError::Matrix { expected, found } => write!(f, "matrix: expected {expected} values or labels, found {found}"),
            DataError::Append(m) => write!(f, "append: {m}"),
            DataError::Shape { expected } => {
                let names: Vec<&str> = expected.iter().map(|s| s.name()).collect();
                match names.len() {
                    0 => write!(f, "data of no shape is accepted"),
                    _ => write!(f, "data is not of the shape this chart takes: expected {}", names.join(" or ")),
                }
            }
            DataError::Buffer { index, columns } => write!(f, "a buffer for column {index}, but the descriptor has {columns} columns"),
        }
    }
}

impl std::error::Error for DataError {}

#[cfg(all(test, feature = "text"))]
mod tests {
    use super::*;

    fn read(json: &str) -> Data {
        Data::from_json(json).unwrap_or_else(|e| panic!("{e}"))
    }

    fn example(name: &str) -> String {
        let path = format!("{}/../../spec/schemas/examples/{name}", env!("CARGO_MANIFEST_DIR"));
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
    }

    /// REQ-DATA-010: every column type of data.schema.json reads, held as the
    /// schema says, and every example parses.
    #[test]
    fn reads_every_column_type() {
        let d = read(&example("data.json"));
        let names: Vec<&str> = d.tables().map(|(n, _)| n).collect();
        assert_eq!(names, ["prices", "nodes", "edges", "stars", "binary"], "tables keep their order");
        let p = d.table("prices").unwrap();
        assert_eq!(p.len(), 3);
        assert_eq!(p.sorted.as_deref(), Some("t"));
        let t = p.by_name("t").unwrap();
        assert_eq!((t.kind, t.role, t.time_unit, t.zone.as_deref()), (ColumnType::Timestamp, Some(Role::X), Some(TimeUnit::Ms), Some("America/New_York")));
        assert_eq!(t.nums().unwrap(), &[1_792_330_200_000.0, 1_792_330_260_000.0, 1_792_335_120_000.0]);
        assert_eq!(p.by_name("open").unwrap().unit.as_deref(), Some("USD"));
        assert_eq!(p.by_name("volume").unwrap().nums().unwrap(), &[12000.0, 9800.0, 15100.0]);
        let sym = p.by_name("sym").unwrap();
        assert_eq!((sym.codes().unwrap(), sym.label(0)), (&[0u32, 0, 0][..], Some("ACME")));
        assert_eq!(p.by_name("halted").unwrap().nums().unwrap(), &[0.0, 0.0, 1.0]);
        assert_eq!(p.by_name("note").unwrap().label(2), Some("halt"));
        assert_eq!(d.table("edges").unwrap().by_name("source").unwrap().kind, ColumnType::U32);
        let stars = d.table("stars").unwrap();
        assert_eq!((stars.frame, stars.epoch.as_deref()), (Some(Frame::Icrs), Some("J2000")));
        assert_eq!(stars.by_name("mag").unwrap().nums().unwrap()[0], -1.46f32 as f64, "f32 is widened from f32");
        let binary = d.table("binary").unwrap();
        assert_eq!(binary.len(), 1_000_000);
        assert!(binary.by_name("rtt").unwrap().is_missing(999_999), "a column without values is reserved as missing");
        let m = d.matrix("latency").unwrap();
        assert_eq!((m.rows(), m.cols(), m.get(1, 1), m.unit.as_deref()), (2, 3, 4.0, Some("ms")));
        assert!(m.get(0, 2).is_nan() && m.get(5, 5).is_nan());

        let dur = read(r#"{"tables":{"a":{"columns":[{"name":"d","type":"duration","time_unit":"s","values":[1.5,null]}]}}}"#);
        assert_eq!(dur.first_table().unwrap().column(0).unwrap().get(0), 1500.0);

        let c = read(&example("data.columns.json"));
        let t = c.table(SHORTHAND_TABLE).unwrap();
        assert_eq!(t.by_name("t").unwrap().kind, ColumnType::Timestamp);
        assert_eq!(t.by_name("t").unwrap().get(0), 1_792_335_120_000.0);
        assert_eq!(t.by_name("close").unwrap().kind, ColumnType::F64);
        let r = read(&example("data.rows.json"));
        let t = r.first_table().unwrap();
        assert_eq!((t.len(), t.by_name("t").unwrap().get(0)), (1, 1.0));
        assert!(t.by_name("v").unwrap().is_missing(0));
    }

    /// REQ-DATA-010: every invalid example is refused with an error.
    #[test]
    fn invalid_examples_are_errors() {
        let dir = format!("{}/../../spec/schemas/examples/invalid", env!("CARGO_MANIFEST_DIR"));
        let mut seen = 0;
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            if !name.starts_with("data.") {
                continue;
            }
            seen += 1;
            let text = std::fs::read_to_string(&path).unwrap();
            assert!(Data::from_json(&text).is_err(), "{name} was accepted");
        }
        assert_eq!(seen, 4);
    }

    /// REQ-DATA-020: `ns` timestamps past 2^53 round-trip exactly as offsets
    /// from an `i64` origin.
    #[test]
    fn ns_timestamps_round_trip() {
        let json = r#"{"tables":{"p":{"columns":[{"name":"t","type":"timestamp","time_unit":"ns",
            "values":[1792281600000000003,1792281600000000001,null,"2026-10-18T00:00:00.000000004Z",1792281600000000002]}]}}}"#;
        let d = read(json);
        let t = d.first_table().unwrap().column(0).unwrap();
        assert_eq!(t.origin, 1_792_281_600_000_000_001);
        let back: Vec<Option<i64>> = (0..5).map(|i| t.exact(i)).collect();
        assert_eq!(
            back,
            [Some(1_792_281_600_000_000_003), Some(1_792_281_600_000_000_001), None, Some(1_792_281_600_000_000_004), Some(1_792_281_600_000_000_002)]
        );
        // In ms, the instant is within one ULP of 1792281600000 ms.
        assert_eq!(t.to_ms(t.get(1)), 1_792_281_600_000.000001);
        let built = Column::timestamps("t", TimeUnit::Us, &[Some(1_792_281_600_000_001), None]);
        assert_eq!((built.exact(0), built.exact(1)), (Some(1_792_281_600_000_001), None));
        let ms = Column::timestamps("t", TimeUnit::S, &[Some(1_792_281_600)]);
        assert_eq!((ms.get(0), ms.exact(0)), (1_792_281_600_000.0, Some(1_792_281_600)));
    }

    /// REQ-DATA-030: JSON null in every type, and NaN, read as missing.
    #[test]
    fn every_missing_form_reads_as_missing() {
        let json = r#"{"tables":{"a":{"columns":[
            {"name":"f","type":"f64","values":[null]},{"name":"g","type":"f32","values":[null]},
            {"name":"i","type":"i64","values":[null]},{"name":"u","type":"u32","values":[null]},
            {"name":"b","type":"bool","values":[null]},{"name":"t","type":"timestamp","values":[null]},
            {"name":"n","type":"timestamp","time_unit":"ns","values":[null]},
            {"name":"d","type":"duration","values":[null]},{"name":"c","type":"category","categories":["x"],"values":[null]},
            {"name":"s","type":"string","values":[null]}]}}}"#;
        let d = read(json);
        let t = d.first_table().unwrap();
        for c in t.columns() {
            assert!(c.is_missing(0), "{} is not missing", c.name);
            assert!(c.get(0).is_nan(), "{} reads as a number", c.name);
        }
        assert_eq!(t.by_name("c").unwrap().codes().unwrap(), &[MISSING_CODE]);
        assert_eq!(t.by_name("c").unwrap().label(0), None);
        let nan = Table::from_columns(vec![Column::f64("y", vec![f64::NAN, 1.0])]).unwrap();
        assert!(nan.column(0).unwrap().is_missing(0) && !nan.column(0).unwrap().is_missing(1));
        assert!(is_missing(f64::NAN) && !is_missing(f64::INFINITY));
        let shorthand = read(r#"{"a":[null,"x"],"b":[null,true]}"#);
        let t = shorthand.first_table().unwrap();
        assert!(t.by_name("a").unwrap().is_missing(0) && t.by_name("b").unwrap().is_missing(0));
    }

    /// REQ-DATA-040: x out of order gives a stably sorted copy, every column
    /// permuted alike, and one warning however often it is checked.
    #[test]
    fn unsorted_x_is_sorted_stably_with_one_warning() {
        let t = Table::from_columns(vec![
            Column::f64("x", vec![3.0, 1.0, f64::NAN, 1.0, 2.0]),
            Column::f64("y", vec![30.0, 10.0, 99.0, 11.0, 20.0]),
            Column::categories("s", vec!["a".into(), "b".into()], vec![0, 1, MISSING_CODE, 0, 1]),
            Column::strings("l", vec!["c".into(), "a".into(), "n".into(), "a2".into(), "b".into()]),
        ])
        .unwrap();
        assert!(!t.is_sorted_by(0));
        let mut warnings = Warnings::new();
        let sorted = t.clone().into_sorted(0, &mut warnings);
        assert_eq!(sorted.column(1).unwrap().nums().unwrap(), &[10.0, 11.0, 20.0, 30.0, 99.0]);
        assert_eq!(sorted.column(2).unwrap().codes().unwrap(), &[1, 0, 1, 0, MISSING_CODE]);
        assert_eq!(sorted.column(3).unwrap().texts().unwrap(), &["a", "a2", "b", "c", "n"]);
        assert!(sorted.is_sorted_by(0));
        let _ = t.clone().into_sorted(0, &mut warnings);
        assert_eq!(warnings.as_slice(), &[Warning::Unsorted { column: "x".into() }]);
        assert!(sorted.sorted_by(0).is_none(), "sorted data is left alone");
        let in_order = Table::from_columns(vec![Column::f64("x", vec![1.0, f64::NAN, 1.0, 2.0])]).unwrap();
        assert!(in_order.is_sorted_by(0) && in_order.sorted_by(0).is_none());
    }

    /// REQ-DATA-080: encode first, then role, then name; a warning naming the
    /// channel when none matches.
    #[test]
    fn channels_resolve_by_encode_role_then_name() {
        let t = Table::from_columns(vec![
            Column::f64("y", vec![1.0]),
            Column::f64("p50", vec![1.0]).role(Role::Y),
            Column::f64("p99", vec![1.0]).role(Role::Y),
            Column::f64("close", vec![1.0]),
            Column::timestamps_ms("t", vec![0.0]),
        ])
        .unwrap();
        let none = Encode::default();
        assert_eq!(t.channels("y", &none), Ok(vec![1, 2]), "role before name");
        assert_eq!(t.channel("close", &none), Ok(3), "then name");
        let enc: Encode = serde_json::from_str(r#"{"x":"t","y":["close","y"]}"#).unwrap();
        assert_eq!(t.channel("x", &enc), Ok(4), "encode first");
        assert_eq!(t.channels("y", &enc), Ok(vec![3, 0]));
        assert_eq!(t.channel("x", &none), Err(Warning::NoChannel { channel: "x".into() }));
        let bad = Encode::default().with("y", &["nope"]);
        assert_eq!(t.channel("y", &bad), Err(Warning::UnknownColumn { channel: "y".into(), column: "nope".into() }));
        assert!(Warning::NoChannel { channel: "open".into() }.to_string().contains("`open`"));
    }

    /// REQ-DATA-110: a shape the chart does not take is an error naming the
    /// shapes it does, never a panic.
    #[test]
    fn wrong_shape_is_an_error_naming_the_shape() {
        let series = Data::from_table(Table::from_columns(vec![Column::f64("x", vec![1.0]), Column::f64("y", vec![2.0])]).unwrap());
        let none = Encode::default();
        assert_eq!(series.shape(&[Shape::Series], &none), Ok(Shape::Series));
        assert_eq!(series.shape(&[Shape::Ohlcv, Shape::Points], &none), Ok(Shape::Points), "a full fit comes first");
        assert_eq!(series.shape(&[Shape::Ohlcv], &none), Ok(Shape::Ohlcv), "missing channels warn later");
        let err = series.shape(&[Shape::Matrix], &none).unwrap_err();
        assert_eq!(err, DataError::Shape { expected: vec![Shape::Matrix] });
        assert!(err.to_string().contains("expected matrix"));
        let mut matrix = Data::new();
        matrix.push_matrix("m", Matrix::new(1, 1, vec![0.0]).unwrap()).unwrap();
        assert!(matrix.shape(&[Shape::Series, Shape::Counts], &none).unwrap_err().to_string().contains("series or counts"));
        assert_eq!(matrix.shape(&[Shape::Matrix], &none), Ok(Shape::Matrix));
        assert!(Data::new().shape(&[], &none).is_err());
        for s in Shape::ALL {
            assert_eq!(Shape::from_name(s.name()), Some(s));
            let _ = Data::new().shape(&[s], &none);
            let _ = series.shape(&[s], &none);
        }
        let graph = read(&example("data.json"));
        assert_eq!(graph.shape(&[Shape::Graph], &none), Ok(Shape::Graph));
        assert_eq!(graph.shape(&[Shape::Ohlcv], &none), Ok(Shape::Ohlcv));
    }

    /// REQ-DATA-090: an instant is the same whatever offset it is written in,
    /// and `zone` is kept as text, not applied.
    #[test]
    fn timestamps_are_utc_instants_and_zone_is_text() {
        let d = read(
            r#"{"tables":{"a":{"columns":[{"name":"t","type":"timestamp","zone":"Asia/Tokyo",
            "values":["2026-10-18T09:00:00+09:00","2026-10-18T00:00:00Z","2026-10-17T20:00:00.000-04:00"]}]}}}"#,
        );
        let t = d.first_table().unwrap().column(0).unwrap();
        assert_eq!(t.nums().unwrap(), &[1_792_281_600_000.0; 3]);
        assert_eq!(t.zone.as_deref(), Some("Asia/Tokyo"));
    }

    /// REQ-DATA-130: data has no history; replacing it leaves nothing of the
    /// old data behind.
    #[test]
    fn replacing_data_applies_at_once() {
        let a = r#"{"x":[1,2,3],"y":[1,2,3]}"#;
        let b = r#"{"x":[5,6],"y":[50,60]}"#;
        let mut held = read(a);
        assert_eq!(held.first_table().map(Table::len), Some(3));
        held = read(b);
        assert!(held.bitwise_eq(&read(b)));
        assert!(!held.bitwise_eq(&read(a)));
    }

    #[test]
    fn tables_check_their_columns() {
        let e = |c: Vec<Column>| Table::from_columns(c).unwrap_err();
        assert!(matches!(e(vec![Column::f64("a", vec![1.0]), Column::f64("b", vec![])]), DataError::Length { .. }));
        assert!(matches!(e(vec![Column::f64("a", vec![]), Column::f64("a", vec![])]), DataError::Duplicate { .. }));
        assert!(matches!(e(vec![Column::f64("1a", vec![])]), DataError::Name { .. }));
        assert!(matches!(e(vec![Column::categories("c", vec![], vec![0])]), DataError::Code { .. }));
        assert!(matches!(e(vec![Column::numbers("c", ColumnType::String, vec![])]), DataError::Kind { .. }));
        assert!(Table::from_columns(vec![]).unwrap().is_empty());
        assert!(is_name("_a.b-c9") && !is_name("") && !is_name("a b") && !is_name(&"a".repeat(65)) && is_name(&"a".repeat(64)));
        for r in Role::ALL {
            assert_eq!(Role::from_name(r.name()), Some(r));
            assert_eq!(serde_json::to_string(&r).unwrap(), format!("\"{}\"", r.name()));
        }
    }
}
