//! What the contract reads off a chart from outside: the plot found by
//! probing `hit`, the data as a chart holds it (sorted by x), the datum a
//! hit names, and the labels a frame writes (rules 19–24, 26).

use glyphplot::chart::{CellPos, Chart};
use glyphplot::data::{Column, Data, Role, Table, Values};
use glyphplot::grid::{Grid, BLANK};
use unicode_width::UnicodeWidthChar;

/// The plot: the cells where `hit` answers, as columns `x..x + w` and rows
/// `y..y + h`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Plot {
    /// Its first column.
    pub x: usize,
    /// Its first row.
    pub y: usize,
    /// Its columns.
    pub w: usize,
    /// Its rows.
    pub h: usize,
}

impl Plot {
    /// The plot of the last frame, `cols × rows`: the bounding box of the
    /// cells whose centre `hit` maps to a datum, and whether every cell in it
    /// does. `None` when no cell does.
    pub fn find(chart: &dyn Chart, cols: usize, rows: usize) -> Option<(Plot, bool)> {
        Plot::from_hits(&hit_map(chart, cols, rows), cols, rows)
    }

    /// The plot of a [`hit_map`] of `cols × rows`, as [`Plot::find`].
    pub fn from_hits(hits: &[bool], cols: usize, rows: usize) -> Option<(Plot, bool)> {
        let cs: Vec<usize> = (0..cols).filter(|&c| (0..rows).any(|r| hits[r * cols + c])).collect();
        let rs: Vec<usize> = (0..rows).filter(|&r| (0..cols).any(|c| hits[r * cols + c])).collect();
        let (x0, x1, y0, y1) = (*cs.first()?, *cs.last()?, *rs.first()?, *rs.last()?);
        let solid = (y0..=y1).all(|r| (x0..=x1).all(|c| hits[r * cols + c]));
        Some((Plot { x: x0, y: y0, w: x1 - x0 + 1, h: y1 - y0 + 1 }, solid))
    }

    /// Whether cell `(c, r)` is in the plot.
    pub fn contains(&self, c: usize, r: usize) -> bool {
        c >= self.x && c < self.x + self.w && r >= self.y && r < self.y + self.h
    }
}

/// Whether `hit` answers at the centre of each cell of the last frame,
/// `cols × rows`, row-major.
pub fn hit_map(chart: &dyn Chart, cols: usize, rows: usize) -> Vec<bool> {
    (0..rows * cols).map(|i| chart.hit(CellPos::center(i % cols, i / cols)).is_some()).collect()
}

/// The x column of a table: the column of role x, else one named `x` or `t`.
pub fn x_column(t: &Table) -> Option<usize> {
    let cols = t.columns();
    cols.iter().position(|c| c.role == Some(Role::X)).or_else(|| cols.iter().position(|c| c.name == "x" || c.name == "t"))
}

/// `data`'s first table with its rows stably sorted by x, missing x last
/// (REQ-DATA-040's sorted copy), computed here rather than by the core; the
/// data itself when it has no table or no x.
pub fn sorted_copy(data: &Data) -> Data {
    let Some(t) = data.first_table() else { return data.clone() };
    let Some(xi) = x_column(t) else { return data.clone() };
    let xs: Vec<f64> = (0..t.len()).map(|r| t.columns()[xi].get(r)).collect();
    let mut order: Vec<usize> = (0..t.len()).collect();
    order.sort_by(|&a, &b| match (xs[a].is_nan(), xs[b].is_nan()) {
        (false, false) => xs[a].total_cmp(&xs[b]),
        (x, y) => x.cmp(&y),
    });
    let columns = t
        .columns()
        .iter()
        .map(|c| {
            let mut n = c.clone();
            n.values = match &c.values {
                Values::Num(v) => Values::Num(order.iter().map(|&i| v[i]).collect()),
                Values::Codes(v) => Values::Codes(order.iter().map(|&i| v[i]).collect()),
                Values::Text(v) => Values::Text(order.iter().map(|&i| v[i].clone()).collect()),
            };
            n
        })
        .collect::<Vec<Column>>();
    Data::from_table(Table::from_columns(columns).expect("a permutation of a valid table"))
}

/// The data as a chart holds it, for looking up the datum a hit names: the
/// sorted copy's columns. Data without an x column (categories, a `values`
/// column) is held in its own order, and a row's x is its position there,
/// as a band chart's x is its category's index.
pub struct Held {
    table: Option<Table>,
    x: Option<usize>,
    /// Every finite x, sorted.
    pub xs: Vec<f64>,
}

impl Held {
    /// `data` as held: sorted by x.
    pub fn new(data: &Data) -> Held {
        let sorted = sorted_copy(data);
        let table = sorted.first_table().cloned();
        let x = table.as_ref().and_then(x_column);
        let mut xs: Vec<f64> = match (&table, x) {
            (Some(t), Some(xi)) => (0..t.len()).map(|r| t.columns()[xi].get(r)).filter(|v| v.is_finite()).collect(),
            (Some(t), None) => (0..t.len()).map(|r| r as f64).collect(),
            _ => Vec::new(),
        };
        xs.sort_by(f64::total_cmp);
        Held { table, x, xs }
    }

