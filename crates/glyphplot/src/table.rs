//! The accessible text of a chart's view: the hidden table's rows, the
//! drawing's label and the slider's value text (REQ-A11Y-020, REQ-A11Y-030).
//!
//! `Chart::table(max_rows)` gives a [`Table`] of the data in the current
//! view, at most `max_rows` rows; past that, [`pick_rows`] keeps each
//! bucket's least and greatest value, so a spike is never dropped, and the
//! shell adds to the caption how many points the view holds. Every cell is
//! written by the core's formatters (REQ-FMT-030). The shell builds the light-DOM
//! `<table>` from it at most once a second, on `final` view changes and
//! data commits, and sets the drawing's label to [`Table::summary`] and the
//! slider's `aria-valuetext` to [`Table::value`].

use serde::Serialize;

/// The accessible text of a chart's view.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Table {
    /// The table's caption: what the rows are, with no count; the shell adds
    /// how many points the view holds when it holds more than the rows
    /// (`captionOf` in web/src/a11y.ts).
    pub caption: String,
    /// The column headers (`th scope="col"`): the x column, then one a
    /// series.
    pub columns: Vec<String>,
    /// The rows, each its x (the row header) and then each series' value,
    /// empty where a series has none.
    pub rows: Vec<Vec<String>>,
    /// The points the view holds.
    pub total: u64,
    /// The drawing's label: the chart, its series, its ranges and its count
    /// (REQ-A11Y-030).
    pub summary: String,
    /// The slider's `aria-valuetext`: the cursor's x and each series' value
    /// there, up to four, then `and N more`; `None` with no cursor
    /// (REQ-A11Y-030).
    pub value: Option<String>,
}

/// The hidden table as the shell reads it (`TableJson` in web/src/api.ts).
#[derive(Serialize)]
struct Shown<'a> {
    caption: &'a str,
    columns: &'a [String],
    rows: Vec<Vec<Option<&'a str>>>,
    total: u64,
}

impl Table {
    /// The table as JSON, every field.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// The table as the wasm boundary's `table_json(max_rows)` returns it:
    /// `{caption, columns, rows, total}`, each cell the text the core wrote,
    /// an empty cell `null` (no value) (`TableJson` in web/src/api.ts,
    /// REQ-A11Y-020).
    pub fn to_shell_json(&self) -> String {
        let rows = self.rows.iter().map(|r| r.iter().map(|c| Some(c.as_str()).filter(|c| !c.is_empty())).collect()).collect();
        serde_json::to_string(&Shown { caption: &self.caption, columns: &self.columns, rows, total: self.total }).unwrap_or_default()
    }
}

/// Which of `n` rows to keep for a table of at most `max_rows` rows, in
/// order: every row when they fit; otherwise `max_rows / 2` equal buckets,
/// each keeping its row of least and of greatest finite `y` (its first row
/// when none is finite) (REQ-A11Y-020).
pub fn pick_rows(n: usize, max_rows: usize, y: impl Fn(usize) -> f64) -> Vec<usize> {
    if n <= max_rows {
        return (0..n).collect();
    }
    if max_rows < 2 {
        return (0..max_rows.min(n)).collect();
    }
    let buckets = max_rows / 2;
    let mut out = Vec::with_capacity(max_rows);
    for b in 0..buckets {
        let (from, to) = (b * n / buckets, (b + 1) * n / buckets);
        let mut lo: Option<(usize, f64)> = None;
        let mut hi: Option<(usize, f64)> = None;
        for i in from..to {
            let v = y(i);
            if !v.is_finite() {
                continue;
            }
            if lo.is_none_or(|(_, l)| v < l) {
                lo = Some((i, v));
            }
            if hi.is_none_or(|(_, h)| v > h) {
                hi = Some((i, v));
            }
        }
        match (lo, hi) {
            (Some((a, _)), Some((b, _))) => {
                out.push(a.min(b));
                if a != b {
                    out.push(a.max(b));
                }
            }
            _ if from < to => out.push(from),
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::generate;

    /// REQ-A11Y-020: 100,000 rows become at most 500, in order, keeping the
    /// view's least and greatest value.
    #[test]
    fn table_keeps_extremes_under_500_rows() {
        let data = generate("random-walk-100k").unwrap();
        let y = data.first_table().unwrap().by_name("y").unwrap().nums().unwrap().to_vec();
        let rows = pick_rows(y.len(), 500, |i| y[i]);
        assert!(rows.len() <= 500 && rows.len() >= 250, "{} rows", rows.len());
        assert!(rows.windows(2).all(|w| w[0] < w[1]), "in order, each once");
        let (min, max) = y.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &v| (a.min(v), b.max(v)));
        assert!(rows.iter().any(|&i| y[i] == min) && rows.iter().any(|&i| y[i] == max));
        assert_eq!(pick_rows(3, 500, |i| i as f64), [0, 1, 2]);
        assert_eq!(pick_rows(10, 0, |i| i as f64), Vec::<usize>::new());
        assert_eq!(pick_rows(10, 4, |_| f64::NAN), [0, 5]);
    }

    /// The shell's form: four fields, an empty cell null.
    #[test]
    fn shell_json_has_null_for_no_value() {
        let t = Table { caption: "c".into(), columns: vec!["x".into(), "y".into()], rows: vec![vec!["1".into(), String::new()]], total: 7, ..Table::default() };
        assert_eq!(t.to_shell_json(), r#"{"caption":"c","columns":["x","y"],"rows":[["1",null]],"total":7}"#);
    }
}
