//! The chart contract: what a chart type says about itself ([`Meta`]), what
//! every chart does ([`Chart`]) and what it is told about the host
//! ([`Env`]) (REQ-CHART-010 to 060, the plan's "The chart contract").
//!
//! A chart family is a folder `charts/<family>/` behind a Cargo feature;
//! each type it plays has a `pub static META: Meta`, a [`Make`] and one line
//! in the family's `REGISTRY` (REQ-CHART-010, REQ-REG-010). A frame is a
//! pure function of the data, the options, the view state and `t`
//! (REQ-VIEW-010, ADR-009): whatever a chart caches is keyed by what it
//! depends on, so frames come out the same in any order of calls.

use std::fmt;
use std::sync::OnceLock;

use serde::ser::{SerializeStruct, Serializer};
use serde::Serialize;

use crate::data::{Data, DataError, Shape, Warning};
use crate::events::Event;
use crate::grid::Grid;
use crate::input::Input;
use crate::locale::{LocalePack, ZoneTable};
use crate::options::Options;
use crate::view::{View, ViewPatch};
use crate::Budget;

pub use crate::hit::Hit;
pub use crate::table::Table;

/// A size in cells: a chart's minimum useful size with axes (REQ-LAYOUT-070).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct Size {
    /// Width in cells, 1 to 320.
    pub cols: u16,
    /// Height in cells, 1 to 120.
    pub rows: u16,
}

/// A part a chart sheds below its minimum size, in its declared order
/// (meta.schema.json `drop`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Part {
    /// The title row.
    Title,
    /// The legend.
    Legend,
    /// The tick labels.
    Labels,
    /// The axes.
    Axes,
}

/// How a chart looks in one ink (meta.schema.json `mono`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mono {
    /// Its colour frame without the colours.
    Auto,
    /// Drawn for one ink on purpose: dash patterns, markers, fills
    /// (REQ-MONO-010 to 040).
    Authored,
}

/// Which glyphs a chart may draw outside label text (meta.schema.json
/// `charset`, REQ-CHAR-010).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Charset {
    /// ASCII, a few symbols, box drawing and blocks.
    Basic,
    /// Also arrows, geometric shapes, braille, octants and legacy computing.
    Extended,
    /// Any scalar, double-width glyphs in two cells.
    Wide,
}

/// What a chart type says about itself, with the fields of meta.schema.json
/// (REQ-CHART-020). Serialised by serde to the JSON the schema accepts;
/// `options` is held as a JSON object's text and written as the object.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Meta {
    /// The chart type, kebab-case and unique; never `render`, `demo` or
    /// `list` (REQ-REG-020).
    pub slug: &'static str,
    /// The display name.
    pub name: &'static str,
    /// One lowercase line, 1 to 72 characters, saying what it shows.
    pub note: &'static str,
    /// The family that plays it: its Cargo feature and wasm module.
    pub family: &'static str,
    /// The modes the `mode` option may name, the default first
    /// (REQ-CHART-030).
    pub modes: &'static [&'static str],
    /// The data shapes it accepts (REQ-DATA-110).
    pub shapes: &'static [Shape],
    /// The defaults of every option it reads, as a JSON object's text, or
    /// `None` for a chart that reads none (REQ-OPT-010).
    pub options: Option<&'static str>,
    /// The smallest grid at which it draws with its axes (REQ-LAYOUT-070).
    pub min: Size,
    /// The parts it sheds below `min`, first shed first.
    pub drop: &'static [Part],
    /// How it looks in one ink.
    pub mono: Mono,
    /// The glyphs it may draw outside label text (REQ-CHAR-010).
    pub charset: Charset,
    /// What it may cost.
    pub budget: Budget,
}

impl Serialize for Meta {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let options = self.options.map(|o| serde_json::from_str::<serde_json::Value>(o).unwrap_or(serde_json::Value::Null));
        let mut m = s.serialize_struct("Meta", 12)?;
        m.serialize_field("slug", self.slug)?;
        m.serialize_field("name", self.name)?;
        m.serialize_field("note", self.note)?;
        m.serialize_field("family", self.family)?;
        m.serialize_field("modes", self.modes)?;
        m.serialize_field("shapes", self.shapes)?;
        m.serialize_field("options", &options)?;
        m.serialize_field("min", &self.min)?;
        m.serialize_field("drop", self.drop)?;
        m.serialize_field("mono", &self.mono)?;
        m.serialize_field("charset", &self.charset)?;
        m.serialize_field("budget", &self.budget)?;
        m.end()
    }
}

