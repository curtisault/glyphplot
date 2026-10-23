//! The ring a chart keeps its rows in, and the append rules (REQ-STREAM-020
//! to REQ-STREAM-040, ADR-107).
//!
//! A [`Ring`] holds a table's columns in buffers of one length, its
//! *slots*, read from `head` for `len` rows and wrapping at the end. It
//! takes a replace's columns as they are, with no copy (REQ-MEM-020): their
//! buffers become its slots. It grows only while it holds fewer rows than
//! `retain.rows`, by doubling, capped at `retain.rows`; once it holds
//! `retain.rows` rows it never allocates again, and each row appended
//! overwrites the oldest (REQ-STREAM-020). A ring given a replace of more
//! rows than `retain.rows` keeps the last `retain.rows` of them in the
//! replace's own buffers.
//!
//! The rows are read in place as a [`Seq`]: the slots from `head` to the end
//! of the buffers, then those from the start, two slices that read as one
//! sequence. Charts binary-search, reduce (M4) and hit-test through it, so a
//! frame copies no data (ADR-107: "the downsamplers read the ring's two
//! segments in place").
//!
//! The rules of [`Ring::append`], with x the column a chart draws along:
//!
//! - A row whose x is missing is dropped, as on a replace (REQ-DATA-030).
//! - A row whose x is less than the last row's is dropped and counted as
//!   `late` (REQ-STREAM-040).
//! - A row whose x equals the last row's replaces it in place; with a `key`
//!   column (a long table's series), it replaces the row of the same key
//!   among the last rows of that x, and is added when there is none.
//! - The ring then drops its oldest rows past `retain.rows`, and past
//!   `retain.age` the rows older than the newest x minus the age, so the
//!   oldest kept row is the first with x ≥ newest − age (REQ-STREAM-030);
//!   each is counted as `dropped`.
//!
//! Without x (a band chart's categories) every row is added. Rows are
//! numbered from the oldest held, 0 up; [`Ring::first`] counts the rows
//! dropped before it since the last replace.

use std::collections::BTreeMap;
use std::ops::{Index, Range};

use crate::data::{Column, ColumnType, DataError, Frame, Table, TimeUnit, Values, MAX_CATEGORIES, MISSING_CODE};
use crate::events::Value;
use crate::options::Options;

/// `retain.rows` when the option does not give it (REQ-STREAM-020).
pub const DEFAULT_ROWS: usize = 1_000_000;

/// The most rows `retain.rows` may ask for.
pub const MAX_ROWS: usize = 100_000_000;

/// The fewest slots a growing ring takes.
const MIN_SLOTS: usize = 16;

/// What a ring keeps: the `retain` option (options.schema.json).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Retain {
    /// The most rows held: `retain.rows`, 1 to [`MAX_ROWS`].
    pub rows: usize,
    /// The span of x kept behind the newest row, in x units (milliseconds
    /// for time): `retain.age`; `None` keeps every row.
    pub age: Option<f64>,
}

impl Default for Retain {
    fn default() -> Retain {
        Retain { rows: DEFAULT_ROWS, age: None }
    }
}

impl Retain {
    /// The `retain` option: `rows` a whole number of at least 1 (else
    /// [`DEFAULT_ROWS`]), `age` a number above 0 in x units or a duration
    /// (`90s`, `15min`, `1h`, `7d`) in milliseconds (else none).
    pub fn from_options(o: &Options) -> Retain {
        let Some(m) = o.object("retain") else { return Retain::default() };
        let rows = m.get("rows").and_then(serde_json::Value::as_f64).filter(|r| r.is_finite() && *r >= 1.0 && r.fract() == 0.0).map_or(DEFAULT_ROWS, |r| {
            if r > MAX_ROWS as f64 {
                MAX_ROWS
            } else {
                r as usize
            }
        });
        let age = match m.get("age") {
            Some(serde_json::Value::Number(n)) => n.as_f64(),
            Some(serde_json::Value::String(s)) => duration_ms(s),
            _ => None,
        };
        Retain { rows, age: age.filter(|a| a.is_finite() && *a > 0.0) }
    }
}

