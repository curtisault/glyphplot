# ADR-114: A base and an overlay

Status: Proposed (2026-10-18)

## Context

The crosshair, readout, brush and focus change at pointer rate; the data, axes
and legend do not. `discovery/interaction.md` weighed:

- **(a) An overlay composed in wasm into the grid**, over a cached base.
- **(b) An overlay drawn by the shell** as DOM or a second canvas over the
  chart. It allows pixel-smooth lines, but breaks the one-glyph-a-cell look
  and needs a second implementation for the `<pre>`, the terminal and the
  exports.
- **(c) No layers**: redraw the whole frame from the data on every move.

Measured in interaction.md (1M sorted points, native release and wasm in Node,
minimum of 15 and median of 3 run medians, µs an event):

| Event | Size | Native | Wasm, from JS |
|---|---|---|---|
| Hover over the cached base | 80 × 24 | 0.55 / 0.61 | 0.73 / 0.79 |
| Hover over the cached base | 320 × 120 | 3.8 / 4.9 | 4.1 / 5.0 |
| Hover that redraws the frame | 80 × 24 | 1,689 / 1,745 | 1,480 / 1,660 |
| Hover that redraws the frame | 320 × 120 | 1,989 / 2,310 | 2,124 / 2,267 |

A hover changed 8.2 of 1,920 cells at 80 × 24 and 202 of 38,400 at 320 × 120.
`discovery/budgets.md` (Pointer move) found 1 to 8 µs a move at 1M points
over 5 × 300 moves.

## Decision

- A frame is a cached base (data, axes, legend) and an overlay (crosshair,
  readout, brush, time cursor, focus) composed in wasm into the grid; a hover
  that does not change the view never redraws the base, and costs under
  0.1 ms median natively at 320 × 120 with 1M points (REQ-OVERLAY-010).
- The readout stays at a fixed place and hides on Escape (REQ-OVERLAY-020).
- The painter redraws only cells whose glyph, ink or back changed
  (REQ-RENDER-030).
- A pointer move averages at most 0.5 ms, none over 2 ms (REQ-PERF-050).

## Consequences

- Hover costs at most 0.03 % of a 16.7 ms frame; the `<pre>`, the canvas, the
  terminal and the exports show the same crosshair.
- The base copy costs memory (5 bytes a cell), and at 320 × 120 copying it
  dominates a hover; restoring only the cells the last overlay touched is
  **(unverified)**.
- Pan and zoom still redraw the base: their cost is the frame budget's.
- The crosshair moves by dots and cells, never by pixels.
