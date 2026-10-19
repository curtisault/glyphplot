//! glyphplot: interactive charts drawn in Unicode glyphs, one code point and
//! one colour a cell.
//!
//! The core holds no DOM, canvas or terminal code (REQ-ARCH-010): the wasm
//! crate and the CLI draw what it computes. It holds the chart families and
//! what each may cost; the grid, the canvases and the charts arrive from
//! Phase 1 of `docs/IMPLEMENTATION_PLAN.md`.
#![warn(missing_docs)]

pub mod budget;
pub mod charts;
pub mod registry;

pub use budget::Budget;
pub use registry::{Family, FAMILIES};
