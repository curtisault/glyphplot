//! glyphplot: interactive charts drawn in Unicode glyphs, one code point and
//! one colour a cell.
//!
//! The core holds no DOM, canvas or terminal code (REQ-ARCH-010): the wasm
//! crate and the CLI draw what it computes: the grid and its canvases, the
//! palette and themes, data, scales, ticks, formats, layout and text, and the
//! chart families. The charts arrive from Phase 2 of
//! `docs/IMPLEMENTATION_PLAN.md`.
#![warn(missing_docs)]

pub mod budget;
pub mod canvas;
pub mod cells;
pub mod charts;
pub mod data;
pub mod fixtures;
pub mod format;
pub mod grid;
pub mod layout;
pub mod palette;
pub mod registry;
pub mod scale;
pub mod text;
pub mod theme;
pub mod ticks;
pub mod time;

pub use budget::Budget;
pub use registry::{Family, FAMILIES};
