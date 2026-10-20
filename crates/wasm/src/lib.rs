//! glyphplot's wasm-bindgen boundary (the plan's "The boundary",
//! REQ-WASM-010). `mise run build:wasm` builds this crate once a chart
//! family, with that family's feature alone, into
//! `web/pkg/<family>/glyphplot{.js,_bg.wasm}`; `mise run build:wasm:debug`
//! builds it once with every family and `debug` into `web/pkg/all/`
//! (REQ-BUILD-040). The shell (`web/src`) is its only consumer
//! (REQ-ARCH-030), and `web/src/api.ts` states what it expects.
//!
//! It exports a [`Chart`] object with exactly the plan table's members and
//! the module functions [`charts_json`], [`wide_tail`] and
//! [`resolve_palette`], plus `start` in a debug build. Charts are reached
//! only through the core's registry (REQ-ARCH-020); the one chart defined
//! here is the debug build's `debug-panic` (REQ-WASM-060).
//!
//! Data crosses as typed arrays the shell writes into buffers this module
//! reserved, plus a descriptor without numeric values (REQ-DATA-050): the
//! buffers move into the chart's columns with no further copy
//! (REQ-MEM-020). The module holds no JSON data parser, because the core's
//! `text` feature is off here.
//!
//! A panic traps (`panic = "abort"`, REQ-WASM-040). Nothing here is static
//! but the debug hook's flag, and every chart owns its locale, zone, grid
//! and buffers, so a trap leaves no global lock held and nothing global
//! half-written: other charts keep drawing and new ones can be made. The
//! trapped chart stays borrowed in wasm-bindgen's glue, so every later call
//! on it throws; the shell refuses them first.
#![warn(missing_docs)]

use glyphplot::chart::{Chart as Plot, Env, Meta};
use glyphplot::data::{Buffer, Data, MISSING_CODE};
use glyphplot::events::{self, Event};
use glyphplot::grid::{Grid, WIDE_TAIL};
use glyphplot::input::Input;
use glyphplot::locale::{LocalePack, ZoneTable};
use glyphplot::options::Options;
use glyphplot::registry;
use glyphplot::theme::Theme;
use glyphplot::view::ViewPatch;
use wasm_bindgen::prelude::*;

#[cfg(feature = "debug")]
mod debug;

/// The grid a chart draws into until the shell sizes it.
const START: (usize, usize) = (80, 24);

/// The most cells a side the grid takes; larger sizes are clamped.
const MAX_SIDE: u32 = 4096;

/// The most columns `reserve_column` numbers: 256 a table, many tables.
const MAX_COLUMN_INDEX: usize = 1 << 16;

/// `kind` of `reserve_column` for a column of `f64` (`ColumnKind.F64`).
const KIND_F64: u32 = 0;

/// `kind` of `reserve_column` for a column of `u32` category codes
/// (`ColumnKind.U32`).
const KIND_U32: u32 = 1;

/// One chart and everything it draws with: its grid, the buffers reserved
/// for its next commit, its locale pack and zone table, and the events its
/// inputs produced (the plan's "The wasm `Chart`").
#[wasm_bindgen]
pub struct Chart {
    meta: &'static Meta,
    chart: Box<dyn Plot>,
    grid: Grid,
    reserved: Vec<Option<Buffer>>,
    locale: LocalePack,
    zone: ZoneTable,
    mono: bool,
    events: Vec<Event>,
}