/// A duration as `retain.age` writes it, a number then `ns`, `us`, `ms`,
/// `s`, `min`, `h`, `d` or `w`, in milliseconds; `None` for any other text.
pub fn duration_ms(text: &str) -> Option<f64> {
    let split = text.find(|c: char| !(c.is_ascii_digit() || c == '.'))?;
    let (number, unit) = text.split_at(split);
    let (whole, fraction) = number.split_once('.').unwrap_or((number, ""));
    if whole.is_empty() || (number.contains('.') && fraction.is_empty()) || fraction.contains('.') {
        return None;
    }
    // Digit by digit, not `str::parse`, which would bring a float parser
    // into every module for this alone.
    let digits = |s: &str| s.bytes().fold(0.0, |v, b| v * 10.0 + f64::from(b - b'0'));
    let v = digits(whole) + digits(fraction) / 10f64.powi(fraction.len() as i32);
    let per = match unit {
        "ns" => 1e-6,
        "us" => 1e-3,
        "ms" => 1.0,
        "s" => 1e3,
        "min" => 6e4,
        "h" => 3.6e6,
        "d" => 8.64e7,
        "w" => 6.048e8,
        _ => return None,
    };
    Some(v * per)
}

/// Two slices read as one sequence: a ring's rows in order, the slots from
/// its head to the end of its buffers, then those from the start. A slice
/// is a `Seq` of one part. Indexing, `partition_point` and iteration read
/// both parts in place.
#[derive(Debug)]
pub struct Seq<'a, T> {
    a: &'a [T],
    b: &'a [T],
}

impl<T> Clone for Seq<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Seq<'_, T> {}

impl<T> Default for Seq<'_, T> {
    fn default() -> Self {
        Seq { a: &[], b: &[] }
    }
}

impl<'a, T> Seq<'a, T> {
    /// `a`, then `b`.
    pub const fn new(a: &'a [T], b: &'a [T]) -> Seq<'a, T> {
        Seq { a, b }
    }

    /// One slice.
    pub const fn one(a: &'a [T]) -> Seq<'a, T> {
        Seq { a, b: &[] }
    }

    /// The number of elements.
    pub fn len(&self) -> usize {
        self.a.len() + self.b.len()
    }

    /// Whether there are none.
    pub fn is_empty(&self) -> bool {
        self.a.is_empty() && self.b.is_empty()
    }

    /// Element `i`, or `None` past the end.
    pub fn get(&self, i: usize) -> Option<&'a T> {
        match i.checked_sub(self.a.len()) {
            None => self.a.get(i),
            Some(j) => self.b.get(j),
        }
    }

    /// The first element.
    pub fn first(&self) -> Option<&'a T> {
        self.a.first().or_else(|| self.b.first())
    }

    /// The last element.
    pub fn last(&self) -> Option<&'a T> {
        self.b.last().or_else(|| self.a.last())
    }

    /// The elements in order.
    pub fn iter(&self) -> std::iter::Chain<std::slice::Iter<'a, T>, std::slice::Iter<'a, T>> {
        self.a.iter().chain(self.b.iter())
    }

    /// The two parts.
    pub fn parts(&self) -> (&'a [T], &'a [T]) {
        (self.a, self.b)
    }

    /// Elements `r`, as a `Seq`; panics past the end, as a slice does.
    pub fn slice(&self, r: Range<usize>) -> Seq<'a, T> {
        assert!(r.start <= r.end && r.end <= self.len(), "range {r:?} out of a sequence of {}", self.len());
        let n = self.a.len();
        let a = &self.a[r.start.min(n)..r.end.min(n)];
        let b = &self.b[r.start.saturating_sub(n)..r.end.saturating_sub(n)];
        Seq { a, b }
    }

    /// The parts of elements `r`, each with the index of its first element
    /// in the whole: how a scan visits the rows of a range part by part.
    pub fn split(&self, r: Range<usize>) -> [(usize, &'a [T]); 2] {
        let s = self.slice(r.clone());
        [(r.start, s.a), (r.start + s.a.len(), s.b)]
    }

    /// The index of the first element for which `pred` is false, `pred`
    /// being true for a prefix and false after it, as
    /// `slice::partition_point`.
    pub fn partition_point(&self, mut pred: impl FnMut(&T) -> bool) -> usize {
        match self.a.last() {
            Some(last) if !pred(last) => self.a.partition_point(pred),
            _ => self.a.len() + self.b.partition_point(pred),
        }
    }

    /// The elements, copied into one vector.
    pub fn to_vec(&self) -> Vec<T>
    where
        T: Clone,
    {
        let mut v = Vec::with_capacity(self.len());
        v.extend_from_slice(self.a);
        v.extend_from_slice(self.b);
        v
    }
}

impl<T> Index<usize> for Seq<'_, T> {
    type Output = T;

    fn index(&self, i: usize) -> &T {
        let n = self.a.len();
        if i < n {
            &self.a[i]
        } else {
            &self.b[i - n]
        }
    }
}

