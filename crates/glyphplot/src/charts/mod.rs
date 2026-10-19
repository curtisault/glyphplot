//! The chart families, one folder and one Cargo feature each
//! (REQ-ARCH-040). A chart uses the core and its own family's folder, never
//! another family's: its wasm module would not contain the other.

#[cfg(feature = "line")]
pub mod line;
