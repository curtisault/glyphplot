# ADR-009: A frame is a pure function of data, options, view and t

Status: Accepted (2026-10-18)

## Context

unicel's pieces are deterministic in `t` (unicel's ADR-010, REQ-PIECE-040), so
snapshots, seeks, exports and the contract test can ask for any frame at any
time. A chart adds inputs: data that streams, a view the pointer moves.

## Decision

A chart's frame is a pure function of its data, its options, its view state
and `t`. Streaming data appends to the data; it does not break this: the same
data reached by appends or by one replace gives the same frame.

## Consequences

- Snapshots, exports and tests can draw any frame from those four inputs.
- Input (pointer, wheel, keys) changes the view state; how and where input is
  folded into view state is a decision of its own ([ADR-113](113-view-state-is-the-model.md)), but the frame
  depends only on the result.
- A chart may cache (downsampled columns, laid-out axes) but must give the same
  frame whatever order it is asked in.