impl<'a, T> IntoIterator for Seq<'a, T> {
    type Item = &'a T;
    type IntoIter = std::iter::Chain<std::slice::Iter<'a, T>, std::slice::Iter<'a, T>>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T> From<&'a [T]> for Seq<'a, T> {
    fn from(a: &'a [T]) -> Seq<'a, T> {
        Seq::one(a)
    }
}

impl<'a, T> From<&'a Vec<T>> for Seq<'a, T> {
    fn from(a: &'a Vec<T>) -> Seq<'a, T> {
        Seq::one(a)
    }
}

impl<'a, T, const N: usize> From<&'a [T; N]> for Seq<'a, T> {
    fn from(a: &'a [T; N]) -> Seq<'a, T> {
        Seq::one(a)
    }
}

/// What an append did (`plot-data`'s counts, REQ-STREAM-070).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    /// Rows dropped from the front by `retain.rows` or `retain.age`.
    pub dropped: usize,
    /// Rows refused because their x was below the last row's.
    pub late: usize,
}

/// How a batch column's held values become the ring column's.
enum Conv<'b> {
    /// The ring column is missing in the batch.
    Missing,
    /// Numbers as they are.
    Same(&'b [f64]),
    /// Numbers rounded to `f32`.
    F32(&'b [f64]),
    /// Offsets in one unit moved to another origin: plus the difference.
    Shift(&'b [f64], f64),
    /// A timestamp to epoch milliseconds by its column.
    ToMs(&'b Column),
    /// A timestamp to offsets from `origin` in a unit `per` to the ms.
    FromMs(&'b Column, f64, f64),
    /// Category codes through a map to the ring's.
    Codes(&'b [u32], Vec<u32>),
    /// Labels.
    Text(&'b [String]),
}

impl Conv<'_> {
    /// Row `r` as the ring holds a number; NaN for a missing number.
    fn num(&self, r: usize) -> f64 {
        match self {
            Conv::Same(v) => v[r],
            Conv::F32(v) => v[r] as f32 as f64,
            Conv::Shift(v, by) => v[r] + by,
            Conv::ToMs(c) => c.nums().map_or(f64::NAN, |v| c.to_ms(v[r])),
            Conv::FromMs(c, per, origin) => c.nums().map_or(f64::NAN, |v| c.to_ms(v[r]) * per - origin),
            _ => f64::NAN,
        }
    }

    /// Row `r` as the ring holds a code.
    fn code(&self, r: usize) -> u32 {
        match self {
            Conv::Codes(v, map) => map.get(v[r] as usize).copied().unwrap_or(MISSING_CODE),
            _ => MISSING_CODE,
        }
    }
}

/// A chart's rows in a ring of `retain.rows` (REQ-STREAM-020, ADR-107).
#[derive(Clone, Debug)]
pub struct Ring {
    /// The columns, each value list a buffer of `slots` values.
    columns: Vec<Column>,
    slots: usize,
    head: usize,
    len: usize,
    first: u64,
    retain: Retain,
    sorted: Option<String>,
    frame: Option<Frame>,
    epoch: Option<String>,
}

impl Ring {
    /// A ring holding `table`'s rows in its own buffers, with no copy: when
    /// `x` is given, the table must be sorted by that column with missing x
    /// last, and the rows of missing x are dropped (REQ-DATA-030); then the
    /// oldest past `retain` are, counted in what it returns.
    pub fn new(table: Table, retain: Retain, x: Option<usize>) -> (Ring, usize) {
        let sorted = table.sorted.clone();
        let (frame, epoch) = (table.frame, table.epoch.clone());
        let (columns, len) = table.into_parts();
        let mut ring = Ring { columns, slots: len, head: 0, len, first: 0, retain, sorted, frame, epoch };
        if let Some(xs) = x.and_then(|i| ring.nums(i)) {
            ring.len = xs.len() - xs.iter().rev().take_while(|v| v.is_nan()).count();
        }
        let dropped = ring.trim(x);
        (ring, dropped)
    }

    /// The rows held.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether no row is held.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The rows dropped before the oldest held since the ring was made.
    pub fn first(&self) -> u64 {
        self.first
    }

    /// The slots its buffers hold: at most `retain.rows` once it has grown.
    pub fn slots(&self) -> usize {
        self.slots
    }

    /// What it keeps.
    pub fn retain(&self) -> Retain {
        self.retain
    }

    /// The columns: their names, types, roles, units and labels. Their
    /// values are the ring's buffers, in slot order: read them through
    /// [`Ring::nums`], [`Ring::codes`], [`Ring::get`] and [`Ring::label`].
    pub fn columns(&self) -> &[Column] {
        &self.columns
    }

    /// The column at `index`, as [`Ring::columns`] gives it.
    pub fn column(&self, index: usize) -> Option<&Column> {
        self.columns.get(index)
    }

    /// The index of the column named `name`.
    pub fn find(&self, name: &str) -> Option<usize> {
        self.columns.iter().position(|c| c.name == name)
    }

    /// The slot ranges of the rows, in order.
    fn ranges(&self) -> (Range<usize>, Range<usize>) {
        let end = self.head + self.len;
        if end <= self.slots {
            (self.head..end, 0..0)
        } else {
            (self.head..self.slots, 0..end - self.slots)
        }
    }

    /// The slot of row `i`.
    fn slot(&self, i: usize) -> usize {
        let s = self.head + i;
        if s >= self.slots {
            s - self.slots
        } else {
            s
        }
    }

    /// Column `i`'s numbers in row order, in place; `None` for a category
    /// or string column.
    pub fn nums(&self, i: usize) -> Option<Seq<'_, f64>> {
        let (a, b) = self.ranges();
        match &self.columns.get(i)?.values {
            Values::Num(v) => Some(Seq::new(&v[a], &v[b])),
            _ => None,
        }
    }

    /// Column `i`'s category codes in row order, in place.
    pub fn codes(&self, i: usize) -> Option<Seq<'_, u32>> {
        let (a, b) = self.ranges();
        match &self.columns.get(i)?.values {
            Values::Codes(v) => Some(Seq::new(&v[a], &v[b])),
            _ => None,
        }
    }

    /// Column `i`'s labels in row order, in place.
    pub fn texts(&self, i: usize) -> Option<Seq<'_, String>> {
        let (a, b) = self.ranges();
        match &self.columns.get(i)?.values {
            Values::Text(v) => Some(Seq::new(&v[a], &v[b])),
            _ => None,
        }
    }

    /// Row `row` of column `i` as a number, as [`Column::get`] reads it.
    pub fn get(&self, i: usize, row: usize) -> f64 {
        if row >= self.len {
            return f64::NAN;
        }
        self.columns.get(i).map_or(f64::NAN, |c| c.get(self.slot(row)))
    }

    /// The label of row `row` of a category or string column.
    pub fn label(&self, i: usize, row: usize) -> Option<&str> {
        if row >= self.len {
            return None;
        }
        self.columns.get(i)?.label(self.slot(row))
    }

    /// Whether row `row` of column `i` is missing; a row past the end is.
    pub fn is_missing(&self, i: usize, row: usize) -> bool {
        row >= self.len || self.columns.get(i).is_none_or(|c| c.is_missing(self.slot(row)))
    }

    /// Row `row`, column by column, as events show it: numbers as held,
    /// timestamps in epoch ms, categories and strings as labels, missing as
    /// null.
    pub fn datum(&self, row: usize) -> BTreeMap<String, Value> {
        let mut out = BTreeMap::new();
        if row >= self.len {
            return out;
        }
        let slot = self.slot(row);
        for c in &self.columns {
            let v = match &c.values {
                Values::Num(v) => match v.get(slot) {
                    Some(&h) if !h.is_nan() => Value::Number(c.to_ms(h)),
                    _ => Value::Null,
                },
                _ => c.label(slot).map_or(Value::Null, |l| Value::Text(l.to_string())),
            };
            out.insert(c.name.clone(), v);
        }
        out
    }

    /// The rows as a table, in order, its buffers rotated in place and
    /// trimmed to the rows: how a chart prepares its data again.
    pub fn into_table(mut self) -> Table {
        self.linearize();
        let len = self.len;
        for c in &mut self.columns {
            match &mut c.values {
                Values::Num(v) => v.truncate(len),
                Values::Codes(v) => v.truncate(len),
                Values::Text(v) => v.truncate(len),
            }
        }
        let mut t = Table::from_parts(self.columns, len);
        t.sorted = self.sorted;
        t.frame = self.frame;
        t.epoch = self.epoch;
        t
    }

    /// The rows as a table, copied.
    pub fn to_table(&self) -> Table {
        self.clone().into_table()
    }

    /// Moves the rows to the buffers' start, in order, in place.
    fn linearize(&mut self) {
        if self.head == 0 {
            return;
        }
        let head = self.head;
        for c in &mut self.columns {
            match &mut c.values {
                Values::Num(v) => v.rotate_left(head),
                Values::Codes(v) => v.rotate_left(head),
                Values::Text(v) => v.rotate_left(head),
            }
        }
        self.head = 0;
    }

    /// Room for one more row: the slots doubled, to `retain.rows` at most,
    /// the rows first moved to the start.
    fn grow(&mut self) {
        let to = (self.slots * 2).max(MIN_SLOTS).min(self.retain.rows).max(self.len + 1);
        self.linearize();
        let more = to - self.slots;
        for c in &mut self.columns {
            match &mut c.values {
                Values::Num(v) => {
                    v.reserve_exact(more);
                    v.resize(to, f64::NAN);
                }
                Values::Codes(v) => {
                    v.reserve_exact(more);
                    v.resize(to, MISSING_CODE);
                }
                Values::Text(v) => {
                    v.reserve_exact(more);
                    v.resize(to, String::new());
                }
            }
        }
        self.slots = to;
    }

    /// Drops the `n` oldest rows.
    fn evict(&mut self, n: usize) {
        let n = n.min(self.len);
        if n == 0 {
            return;
        }
        self.head = self.slot(n);
        self.len -= n;
        self.first += n as u64;
        if self.len == 0 {
            self.head = 0;
        }
    }

    /// Row `row` of column `x` in milliseconds when it is a timestamp, else
    /// as held.
    fn x_ms(&self, x: usize, held: f64) -> f64 {
        self.columns.get(x).map_or(held, |c| c.to_ms(held))
    }

    /// Drops the oldest rows past `retain.rows`, then those older than the
    /// newest x minus `retain.age`; the number dropped.
    fn trim(&mut self, x: Option<usize>) -> usize {
        let over = self.len.saturating_sub(self.retain.rows);
        self.evict(over);
        let old = match (self.retain.age, x.and_then(|xi| Some(xi).zip(self.nums(xi)))) {
            (Some(age), Some((xi, xs))) => xs.last().map_or(0, |&newest| {
                let cutoff = self.x_ms(xi, newest) - age;
                xs.partition_point(|&v| self.x_ms(xi, v) < cutoff)
            }),
            _ => 0,
        };
        self.evict(old);
        over + old
    }

    /// New retention: the rows past it dropped now, counted in what it
    /// returns. Fewer `rows` never shrinks the buffers.
    pub fn set_retain(&mut self, retain: Retain, x: Option<usize>) -> usize {
        self.retain = retain;
        self.trim(x)
    }

    /// Writes batch row `r` into slot `slot`.
    fn write(&mut self, slot: usize, convs: &[Conv], r: usize) {
        for (c, conv) in self.columns.iter_mut().zip(convs) {
            match &mut c.values {
                Values::Num(v) => v[slot] = conv.num(r),
                Values::Codes(v) => v[slot] = conv.code(r),
                Values::Text(v) => {
                    v[slot] = match conv {
                        Conv::Text(t) => t[r].clone(),
                        _ => String::new(),
                    }
                }
            }
        }
    }

    /// How each ring column takes the batch's: by name, of a kind it holds;
    /// new category labels are added after the ring's.
    fn convs<'b>(&mut self, batch: &'b Table) -> Result<Vec<Conv<'b>>, DataError> {
        if let Some(c) = batch.columns().iter().find(|c| self.find(&c.name).is_none()) {
            return Err(DataError::Append(format!("column `{}` is not in the data", c.name)));
        }
        let mut out = Vec::with_capacity(self.columns.len());
        for c in &self.columns {
            let Some(b) = batch.by_name(&c.name) else {
                out.push(Conv::Missing);
                continue;
            };
            let other = || DataError::Append(format!("column `{}` holds values of another kind", c.name));
            let conv = match (&c.values, &b.values) {
                (Values::Num(_), Values::Num(v)) => match (c.kind, b.kind) {
                    (ColumnType::Timestamp, ColumnType::Timestamp) => {
                        let (cu, bu) = (c.time_unit.unwrap_or_default(), b.time_unit.unwrap_or_default());
                        match (cu.has_origin(), bu.has_origin()) {
                            (false, false) => Conv::Same(v),
                            (true, true) if cu == bu => Conv::Shift(v, (i128::from(b.origin) - i128::from(c.origin)) as f64),
                            (false, true) => Conv::ToMs(b),
                            _ => Conv::FromMs(b, if cu == TimeUnit::Ns { 1e6 } else { 1e3 }, c.origin as f64),
                        }
                    }
                    (ColumnType::F32, _) => Conv::F32(v),
                    _ => Conv::Same(v),
                },
                (Values::Codes(_), Values::Codes(v)) => {
                    let map = if b.categories == c.categories {
                        (0..c.categories.len() as u32).collect()
                    } else {
                        let new = b.categories.iter().filter(|l| !c.categories.contains(l)).count();
                        if c.categories.len() + new > MAX_CATEGORIES {
                            return Err(DataError::TooLong { column: c.name.clone(), what: "categories", max: MAX_CATEGORIES });
                        }
                        Vec::new()
                    };
                    Conv::Codes(v, map)
                }
                (Values::Text(_), Values::Text(v)) => Conv::Text(v),
                _ => return Err(other()),
            };
            out.push(conv);
        }
        // New labels, in the batch's order, after the ring's.
        for (c, conv) in self.columns.iter_mut().zip(out.iter_mut()) {
            if let Conv::Codes(_, map) = conv {
                if map.is_empty() {
                    if let Some(b) = batch.by_name(&c.name) {
                        *map = b
                            .categories
                            .iter()
                            .map(|l| {
                                c.categories.iter().position(|k| k == l).unwrap_or_else(|| {
                                    c.categories.push(l.clone());
                                    c.categories.len() - 1
                                }) as u32
                            })
                            .collect();
                    }
                }
            }
        }
        Ok(out)
    }

    /// Appends `batch`'s rows by the rules of the module's documentation:
    /// along column `x` when given, a row of `key`'s code replacing only the
    /// row of that code (REQ-STREAM-040), then trims to `retain`
    /// (REQ-STREAM-020, REQ-STREAM-030). The batch's columns are matched by
    /// name; a ring column the batch lacks takes missing values, and a batch
    /// column the ring lacks, or of another kind, is an error, before any row
    /// is taken.
    pub fn append(&mut self, batch: &Table, x: Option<usize>, key: Option<usize>) -> Result<Counts, DataError> {
        let convs = self.convs(batch)?;
        let mut counts = Counts::default();
        for r in 0..batch.len() {
            if let Some(xi) = x {
                let xv = convs[xi].num(r);
                if xv.is_nan() {
                    continue;
                }
                let last = self.len.checked_sub(1).map(|l| self.get(xi, l));
                if let Some(last) = last.filter(|l| !l.is_nan()) {
                    if xv < last {
                        counts.late += 1;
                        continue;
                    }
                    if xv == last {
                        if let Some(at) = self.same(xi, last, key.map(|k| (k, convs[k].code(r)))) {
                            let slot = self.slot(at);
                            self.write(slot, &convs, r);
                            continue;
                        }
                    }
                }
            }
            if self.len >= self.retain.rows {
                let over = self.len + 1 - self.retain.rows;
                self.evict(over);
                counts.dropped += over;
            }
            if self.len == self.slots {
                self.grow();
            }
            let slot = self.slot(self.len);
            self.write(slot, &convs, r);
            self.len += 1;
        }
        counts.dropped += self.trim(x);
        Ok(counts)
    }

    /// The row an equal x replaces: the last row, or with a key, the last
    /// row of x `last` whose key is `code`.
    fn same(&self, x: usize, last: f64, key: Option<(usize, u32)>) -> Option<usize> {
        let Some((k, code)) = key else { return self.len.checked_sub(1) };
        let codes = self.codes(k)?;
        let xs = self.nums(x)?;
        (0..self.len).rev().take_while(|&i| xs[i] == last).find(|&i| codes[i] == code)
    }
}

/// A table of the rows of `table` in `rows`, its columns' types, roles, units
/// and labels kept: a batch to append, or the rows a replace is compared
/// with.
pub fn rows_of(table: &Table, rows: Range<usize>) -> Table {
    let columns = table
        .columns()
        .iter()
        .map(|c| {
            let values = match &c.values {
                Values::Num(v) => Values::Num(v[rows.clone()].to_vec()),
                Values::Codes(v) => Values::Codes(v[rows.clone()].to_vec()),
                Values::Text(v) => Values::Text(v[rows.clone()].to_vec()),
            };
            c.with_values(values)
        })
        .collect();
    let mut t = Table::from_parts(columns, rows.len());
    t.sorted = table.sorted.clone();
    t.frame = table.frame;
    t.epoch = table.epoch.clone();
    t
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{Data, Role};

    fn table(x: Vec<f64>, y: Vec<f64>) -> Table {
        Table::from_columns(vec![Column::f64("x", x).role(Role::X), Column::f64("y", y).role(Role::Y)]).unwrap()
    }

    fn xs(r: &Ring) -> Vec<f64> {
        r.nums(0).unwrap().to_vec()
    }

    fn ys(r: &Ring) -> Vec<f64> {
        r.nums(1).unwrap().to_vec()
    }

    fn retain(rows: usize) -> Retain {
        Retain { rows, age: None }
    }

    /// A `Seq` reads its two parts as one: indexing, slicing, splitting and
    /// `partition_point` agree with the joined vector's.
    #[test]
    fn a_seq_reads_its_parts_as_one() {
        let (a, b) = ([1.0, 2.0, 3.0], [4.0, 5.0]);
        let s = Seq::new(&a[..], &b[..]);
        let whole = s.to_vec();
        assert_eq!(whole, [1.0, 2.0, 3.0, 4.0, 5.0]);
        assert_eq!((s.len(), s[3], s.first(), s.last()), (5, 4.0, Some(&1.0), Some(&5.0)));
        for v in 0..7 {
            let v = v as f64;
            assert_eq!(s.partition_point(|&x| x < v), whole.partition_point(|&x| x < v));
        }
        assert_eq!(s.slice(2..4).to_vec(), [3.0, 4.0]);
        assert_eq!(s.split(1..5), [(1, &a[1..]), (3, &b[..])]);
        assert_eq!(s.iter().rev().copied().collect::<Vec<_>>(), [5.0, 4.0, 3.0, 2.0, 1.0]);
        assert!(Seq::<f64>::default().is_empty());
    }

    /// REQ-STREAM-040: an appended row whose x equals the last row's replaces
    /// it; one whose x is less is dropped and counted late; a missing x is
    /// dropped uncounted; a key replaces only the row of its key.
    #[test]
    fn equal_x_replaces_and_less_is_late() {
        let (mut r, _) = Ring::new(table(vec![1.0, 2.0, 3.0], vec![10.0, 20.0, 30.0]), retain(10), Some(0));
        let c = r.append(&table(vec![3.0, 2.5, 4.0, 4.0, f64::NAN, 5.0], vec![31.0, 0.0, 40.0, 41.0, 0.0, 50.0]), Some(0), None).unwrap();
        assert_eq!(c, Counts { dropped: 0, late: 1 });
        assert_eq!(xs(&r), [1.0, 2.0, 3.0, 4.0, 5.0]);
        assert_eq!(ys(&r), [10.0, 20.0, 31.0, 41.0, 50.0]);
        // A long table: rows of one x, one a series; an equal x replaces its
        // series' row only.
        let long = |x: Vec<f64>, y: Vec<f64>, s: Vec<u32>| {
            Table::from_columns(vec![Column::f64("x", x), Column::f64("y", y), Column::categories("s", vec!["a".into(), "b".into()], s).role(Role::Series)])
                .unwrap()
        };
        let (mut r, _) = Ring::new(long(vec![1.0, 1.0], vec![1.0, 2.0], vec![0, 1]), retain(10), Some(0));
        r.append(&long(vec![1.0, 2.0, 2.0, 2.0], vec![3.0, 4.0, 5.0, 6.0], vec![1, 0, 1, 0]), Some(0), Some(2)).unwrap();
        assert_eq!(xs(&r), [1.0, 1.0, 2.0, 2.0]);
        assert_eq!(ys(&r), [1.0, 3.0, 6.0, 5.0]);
        // Without x every row is added.
        let (mut r, _) = Ring::new(table(vec![5.0], vec![1.0]), retain(10), None);
        let c = r.append(&table(vec![1.0, 1.0], vec![2.0, 3.0]), None, None).unwrap();
        assert_eq!((c.late, xs(&r)), (0, vec![5.0, 1.0, 1.0]));
    }

    /// REQ-STREAM-030: with `retain.age`, the oldest kept row is the first
    /// with x ≥ newest − age, after a replace and after each append.
    #[test]
    fn retain_age_keeps_the_newest_span() {
        let age = Retain { rows: 100, age: Some(10.0) };
        let (mut r, dropped) = Ring::new(table((0..30).map(f64::from).collect(), vec![0.0; 30]), age, Some(0));
        assert_eq!((dropped, xs(&r).first().copied(), r.len()), (19, Some(19.0), 11));
        let c = r.append(&table(vec![35.0, 35.5], vec![1.0, 2.0]), Some(0), None).unwrap();
        assert_eq!(c.dropped, 7);
        assert_eq!(xs(&r).first().copied(), Some(26.0), "26 ≥ 35.5 − 10 and 25 is not");
        // A time x: the age in milliseconds, through the column's unit.
        let t =
            Table::from_columns(vec![Column::timestamps("t", TimeUnit::Us, &[Some(0), Some(1_000), Some(5_000), Some(9_000)]), Column::f64("y", vec![0.0; 4])])
                .unwrap();
        let (r, dropped) = Ring::new(t, Retain { rows: 10, age: Some(5.0) }, Some(0));
        assert_eq!((dropped, r.len(), r.get(0, 0)), (2, 2, 5_000.0));
        assert_eq!(duration_ms("15min"), Some(900_000.0));
        assert_eq!(duration_ms("1.5h"), Some(5_400_000.0));
        assert_eq!((duration_ms("h"), duration_ms("1.h"), duration_ms("3 s")), (None, None, None));
    }

    /// REQ-STREAM-020: past `retain.rows` the ring overwrites its oldest rows
    /// and keeps the last `retain.rows`, as a replace with those rows holds.
    #[test]
    fn the_ring_keeps_the_last_rows() {
        let n = 1_000;
        let all = table((0..n).map(f64::from).collect(), (0..n).map(|i| f64::from(i) * 2.0).collect());
        let (mut r, _) = Ring::new(rows_of(&all, 0..1), retain(64), Some(0));
        let mut at = 1;
        let mut dropped = 0;
        for size in [1, 7, 333, 658] {
            dropped += r.append(&rows_of(&all, at..at + size), Some(0), None).unwrap().dropped;
            at += size;
        }
        assert_eq!((at, dropped, r.len(), r.first()), (1_000, 936, 64, 936));
        assert_eq!(xs(&r), (936..1_000).map(f64::from).collect::<Vec<_>>());
        assert!(r.slots() <= 64);
        let (whole, d) = Ring::new(all.clone(), retain(64), Some(0));
        assert_eq!((d, xs(&whole), ys(&whole)), (936, xs(&r), ys(&r)));
        assert!(r.into_table().bitwise_eq(&rows_of(&all, 936..1_000)));
        // Missing x at the end of a sorted replace is not kept.
        let (r, _) = Ring::new(table(vec![1.0, f64::NAN], vec![1.0, 2.0]), retain(10), Some(0));
        assert_eq!(r.len(), 1);
        let d = Data::from_table(r.to_table());
        assert_eq!(d.first_table().map(Table::len), Some(1));
    }

    /// REQ-STREAM-020: once the ring holds `retain.rows` rows, appending
    /// never reallocates: every buffer keeps its address and capacity
    /// through three times as many rows.
    #[test]
    fn appending_past_capacity_never_reallocates() {
        let cap = 100_000;
        let (mut r, _) = Ring::new(table(Vec::new(), Vec::new()), retain(cap), Some(0));
        let batch = |from: usize, n: usize| table((from..from + n).map(|i| i as f64).collect(), vec![1.0; n]);
        let buffers = |r: &Ring| -> Vec<(usize, usize)> {
            r.columns
                .iter()
                .map(|c| match &c.values {
                    Values::Num(v) => (v.as_ptr() as usize, v.capacity()),
                    _ => (0, 0),
                })
                .collect()
        };
        let mut at = 0;
        while at < cap {
            r.append(&batch(at, 1_000), Some(0), None).unwrap();
            at += 1_000;
        }
        let full = buffers(&r);
        assert_eq!(full.iter().map(|b| b.1).collect::<Vec<_>>(), [cap, cap], "grown to exactly retain.rows");
        while at < 3 * cap {
            r.append(&batch(at, 1), Some(0), None).unwrap();
            r.append(&batch(at + 1, 999), Some(0), None).unwrap();
            at += 1_000;
        }
        assert_eq!(buffers(&r), full);
        assert_eq!((r.len(), r.first(), r.get(0, 0)), (cap, 2 * cap as u64, 2.0 * cap as f64));
    }

    /// Category labels a batch brings are added after the ring's; a column
    /// the ring lacks, or of another kind, is an error and takes no row.
    #[test]
    fn batches_match_columns_by_name() {
        let cat = |labels: &[&str], codes: Vec<u32>| {
            Table::from_columns(vec![Column::categories("c", labels.iter().map(|s| s.to_string()).collect(), codes), Column::f64("v", vec![1.0; 2])]).unwrap()
        };
        let (mut r, _) = Ring::new(cat(&["a", "b"], vec![0, 1]), retain(10), None);
        r.append(&cat(&["c", "a"], vec![0, 1]), None, None).unwrap();
        assert_eq!(r.columns()[0].categories, ["a", "b", "c"]);
        assert_eq!((0..4).map(|i| r.label(0, i).unwrap()).collect::<Vec<_>>(), ["a", "b", "c", "a"]);
        let bad = Table::from_columns(vec![Column::f64("z", vec![1.0])]).unwrap();
        assert!(r.append(&bad, None, None).unwrap_err().to_string().contains("`z`"));
        let bad = Table::from_columns(vec![Column::f64("c", vec![1.0])]).unwrap();
        assert!(r.append(&bad, None, None).is_err());
        assert_eq!(r.len(), 4);
        let only_v = Table::from_columns(vec![Column::f64("v", vec![9.0])]).unwrap();
        r.append(&only_v, None, None).unwrap();
        assert!(r.is_missing(0, 4) && r.get(1, 4) == 9.0);
    }
}
