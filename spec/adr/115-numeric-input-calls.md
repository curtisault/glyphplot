# ADR-115: Numeric input calls

Status: Proposed (2026-10-18)

## Context

Every pointer move crosses from the shell into the reducer (ADR-113).
`discovery/interaction.md` weighed three encodings:

- **(a) A numeric call**, `input(kind, x, y, a, b, buttons, mods, t)`,
  returning a `u32` bitmask.
- **(b) A JSON string an event**, parsed by serde_json in wasm.
- **(c) A shared ring of events** that the shell writes into wasm memory.

Measured in interaction.md (1M points, wasm in Node, minimum and median):

- A crossing alone: 0.027 / 0.049 µs.
- A hover event at 80 × 24 through (a): 0.73 / 0.79 µs; through (b):
  1.16 / 1.22 µs. JSON adds 0.4–0.5 µs an event.
- (c) saves nothing measurable at one call a frame.

Chrome has aligned `pointermove` and `wheel` to animation frames since
Chrome 60, and `getCoalescedEvents()` is not Baseline (Chrome's "Aligned input
events" and MDN, read 2026-10-18). A crosshair and a pan need only the latest
position.

## Decision

- Input events have the shape of `input.schema.json`, positions in cells with
  fractions inside a cell. The wasm boundary takes them as a numeric call
  returning a bitmask: overlay changed, base changed, events pending
  (REQ-INPUT-020).
- The JSON form is canonical and documented: tests, the contract's scripts,
  the CLI's scripted input and recorded sessions use it.
- The shell sends at most one `move` per animation frame, the latest, and
  sends `down`, `up`, `leave`, `tap`, `key` and summed `wheel` deltas in order
  before it (REQ-INPUT-030).
- The shell reads events out, as one JSON string, only when the bitmask says
  events are pending.

## Consequences

- A hover that stays on one datum crosses once and dispatches nothing.
- Two encodings of one event must agree; a test feeds both and compares the
  views.
- The meaning of `a` and `b` changes with `kind` and must be documented with
  the schema.
- Positions between frames are dropped; a future freehand gesture would need
  coalesced points.
