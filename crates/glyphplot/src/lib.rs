//! glyphplot: interactive charts drawn in Unicode glyphs, one code point and
//! one colour a cell.
//!
//! The core holds no DOM, canvas or terminal code (REQ-ARCH-010): the wasm
//! crate and the CLI draw what it computes. It holds the grid and its
//! canvases, the palette and themes, data, scales, ticks, formats, layout and
//! text (Phase 1), and the chart framework (Phase 2): the chart contract
//! ([`chart`]), options, the view state, input and its reducer, events, hit
//! tests, the overlay, M4 downsampling, the accessible table, the registry
//! and the chart families ([`charts`]).
#![warn(missing_docs)]

pub mod annotate;
pub mod budget;
pub mod canvas;
pub mod cells;
pub mod chart;
pub mod charts;
pub mod data;
pub mod downsample;
pub mod events;
pub mod fixtures;
pub mod format;
pub mod grid;
pub mod hit;
pub mod input;
pub mod layout;
pub mod locale;
pub mod mono;
pub mod options;
pub mod overlay;
pub mod palette;
pub mod reduce;
pub mod registry;
pub mod scale;
pub mod table;
pub mod text;
pub mod theme;
pub mod ticks;
pub mod time;
pub mod view;

pub use budget::Budget;
pub use chart::{Chart, Env, Meta};
pub use registry::{Family, FAMILIES};
