//! The line chart's data: its table sorted by x, and its series found by
//! channel (REQ-DATA-040, REQ-DATA-080).
//!
//! Channels come from the option `encode`, then a column's `role`, then a
//! column of the channel's name (`Table::channel`). When no column plays x,
//! the chart falls back to the first timestamp column, then the first
//! numeric column that no y channel names, so `{"t": […], "close": […]}`
//! draws with no configuration. When no column plays y, every numeric column
//! but x, the series column and timestamps is a series. A category column
//! playing `series` makes the table long: one series a category, in label
//! order, each the rows of its label. When x is still missing, or `encode`
//! names a column the table lacks, the chart draws its empty-data frame and
//! warns once naming the channel.
//!
//! The table is drawn as a copy stably sorted by x when it is out of order
//! (one warning), so x is found by binary search (REQ-SAMPLE-020); rows of
//! missing x are dropped (REQ-DATA-030). The rows live in a [`Ring`] of
//! `retain.rows` (REQ-STREAM-020) and are read in place: a wide table's
//! series are its columns' rows as the ring holds them. A long table's
//! series and the milliseconds of a `us` or `ns` x are derived from the ring
//! ([`Tail`]): after an append only the rows it added or replaced are read,
//! and those the ring dropped leave the front, so a frame copies no data.

use std::collections::BTreeMap;

use crate::data::{ColumnType, DataError, Encode, Role, Table, Warning, Warnings};
use crate::events::Value;
use crate::format::Display;
use crate::ring::{Counts, Retain, Ring, Seq};
use crate::scale::Kind;

/// Values derived from the ring's rows, `v[start..]` live: rows the ring
/// drops leave the front, rows an append replaces leave the end, and new
/// rows are pushed. The buffer is compacted in place once more than half of
/// it is dropped, so a steady stream does not grow it.
#[derive(Clone, Debug, Default)]
struct Tail<T> {
    v: Vec<T>,
    start: usize,
}

impl<T: Copy> Tail<T> {
    /// The live values.
    fn live(&self) -> &[T] {
        &self.v[self.start..]
    }

    /// Drops the first `n` live values.
    fn drop_front(&mut self, n: usize) {
        self.start += n.min(self.v.len() - self.start);
        if self.start * 2 > self.v.len() {
            self.v.drain(..self.start);
            self.start = 0;
        }
    }

    /// Keeps the first `n` live values.
    fn truncate(&mut self, n: usize) {
        self.v.truncate(self.start + n);
    }

    /// Adds a value after the others.
    fn push(&mut self, x: T) {
        self.v.push(x);
    }
}

/// Where a series' values are.
#[derive(Clone, Debug)]
enum Src {
    /// A y column of a wide table, sharing the table's x.
    Wide(usize),
    /// One label's rows of a long table: their x, y and rows, each row
    /// counted from the data's replace (`Ring::first` plus its place), and
    /// the y column.
    Long { x: Tail<f64>, y: Tail<f64>, rows: Tail<u64>, col: usize },
}

/// One series: its name and its values.
#[derive(Clone, Debug)]
pub struct Series {
    /// The series' name: its column's, or its label's in a long table.
    pub name: String,
    src: Src,
}

/// The data as the chart draws it.
#[derive(Clone, Debug)]
pub struct Prepared {
    /// The rows, sorted by x, in a ring (REQ-STREAM-020).
    pub ring: Option<Ring>,
    /// The x column.
    x_col: Option<usize>,
    /// The x values in epoch ms, when the column holds offsets from an
    /// origin (`us`, `ns`): a value a row held.
    x_owned: Option<Tail<f64>>,
    /// The ring's `first` when the derived values were last brought up to
    /// date.
    seen: u64,
    /// A long table's series column: an equal x replaces the row of its
    /// series (REQ-STREAM-040).
    group: Option<usize>,
    /// A long table's y column.
    long_y: Option<usize>,
    /// How x is scaled: time for a timestamp column.
    pub kind: Kind,
    /// The x column's name.
    pub x_name: String,
    /// The series, in order: z-order, later above.
    pub series: Vec<Series>,
    /// What the data says once.
    pub warnings: Warnings,
}

/// Whether a column type is drawn as numbers.
fn numeric(kind: ColumnType) -> bool {
    kind.is_numeric()
}