impl Meta {
    /// The meta as JSON, for `meta_json()`, `charts_json()` and the manifest.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

/// What a chart is told about the host, and nothing else (REQ-CHART-050).
#[derive(Clone, Copy, Debug)]
pub struct Env<'a> {
    /// Dark ink on a light ground: the frame paints its ground.
    pub paper: bool,
    /// One ink: the frame tells apart without colour what colour tells
    /// (REQ-MONO-010).
    pub mono: bool,
    /// The locale pack labels are written with (REQ-I18N-020).
    pub locale: &'a LocalePack,
    /// The zone table time labels are drawn by (REQ-TZ-020).
    pub zone: &'a ZoneTable,
}

/// The default locale pack, as one value for the life of the program.
pub fn default_locale() -> &'static LocalePack {
    static PACK: OnceLock<LocalePack> = OnceLock::new();
    PACK.get_or_init(LocalePack::default)
}

/// The UTC zone table, as one value for the life of the program.
pub fn default_zone() -> &'static ZoneTable {
    static ZONE: OnceLock<ZoneTable> = OnceLock::new();
    ZONE.get_or_init(ZoneTable::default)
}

impl Env<'static> {
    /// Colour, no paper, the default locale pack and UTC.
    pub fn plain() -> Env<'static> {
        Env { paper: false, mono: false, locale: default_locale(), zone: default_zone() }
    }
}

impl Default for Env<'static> {
    fn default() -> Env<'static> {
        Env::plain()
    }
}

/// What a chart took from `set_data`.
#[derive(Clone, Debug, PartialEq)]
pub struct Accepted {
    /// The rows it holds.
    pub rows: usize,
    /// The data version, one more each commit (`plot-data`'s `version`).
    pub version: u64,
    /// What it says once about the data it draws anyway (REQ-DATA-040,
    /// REQ-DATA-080).
    pub warnings: Vec<Warning>,
}

/// What a chart took from `append` (`plot-data`'s detail).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Appended {
    /// The rows it holds after the append.
    pub rows: usize,
    /// Rows trimmed by retention in this append.
    pub dropped: usize,
    /// Appended rows dropped because their x was below the last row's.
    pub late: usize,
    /// The data version.
    pub version: u64,
}

/// What an input changed (REQ-INPUT-020): the overlay, the base, and the
/// events it produced, in order, for the caller to dispatch.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Changes {
    /// The overlay (crosshair, readout, brush, time cursor) must be drawn
    /// again.
    pub overlay: bool,
    /// The base (data, axes, legend) must be drawn again: the view's
    /// domains, hidden series or selection changed.
    pub base: bool,
    /// The events the input produced (REQ-EVENT-010).
    pub events: Vec<Event>,
}

impl Changes {
    /// The bitmask bit of an overlay change.
    pub const OVERLAY: u32 = 1;
    /// The bitmask bit of a base change.
    pub const BASE: u32 = 2;
    /// The bitmask bit of pending events.
    pub const EVENTS: u32 = 4;

    /// The boundary's bitmask: overlay changed 1, base changed 2, events
    /// pending 4 (REQ-INPUT-020).
    pub fn bits(&self) -> u32 {
        u32::from(self.overlay) * Changes::OVERLAY + u32::from(self.base) * Changes::BASE + u32::from(!self.events.is_empty()) * Changes::EVENTS
    }

    /// Whether nothing changed and nothing is to be dispatched.
    pub fn is_empty(&self) -> bool {
        self.bits() == 0
    }

    /// These changes and `other`'s, events in order.
    pub fn merge(&mut self, other: Changes) {
        self.overlay |= other.overlay;
        self.base |= other.base;
        self.events.extend(other.events);
    }
}

/// A position on the grid in cells, with fractions inside a cell, as input
/// carries it: cell `(c, r)` covers `c..c + 1 × r..r + 1`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CellPos {
    /// Column.
    pub x: f64,
    /// Row.
    pub y: f64,
}

impl CellPos {
    /// A position.
    pub const fn new(x: f64, y: f64) -> CellPos {
        CellPos { x, y }
    }

    /// The centre of cell `(col, row)`, as the terminal sends a cell.
    pub fn center(col: usize, row: usize) -> CellPos {
        CellPos { x: col as f64 + 0.5, y: row as f64 + 0.5 }
    }
}

