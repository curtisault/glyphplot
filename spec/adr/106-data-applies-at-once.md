# ADR-106: Data applies at once

Status: Proposed (2026-10-18)

## Context

unicel eases a data-driven piece from its old values to its new ones over up
to a second, as a function of `t`, through its input log (unicel's
REQ-WASM-503 and ADR-114). A chart could do the same, or apply data the moment
it arrives. `discovery/data.md` (decision 6) names the options:

- **(a) unicel's model.** Data changes enter a log and are eased in `t`.
- **(b) Data applies at once.** The frame is a pure function of the data's
  current version.

What decides it:

- A chart is read for values. A tween draws values that never existed in the
  data (data.md).
- To seek, (a) replays the log. A feed at 1 kHz is 1,000 data events a second
  (`discovery/budgets.md` Streaming, the rate the 1M-ring benchmark ran at),
  and unicel's log keeps at most 512 events (unicel ADR-114), so at that rate
  it holds about half a second: arithmetic, not a measurement.
- Under (b) the appends cost nothing to time: one sample took 0.062 µs and a
  batch of 1,000 took 3.78 µs in wasm under Node (data.md, median).

`discovery/charts.md` lists unicel's bar-chart easing between datasets among
what carries over, as a transition on replace keyed to `t`. This decision does
not take it.

## Decision

- Data changes take effect at once; a frame is a function of the data as it
  is, never of an easing between versions (REQ-DATA-130).
- After `data` is replaced, a chart's frame equals the frame of a new chart
  given the same data, options, view and `t` (REQ-STREAM-060).
- There is no log of data events. Animated changes of the view are `motion`
  records in the view state (ADR-113), not of the data.

## Consequences

- Streaming at any rate keeps frames reproducible from four inputs (ADR-009).
- Switching a bar chart between datasets jumps; no bar grows into place.
- Snapshots, exports and the contract need no data history.
- Revisit if the owner wants data transitions: they would come as a view
  option that eases the drawing, with the data itself still current.
