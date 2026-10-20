# ADR-108: M4 per dot column

Status: Proposed (2026-10-18)

## Context

A million points must draw within budget. The options for line-like charts:
draw every segment; reduce each dot column to its minimum and maximum; M4
(first, last, minimum and maximum a column; Jugel et al., PVLDB 2014); LTTB
(one point a bucket); and, on top of any of them, a min/max pyramid built per
data change and read in O(columns × log n).

- `discovery/charts.md` (native release, 1M points of uniform noise at
  320 × 120, 3 × 5 runs): every segment 362.8 ms min, 387.9 median; ratatui's
  `Chart` 325.2 / 347.1 ms; min/max a column 3.54 / 4.18 ms. Lit-cell counts
  were equal between per-segment and per-column frames in all 16 cases.
- `discovery/data.md` (wasm in Node, 1M points in view, 640 dot columns,
  median): M4 2.78 ms, min-max 2.87 ms, LTTB 2.31 ms. LTTB missed a column's
  true minimum or maximum in 607 of 640 columns; M4 and min-max in none. A
  zoomed view of 10k rows took 0.077 ms.
- `discovery/budgets.md` (5 × 300 panning frames): M4 cut the 1M frame at
  320 × 120 from 5.03 to 2.40 ms natively and from 5.51 to 3.70 ms in
  Chromium; at 80 × 24 it cost more in Chromium (2.79 to 3.26 ms median).
  A pyramid was not built; per-frame M4 at 10M points would take 20–50 ms
  **(unverified: extrapolated linearly)**.

## Decision

- Line-like charts (line, step, area, band, sparkline) draw from M4
  aggregates of the rows in view whenever they outnumber four times the
  plot's dot columns, and for x-sorted data the frame equals, cell for cell,
  the frame drawn from every segment (REQ-SAMPLE-010).
- Rows in view are found by binary search on x; aggregates are cached by data
  version, view x range and width, and a frame with all three unchanged reads
  no data (REQ-SAMPLE-020).
- M4 over 1M rows at 640 dot columns takes at most 8 ms in wasm in Node
  (REQ-SAMPLE-030). Downsampling is on by default (REQ-PERF-010).
- LTTB is opt-in as `sample: "lttb"`, never a default (REQ-SAMPLE-060).
- No pyramid until 10M points is a goal.

## Consequences

- Frames are exact and keep every spike; a hover reuses the cache.
- A pan re-scans the rows in view: its cost grows with them, linearly.
- Revisit with a pyramid when a domain asks for 10M points.
