# ADR-117: Hit testing by chart kind

Status: Proposed (2026-10-18)

## Context

A pointer must resolve to a datum within the input budget (0.5 ms, ADR-114).
The options, from `discovery/interaction.md` and `discovery/charts.md`:

- **Binary search on sorted x** for x-ordered charts (uPlot's `closestIdx`).
  Measured over 1M points: 0.033 / 0.040 µs natively and 0.046 / 0.054 µs
  inside wasm, minimum and median (interaction.md).
- **Arithmetic by band or bin** for bars, histograms and heatmaps: O(1).
- For scatter: a **screen-space grid rebuilt per view** (build 3.8–4.3 ms at
  80 × 24 and 5.7–8.8 ms at 320 × 120 natively, query 0.53–15.9 µs); a
  **linear scan** (1,182–1,650 µs natively); a **data-space index built once a
  data change** (not measured); or charts.md's **pick buffer**, a `u32` datum
  id a cell written while drawing, 153,600 bytes at 320 × 120 (arithmetic).

The mapping between data and cells must be one function.
`discovery/testing.md` M5 found `x * cols / n` and `x * (cols / n)`, equal in
algebra, disagree for 1,137 of 12,006,000 points (n from 1 to 2,000, cols in
{40, 80, 160, 316, 320, 640}; first at x = 75, n = 150, cols = 316: column
158 against 157).

## Decision

- For x-ordered charts the cursor snaps to the datum nearest the pointer's
  dot column by binary search on x; for binned charts to the bin under the
  pointer, by band or bin arithmetic in O(1); tests compare both with a
  brute-force answer (REQ-HIT-010, REQ-BAR-020).
- Scatter resolves the pointer through a data-space index built once a data
  change, and by scan until it exists; once built, a hover over 1M points
  takes under 0.1 ms median natively (REQ-HIT-030, REQ-SCATTER-010). The pick
  buffer is not adopted.
- The core has one module of cell mappings that frames, hit tests, readouts,
  reducers and the contract all call; a property test checks it
  (REQ-CANVAS-020).

## Consequences

- A pan does not rebuild the scatter index, as a screen-space grid would.
- The data-space index is unmeasured; measure it when scatter is built and
  fall back to the screen-space grid if it loses.
- The contract's "a hit names what was drawn" rule catches a chart that maps
  cells its own way.
- Long-tail layouts (graphs, Sankey) may revisit the pick buffer.
