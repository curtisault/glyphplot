# ADR-111: Ticks fit by lowering the count

Status: Proposed (2026-10-18)

## Context

Tick labels in a grid are whole cells wide, and two must never share a cell.
`discovery/scales.md` (decisions 4 and 5) names the options:

- **Prevent overlap**: ask for fewer ticks until whole labels fit with a gap,
  Vega's minimum-step idea decided in cells.
- **Remove overlap**: make the ticks, then hide labels that collide, as Vega's
  `parity` and `greedy` do (`vega-view-transforms/src/Overlap.js`, code read).
  It can leave uneven gaps. `discovery/rendering.md` §4.4 proposed a one-cell
  gap with overlaps dropped left to right.
- **Place marks at the nearest cell**, or **widen the domain** so ticks land on
  cells exactly.

Measured in scales.md:

- Property tests, 3 × 200,000 cases (linear axes of 4 to 304 cells, UTC axes
  over 10 ms to 32 years): 0 failures; every label inside the axis and apart.
  14 linear and 1,021 time axes had no label, all too narrow for one.
- `fit_x` took 2.6 µs for an 80-cell linear axis, 4.0 µs for a time axis and
  14.1 µs at 320 cells (native, minimum of 5 runs of 20,000 calls).
- Sample axes: a one-cell gap reads as crowded, two reads well (by eye).
- A mark at the nearest cell is off by at most half a cell, 0.6 % of an
  80-cell axis (arithmetic).

## Decision

- The tick count starts at `floor(len / 4)` on x and `floor(rows / 2)` on y
  and falls until labels fit with the gap: two cells on x, one row on y
  (REQ-TICK-030). There is no pass that hides labels; a tick whose label does
  not fit keeps its mark.
- A mark goes in the nearest cell and a gridline at the nearest braille dot
  (REQ-TICK-040).
- `snap: "rows"` puts every y tick on a row with a nice step per row, the
  default for bar and candle y axes under 20 rows (REQ-TICK-050).
- Layout is in whole cells, outside in: title, legend, x axis, panes, y
  gutters, x ticks, annotations, a pure function of size, options, extent and
  view (REQ-LAYOUT-010), with the drop sizes of REQ-LAYOUT-020 and 030.

## Consequences

- Every shown label sits on a nice step; axes show fewer labels than fit.
- Tick spacing alternates by a cell (7, 6, 7 on scales.md's six-hour axis).
- A very narrow axis may show no label at all.
- Layout is integer arithmetic, identical on every surface, and cacheable by
  domain and width.