/// `table` sorted by column `x`, stably, with missing x last: itself when in
/// order, else a copy, warning once when x was out of order (REQ-DATA-040).
fn sorted(table: Table, x: usize, warnings: &mut Warnings) -> Table {
    let table = table.into_sorted(x, warnings);
    let Some(xs) = table.column(x).and_then(|c| c.nums()) else { return table };
    // Missing x in the middle of an ordered column: move those rows last.
    let first_nan = xs.iter().position(|v| v.is_nan());
    match first_nan {
        Some(i) if xs[i..].iter().any(|v| !v.is_nan()) => {
            let mut order: Vec<usize> = (0..xs.len()).filter(|&r| !xs[r].is_nan()).collect();
            order.extend((0..xs.len()).filter(|&r| xs[r].is_nan()));
            table.permuted(&order)
        }
        _ => table,
    }
}

/// A long table's empty series of `label`, its y column `col`.
fn long(label: &str, col: usize) -> Series {
    Series { name: label.to_string(), src: Src::Long { x: Tail::default(), y: Tail::default(), rows: Tail::default(), col } }
}

impl Prepared {
    /// No data.
    pub fn empty() -> Prepared {
        Prepared {
            ring: None,
            x_col: None,
            x_owned: None,
            seen: 0,
            group: None,
            long_y: None,
            kind: Kind::Linear,
            x_name: "x".into(),
            series: Vec::new(),
            warnings: Warnings::new(),
        }
    }

    /// The table's series by the channels `encode` and the columns give
    /// (REQ-DATA-080), its rows sorted by x (REQ-DATA-040) in a ring that
    /// keeps `retain` (REQ-STREAM-020); with the rows the ring dropped.
    pub fn new(table: Table, encode: &Encode, retain: Retain) -> (Prepared, usize) {
        let mut p = Prepared::empty();
        let cols = table.columns();
        // y, as encode, role or name give it; numeric columns only.
        let y_named = match table.channels("y", encode) {
            Ok(v) => Some(v.into_iter().filter(|&i| numeric(cols[i].kind)).collect::<Vec<_>>()),
            Err(w @ Warning::UnknownColumn { .. }) => {
                p.warnings.push(w);
                return p.held(table, retain);
            }
            Err(_) => None,
        };
        let x = match table.channel("x", encode) {
            Ok(i) if cols[i].nums().is_some() => Some(i),
            Ok(_) => None,
            Err(w @ Warning::UnknownColumn { .. }) => {
                p.warnings.push(w);
                return p.held(table, retain);
            }
            Err(_) => {
                let claimed = |i: usize| y_named.as_ref().is_some_and(|v| v.contains(&i));
                let ts = cols.iter().position(|c| c.kind == ColumnType::Timestamp);
                ts.or_else(|| cols.iter().enumerate().position(|(i, c)| numeric(c.kind) && c.role != Some(Role::Series) && !claimed(i)))
            }
        };
        let Some(x) = x else {
            p.warnings.push(Warning::NoChannel { channel: "x".into() });
            return p.held(table, retain);
        };
        let table = sorted(table, x, &mut p.warnings);
        let cols = table.columns();
        let xc = &cols[x];
        p.kind = if xc.kind == ColumnType::Timestamp { Kind::Time } else { Kind::Linear };
        p.x_name = xc.name.clone();
        p.x_col = Some(x);
        let group = table.channel("series", encode).ok().filter(|&i| cols[i].kind == ColumnType::Category && i != x);
        let ys: Vec<usize> = match y_named {
            Some(v) => v.into_iter().filter(|&i| i != x).collect(),
            None => (0..cols.len()).filter(|&i| i != x && Some(i) != group && numeric(cols[i].kind) && cols[i].kind != ColumnType::Timestamp).collect(),
        };
        if ys.is_empty() {
            p.warnings.push(Warning::NoChannel { channel: "y".into() });
        }
        match (group, ys.first()) {
            (Some(g), Some(&y)) => {
                p.group = Some(g);
                p.long_y = Some(y);
                p.series = cols[g].categories.iter().map(|label| long(label, y)).collect();
            }
            _ => p.series = ys.into_iter().map(|i| Series { name: cols[i].name.clone(), src: Src::Wide(i) }).collect(),
        }
        let (ring, dropped) = Ring::new(table, retain, Some(x));
        p.ring = Some(ring);
        p.derive(0);
        (p, dropped)
    }

    /// The table held as it is, without an x to order it by.
    fn held(mut self, table: Table, retain: Retain) -> (Prepared, usize) {
        let (ring, dropped) = Ring::new(table, retain, None);
        self.ring = Some(ring);
        (self, dropped)
    }

