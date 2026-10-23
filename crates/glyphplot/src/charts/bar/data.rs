//! The bar chart's data: its categories, one a row in the data's order, and
//! its series, one a value column (REQ-DATA-080, REQ-DATA-100).
//!
//! The category channel comes from the option `encode`, then a column's
//! `role`, then a column named `category`; with none of them, the first
//! category column, then the first string column; with neither (the `values`
//! shape), each row is its own category, labelled by its number from 1. The
//! value channel takes every column `encode` names, else every column of
//! role `value`, else the column named `value`; with none of them, every
//! numeric column but the category's and timestamps. A table with no value
//! column, or an `encode` naming a column the table lacks, draws the
//! empty-data frame and warns once naming the channel.
//!
//! Each series keeps its column's display transform, [`Display`]: values are
//! drawn and ticked as displayed (stored × `scale` + `offset`, an affine map,
//! so the bars stand where the stored values would) and read out with the
//! unit, while the stored values never change (REQ-DATA-100).
//!
//! The rows live in the core's ring of `retain.rows` (`ring.rs`,
//! REQ-STREAM-020), read in place. A band chart has no x to order its rows
//! by, so an append adds every row as a new category after the others, new
//! labels after the old ones, and the oldest categories go past
//! `retain.rows`.

use std::cell::OnceCell;
use std::collections::BTreeMap;

use crate::data::{ColumnType, DataError, Encode, Table, Warning, Warnings};
use crate::events::Value;
use crate::format::{Display, Locale, NumberFormat, Specifier};
use crate::ring::{Counts, Retain, Ring, Seq};

/// Where the categories' labels come from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Labels {
    /// A category or string column's labels.
    Column(usize),
    /// A numeric column's numbers.
    Numbers(usize),
    /// A timestamp column's dates.
    Dates(usize),
    /// The rows' numbers, from 1.
    Rows,
}

/// One series: a value column.
#[derive(Clone, Debug)]
pub(super) struct Series {
    /// The column's name.
    pub name: String,
    /// The column's index in the table.
    col: usize,
    /// The column's display transform (REQ-DATA-100).
    pub display: Display,
}

/// The data as the bar chart draws it.
#[derive(Clone, Debug)]
pub(super) struct Prepared {
    /// The rows, in their own order, in a ring (REQ-STREAM-020).
    pub ring: Option<Ring>,
    /// Where the labels come from.
    labels: Labels,
    /// The category column's name, the table's first header.
    pub cat_name: String,
    /// The series, in order: z-order and stacking order, the first lowest.
    pub series: Vec<Series>,
    /// The categories: one a row.
    n: usize,
    /// Each series' least finite value as displayed (NaN when none is),
    /// found when first asked after the rows changed: only `signed: auto`
    /// asks, so an append reads no row it does not add.
    least: OnceCell<Vec<f64>>,
    /// What the data says once.
    pub warnings: Warnings,
}

impl Prepared {
    /// No data.
    pub fn empty() -> Prepared {
        Prepared { ring: None, labels: Labels::Rows, cat_name: "category".into(), series: Vec::new(), n: 0, least: OnceCell::new(), warnings: Warnings::new() }
    }

