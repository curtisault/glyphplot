# ADR-120: An options change keeps the chart

Status: Proposed (2026-10-18)

## Context

unicel has no `set_options`: any attribute change other than `time` frees the
player and makes a new one, restarting the piece (unicel's ADR-025). That
suits a piece, which holds nothing the page gave it. A glyphplot chart holds
the page's data, up to a million rows. `discovery/surfaces.md` (decision 2)
weighed:

- **(a) Re-create the chart** on an options change and re-send its data from a
  JS copy, which doubles the page's memory for the data.
- **(b) `set_options` on the live chart**, keeping data and view.

The costs of (a), measured elsewhere:

- Re-sending two 1M-row `Float64Array` columns took 4.27 ms and grew wasm
  memory by 15.4 MiB (`discovery/data.md`, wasm in Node, median).
- wasm memory never shrinks, and a copying load of 1M points peaked at 516
  pages (33.8 MB) against 270 pages (17.7 MB) without the extra copy
  (`discovery/budgets.md` Memory, Node, `memory.buffer.byteLength`).
- Streamed data held only in the ring would be lost unless the page kept its
  own copy.

## Decision

- `set_options` changes a live chart's options and keeps its data and view;
  only a change of chart type makes a new chart (REQ-OPT-020).
- On the element, a change of `options` or `theme` makes no new chart: data
  and view after it equal those before (REQ-EL-040).
- A change of `type` makes a new chart and carries data and view across when
  the new type accepts the data's shape (REQ-EL-050).
- This reverses unicel's ADR-025 for glyphplot.

## Consequences

- An option edit on a dashboard costs a frame, not a reload of the data.
- Every chart must take new options on live state: caches keyed by options
  are cleared, and by ADR-009 the next frame equals that of a fresh chart
  given the same data, options, view and `t`.
- The wasm API has one more method than unicel's.
