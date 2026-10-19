# ADR-005: One wasm module per chart type

Status: Accepted (2026-10-18)

## Context

unicel splits its wasm by category (unicel's ADR-008), because wasm-bindgen
cannot split one module and a single module grew past 249 KB gzipped. A chart
library could ship one module with every chart, a module per family of charts,
or a module per chart.

## Decision

Each type of chart (candlestick, line, bar, histogram, scatter, heatmap,
sparkline, ...) is its own Cargo feature and its own wasm module, so a page pays
only for the charts it shows. Shared code lives in the core. A chart that is
another chart with a mode (an area as a filled line, say) may share its module
with it; discovery proposes those groupings (`discovery/charts.md`, [ADR-101](101-chart-families-with-modes.md)).

## Consequences

- Every module repeats the base (std, alloc, the boundary, the core it uses);
  the base is what the size threshold (ADR-008) mostly pays for.
- A page showing five chart types loads five modules.
- A chart may use the core and its own folder, never another chart's: the module
  built for it alone would not contain the other.
- The build runs wasm-pack once per chart type.
