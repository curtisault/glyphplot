//! The contract's cases: each fixture a chart accepts, and the series
//! fixtures some rules turn on (`empty`, `one`, `multi-4`, `nan-gaps`,
//! `unsorted-x`, `duplicate-x`) given in a shape the chart takes when it
//! takes no series: as `counts` or `values` for a band chart, as `ohlcv`
//! for candles. Without them rules 11, 15, 21, 23 and 26 would check
//! nothing for a chart of another shape.
//!
//! The forms keep the fixture's rows and their order:
//! - `counts`: a category column `category` (role category) whose labels are the
//!   rows' x as text, and each y column as a value column of its name (role
//!   value). `values`: the one y column (role value), each row its own
//!   category. A band chart draws a category a cell at the least, so both
//!   keep every k-th row, k the least that leaves at most [`CATEGORY_ROWS`]
//!   (`nan-gaps`: every 50th, 200 rows, each run of 500 missing values ten
//!   missing categories).
//! - `ohlcv`: `t` the row's x as minutes from `T0_MS` (role x), `close` its
//!   y, `open` the last row's close (its own y on the first row), `high`
//!   and `low` the greater and lesser of the two, `volume` 1,000 × the
//!   close's move (roles open to volume). A missing y leaves the row's four
//!   prices and volume missing.

use glyphplot::data::{Column, ColumnType, Data, Role, Shape, Table};
use glyphplot::fixtures::{Fixture, FIXTURES, T0_MS};
use glyphplot::registry::Entry;

/// The series fixtures given in other shapes.
pub const FORMED: [&str; 6] = ["empty", "one", "multi-4", "nan-gaps", "unsorted-x", "duplicate-x"];

/// The most rows a `counts` or `values` form keeps.
pub const CATEGORY_ROWS: usize = 200;

/// A fixture as a chart is fed it: the fixture itself, or its rows in
/// another shape (`form`).
#[derive(Clone, Copy, Debug)]
pub struct Case {
    /// The fixture.
    pub fixture: &'static Fixture,
    /// The shape it is given in, when not its own.
    pub form: Option<Shape>,
    /// Its name in reports: the fixture's, then ` as <shape>` for a form.
    pub name: &'static str,
    /// The fixture's name, which the rules key on.
    pub base: &'static str,
    /// Rows a series, as given.
    pub rows: usize,
    /// Series, as given.
    pub series: usize,
    /// The fixture's seed.
    pub seed: u64,
}

impl Case {
    /// The fixture as it is.
    pub fn native(fixture: &'static Fixture) -> Case {
        Case { fixture, form: None, name: fixture.name, base: fixture.name, rows: fixture.rows, series: fixture.series, seed: fixture.seed }
    }

    /// `fixture`, a series, in `shape`; `None` when the shape cannot hold it
    /// (`values` and `ohlcv` take one series).
    pub fn formed(fixture: &'static Fixture, shape: Shape) -> Option<Case> {
        let fits = fixture.shape == Shape::Series
            && match shape {
                Shape::Counts => true,
                Shape::Values | Shape::Ohlcv => fixture.series == 1,
                _ => false,
            };
        if !fits {
            return None;
        }
        let rows = if shape == Shape::Ohlcv { fixture.rows } else { fixture.rows.div_ceil(stride(fixture.rows)) };
        // A few names a test run; leaked so a case stays `Copy`.
        let name: &'static str = Box::leak(format!("{} as {}", fixture.name, shape.name()).into_boxed_str());
        Some(Case { fixture, form: Some(shape), name, base: fixture.name, rows, series: fixture.series, seed: fixture.seed })
    }

    /// The data, generated afresh.
    pub fn generate(&self) -> Data {
        let data = self.fixture.generate();
        match self.form {
            None => data,
            Some(shape) => convert(&data, shape).unwrap_or_else(|| panic!("{} has no {} form", self.base, shape.name())),
        }
    }
}

/// The cases of a chart: every fixture it accepts, in the plan's order, then
/// each of [`FORMED`] it does not accept in the first of its shapes that
/// holds it.
pub fn cases(entry: &Entry) -> Vec<Case> {
    let shapes = entry.meta.shapes;
    let mut out: Vec<Case> = FIXTURES.iter().filter(|f| shapes.contains(&f.shape)).map(Case::native).collect();
    for f in FIXTURES.iter().filter(|f| FORMED.contains(&f.name) && !shapes.contains(&f.shape)) {
        if let Some(c) = shapes.iter().find_map(|&s| Case::formed(f, s)) {
            out.push(c);
        }
    }
    out
}