    /// The x and y of row `index` of series `series`: a column of that name
    /// (a wide table), or the row of a long table whose series label is
    /// `series`, its y from the column of role y. x is the row's position
    /// when the data has no x. `None` when the table has no such row or
    /// series.
    pub fn datum(&self, series: &str, index: usize) -> Option<(f64, f64)> {
        let t = self.table.as_ref()?;
        if index >= t.len() {
            return None;
        }
        let x = self.x.map_or(index as f64, |xi| t.columns()[xi].get(index));
        if let Some(c) = t.by_name(series).filter(|c| c.role != Some(Role::Series)) {
            return Some((x, c.get(index)));
        }
        let label = t.columns().iter().find(|c| c.role == Some(Role::Series))?.label(index)?;
        let y = t.columns().iter().find(|c| c.role == Some(Role::Y))?;
        (label == series).then(|| (x, y.get(index)))
    }
}

/// Whether two values are the same, NaN equal to NaN.
pub fn same(a: f64, b: f64) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

/// Whether a glyph puts ink in its cell.
pub fn inked(g: u32) -> bool {
    g != BLANK && g != 0
}

/// The dot columns a glyph lights, left and right: a braille cell's by its
/// dots (left 1, 2, 3, 7; right 4, 5, 6, 8), any other glyph both.
pub fn lit_halves(g: u32) -> (bool, bool) {
    if (0x2800..=0x28FF).contains(&g) {
        let m = g - 0x2800;
        (m & 0x47 != 0, m & 0xB8 != 0)
    } else {
        (inked(g), inked(g))
    }
}

/// The text of row `r` from column `from` to `to`, wide glyphs once.
pub fn row_text(g: &Grid, r: usize, from: usize, to: usize) -> String {
    (from..to.min(g.cols())).filter_map(|c| char::from_u32(g.glyph(c, r)).filter(|_| g.glyph(c, r) != 0)).collect()
}

/// A name as the grid shows it: without its zero-width characters
/// (combining marks), which take no cell.
pub fn as_shown(name: &str) -> String {
    name.chars().filter(|c| c.width() != Some(0)).collect()
}

/// The words of a line: runs of characters other than spaces, braille,
/// box drawing and blocks (label text, not marks).
pub fn words(line: &str) -> Vec<String> {
    let mark = |c: char| c.is_whitespace() || ('\u{2500}'..='\u{259F}').contains(&c) || ('\u{2800}'..='\u{28FF}').contains(&c) || c as u32 >= 0x1_CD00;
    line.split(mark).filter(|w| !w.is_empty()).map(str::to_string).collect()
}

/// Whether a word reads as a non-number or a negative zero: `NaN`, `inf`,
/// `Infinity`, `∞`, or `-0` with only zeros after the point (rule 22,
/// REQ-FMT-030).
pub fn bad_number(word: &str) -> bool {
    let w = word.trim_matches(|c: char| matches!(c, ',' | ';' | ':' | '(' | ')' | '[' | ']'));
    if w.contains("NaN") || w.contains("inf") || w.contains("Inf") || w.contains('∞') {
        return true;
    }
    let Some(rest) = w.strip_prefix('-').or_else(|| w.strip_prefix('\u{2212}')) else { return false };
    let rest = rest.trim_end_matches(|c: char| c.is_alphabetic() || c == '%');
    !rest.is_empty() && rest.starts_with('0') && rest.chars().all(|c| c == '0' || c == '.' || c == ',')
}

/// A label's number: grouping commas dropped, the minus sign read as `-`.
pub fn parse_number(word: &str) -> Option<f64> {
    let w: String = word.chars().filter(|&c| c != ',').map(|c| if c == '\u{2212}' { '-' } else { c }).collect();
    w.parse::<f64>().ok().filter(|v| v.is_finite())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negative_zero_and_non_numbers_are_found() {
        for w in ["-0", "-0.0", "\u{2212}0.00", "NaN", "inf", "-Infinity", "∞", "-0,"] {
            assert!(bad_number(w), "{w}");
        }
        for w in ["0", "0.0", "-0.5", "-10", "-0.01", "x", "1e-300", "-1.0"] {
            assert!(!bad_number(w), "{w}");
        }
        assert_eq!(words("|-10┤⣿ a  b│"), vec!["|-10", "a", "b"]);
        assert_eq!(parse_number("\u{2212}1,234.5"), Some(-1234.5));
        assert_eq!(lit_halves(0x2800 + 0x01), (true, false));
        assert_eq!(lit_halves(0x2800 + 0x80), (false, true));
    }
}
