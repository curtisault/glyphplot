//! The `line` family: line, step and area charts, and the sparkline. It holds
//! no chart yet; Phase 2 draws the first.

use crate::{Budget, Family};

/// The family's registry entry.
pub const FAMILY: Family = Family { name: "line", budget: Budget::DEFAULT };