/// The step between rows a `counts` or `values` form keeps of `n`.
fn stride(n: usize) -> usize {
    n.div_ceil(CATEGORY_ROWS).max(1)
}

/// A number as a category label: integers without a point.
fn label(x: f64) -> String {
    if x.fract() == 0.0 && x.abs() < 1e15 {
        format!("{}", x as i64)
    } else {
        format!("{x}")
    }
}

/// A series table, `data`'s first, in `shape`, as the module says.
pub fn convert(data: &Data, shape: Shape) -> Option<Data> {
    let t = data.first_table()?;
    let cols = t.columns();
    let x = cols.iter().find(|c| c.role == Some(Role::X))?;
    let ys: Vec<&Column> = cols.iter().filter(|c| c.role == Some(Role::Y)).collect();
    let n = t.len();
    let xs: Vec<f64> = (0..n).map(|r| x.get(r)).collect();
    let columns = match shape {
        Shape::Counts | Shape::Values => {
            let keep: Vec<usize> = (0..n).step_by(stride(n)).collect();
            let mut out = Vec::new();
            if shape == Shape::Counts {
                let mut labels: Vec<String> = Vec::new();
                let codes = keep
                    .iter()
                    .map(|&r| {
                        let l = label(xs[r]);
                        let at = labels.iter().position(|e| *e == l).unwrap_or_else(|| {
                            labels.push(l);
                            labels.len() - 1
                        });
                        at as u32
                    })
                    .collect();
                out.push(Column::categories("category", labels, codes).role(Role::Category));
            } else if ys.len() != 1 {
                return None;
            }
            out.extend(ys.iter().map(|y| Column::f64(&y.name, keep.iter().map(|&r| y.get(r)).collect()).role(Role::Value)));
            out
        }
        Shape::Ohlcv => {
            let [y] = ys.as_slice() else { return None };
            let close: Vec<f64> = (0..n).map(|r| y.get(r)).collect();
            let open: Vec<f64> = (0..n).map(|r| if r > 0 && close[r - 1].is_finite() && close[r].is_finite() { close[r - 1] } else { close[r] }).collect();
            let high = (0..n).map(|r| if close[r].is_nan() { f64::NAN } else { open[r].max(close[r]) }).collect();
            let low = (0..n).map(|r| if close[r].is_nan() { f64::NAN } else { open[r].min(close[r]) }).collect();
            let volume = (0..n).map(|r| ((close[r] - open[r]).abs() * 1000.0).round()).collect();
            vec![
                Column::timestamps_ms("t", xs.iter().map(|&x| T0_MS + x * 60_000.0).collect()).role(Role::X),
                Column::f64("open", open).role(Role::Open),
                Column::f64("high", high).role(Role::High),
                Column::f64("low", low).role(Role::Low),
                Column::f64("close", close).role(Role::Close),
                Column::numbers("volume", ColumnType::I64, volume).role(Role::Volume),
            ]
        }
        _ => return None,
    };
    Table::from_columns(columns).ok().map(Data::from_table)
}

#[cfg(test)]
mod tests {
    use super::*;
    use glyphplot::data::Encode;
    use glyphplot::fixtures::fixture;

    #[test]
    fn forms_are_of_their_shape_and_keep_rows_in_order() {
        let gaps = fixture("nan-gaps").expect("nan-gaps");
        let c = Case::formed(gaps, Shape::Counts).expect("a counts form");
        let d = c.generate();
        assert_eq!(d.shape(&[Shape::Counts], &Encode::default()).ok(), Some(Shape::Counts));
        let t = d.first_table().expect("a table");
        assert_eq!((c.rows, t.len(), c.name), (200, 200, "nan-gaps as counts"));
        let v = t.by_name("y").expect("y");
        let missing: Vec<usize> = (0..t.len()).filter(|&r| v.get(r).is_nan()).collect();
        assert_eq!(missing, (30..40).chain(70..80).chain(110..120).chain(150..160).chain(190..200).collect::<Vec<_>>());
        assert_eq!(t.columns()[0].label(3), Some("150"));
        let o = Case::formed(gaps, Shape::Ohlcv).expect("an ohlcv form").generate();
        assert_eq!(o.shape(&[Shape::Ohlcv], &Encode::default()).ok(), Some(Shape::Ohlcv));
        assert!(Case::formed(fixture("multi-4").expect("multi-4"), Shape::Values).is_none());
    }
}
