//! The chart families, one folder and one Cargo feature each
//! (REQ-ARCH-040, REQ-CHART-010). A chart uses the core and its own family's
//! folder, never another family's: its wasm module would not contain the
//! other.

use crate::registry::Family;

#[cfg(feature = "line")]
pub mod line;
#[cfg(feature = "contract-selftest")]
pub mod selftest;

/// Every family compiled in, in registry order, each behind its feature
/// (REQ-REG-010).
pub const FAMILIES: &[Family] = &[
    #[cfg(feature = "line")]
    line::FAMILY,
    // The contract's self-test: broken charts it must reject
    // (REQ-CONTRACT-060), never in a default or wasm build.
    #[cfg(feature = "contract-selftest")]
    selftest::FAMILY,
];
