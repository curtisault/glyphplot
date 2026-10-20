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
//! (one warning), with rows of missing x last, so x is found by binary
//! search (REQ-SAMPLE-020).

use std::collections::BTreeMap;

use crate::data::{ColumnType, Encode, Role, Table, Values, Warning, Warnings};
use crate::events::Value;
use crate::format::Display;
use crate::scale::Kind;

/// Where a series' values are.
#[derive(Clone, Debug)]
enum Src {
    /// A y column of a wide table, sharing the table's x.
    Wide(usize),
    /// One label's rows of a long table: their x, y and data rows, and the
    /// y column.
    Long { x: Vec<f64>, y: Vec<f64>, rows: Vec<usize>, col: usize },
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
    /// The table, sorted by x with missing x last.
    pub table: Option<Table>,
    /// The x column.
    x_col: Option<usize>,
    /// The x values in epoch ms, when the column holds offsets from an
    /// origin (`us`, `ns`).
    x_owned: Option<Vec<f64>>,
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

impl Prepared {
    /// No data.
    pub fn empty() -> Prepared {
        Prepared { table: None, x_col: None, x_owned: None, kind: Kind::Linear, x_name: "x".into(), series: Vec::new(), warnings: Warnings::new() }
    }

    /// The table's series by the channels `encode` and the columns give
    /// (REQ-DATA-080), its rows sorted by x (REQ-DATA-040).
    pub fn new(table: Table, encode: &Encode) -> Prepared {
        let mut p = Prepared::empty();
        let cols = table.columns();
        // y, as encode, role or name give it; numeric columns only.
        let y_named = match table.channels("y", encode) {
            Ok(v) => Some(v.into_iter().filter(|&i| numeric(cols[i].kind)).collect::<Vec<_>>()),
            Err(w @ Warning::UnknownColumn { .. }) => {
                p.warnings.push(w);
                p.table = Some(table);
                return p;
            }
            Err(_) => None,
        };
        let x = match table.channel("x", encode) {
            Ok(i) if cols[i].nums().is_some() => Some(i),
            Ok(_) => None,
            Err(w @ Warning::UnknownColumn { .. }) => {
                p.warnings.push(w);
                p.table = Some(table);
                return p;
            }
            Err(_) => {
                let claimed = |i: usize| y_named.as_ref().is_some_and(|v| v.contains(&i));
                let ts = cols.iter().position(|c| c.kind == ColumnType::Timestamp);
                ts.or_else(|| cols.iter().enumerate().position(|(i, c)| numeric(c.kind) && c.role != Some(Role::Series) && !claimed(i)))
            }
        };
        let Some(x) = x else {
            p.warnings.push(Warning::NoChannel { channel: "x".into() });
            p.table = Some(table);
            return p;
        };
        let table = sorted(table, x, &mut p.warnings);
        let cols = table.columns();
        let xc = &cols[x];
        p.kind = if xc.kind == ColumnType::Timestamp { Kind::Time } else { Kind::Linear };
        p.x_name = xc.name.clone();
        p.x_col = Some(x);
        if xc.origin != 0 {
            p.x_owned = xc.nums().map(|v| v.iter().map(|&h| xc.to_ms(h)).collect());
        }
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
                let (Values::Codes(codes), Some(yv)) = (&cols[g].values, cols[y].nums()) else { unreachable!("a category column holds codes") };
                let xs: &[f64] = match &p.x_owned {
                    Some(v) => v,
                    None => cols[x].nums().unwrap_or(&[]),
                };
                for (code, label) in cols[g].categories.iter().enumerate() {
                    let rows: Vec<usize> = (0..table.len()).filter(|&r| codes[r] == code as u32).collect();
                    let sx = rows.iter().map(|&r| xs[r]).collect();
                    let sy = rows.iter().map(|&r| yv[r]).collect();
                    p.series.push(Series { name: label.clone(), src: Src::Long { x: sx, y: sy, rows, col: y } });
                }
            }
            _ => p.series = ys.into_iter().map(|i| Series { name: cols[i].name.clone(), src: Src::Wide(i) }).collect(),
        }
        p.table = Some(table);
        p
    }

    /// The table back, for preparing again under new options.
    pub fn into_table(self) -> Option<Table> {
        self.table
    }

    /// The table's rows.
    pub fn rows(&self) -> usize {
        self.table.as_ref().map_or(0, Table::len)
    }

    /// The x of every row of the table, in epoch ms for time; empty with no x.
    pub fn table_xs(&self) -> &[f64] {
        match (&self.x_owned, self.x_col, &self.table) {
            (Some(v), _, _) => v,
            (None, Some(i), Some(t)) => t.column(i).and_then(|c| c.nums()).unwrap_or(&[]),
            _ => &[],
        }
    }

    /// Series `s`'s x values, sorted with missing last.
    pub fn xs(&self, s: usize) -> &[f64] {
        match &self.series[s].src {
            Src::Wide(_) => self.table_xs(),
            Src::Long { x, .. } => x,
        }
    }

    /// Series `s`'s y values.
    pub fn ys(&self, s: usize) -> &[f64] {
        match &self.series[s].src {
            Src::Wide(i) => self.table.as_ref().and_then(|t| t.column(*i)).and_then(|c| c.nums()).unwrap_or(&[]),
            Src::Long { y, .. } => y,
        }
    }

    /// The table row of series `s`'s `i`th value.
    pub fn row(&self, s: usize, i: usize) -> usize {
        match &self.series[s].src {
            Src::Wide(_) => i,
            Src::Long { rows, .. } => rows[i],
        }
    }

    /// The position in series `s` of table row `row`, if the series holds it.
    pub fn pos(&self, s: usize, row: usize) -> Option<usize> {
        match &self.series[s].src {
            Src::Wide(_) => (row < self.xs(s).len()).then_some(row),
            Src::Long { rows, .. } => rows.binary_search(&row).ok(),
        }
    }

    /// Series `s`'s display transform: its y column's `scale`, `offset` and
    /// `unit` (REQ-DATA-100).
    pub fn display(&self, s: usize) -> Display {
        let col = match &self.series[s].src {
            Src::Wide(i) => *i,
            Src::Long { col, .. } => *col,
        };
        self.table.as_ref().and_then(|t| t.column(col)).map_or_else(Display::default, Display::of)
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
        match (self.kind, self.x_col, &self.table) {
            (Kind::Linear, Some(i), Some(t)) => t.column(i).map_or_else(Display::default, Display::of),
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
            for &y in &ys[rows] {
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

    /// Table row `row`, column by column, as events show it: numbers as
    /// held, timestamps in epoch ms, categories and strings as labels,
    /// missing as null.
    pub fn datum(&self, row: usize) -> BTreeMap<String, Value> {
        let mut out = BTreeMap::new();
        let Some(t) = &self.table else { return out };
        for c in t.columns() {
            let v = match &c.values {
                Values::Num(v) => match v.get(row) {
                    Some(&h) if !h.is_nan() => Value::Number(c.to_ms(h)),
                    _ => Value::Null,
                },
                _ => c.label(row).map_or(Value::Null, |l| Value::Text(l.to_string())),
            };
            out.insert(c.name.clone(), v);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{Column, Data};
    use crate::fixtures::generate;

    fn prep(data: Data) -> Prepared {
        Prepared::new(data.into_tables().remove(0).1, &Encode::default())
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

    #[test]
    fn long_tables_split_by_label_and_unsorted_x_is_sorted() {
        let p = prep(generate("labels-wide").unwrap());
        assert_eq!(p.series.len(), 3);
        assert_eq!(p.xs(1).len(), 100);
        assert_eq!(p.row(1, 0), 1);
        assert_eq!(p.pos(2, 5), Some(1));
        let p = prep(generate("unsorted-x").unwrap());
        assert!(p.table_xs().windows(2).all(|w| w[0] <= w[1]));
        assert_eq!(p.warnings.len(), 1);
        let t = Table::from_columns(vec![Column::f64("x", vec![1.0, f64::NAN, 2.0]), Column::f64("y", vec![1.0, 2.0, 3.0])]).unwrap();
        let p = prep(Data::from_table(t));
        assert_eq!(p.table_xs()[..2], [1.0, 2.0]);
        assert!(p.table_xs()[2].is_nan(), "missing x goes last");
        assert_eq!(p.ys(0), &[1.0, 3.0, 2.0]);
    }
}
