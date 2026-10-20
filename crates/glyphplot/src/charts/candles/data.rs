//! The candlestick chart's data: its table sorted by x, and its channels
//! found by `encode`, role or name (REQ-DATA-040, REQ-DATA-080).
//!
//! `open`, `high`, `low` and `close` come by `encode`, role or name;
//! `volume` is optional and draws the volume pane (REQ-CANDLE-040). x comes
//! the same way, else the first timestamp column, else the first numeric
//! column no other channel takes, as the line chart finds it. A table
//! without all four prices gives one value a row (its `close`, else its
//! first `y`, else its first other number) to all four, so daily closes
//! resample into weekly candles; a price channel found without the others
//! is warned of once. With no value, or when `encode` names a column the
//! table lacks, the chart draws its empty-data frame and warns once naming
//! the channel. The rows drawn are
//! those of finite x: the table is sorted with missing x last, so they are
//! one range, found once.

use std::collections::BTreeMap;
use std::ops::Range;

use crate::data::{Column, ColumnType, Encode, Table, Values, Warning, Warnings};
use crate::events::Value;
use crate::format::Display;
use crate::scale::Kind;

/// The four price channels, in the readout's order.
pub const PRICES: [&str; 4] = ["open", "high", "low", "close"];

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
    /// The open, high, low and close columns, when all four resolve.
    prices: Option<[usize; 4]>,
    /// The volume column, when there is one.
    volume: Option<usize>,
    /// The rows of finite x.
    pub rows: Range<usize>,
    /// How x is scaled: time for a timestamp column.
    pub kind: Kind,
    /// The x column's name.
    pub x_name: String,
    /// The price columns' names, open to close.
    pub names: [String; 4],
    /// Whether one column gives all four prices.
    pub single: bool,
    /// The volume column's name.
    pub volume_name: String,
    /// The prices' display transform: the close column's (REQ-DATA-100).
    pub price: Display,
    /// The volume's display transform.
    pub vol: Display,
    /// What the data says once.
    pub warnings: Warnings,
}

