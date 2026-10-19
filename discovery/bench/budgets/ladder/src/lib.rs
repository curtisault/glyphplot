//! Throwaway bench for discovery/budgets.md: one crate whose Cargo features
//! are the steps of the size ladder (scripts/ladder.sh builds each).

// f64::clamp is avoided throughout: its assert formats both bounds with Debug,
// which links core's float formatting (measured in discovery/budgets.md).

use wasm_bindgen::prelude::*;

#[cfg(feature = "candle")]
pub mod candle;
#[cfg(feature = "line")]
pub mod grid;
#[cfg(feature = "line")]
pub mod line;
#[cfg(feature = "downsample")]
pub mod m4;
#[cfg(feature = "serde")]
pub mod opts;
#[cfg(feature = "scale")]
pub mod scale;
#[cfg(feature = "line")]
pub mod series;
#[cfg(feature = "time")]
pub mod time;

/// Step 0's only export: a string across the boundary, so the module carries
/// std's allocator and wasm-bindgen's string passing.
#[wasm_bindgen]
pub fn version() -> String {
    String::from("0.0.0")
}