    /// The table's categories and series by the channels `encode` and the
    /// columns give (REQ-DATA-080), its rows in a ring that keeps `retain`
    /// (REQ-STREAM-020); with the rows the ring dropped.
    pub fn new(table: Table, encode: &Encode, retain: Retain) -> (Prepared, usize) {
        let mut p = Prepared::empty();
        let cols = table.columns();
        let labels = match table.channel("category", encode) {
            Ok(i) => Some(i),
            Err(w @ Warning::UnknownColumn { .. }) => {
                p.warnings.push(w);
                return p.held(table, retain);
            }
            Err(_) => cols.iter().position(|c| c.kind == ColumnType::Category).or_else(|| cols.iter().position(|c| c.kind == ColumnType::String)),
        };
        let values = match table.channels("value", encode) {
            Ok(v) => v.into_iter().filter(|&i| cols[i].kind.is_numeric() && Some(i) != labels).collect::<Vec<_>>(),
            Err(w @ Warning::UnknownColumn { .. }) => {
                p.warnings.push(w);
                return p.held(table, retain);
            }
            Err(_) => (0..cols.len()).filter(|&i| Some(i) != labels && cols[i].kind.is_numeric() && cols[i].kind != ColumnType::Timestamp).collect(),
        };
        if values.is_empty() {
            p.warnings.push(Warning::NoChannel { channel: "value".into() });
            return p.held(table, retain);
        }
        p.labels = match labels {
            None => Labels::Rows,
            Some(i) => match cols[i].kind {
                ColumnType::Category | ColumnType::String => Labels::Column(i),
                ColumnType::Timestamp => Labels::Dates(i),
                _ => Labels::Numbers(i),
            },
        };
        p.cat_name = labels.map_or_else(|| "row".to_string(), |i| cols[i].name.clone());
        p.series = values.into_iter().map(|i| Series { name: cols[i].name.clone(), col: i, display: Display::of(&cols[i]) }).collect();
        let (ring, dropped) = Ring::new(table, retain, None);
        p.ring = Some(ring);
        p.derive();
        (p, dropped)
    }

    /// The table held as it is, with nothing to draw.
    fn held(mut self, table: Table, retain: Retain) -> (Prepared, usize) {
        let (ring, dropped) = Ring::new(table, retain, None);
        self.ring = Some(ring);
        (self, dropped)
    }

    /// What is derived from the rows: the categories; each series' least
    /// value is found again when asked.
    fn derive(&mut self) {
        let Some(ring) = &self.ring else { return };
        self.least = OnceCell::new();
        self.n = if self.series.is_empty() { 0 } else { ring.len() };
    }

    /// Series `s`'s least finite value as displayed; NaN when none is.
    pub fn least(&self, s: usize) -> f64 {
        let all = self.least.get_or_init(|| {
            (0..self.series.len())
                .map(|s| self.values(s).iter().map(|&v| self.series[s].display.shown(v)).filter(|v| v.is_finite()).fold(f64::NAN, f64::min))
                .collect()
        });
        all.get(s).copied().unwrap_or(f64::NAN)
    }

    /// Appends `batch`'s rows, each a category after the others
    /// (REQ-STREAM-020).
    pub fn append(&mut self, batch: &Table) -> Result<Counts, DataError> {
        let Some(ring) = &mut self.ring else { return Ok(Counts::default()) };
        let counts = ring.append(batch, None, None)?;
        self.derive();
        Ok(counts)
    }

    /// New retention, the rows past it dropped now; how many.
    pub fn set_retain(&mut self, retain: Retain) -> usize {
        let Some(ring) = &mut self.ring else { return 0 };
        if ring.retain() == retain {
            return 0;
        }
        let dropped = ring.set_retain(retain, None);
        self.derive();
        dropped
    }

    /// The table back, for preparing again under new options.
    pub fn into_table(self) -> Option<Table> {
        self.ring.map(Ring::into_table)
    }

    /// The categories: the table's rows when it has a value column, else 0.
    pub fn len(&self) -> usize {
        self.n
    }

    /// The rows held.
    pub fn rows(&self) -> usize {
        self.ring.as_ref().map_or(0, Ring::len)
    }