/// Why a chart could not be made or given options (REQ-REG-010,
/// REQ-WASM-020, REQ-CHART-030).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MakeError {
    /// No chart of this type is compiled in.
    NoChart {
        /// The type asked for.
        name: String,
    },
    /// The options are not valid JSON.
    Options {
        /// The parser's message.
        message: String,
    },
    /// The `mode` option names no mode of the chart (REQ-CHART-030).
    Mode {
        /// The mode given.
        mode: String,
        /// The valid modes.
        modes: &'static [&'static str],
    },
    /// An option's value cannot be used (a number format outside the
    /// subset, REQ-FMT-020).
    Option {
        /// The option.
        key: String,
        /// What is wrong with it.
        message: String,
    },
}

impl fmt::Display for MakeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MakeError::NoChart { name } => write!(f, "there is no chart named \"{name}\""),
            MakeError::Options { message } => write!(f, "options are not a JSON object: {message}"),
            MakeError::Mode { mode, modes } => write!(f, "there is no mode \"{}\": the modes are {}", mode.trim_matches('"'), modes.join(", ")),
            MakeError::Option { key, message } => write!(f, "option `{key}`: {message}"),
        }
    }
}

impl std::error::Error for MakeError {}

/// A chart type's maker: a fresh chart for options already layered on its
/// `META.options` (REQ-CHART-010).
pub type Make = fn(&Options) -> Result<Box<dyn Chart>, MakeError>;

/// What every chart does, with exactly the plan's methods (REQ-CHART-040).
/// `frame` takes its size from the grid it is given. A chart keeps its
/// interactive state in its [`View`] (REQ-VIEW-010) and folds input into it
/// with [`Chart::input`], the one reducer the wasm module and the CLI share
/// (REQ-INPUT-010).
pub trait Chart {
    /// Replaces all the data (REQ-DATA-130, REQ-STREAM-060). Data of a shape
    /// the chart does not take is an error naming the shapes it does
    /// (REQ-DATA-110); the chart then keeps what it had.
    fn set_data(&mut self, data: Data) -> Result<Accepted, DataError>;

    /// Appends rows to the end of the data (streaming, v0.6.0).
    fn append(&mut self, batch: Data) -> Result<Appended, DataError>;

    /// New options, layered on the chart's defaults; data and view kept
    /// (REQ-OPT-020). A mode not listed fails, naming the valid ones
    /// (REQ-CHART-030), and the chart keeps the options it had.
    fn set_options(&mut self, options: &Options) -> Result<(), MakeError>;

    /// The view state, whole (REQ-VIEW-010).
    fn view(&self) -> &View;

    /// Merges a whole or partial view into the view state, a key set to
    /// null returning to its default (REQ-VIEW-020, REQ-VIEW-030). Fires no
    /// event (REQ-VIEW-040).
    fn set_view(&mut self, patch: &ViewPatch);

    /// Folds one input event into the view state and says what changed and
    /// which events it produced (REQ-INPUT-010, REQ-INPUT-020). Reads no
    /// clock: `t` comes with the event.
    fn input(&mut self, event: &Input) -> Changes;

    /// Draws the frame at play time `t` into `grid`, whose size it takes
    /// (REQ-CHART-040): a pure function of the data, the options, the view
    /// and `t` (REQ-VIEW-010).
    fn frame(&mut self, t: f64, env: &Env, grid: &mut Grid);

    /// The datum at a position of the last frame's size, as the cursor would
    /// snap to it (REQ-HIT-010), or `None` off the plot or before any frame.
    fn hit(&self, at: CellPos) -> Option<Hit>;

    /// The accessible text of the current view: the hidden table's rows (at
    /// most `max_rows`), the drawing's label and the slider's value text
    /// (REQ-A11Y-020, REQ-A11Y-030).
    fn table(&self, max_rows: usize) -> Table;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changes_bitmask() {
        let mut c = Changes::default();
        assert_eq!(c.bits(), 0);
        c.overlay = true;
        assert_eq!(c.bits(), 1);
        c.base = true;
        c.events.push(Event::Hover(None));
        assert_eq!(c.bits(), 7);
    }

    #[test]
    fn make_errors_read_as_the_plan_writes_them() {
        assert_eq!(MakeError::NoChart { name: "pie".into() }.to_string(), "there is no chart named \"pie\"");
        assert_eq!(MakeError::Options { message: "EOF".into() }.to_string(), "options are not a JSON object: EOF");
        assert_eq!(
            MakeError::Mode { mode: "zig".into(), modes: &["line", "step", "area"] }.to_string(),
            "there is no mode \"zig\": the modes are line, step, area"
        );
    }
}