    /// Brings what is derived from the ring's rows up to date: the
    /// milliseconds of a `us` or `ns` x, and a long table's series. Rows
    /// before row `from` (counted from the replace) are as they were, but
    /// for those the ring has dropped since; rows from it are read again.
    fn derive(&mut self, from: u64) {
        let Some(ring) = &self.ring else { return };
        let first = ring.first();
        let gone = (first.saturating_sub(self.seen)) as usize;
        let keep = (from.max(first) - first).min(ring.len() as u64) as usize;
        self.seen = first;
        match self.x_col.and_then(|x| ring.column(x).filter(|c| c.origin != 0).zip(ring.nums(x))) {
            Some((c, v)) => {
                let t = self.x_owned.get_or_insert_with(Tail::default);
                t.drop_front(gone);
                t.truncate(keep);
                v.slice(keep..v.len()).iter().for_each(|&h| t.push(c.to_ms(h)));
            }
            None => self.x_owned = None,
        }
        let (Some(g), Some(y)) = (self.group, self.long_y) else { return };
        let (Some(codes), Some(yv)) = (ring.codes(g), ring.nums(y)) else { return };
        // Labels a batch brought are new series, after the others.
        let labels = &ring.columns()[g].categories;
        let known = self.series.len();
        self.series.extend(labels.iter().skip(known).map(|label| long(label, y)));
        let xs: Seq<f64> = match &self.x_owned {
            Some(t) => Seq::one(t.live()),
            None => self.x_col.and_then(|x| ring.nums(x)).unwrap_or_default(),
        };
        let from = first + keep as u64;
        for s in &mut self.series {
            if let Src::Long { x, y, rows, .. } = &mut s.src {
                let old = rows.live().partition_point(|&r| r < first);
                let kept = rows.live().partition_point(|&r| r < from) - old;
                for t in [&mut *x, &mut *y] {
                    t.drop_front(old);
                    t.truncate(kept);
                }
                rows.drop_front(old);
                rows.truncate(kept);
            }
        }
        for i in keep..ring.len() {
            if let Some(Series { src: Src::Long { x, y, rows, .. }, .. }) = self.series.get_mut(codes[i] as usize) {
                x.push(xs[i]);
                y.push(yv[i]);
                rows.push(first + i as u64);
            }
        }
    }

    /// Appends `batch`'s rows to the ring by the append rules
    /// (REQ-STREAM-040): along x, a long table's equal x replacing the row
    /// of its series. What is derived is read again from the last rows of
    /// the last x before the append, the first an equal x may replace.
    pub fn append(&mut self, batch: &Table) -> Result<Counts, DataError> {
        let Some(ring) = &mut self.ring else { return Ok(Counts::default()) };
        let n = ring.len();
        let tail = match self.x_col.and_then(|x| ring.nums(x)) {
            Some(xs) if n > 0 => {
                let last = xs[n - 1];
                (0..n).rev().take_while(|&i| xs[i] == last).last().unwrap_or(n)
            }
            _ => n,
        };
        let from = ring.first() + tail as u64;
        let counts = ring.append(batch, self.x_col, self.group)?;
        self.derive(from);
        Ok(counts)
    }

    /// New retention, the rows past it dropped now; how many.
    pub fn set_retain(&mut self, retain: Retain) -> usize {
        let Some(ring) = &mut self.ring else { return 0 };
        if ring.retain() == retain {
            return 0;
        }
        let dropped = ring.set_retain(retain, self.x_col);
        self.derive(u64::MAX);
        dropped
    }

    /// The table back, for preparing again under new options.
    pub fn into_table(self) -> Option<Table> {
        self.ring.map(Ring::into_table)
    }

    /// The rows held.
    pub fn rows(&self) -> usize {
        self.ring.as_ref().map_or(0, Ring::len)
    }

    /// The rows dropped before the first held since the data was replaced.
    pub fn first(&self) -> u64 {
        self.ring.as_ref().map_or(0, Ring::first)
    }