    /// Series `s`'s stored values, one a category, in place.
    pub fn values(&self, s: usize) -> Seq<'_, f64> {
        let col = self.series[s].col;
        self.ring.as_ref().and_then(|r| r.nums(col)).unwrap_or_default()
    }

    /// Series `s`'s stored value in category `i`; NaN when missing.
    pub fn stored(&self, s: usize, i: usize) -> f64 {
        self.values(s).get(i).copied().unwrap_or(f64::NAN)
    }

    /// Series `s`'s value in category `i` as displayed (REQ-DATA-100).
    pub fn shown(&self, s: usize, i: usize) -> f64 {
        self.series[s].display.shown(self.stored(s, i))
    }

    /// The label of category `i`, written into `out`.
    pub fn label_into(&self, i: usize, locale: &Locale, out: &mut String) {
        let Some(t) = &self.ring else { return };
        match self.labels {
            Labels::Column(c) => {
                if let Some(l) = t.label(c, i) {
                    out.push_str(l);
                }
            }
            Labels::Numbers(c) => {
                let v = t.get(c, i);
                if v.is_finite() {
                    NumberFormat::new(&Specifier::default(), locale).format_into(v, out);
                }
            }
            Labels::Dates(c) => {
                let v = t.column(c).map_or(f64::NAN, |col| col.to_ms(t.get(c, i)));
                if v.is_finite() {
                    crate::time::strftime("%Y-%m-%d", v, &locale.time, out);
                }
            }
            Labels::Rows => {
                if let Ok(spec) = Specifier::parse("category", "d") {
                    NumberFormat::new(&spec, locale).format_into((i + 1) as f64, out);
                }
            }
        }
    }

    /// The label of category `i`.
    pub fn label(&self, i: usize, locale: &Locale) -> String {
        let mut s = String::new();
        self.label_into(i, locale, &mut s);
        s
    }

    /// The series named `name`.
    pub fn find(&self, name: &str) -> Option<usize> {
        self.series.iter().position(|s| s.name == name)
    }

    /// Table row `row`, column by column, as events show it: numbers as
    /// held, timestamps in epoch ms, categories and strings as labels,
    /// missing as null.
    pub fn datum(&self, row: usize) -> BTreeMap<String, Value> {
        self.ring.as_ref().map(|r| r.datum(row)).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{Column, Data, Role};
    use crate::fixtures::generate;

    fn prep(data: Data) -> Prepared {
        Prepared::new(data.into_tables().remove(0).1, &Encode::default(), Retain::default()).0
    }

    /// REQ-DATA-080: categories and values by role, by fallback and by row.
    #[test]
    fn channels_by_role_then_fallback() {
        let p = prep(generate("categorical-12").unwrap());
        assert_eq!((p.len(), p.cat_name.as_str(), p.series.len(), p.series[0].name.as_str()), (12, "region", 1, "count"));
        assert_eq!(p.label(0, &Locale::default()), "North Atlantic distribution centre");
        let t = Table::from_columns(vec![
            Column::strings("name", vec!["a".into(), "b".into()]),
            Column::f64("v", vec![1.0, 2.0]),
            Column::f64("w", vec![3.0, 4.0]),
        ])
        .unwrap();
        let p = prep(Data::from_table(t));
        assert_eq!((p.cat_name.as_str(), p.series.len()), ("name", 2));
        assert_eq!(p.label(1, &Locale::default()), "b");
        let t = Table::from_columns(vec![Column::f64("value", vec![5.0, 6.0, 7.0]).role(Role::Value)]).unwrap();
        let p = prep(Data::from_table(t));
        assert_eq!((p.len(), p.label(2, &Locale::default()), p.cat_name.as_str()), (3, "3".to_string(), "row"));
        let t = Table::from_columns(vec![Column::strings("s", vec!["a".into()])]).unwrap();
        let p = prep(Data::from_table(t));
        assert_eq!(p.warnings.as_slice(), &[Warning::NoChannel { channel: "value".into() }]);
        assert_eq!(p.len(), 0);
    }

    /// REQ-DATA-100: a column's scale changes what is shown, not what is held.
    #[test]
    fn display_scales_shown_values() {
        let mut c = Column::f64("price", vec![18240.0]);
        c.scale = Some(0.01);
        let t = Table::from_columns(vec![Column::strings("k", vec!["a".into()]), c]).unwrap();
        let p = prep(Data::from_table(t));
        assert_eq!(p.stored(0, 0), 18240.0);
        assert!((p.shown(0, 0) - 182.4).abs() < 1e-9);
    }
}
