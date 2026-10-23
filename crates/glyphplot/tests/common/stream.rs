//! Rule 14's data (REQ-CONTRACT-040): a fixture cut into the batches 1, 7,
//! ⌊n/3⌋ and the rest, and the rows the append rules keep of it, worked out
//! here from the rules' words (REQ-STREAM-040) and not from the core's ring,
//! so the contract checks the ring against them:
//!
//! - the first batch reaches a chart with no data, so it is a replace: its
//!   rows sorted stably by x, a row of missing x dropped;
//! - after it, a row of missing x is dropped; a row whose x is below the
//!   last kept row's is late; a row whose x equals it replaces the last kept
//!   row, or in a long table (a category column of role `series`) the last
//!   kept row of that x and series, and is kept after them when there is
//!   none; any other row is kept after the rest.
//!
//! x is the column of role `x`; a table without one (a band chart's
//! categories) keeps every row in order. A fixture of sorted, distinct x
//! keeps every row, so its reference is the replace with the same rows.

use std::ops::Range;

use glyphplot::data::{ColumnType, Data, Role, Table, Values};

/// The cuts of `n` rows into the batches 1, 7, ⌊n/3⌋ and the rest, each cut
/// at most `n`: an empty batch where the rows run out.
pub fn batches(n: usize) -> Vec<Range<usize>> {
    let cuts = [0, 1, 8, 8 + n / 3, n].map(|c| c.min(n));
    cuts.windows(2).map(|w| w[0]..w[1]).collect()
}

/// The rows `rows` of `table`, in that order, as data of their own: every
/// column's type, role, unit and labels kept.
pub fn rows_of(table: &Table, rows: &[usize]) -> Data {
    let columns = table
        .columns()
        .iter()
        .map(|c| {
            let mut n = c.clone();
            n.values = match &c.values {
                Values::Num(v) => Values::Num(rows.iter().map(|&r| v[r]).collect()),
                Values::Codes(v) => Values::Codes(rows.iter().map(|&r| v[r]).collect()),
                Values::Text(v) => Values::Text(rows.iter().map(|&r| v[r].clone()).collect()),
            };
            n
        })
        .collect();
    let mut t = Table::from_columns(columns).expect("rows of a valid table");
    t.sorted = table.sorted.clone();
    Data::from_table(t)
}

/// The rows `range` of `table`, as data of their own.
pub fn range_of(table: &Table, range: Range<usize>) -> Data {
    rows_of(table, &range.collect::<Vec<_>>())
}

/// What the append rules make of a table given in [`batches`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Kept {
    /// The rows kept, in order.
    pub rows: Vec<usize>,
    /// The rows refused as late.
    pub late: usize,
}

/// The rows the append rules keep of `table` when its first `first` rows
/// are a replace and the rest are appended in order (the module's
/// documentation).
pub fn kept(table: &Table, first: usize) -> Kept {
    let n = table.len();
    let cols = table.columns();
    let Some(x) = cols.iter().position(|c| c.role == Some(Role::X)) else { return Kept { rows: (0..n).collect(), late: 0 } };
    let key = cols.iter().position(|c| c.role == Some(Role::Series) && c.kind == ColumnType::Category);
    let xv = |r: usize| cols[x].get(r);
    let kv = |r: usize| key.map(|k| cols[k].get(r).to_bits());
    let mut out: Vec<usize> = (0..first.min(n)).filter(|&r| !xv(r).is_nan()).collect();
    out.sort_by(|&a, &b| xv(a).total_cmp(&xv(b)));
    let mut late = 0;
    for r in first.min(n)..n {
        let v = xv(r);
        if v.is_nan() {
            continue;
        }
        let Some(&last) = out.last() else {
            out.push(r);
            continue;
        };
        if v < xv(last) {
            late += 1;
        } else if v == xv(last) {
            let tail = out.iter().rposition(|&k| xv(k) != v).map_or(0, |i| i + 1);
            match (tail..out.len()).rev().find(|&i| kv(out[i]) == kv(r)) {
                Some(i) => out[i] = r,
                None => out.push(r),
            }
        } else {
            out.push(r);
        }
    }
    Kept { rows: out, late }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glyphplot::data::Column;

    #[test]
    fn the_rules_as_worded() {
        assert_eq!(batches(30), [0..1, 1..8, 8..18, 18..30]);
        assert_eq!(batches(2), [0..1, 1..2, 2..2, 2..2]);
        let t = Table::from_columns(vec![
            Column::f64("x", vec![5.0, 1.0, 2.0, 2.0, f64::NAN, 1.5, 3.0]).role(Role::X),
            Column::f64("y", vec![0.0; 7]).role(Role::Y),
        ])
        .unwrap();
        assert_eq!(kept(&t, 2), Kept { rows: vec![1, 0], late: 4 });
        assert_eq!(kept(&t, 1), Kept { rows: vec![0], late: 5 });
        let t = Table::from_columns(vec![
            Column::f64("x", vec![1.0, 2.0, 2.0, 2.0, 3.0]).role(Role::X),
            Column::categories("s", vec!["a".into(), "b".into()], vec![0, 0, 1, 0, 1]).role(Role::Series),
        ])
        .unwrap();
        assert_eq!(kept(&t, 1), Kept { rows: vec![0, 3, 2, 4], late: 0 });
    }
}