    /// The x of every row held, in epoch ms for time; empty with no x.
    pub fn table_xs(&self) -> Seq<'_, f64> {
        match (&self.x_owned, self.x_col, &self.ring) {
            (Some(t), _, _) => Seq::one(t.live()),
            (None, Some(i), Some(r)) => r.nums(i).unwrap_or_default(),
            _ => Seq::default(),
        }
    }

    /// Series `s`'s x values, sorted.
    pub fn xs(&self, s: usize) -> Seq<'_, f64> {
        match &self.series[s].src {
            Src::Wide(_) => self.table_xs(),
            Src::Long { x, .. } => Seq::one(x.live()),
        }
    }

    /// Series `s`'s y values.
    pub fn ys(&self, s: usize) -> Seq<'_, f64> {
        match &self.series[s].src {
            Src::Wide(i) => self.ring.as_ref().and_then(|r| r.nums(*i)).unwrap_or_default(),
            Src::Long { y, .. } => Seq::one(y.live()),
        }
    }

    /// The row held of series `s`'s `i`th value.
    pub fn row(&self, s: usize, i: usize) -> usize {
        match &self.series[s].src {
            Src::Wide(_) => i,
            Src::Long { rows, .. } => (rows.live()[i] - self.first()) as usize,
        }
    }

    /// The position in series `s` of row `row`, if the series holds it.
    pub fn pos(&self, s: usize, row: usize) -> Option<usize> {
        match &self.series[s].src {
            Src::Wide(_) => (row < self.xs(s).len()).then_some(row),
            Src::Long { rows, .. } => rows.live().binary_search(&(self.first() + row as u64)).ok(),
        }
    }

    /// Series `s`'s display transform: its y column's `scale`, `offset` and
    /// `unit` (REQ-DATA-100).
    pub fn display(&self, s: usize) -> Display {
        let col = match &self.series[s].src {
            Src::Wide(i) => *i,
            Src::Long { col, .. } => *col,
        };
        self.ring.as_ref().and_then(|r| r.column(col)).map_or_else(Display::default, Display::of)
    }

    /// The display transform every series in `shown` shares, which a y
    /// axis over them takes; the identity when they differ.
    pub fn shared_display(&self, shown: impl Fn(usize) -> bool) -> Display {
        let mut out: Option<Display> = None;
        for s in (0..self.series.len()).filter(|&s| shown(s)) {
            let d = self.display(s);
            match &out {
                None => out = Some(d),
                Some(o) if *o == d => {}
                Some(_) => return Display::default(),
            }
        }
        out.unwrap_or_default()
    }

    /// The x column's display transform (REQ-DATA-100); the identity for
    /// time, whose labels are dates.
    pub fn x_display(&self) -> Display {
        match (self.kind, self.x_col, &self.ring) {
            (Kind::Linear, Some(i), Some(r)) => r.column(i).map_or_else(Display::default, Display::of),
            _ => Display::default(),
        }
    }

    /// The series named `name`.
    pub fn find(&self, name: &str) -> Option<usize> {
        self.series.iter().position(|s| s.name == name)
    }

    /// The least and greatest finite x of every series.
    pub fn x_extent(&self) -> Option<[f64; 2]> {
        let mut out: Option<[f64; 2]> = None;
        for s in 0..self.series.len() {
            let xs = self.xs(s);
            let (Some(&a), Some(&b)) = (xs.iter().find(|v| v.is_finite()), xs.iter().rev().find(|v| v.is_finite())) else { continue };
            out = Some(match out {
                None => [a, b],
                Some([lo, hi]) => [if a < lo { a } else { lo }, if b > hi { b } else { hi }],
            });
        }
        out
    }

    /// The least and greatest finite y of the shown series, over the rows
    /// whose x lies in `range` (every row when `None`).
    pub fn y_extent(&self, shown: impl Fn(usize) -> bool, range: Option<[f64; 2]>) -> Option<[f64; 2]> {
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        for s in (0..self.series.len()).filter(|&s| shown(s)) {
            let (xs, ys) = (self.xs(s), self.ys(s));
            let rows = match range {
                Some([a, b]) => crate::downsample::view_rows(xs, a, b),
                None => 0..ys.len(),
            };
            for &y in ys.slice(rows).iter() {
                if y.is_finite() {
                    if y < lo {
                        lo = y;
                    }
                    if y > hi {
                        hi = y;
                    }
                }
            }
        }
        (lo <= hi).then_some([lo, hi])
    }

    /// Row `row`, column by column, as events show it: numbers as held,
    /// timestamps in epoch ms, categories and strings as labels, missing as
    /// null.
    pub fn datum(&self, row: usize) -> BTreeMap<String, Value> {
        self.ring.as_ref().map(|r| r.datum(row)).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{Column, Data};
    use crate::fixtures::generate;

    fn prep(data: Data) -> Prepared {
        Prepared::new(data.into_tables().remove(0).1, &Encode::default(), Retain::default()).0
    }

    /// REQ-DATA-080: with no column named or playing x, the first timestamp
    /// column is x, and the other numbers are series.
    #[test]
    fn x_falls_back_to_the_first_timestamp_then_number() {
        let t = Table::from_columns(vec![Column::f64("close", vec![3.0, 4.0]), Column::timestamps_ms("t", vec![10.0, 20.0])]).unwrap();
        let p = prep(Data::from_table(t));
        assert_eq!((p.x_name.as_str(), p.kind), ("t", Kind::Time));
        assert_eq!(p.series.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["close"]);
        let t = Table::from_columns(vec![Column::f64("a", vec![1.0, 2.0]), Column::f64("b", vec![5.0, 6.0])]).unwrap();
        let p = prep(Data::from_table(t));
        assert_eq!((p.x_name.as_str(), p.series[0].name.as_str()), ("a", "b"));
        assert!(p.warnings.is_empty());
        let t = Table::from_columns(vec![Column::strings("s", vec!["a".into()])]).unwrap();
        let p = prep(Data::from_table(t));
        assert_eq!(p.warnings.as_slice(), &[Warning::NoChannel { channel: "x".into() }]);
    }

    /// REQ-STREAM-020, REQ-STREAM-040: what a long table and a `us` x derive
    /// is kept up to date through appends, replaced rows and rows the ring
    /// drops, and equals what the same rows prepared afresh derive; the
    /// derived buffers stop growing once the stream is steady.
    #[test]
    fn derived_series_follow_the_ring() {
        use crate::data::TimeUnit;
        let rows = |from: i64, n: i64| {
            let t: Vec<Option<i64>> = (from..from + n).map(|i| Some(1_000_000_000 + (i / 2) * 1_000)).collect();
            let s: Vec<u32> = (from..from + n).map(|i| (i % 2) as u32 ^ u32::from(i % 7 == 0)).collect();
            Table::from_columns(vec![
                Column::timestamps("t", TimeUnit::Us, &t).role(Role::X),
                Column::f64("y", (from..from + n).map(|i| i as f64).collect()).role(Role::Y),
                Column::categories("s", vec!["a".into(), "b".into()], s).role(Role::Series),
            ])
            .unwrap()
        };
        let retain = Retain { rows: 50, age: None };
        let (mut p, _) = Prepared::new(rows(0, 10), &Encode::default(), retain);
        let mut at = 10;
        let mut caps = Vec::new();
        for k in 0..400 {
            let n = 1 + (k % 5) as i64;
            p.append(&rows(at, n)).unwrap();
            at += n;
            let (fresh, _) = Prepared::new(p.ring.as_ref().unwrap().to_table(), &Encode::default(), retain);
            assert_eq!(p.table_xs().to_vec(), fresh.table_xs().to_vec());
            for s in 0..2 {
                assert_eq!((p.xs(s).to_vec(), p.ys(s).to_vec()), (fresh.xs(s).to_vec(), fresh.ys(s).to_vec()), "series {s} after batch {k}");
                assert_eq!((0..p.xs(s).len()).map(|i| p.row(s, i)).collect::<Vec<_>>(), (0..fresh.xs(s).len()).map(|i| fresh.row(s, i)).collect::<Vec<_>>());
            }
            if k >= 300 {
                caps.push(p.x_owned.as_ref().map(|t| t.v.capacity()));
            }
        }
        assert!(p.first() > 0, "the ring dropped rows");
        assert!(caps.windows(2).all(|w| w[0] == w[1]), "the derived buffers stopped growing: {caps:?}");
    }

    /// REQ-DATA-040, REQ-DATA-030: a long table splits by label, unsorted x
    /// is drawn sorted, and a row of missing x is dropped.
    #[test]
    fn long_tables_split_by_label_and_unsorted_x_is_sorted() {
        let p = prep(generate("labels-wide").unwrap());
        assert_eq!(p.series.len(), 3);
        assert_eq!(p.xs(1).len(), 100);
        assert_eq!(p.row(1, 0), 1);
        assert_eq!(p.pos(2, 5), Some(1));
        let p = prep(generate("unsorted-x").unwrap());
        assert!(p.table_xs().to_vec().windows(2).all(|w| w[0] <= w[1]));
        assert_eq!(p.warnings.len(), 1);
        let t = Table::from_columns(vec![Column::f64("x", vec![1.0, f64::NAN, 2.0]), Column::f64("y", vec![1.0, 2.0, 3.0])]).unwrap();
        let p = prep(Data::from_table(t));
        assert_eq!(p.table_xs().to_vec(), [1.0, 2.0], "missing x is dropped");
        assert_eq!(p.ys(0).to_vec(), [1.0, 3.0]);
    }
}
