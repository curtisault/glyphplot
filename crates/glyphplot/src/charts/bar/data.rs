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

use std::collections::BTreeMap;

use crate::data::{ColumnType, Encode, Table, Values, Warning, Warnings};
use crate::events::Value;
use crate::format::{Display, Locale, NumberFormat, Specifier};

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
    /// The least finite value as displayed; NaN when none is finite.
    pub least: f64,
}

/// The data as the bar chart draws it.
#[derive(Clone, Debug)]
pub(super) struct Prepared {
    /// The table, in its own row order.
    pub table: Option<Table>,
    /// Where the labels come from.
    labels: Labels,
    /// The category column's name, the table's first header.
    pub cat_name: String,
    /// The series, in order: z-order and stacking order, the first lowest.
    pub series: Vec<Series>,
    /// The categories: one a row.
    n: usize,
    /// What the data says once.
    pub warnings: Warnings,
}

impl Prepared {
    /// No data.
    pub fn empty() -> Prepared {
        Prepared { table: None, labels: Labels::Rows, cat_name: "category".into(), series: Vec::new(), n: 0, warnings: Warnings::new() }
    }

    /// The table's categories and series by the channels `encode` and the
    /// columns give (REQ-DATA-080).
    pub fn new(table: Table, encode: &Encode) -> Prepared {
        let mut p = Prepared::empty();
        let cols = table.columns();
        let labels = match table.channel("category", encode) {
            Ok(i) => Some(i),
            Err(w @ Warning::UnknownColumn { .. }) => {
                p.warnings.push(w);
                p.table = Some(table);
                return p;
            }
            Err(_) => cols.iter().position(|c| c.kind == ColumnType::Category).or_else(|| cols.iter().position(|c| c.kind == ColumnType::String)),
        };
        let values = match table.channels("value", encode) {
            Ok(v) => v.into_iter().filter(|&i| cols[i].kind.is_numeric() && Some(i) != labels).collect::<Vec<_>>(),
            Err(w @ Warning::UnknownColumn { .. }) => {
                p.warnings.push(w);
                p.table = Some(table);
                return p;
            }
            Err(_) => (0..cols.len()).filter(|&i| Some(i) != labels && cols[i].kind.is_numeric() && cols[i].kind != ColumnType::Timestamp).collect(),
        };
        if values.is_empty() {
            p.warnings.push(Warning::NoChannel { channel: "value".into() });
            p.table = Some(table);
            return p;
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
        p.series = values
            .into_iter()
            .map(|i| {
                let display = Display::of(&cols[i]);
                let least = cols[i].nums().unwrap_or(&[]).iter().map(|&v| display.shown(v)).filter(|v| v.is_finite()).fold(f64::NAN, f64::min);
                Series { name: cols[i].name.clone(), col: i, display, least }
            })
            .collect();
        p.n = table.len();
        p.table = Some(table);
        p
    }

    /// The table back, for preparing again under new options.
    pub fn into_table(self) -> Option<Table> {
        self.table
    }

    /// The categories: the table's rows when it has a value column, else 0.
    pub fn len(&self) -> usize {
        self.n
    }

    /// The rows the table holds.
    pub fn rows(&self) -> usize {
        self.table.as_ref().map_or(0, Table::len)
    }

    /// Series `s`'s stored values, one a category.
    pub fn values(&self, s: usize) -> &[f64] {
        let col = self.series[s].col;
        self.table.as_ref().and_then(|t| t.column(col)).and_then(|c| c.nums()).unwrap_or(&[])
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
        let Some(t) = &self.table else { return };
        match self.labels {
            Labels::Column(c) => {
                if let Some(l) = t.column(c).and_then(|c| c.label(i)) {
                    out.push_str(l);
                }
            }
            Labels::Numbers(c) => {
                let v = t.column(c).map_or(f64::NAN, |c| c.get(i));
                if v.is_finite() {
                    NumberFormat::new(&Specifier::default(), locale).format_into(v, out);
                }
            }
            Labels::Dates(c) => {
                let v = t.column(c).map_or(f64::NAN, |c| c.to_ms(c.get(i)));
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
    use crate::data::{Column, Data, Role};
    use crate::fixtures::generate;

    fn prep(data: Data) -> Prepared {
        Prepared::new(data.into_tables().remove(0).1, &Encode::default())
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
