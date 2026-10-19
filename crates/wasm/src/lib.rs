//! glyphplot's wasm-bindgen boundary. `mise run build:wasm` builds this crate
//! once a chart family, with that family's feature alone, into
//! `web/pkg/<family>/glyphplot{.js,_bg.wasm}`. It exports what names a
//! module; the `Chart` object arrives in Phase 3.
#![warn(missing_docs)]

use wasm_bindgen::prelude::*;

/// glyphplot's version.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// The chart families this module holds: one in a release module.
#[wasm_bindgen]
pub fn families() -> Vec<String> {
    glyphplot::FAMILIES.iter().map(|f| f.name.to_string()).collect()
}
