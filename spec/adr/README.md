# Decision records

One file a decision: `NNN-kebab-title.md`, with `# ADR-NNN: Title`, a
`Status` line, `## Context`, `## Decision` and `## Consequences`.

Numbering: 001–099 are the owner's decisions, Accepted. 100 and up are
discovery's, Proposed until the owner accepts them; on 2026-10-18 the owner
accepted those behind the plan's open questions (101, 102, 103, 112, 116,
118, 119, 121, 123, 126, 133, 136, 140, 141, 143) and rejected 100. A decision that replaces
another marks the older one `Superseded by ADR-NNN`; a number is never reused.

| ADR | Title | Status | Date |
|---|---|---|---|
| [001](001-name-glyphplot.md) | The name is glyphplot | Accepted | 2026-10-18 |
| [002](002-its-own-repository.md) | glyphplot is its own repository | Accepted | 2026-10-18 |
| [003](003-unicels-architecture.md) | unicel's architecture, whole | Accepted | 2026-10-18 |
| [004](004-unicels-frame-buffer.md) | unicel's frame buffer | Accepted | 2026-10-18 |
| [005](005-one-wasm-module-per-chart-type.md) | One wasm module per chart type | Accepted | 2026-10-18 |
| [006](006-data-is-injected.md) | Data is injected | Accepted | 2026-10-18 |
| [007](007-interactive-from-the-first-chart.md) | Interactive from the first chart | Accepted | 2026-10-18 |
| [008](008-a-generous-size-threshold.md) | A size threshold per module, set generously | Accepted | 2026-10-18 |
| [009](009-deterministic-frames.md) | A frame is a pure function of data, options, view and t | Accepted | 2026-10-18 |
| [010](010-unicels-spec-format.md) | unicel's spec format | Accepted | 2026-10-18 |
| [011](011-mit-licence.md) | MIT licence | Accepted | 2026-10-18 |
| [012](012-no-ai-attribution.md) | No AI attribution | Accepted | 2026-10-18 |
| [013](013-independent-of-unicel.md) | Independent of unicel | Accepted | 2026-10-18 |
| [100](100-vendor-unicels-core.md) | Vendor unicel's core | Rejected by ADR-013 | 2026-10-18 |
| [101](101-chart-families-with-modes.md) | Chart families with modes | Accepted | 2026-10-18 |
| [102](102-semantic-palette-of-255.md) | A semantic palette of 255 | Accepted | 2026-10-18 |
| [103](103-cell-from-font-grid-from-box.md) | The cell from the font, the grid from the box | Accepted | 2026-10-18 |
| [104](104-typed-columns-cross-the-boundary.md) | Typed columns cross the boundary | Proposed | 2026-10-18 |
| [105](105-hand-written-arrow-reader.md) | A hand-written Arrow reader | Proposed | 2026-10-18 |
| [106](106-data-applies-at-once.md) | Data applies at once | Proposed | 2026-10-18 |
| [107](107-a-ring-allocated-once.md) | A ring allocated once | Proposed | 2026-10-18 |
| [108](108-m4-per-dot-column.md) | M4 per dot column | Proposed | 2026-10-18 |
| [109](109-candles-resample-to-calendar-intervals.md) | Candles resample to calendar intervals | Proposed | 2026-10-18 |
| [110](110-scales-and-formats-ported-from-d3.md) | Scales and formats ported from d3 | Proposed | 2026-10-18 |
| [111](111-ticks-fit-by-lowering-the-count.md) | Ticks fit by lowering the count | Proposed | 2026-10-18 |
| [112](112-zones-and-locales-from-the-shell.md) | Zones and locales from the shell | Accepted | 2026-10-18 |
| [113](113-view-state-is-the-model.md) | The view state is the model | Proposed | 2026-10-18 |
| [114](114-base-and-overlay.md) | A base and an overlay | Proposed | 2026-10-18 |
| [115](115-numeric-input-calls.md) | Numeric input calls | Proposed | 2026-10-18 |
| [116](116-input-defaults.md) | Input defaults | Accepted | 2026-10-18 |
| [117](117-hit-testing-by-chart-kind.md) | Hit testing by chart kind | Proposed | 2026-10-18 |
| [118](118-one-tag-with-a-type.md) | One tag with a type | Accepted | 2026-10-18 |
| [119](119-no-glyph-and-colour-atlas.md) | No glyph-and-colour atlas | Accepted | 2026-10-18 |
| [120](120-options-change-keeps-the-chart.md) | An options change keeps the chart | Proposed | 2026-10-18 |
| [121](121-extend-the-bundled-font.md) | Extend the bundled font | Accepted | 2026-10-18 |
| [122](122-theme-tokens-and-custom-properties.md) | Theme tokens and custom properties | Proposed | 2026-10-18 |
| [123](123-colour-defaults-held-to-measured-bars.md) | Colour defaults held to measured bars | Accepted | 2026-10-18 |
| [124](124-abort-on-panic.md) | Abort on panic | Proposed | 2026-10-18 |
| [125](125-hand-written-registry.md) | A hand-written registry | Proposed | 2026-10-18 |
| [126](126-correctness-and-timing-apart.md) | Correctness and timing apart | Accepted | 2026-10-18 |
| [127](127-test-profile-without-lto.md) | A test profile without LTO | Proposed | 2026-10-18 |
| [128](128-fixtures-generated-from-seeds.md) | Fixtures generated from seeds | Proposed | 2026-10-18 |
| [129](129-snapshot-colours-in-hex.md) | Snapshot colours in hex | Proposed | 2026-10-18 |
| [130](130-proptest.md) | proptest | Proposed | 2026-10-18 |
| [131](131-index-time-scale-for-session-gaps.md) | An index time scale for session gaps | Proposed | 2026-10-18 |
| [132](132-figure-slider-and-hidden-table.md) | A figure, a slider and a hidden table | Proposed | 2026-10-18 |
| [133](133-a-350-kb-module-gate.md) | A 350 KB module gate | Accepted | 2026-10-18 |
| [134](134-cli-on-crossterm.md) | The CLI on crossterm | Proposed | 2026-10-18 |
| [135](135-exports-in-rust.md) | Exports in Rust | Proposed | 2026-10-18 |
| [136](136-the-sites-market-demo.md) | The site's market demo | Accepted | 2026-10-18 |
| [137](137-dataset-manifest-and-allow-list.md) | A dataset manifest and an allow-list | Proposed | 2026-10-18 |
| [138](138-domain-packs-are-crates-and-modules.md) | Domain packs are crates and modules | Proposed | 2026-10-18 |
| [139](139-indicators-in-the-markets-pack.md) | Indicators in the markets pack | Proposed | 2026-10-18 |
| [140](140-celestial-positions-in-a-pack.md) | Celestial positions in a pack | Accepted | 2026-10-18 |
| [141](141-serde-json-in-chart-modules.md) | serde_json in chart modules | Accepted | 2026-10-18 |
| [142](142-main-thread-not-a-worker.md) | The main thread, not a worker | Proposed | 2026-10-18 |
| [143](143-site-docs-with-live-charts.md) | The site is documentation with live charts | Accepted | 2026-10-18 |
