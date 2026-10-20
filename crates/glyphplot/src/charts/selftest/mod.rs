//! Broken charts the contract must reject (REQ-CONTRACT-060), registered in
//! the `selftest` family only with the `contract-selftest` feature, never in
//! a default or wasm build. Each is the `line` chart with one fault:
//!
//! - `selftest-random`: each fresh chart inks its frame's first cell in a
//!   colour of its own, so two fresh charts draw different frames (contract
//!   rule 8, determinism);
//! - `selftest-charset`: it draws U+0416 Ж, outside the font's ranges, in the
//!   frame's last cell of the first row (rule 5, charset);
//! - `selftest-stateful`: it counts every batch of data it is given, by
//!   replace or append, and inks its first cell by the count, so appending in
//!   batches ends on another frame than a replace (rule 14, v0.6.0's) and a
//!   replace keeps something of the data before it (rule 15).
//!
//! The contract's `selftest_rejects_broken_charts` asserts each is rejected
//! with the matching rule's message. They are test scaffolding, so they
//! build on the `line` family's chart, which a family otherwise may not.

use std::sync::atomic::{AtomicU64, Ordering};

use crate::chart::{Accepted, Appended, CellPos, Chart, Env, Hit, MakeError, Meta, Table};
use crate::charts::line::{self, LineChart};
use crate::data::{Data, DataError};
use crate::grid::Grid;
use crate::input::Input;
use crate::options::Options;
use crate::palette;
use crate::registry::{Entry, Family};
use crate::view::{View, ViewPatch};
use crate::Budget;

/// `selftest-random`: not deterministic (rule 8).
pub static RANDOM: Meta =
    Meta { slug: "selftest-random", name: "selftest random", note: "broken: two fresh charts draw different frames", family: "selftest", ..line::META };

/// `selftest-charset`: draws outside its charset (rule 5).
pub static CHARSET: Meta =
    Meta { slug: "selftest-charset", name: "selftest charset", note: "broken: draws a code point outside the font", family: "selftest", ..line::META };

/// `selftest-stateful`: keeps state across appends and replaces (rules 14
/// and 15).
pub static STATEFUL: Meta =
    Meta { slug: "selftest-stateful", name: "selftest stateful", note: "broken: keeps state across appends and replaces", family: "selftest", ..line::META };

/// The family's types (REQ-REG-010).
pub const REGISTRY: &[Entry] = &[
    Entry { meta: &RANDOM, make: |o| make(o, Fault::Random) },
    Entry { meta: &CHARSET, make: |o| make(o, Fault::Charset) },
    Entry { meta: &STATEFUL, make: |o| make(o, Fault::Stateful) },
];

/// The family's registry entry.
pub const FAMILY: Family = Family { name: "selftest", budget: Budget::DEFAULT, charts: REGISTRY };

/// The code point `selftest-charset` draws: CYRILLIC CAPITAL LETTER ZHE,
/// one column wide and outside the font (REQ-FONT-010).
pub const OUTSIDE: char = '\u{0416}';

/// Which fault a broken chart has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fault {
    /// A colour from a counter of charts made.
    Random,
    /// A glyph outside the font.
    Charset,
    /// A count of data batches kept across replaces.
    Stateful,
}

/// Charts made so far, the source of `selftest-random`'s colours.
static MADE: AtomicU64 = AtomicU64::new(0);

/// A broken chart: the line chart and its fault.
struct Broken {
    inner: LineChart,
    fault: Fault,
    /// For `Random`, this chart's number; for `Stateful`, the batches given.
    n: u64,
}

fn make(options: &Options, fault: Fault) -> Result<Box<dyn Chart>, MakeError> {
    let n = if fault == Fault::Random { MADE.fetch_add(1, Ordering::Relaxed) } else { 0 };
    Ok(Box::new(Broken { inner: LineChart::new(options)?, fault, n }))
}

impl Chart for Broken {
    fn set_data(&mut self, data: Data) -> Result<Accepted, DataError> {
        let a = self.inner.set_data(data)?;
        self.n += u64::from(self.fault == Fault::Stateful);
        Ok(a)
    }

    fn append(&mut self, batch: Data) -> Result<Appended, DataError> {
        let a = self.inner.append(batch)?;
        self.n += u64::from(self.fault == Fault::Stateful);
        Ok(a)
    }

    fn set_options(&mut self, options: &Options) -> Result<(), MakeError> {
        self.inner.set_options(options)
    }

    fn view(&self) -> &View {
        self.inner.view()
    }

    fn set_view(&mut self, patch: &ViewPatch) {
        self.inner.set_view(patch);
    }

    fn input(&mut self, event: &Input) -> crate::chart::Changes {
        self.inner.input(event)
    }

    fn frame(&mut self, t: f64, env: &Env, grid: &mut Grid) {
        self.inner.frame(t, env, grid);
        let series = |n: u64| palette::series((n % u64::from(palette::SERIES_COUNT)) as usize);
        match self.fault {
            // The first cell's glyph kept, its ink changed; a double-width
            // glyph's head is left alone.
            Fault::Random | Fault::Stateful if grid.cols() < 2 || grid.glyph(1, 0) != crate::grid::WIDE_TAIL => {
                let glyph = grid.glyph(0, 0);
                grid.set(0, 0, glyph, series(self.n));
            }
            Fault::Random | Fault::Stateful => {}
            Fault::Charset => {
                let last = grid.cols().saturating_sub(1) as i32;
                grid.set(last, 0, u32::from(OUTSIDE), palette::INK);
            }
        }
    }

    fn hit(&self, at: CellPos) -> Option<Hit> {
        self.inner.hit(at)
    }

    fn table(&self, max_rows: usize) -> Table {
        self.inner.table(max_rows)
    }
}