/// A made chart and its type's meta, or why it could not be made.
type Made = Result<(&'static Meta, Box<dyn Plot>), glyphplot::chart::MakeError>;

/// A thrown JS `Error` carrying `e`'s message.
fn thrown(e: impl std::fmt::Display) -> JsError {
    JsError::new(&e.to_string())
}

/// The type's meta and a fresh chart: the debug charts first in a debug
/// build, then the core's registry (REQ-ARCH-020).
fn make(kind: &str, options_json: &str) -> Made {
    #[cfg(feature = "debug")]
    if let Some(made) = debug::make(kind, options_json) {
        return made;
    }
    registry::make(kind, options_json)
}

#[wasm_bindgen]
impl Chart {
    /// A chart of a type this module plays, `options_json` layered on its
    /// defaults. Throws `there is no chart named "<type>"` or `options are
    /// not a JSON object: …` (REQ-WASM-020); a throw leaves nothing behind.
    #[wasm_bindgen(constructor)]
    pub fn new(kind: &str, options_json: &str) -> Result<Chart, JsError> {
        #[cfg(feature = "debug")]
        debug::install();
        let (meta, chart) = make(kind, options_json).map_err(thrown)?;
        Ok(Chart {
            meta,
            chart,
            grid: Grid::new(START.0, START.1),
            reserved: Vec::new(),
            locale: LocalePack::default(),
            zone: ZoneTable::default(),
            mono: false,
            events: Vec::new(),
        })
    }

    /// New options, layered on the type's defaults; data and view kept
    /// (ADR-120). Throws `options are not a JSON object: …`, or an error
    /// naming the valid modes, and keeps the options it had.
    pub fn set_options(&mut self, json: &str) -> Result<(), JsError> {
        let options = Options::parse(json).map_err(thrown)?;
        self.chart.set_options(&options).map_err(thrown)
    }

    /// A buffer of `n` values for column `i`, as a byte address in this
    /// module's memory for the shell to fill through a view made after this
    /// call (REQ-DATA-060). `kind` 0 is `f64` (missing NaN), 1 is `u32`
    /// category codes (missing 0xFFFFFFFF); the buffer starts missing.
    /// Columns are numbered across the descriptor: every table's columns in
    /// order, then each matrix as one more. Reserving a column again
    /// replaces its buffer. Throws for another kind, a column number past
    /// 65,536, or more values than memory can hold.
    pub fn reserve_column(&mut self, i: usize, kind: u32, n: usize) -> Result<usize, JsError> {
        if i >= MAX_COLUMN_INDEX {
            return Err(JsError::new(&format!("column {i}: columns are numbered below {MAX_COLUMN_INDEX}")));
        }
        let too_many = |_| JsError::new(&format!("column {i}: {n} values are more than memory can hold"));
        // The old buffer goes first, so a buffer reserved again reuses its memory.
        self.slot(i).take();
        let buffer = match kind {
            KIND_F64 => {
                let mut v: Vec<f64> = Vec::new();
                v.try_reserve_exact(n).map_err(too_many)?;
                v.resize(n, f64::NAN);
                Buffer::F64(v)
            }
            KIND_U32 => {
                let mut v: Vec<u32> = Vec::new();
                v.try_reserve_exact(n).map_err(too_many)?;
                v.resize(n, MISSING_CODE);
                Buffer::U32(v)
            }
            _ => return Err(JsError::new(&format!("column {i}: kind {kind} is neither 0 (f64) nor 1 (u32)"))),
        };
        let ptr = match &buffer {
            Buffer::F64(v) => v.as_ptr() as usize,
            Buffer::U32(v) => v.as_ptr() as usize,
        };
        *self.slot(i) = Some(buffer);
        Ok(ptr)
    }

    /// Column `i`'s values from a `Float64Array`, copied once by the glue;
    /// otherwise as `reserve_column` with kind 0 and a fill.
    pub fn set_column(&mut self, i: usize, values: Vec<f64>) -> Result<(), JsError> {
        if i >= MAX_COLUMN_INDEX {
            return Err(JsError::new(&format!("column {i}: columns are numbered below {MAX_COLUMN_INDEX}")));
        }
        *self.slot(i) = Some(Buffer::F64(values));
        Ok(())
    }

    /// Applies the reserved columns as the new data, described by
    /// `descriptor_json` (data.schema.json's full form; numeric columns
    /// without `values`, a `string` column's labels in it). Returns the data
    /// version. The reserved buffers are used up either way; on a throw (the
    /// data error's message) the chart keeps the data it had (REQ-DATA-130).
    pub fn commit(&mut self, descriptor_json: &str) -> Result<f64, JsError> {
        let buffers = std::mem::take(&mut self.reserved);
        let data = Data::from_typed(descriptor_json, buffers).map_err(thrown)?;
        let accepted = self.chart.set_data(data).map_err(thrown)?;
        #[cfg(feature = "debug")]
        accepted.warnings.iter().for_each(|w| debug::warn(&format!("glyphplot: {} chart: {w}", self.meta.slug)));
        Ok(accepted.version as f64)
    }

    /// The locale pack labels are written with (locale.schema.json). Throws
    /// naming what is wrong and keeps the pack it had.
    pub fn set_locale(&mut self, json: &str) -> Result<(), JsError> {
        self.locale = LocalePack::from_json(json).map_err(|e| JsError::new(&format!("locale: {}", e.0)))?;
        Ok(())
    }

    /// The zone table time labels are drawn by (zone.schema.json). Throws
    /// naming what is wrong and keeps the table it had.
    pub fn set_zone(&mut self, json: &str) -> Result<(), JsError> {
        self.zone = ZoneTable::from_json(json).map_err(|e| JsError::new(&format!("zone: {}", e.0)))?;
        Ok(())
    }

    /// The whole view state (view.schema.json).
    pub fn view_json(&self) -> String {
        self.chart.view().to_json()
    }

    /// A whole or partial view merged key by key, a key set to null
    /// returning to its default (REQ-VIEW-030); fires no event. Throws
    /// `view: …` for JSON that does not fit view.schema.json.
    pub fn set_view(&mut self, json: &str) -> Result<(), JsError> {
        let patch = ViewPatch::from_json(json).map_err(thrown)?;
        self.chart.set_view(&patch);
        Ok(())
    }

    /// One input event in the numeric encoding of the plan's "Input
    /// encoding" (REQ-INPUT-020); returns the bitmask overlay changed 1,
    /// base changed 2, events pending 4. A kind or key code past the table
    /// changes nothing.
    #[allow(clippy::too_many_arguments)]
    pub fn input(&mut self, kind: u32, x: f64, y: f64, a: f64, b: f64, buttons: u32, mods: u32, t: f64) -> u32 {
        let Some(event) = Input::decode(kind, x, y, a, b, buttons, mods, t) else { return 0 };
        let changes = self.chart.input(&event);
        let bits = changes.bits();
        self.events.extend(changes.events);
        bits
    }

    /// The events the inputs since the last call produced, as `[{type,
    /// detail}]` (events.schema.json); `[]` when none. Reading them clears
    /// them.
    pub fn events_json(&mut self) -> String {
        let out = events::to_json(&self.events);
        self.events.clear();
        out
    }

    /// A new grid size in cells, each side 1 to 4,096; the view is kept in
    /// data units.
    pub fn resize(&mut self, cols: u32, rows: u32) {
        let side = |n: u32| n.clamp(1, MAX_SIDE) as usize;
        self.grid.resize(side(cols), side(rows));
    }

    /// One ink for the frames that follow (REQ-MONO-010).
    pub fn set_mono(&mut self, on: bool) {
        self.mono = on;
    }

    /// Draws the frame at play time `t` (seconds) into the grid; `paper` is
    /// dark ink on a light ground.
    pub fn frame(&mut self, t: f64, paper: bool) {
        let env = Env { paper, mono: self.mono, locale: &self.locale, zone: &self.zone };
        self.chart.frame(t, &env, &mut self.grid);
    }

    /// The grid's width in cells.
    pub fn cols(&self) -> u32 {
        self.grid.cols() as u32
    }

    /// The grid's height in cells.
    pub fn rows(&self) -> u32 {
        self.grid.rows() as u32
    }

    /// The address of `cols × rows` u32 code points, row by row;
    /// `wide_tail()` follows a double-width glyph. Valid until the next call
    /// that may allocate (REQ-WASM-030).
    pub fn cells_ptr(&self) -> usize {
        self.grid.glyphs().as_ptr() as usize
    }

    /// The address of `cols × rows` u8 ink palette indices.
    pub fn colors_ptr(&self) -> usize {
        self.grid.inks().as_ptr() as usize
    }

    /// The address of `cols × rows` u8 back palette indices; 255 is none.
    pub fn backs_ptr(&self) -> usize {
        self.grid.backs().as_ptr() as usize
    }

    /// The last frame as text: `rows` lines of `cols` cells joined by `\n`,
    /// the `wide_tail()` cells left out (REQ-GRID-060).
    pub fn text(&self) -> String {
        self.grid.text()
    }

    /// The drawing's accessible label: the chart, its series, its ranges
    /// and its count (REQ-A11Y-010, REQ-A11Y-030).
    pub fn summary(&self) -> String {
        self.chart.table(0).summary
    }

    /// The hidden table of the data in the current view, at most `max_rows`
    /// rows, as `{caption, columns, rows, total}` (REQ-A11Y-020).
    pub fn table_json(&self, max_rows: u32) -> String {
        self.chart.table(max_rows as usize).to_shell_json()
    }

    /// This chart type's meta (meta.schema.json).
    pub fn meta_json(&self) -> String {
        self.meta.to_json()
    }
}

impl Chart {
    /// The reservation of column `i`, made when it is not there yet.
    fn slot(&mut self, i: usize) -> &mut Option<Buffer> {
        if self.reserved.len() <= i {
            self.reserved.resize_with(i + 1, || None);
        }
        &mut self.reserved[i]
    }
}

/// Every type's meta this module plays, as a JSON array in registry order;
/// a debug build adds its debug charts (REQ-MANIFEST-010).
#[wasm_bindgen]
pub fn charts_json() -> String {
    let all = registry::charts_json();
    #[cfg(feature = "debug")]
    let all = debug::with_metas(all);
    all
}

/// `WIDE_TAIL`, the code point that follows a double-width glyph: 0
/// (REQ-GRID-010).
#[wasm_bindgen]
pub fn wide_tail() -> u32 {
    WIDE_TAIL
}

/// The 255 RGBA entries, 1,020 bytes, for a theme object (theme.schema.json),
/// as `theme::resolve` gives them (REQ-COLOR-030). Throws `theme
/// \`<field>\`: …`; a debug build logs each series colour under 3:1
/// against the ground (REQ-THEME-070).
#[wasm_bindgen]
pub fn resolve_palette(theme_json: &str) -> Result<Vec<u8>, JsError> {
    let theme = Theme::from_json(theme_json).map_err(thrown)?;
    #[cfg(feature = "debug")]
    glyphplot::theme::warnings(&theme).iter().for_each(|w| debug::warn(&format!("glyphplot: {w}")));
    Ok(glyphplot::theme::resolve(&theme).to_bytes())
}

/// Debug builds only: installs the panic hook that prints a panic's message
/// and location with `console.error` before it traps (REQ-WASM-060). Making
/// a chart installs it too.
#[cfg(feature = "debug")]
#[wasm_bindgen]
pub fn start() {
    debug::install();
}
