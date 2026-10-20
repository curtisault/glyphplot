//! What a chart, and so its wasm module, may cost
//! (`spec/schemas/budgets.schema.json`). `mise run build:wasm` writes each
//! module's budget to `web/pkg/budgets.json`, where `mise run size`, the
//! contract and the benches read it.

use serde::Serialize;

/// What a chart, and so its module, may cost. Serialised as
/// budgets.schema.json's `$defs/budget`: `gzip`, `frameMs`, `worstMs`,
/// `inputMs` (REQ-CHART-020, REQ-MANIFEST-010).
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Budget {
    /// The module's `glyphplot_bg.wasm`, gzip -9, in bytes (REQ-SIZE-010).
    pub gzip: u32,
    /// The average frame in a native release build, in ms (REQ-PERF-030).
    pub frame_ms: f64,
    /// The slowest frame over the same play, in ms (REQ-PERF-030).
    pub worst_ms: f64,
    /// The average pointer move at 1,000,000 points, in ms (REQ-PERF-050).
    pub input_ms: f64,
}

impl Budget {
    /// Every family's budget unless it says otherwise: 350 KB gzipped
    /// (ADR-133), 4 ms a frame on average, 16 ms the worst, 0.5 ms a pointer
    /// move.
    pub const DEFAULT: Budget = Budget { gzip: 358_400, frame_ms: 4.0, worst_ms: 16.0, input_ms: 0.5 };

    /// The size past which `mise run size` warns: half the gzip budget
    /// (REQ-SIZE-040).
    pub const fn gzip_warning(&self) -> u32 {
        self.gzip / 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_350_kb_with_a_warning_at_half() {
        assert_eq!(Budget::DEFAULT.gzip, 350 * 1024);
        assert_eq!(Budget::DEFAULT.gzip_warning(), 179_200);
    }

    #[test]
    fn serialises_as_the_schema_names_it() {
        let json = serde_json::to_string(&Budget::DEFAULT).unwrap();
        assert_eq!(json, r#"{"gzip":358400,"frameMs":4.0,"worstMs":16.0,"inputMs":0.5}"#);
    }
}
