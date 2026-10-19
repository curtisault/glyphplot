# ADR-142: The main thread, not a worker

Status: Proposed (2026-10-18)

## Context

A chart could run its wasm on the page's main thread, as unicel's pieces do,
or in a Web Worker drawing to an `OffscreenCanvas` with buffers transferred to
it. `discovery/surfaces.md` (open question 5) recommended the main thread and
a revisit if million-point frames miss their budget there; it did not measure
a worker.

Measured in `discovery/budgets.md` (headless Chromium 156, 300 panning frames
after a warm-up, 5 runs, min / median of run averages and worsts; 1M points
with M4):

| Cells | Average | Worst |
|---|---|---|
| 80 × 24 | 3.24 / 3.26 ms | 3.69 / 3.73 ms |
| 320 × 120 | 3.70 / 3.71 ms | 4.13 / 4.20 ms |

- WebKit (Playwright's build, not Safari) averaged 3.83–4.29 ms at 1M points.
- Streaming 1 kHz into a full 1M ring with a 60 Hz redraw cost 236 ms of CPU a
  second in Chromium, all of it the redraws.
- A pointer move cost 2–5 µs on average in Chromium.
- The canvas draw comes on top: a full redraw of 320 × 120 braille took
  16–21 ms at best in headless Chrome (`discovery/rendering.md` §2.3), and a
  pointer move redraws only the cells it changes.

A worker would also need input, events and the accessible table (which lives
in the DOM) to cross by messages; its cost is **(unverified)**.

## Decision

- Charts run on the main thread: the module is instantiated in the page, and
  frames are drawn and painted in animation-frame callbacks while the chart
  is in view (REQ-MOUNT-010, REQ-MOUNT-020).
- The browser budgets hold it: 8 ms average and 16 ms worst a frame in
  Chromium (REQ-PERF-060), and 500 ms of CPU a second for a 1 kHz stream
  (REQ-PERF-070).
- The shell has no worker or `OffscreenCanvas` path in v0.3.0.

## Consequences

- Events fire synchronously from the element, and the API stays one object.
- A chart's frames share the main thread with the page's own work.
- A full-screen heatmap redrawn whole drops to about 30 frames a second.
- Revisit with a worker if REQ-PERF-060 or REQ-PERF-070 fail in signoff or on
  the owner's devices.
