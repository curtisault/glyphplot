# ADR-124: Abort on panic

Status: Proposed (2026-10-18)

## Context

ADR-003 takes unicel's architecture but not the decisions made inside it;
each is re-decided here. unicel's ADR-011 keeps `panic = "abort"` and makes the
contract test the guard. Measured in unicel's Phase 2 (its ADR-011): with
abort, a panicking piece traps, the instance survives, and only the panicking
player is left borrowed and unusable.

The options:

- **(a) `panic = "abort"`**, as unicel. A panic traps the call that made it.
- **(b) Unwinding with a catch per call**, so a chart could recover from a
  panic. It needs unwinding support in the wasm build and costs size; its
  size was not measured for glyphplot **(unverified)**.

Every module measured in `discovery/budgets.md` was built with unicel's
release profile, `panic = "abort"` included (ladder, wasm-pack release); the
module sizes that ADR-133 rests on assume it. A glyphplot chart differs from a
piece in one way that bears on this: it holds the page's data, which a
panicked chart takes with it.

## Decision

- The release profile keeps `panic = "abort"` (REQ-BUILD-050).
- A panicking call traps without taking down the instance: other charts keep
  drawing and new ones can be made; the panicked chart refuses every later
  call (REQ-WASM-040).
- The contract test is the guard: every chart draws without panicking at every
  size from 1 × 1 to 320 × 120 with every fixture it accepts (REQ-LAYOUT-040),
  and draws with the grid's clipping methods.
- With the `debug` feature only, a panic prints its message and location with
  `console.error` before it traps, and a `debug-panic` chart exists; a release
  build has neither (REQ-WASM-060).
- The element frees a stuck chart and fires `plot-error`.

## Consequences

- One broken chart stops its own tag, not the page or the module.
- A panicked chart leaks what it held, and wasm memory never shrinks; for a
  1M-point chart that is about 16 MB until the page reloads.
- Data the page streamed into a panicked chart is gone; the page re-makes it
  and re-sends what it kept.
- Revisit if a panic reaches readers despite the contract.