/// `table` sorted by column `x`, stably, missing x last: itself when in order,
/// else a copy, warning once when x was out of order (REQ-DATA-040).
fn sorted(table: Table, x: usize, warnings: &mut Warnings) -> Table {
    let table = table.into_sorted(x, warnings);
    let Some(xs) = table.column(x).and_then(Column::nums) else { return table };
    match xs.iter().position(|v| v.is_nan()) {
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
        Prepared {
            table: None,
            x_col: None,
            x_owned: None,
            prices: None,
            volume: None,
            rows: 0..0,
            kind: Kind::Time,
            x_name: "t".into(),
            names: PRICES.map(String::from),
            single: false,
            volume_name: "volume".into(),
            price: Display::default(),
            vol: Display::default(),
            warnings: Warnings::new(),
        }
    }

    /// The table's channels by `encode` and the columns (REQ-DATA-080), its
    /// rows sorted by x (REQ-DATA-040).
    pub fn new(table: Table, encode: &Encode) -> Prepared {
        let mut p = Prepared::empty();
        let cols = table.columns();
        let numeric = |i: usize| cols[i].nums().is_some() && cols[i].kind.is_numeric();
        // A channel's numeric column, None when it has none; `encode` naming
        // a column the table lacks is an error to warn of.
        let channel = |name: &str| match table.channel(name, encode) {
            Ok(i) => Ok(Some(i).filter(|&i| numeric(i))),
            Err(w @ Warning::UnknownColumn { .. }) => Err(w),
            Err(_) => Ok(None),
        };
        let found = (|| -> Result<_, Warning> {
            let x = match table.channel("x", encode) {
                Ok(i) => Some(i).filter(|&i| cols[i].nums().is_some()),
                Err(w @ Warning::UnknownColumn { .. }) => return Err(w),
                Err(_) => None,
            };
            let four = [channel("open")?, channel("high")?, channel("low")?, channel("close")?];
            Ok((x, four, channel("volume")?))
        })();
        let (x, four, volume) = match found {
            Ok(f) => f,
            Err(w) => {
                p.warnings.push(w);
                p.table = Some(table);
                return p;
            }
        };
        let prices = match four {
            [Some(o), Some(h), Some(l), Some(c)] => [o, h, l, c],
            _ => {
                // One value a row (a close, a y, a price): each row a candle
                // whose four prices are that value, which resampling makes
                // into candles of the interval (the plan's catalogue, "Oil in
                // weekly candles").
                let free = |i: usize| numeric(i) && Some(i) != x && Some(i) != volume && cols[i].kind != ColumnType::Timestamp;
                let ys = table.channels("y", encode).unwrap_or_default();
                let value = four[3].or_else(|| ys.into_iter().find(|&i| free(i))).or_else(|| (0..cols.len()).find(|&i| free(i)));
                let Some(v) = value else {
                    p.warnings.push(Warning::NoChannel { channel: "close".into() });
                    p.table = Some(table);
                    return p;
                };
                if let Some(k) = four.iter().position(Option::is_some).and_then(|_| four.iter().position(Option::is_none)) {
                    p.warnings.push(Warning::NoChannel { channel: PRICES[k].into() });
                }
                p.single = true;
                [v; 4]
            }
        };
        let x = x.or_else(|| {
            let taken = |i: usize| prices.contains(&i) || volume == Some(i);
            let ts = cols.iter().position(|c| c.kind == ColumnType::Timestamp);
            ts.or_else(|| (0..cols.len()).find(|&i| numeric(i) && !taken(i)))
        });
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
        p.names = prices.map(|i| cols[i].name.clone());
        p.price = Display::of(&cols[prices[3]]);
        if let Some(v) = volume {
            p.volume_name = cols[v].name.clone();
            p.vol = Display::of(&cols[v]);
        }
        p.prices = Some(prices);
        p.volume = volume;
        p.table = Some(table);
        let xs = p.all_xs();
        let start = xs.partition_point(|v| *v == f64::NEG_INFINITY);
        let end = start + xs[start..].partition_point(|v| v.is_finite());
        p.rows = start..end;
        p
    }

    /// The table back, for preparing again under new options.
    pub fn into_table(self) -> Option<Table> {
        self.table
    }

    /// The table's rows, drawn or not.
    pub fn len(&self) -> usize {
        self.table.as_ref().map_or(0, Table::len)
    }

    /// Whether every price channel resolved and some row has a finite x.
    pub fn drawable(&self) -> bool {
        self.prices.is_some() && !self.rows.is_empty()
    }

    /// Whether the data has a volume column.
    pub fn has_volume(&self) -> bool {
        self.volume.is_some() && self.prices.is_some()
    }

    /// The x of every row of the table, in epoch ms for time.
    fn all_xs(&self) -> &[f64] {
        match (&self.x_owned, self.x_col, &self.table) {
            (Some(v), _, _) => v,
            (None, Some(i), Some(t)) => t.column(i).and_then(Column::nums).unwrap_or(&[]),
            _ => &[],
        }
    }

    /// Column `i`'s numbers, or nothing.
    fn nums(&self, i: Option<usize>) -> &[f64] {
        match (i, &self.table) {
            (Some(i), Some(t)) => t.column(i).and_then(Column::nums).unwrap_or(&[]),
            _ => &[],
        }
    }

    /// The x of every row, sorted, missing x last; index rows of the table.
    pub fn xs(&self) -> &[f64] {
        self.all_xs()
    }

    /// Price channel `k` (0 open, 1 high, 2 low, 3 close) of every row.
    pub fn price(&self, k: usize) -> &[f64] {
        self.nums(self.prices.map(|p| p[k]))
    }

    /// The volume of every row, or nothing without a volume column.
    pub fn volume(&self) -> &[f64] {
        self.nums(self.volume)
    }

    /// The least and greatest finite x.
    pub fn x_extent(&self) -> Option<[f64; 2]> {
        let xs = self.xs();
        (!self.rows.is_empty()).then(|| [xs[self.rows.start], xs[self.rows.end - 1]])
    }

    /// Table row `row`, column by column, as events show it.
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
    use crate::data::{Data, Role};
    use crate::fixtures::generate;

    fn prep(data: Data) -> Prepared {
        Prepared::new(data.into_tables().remove(0).1, &Encode::default())
    }

    /// REQ-DATA-080: the price channels come by role or name, x falls back
    /// to the first timestamp column, and a table without them warns naming
    /// the channel.
    #[test]
    fn channels_by_role_or_name() {
        let p = prep(generate("ohlcv-days").unwrap());
        assert!(p.drawable() && p.has_volume());
        assert_eq!((p.x_name.as_str(), p.kind, p.rows.clone()), ("t", Kind::Time, 0..500));
        assert_eq!(p.names, PRICES.map(String::from));
        assert_eq!(p.price.unit.as_deref(), Some("USD"));
        let t = Table::from_columns(vec![
            Column::timestamps_ms("when", vec![0.0, 1.0]),
            Column::f64("open", vec![1.0, 2.0]),
            Column::f64("high", vec![2.0, 3.0]),
            Column::f64("low", vec![0.5, 1.0]),
            Column::f64("close", vec![1.5, 2.5]),
        ])
        .unwrap();
        let p = prep(Data::from_table(t));
        assert!(p.drawable() && !p.has_volume());
        assert_eq!(p.x_name, "when");
        // One value a row gives all four prices.
        let p = prep(generate("sine-1k").unwrap());
        assert!(p.drawable() && p.single && p.warnings.is_empty());
        assert_eq!((p.x_name.as_str(), p.names[0].as_str(), p.names[3].as_str()), ("x", "y", "y"));
        let t = Table::from_columns(vec![Column::f64("x", vec![2.0, f64::NAN, 1.0]).role(Role::X), Column::f64("open", vec![1.0, 2.0, 3.0])]).unwrap();
        let p = prep(Data::from_table(t));
        assert!(p.drawable() && p.single);
        assert_eq!(p.warnings.as_slice(), &[Warning::NoChannel { channel: "high".into() }, Warning::Unsorted { column: "x".into() }]);
        let t = Table::from_columns(vec![Column::timestamps_ms("t", vec![1.0]), Column::strings("s", vec!["a".into()])]).unwrap();
        let p = prep(Data::from_table(t));
        assert!(!p.drawable());
        assert_eq!(p.warnings.as_slice(), &[Warning::NoChannel { channel: "close".into() }]);
    }

    /// REQ-DATA-040: unsorted x is drawn sorted, missing x last and out of
    /// the rows drawn.
    #[test]
    fn rows_are_sorted_and_missing_x_left_out() {
        let t = Table::from_columns(vec![
            Column::f64("t", vec![3.0, f64::NAN, 1.0, 2.0]),
            Column::f64("open", vec![30.0, 0.0, 10.0, 20.0]),
            Column::f64("high", vec![31.0, 0.0, 11.0, 21.0]),
            Column::f64("low", vec![29.0, 0.0, 9.0, 19.0]),
            Column::f64("close", vec![30.5, 0.0, 10.5, 20.5]),
        ])
        .unwrap();
        let p = prep(Data::from_table(t));
        assert_eq!(p.rows, 0..3);
        assert_eq!(&p.xs()[..3], &[1.0, 2.0, 3.0]);
        assert_eq!(&p.price(0)[..3], &[10.0, 20.0, 30.0]);
        assert_eq!(p.kind, Kind::Linear);
        assert_eq!(p.warnings.len(), 1);
    }
}
