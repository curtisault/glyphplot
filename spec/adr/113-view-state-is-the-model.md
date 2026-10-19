# ADR-113: The view state is the model

Status: Proposed (2026-10-18)

## Context

ADR-009 makes a frame a pure function of data, options, view state and `t`,
and leaves open how input is folded into view state. `discovery/interaction.md`
weighed:

- **(a) A view state reduced from input**: one plain object, read, set,
  linked and serialised, as Vega's `view.getState()` and `setState()`.
- **(b) An input log replayed each frame**, unicel's ADR-114: at most 512
  events stamped with play time. A zoom made 600 events ago would be
  forgotten, replay cost grows with the session, and a view set by the page or
  a URL is not an event.
- **(c) Both**, a log with the view as a cache.

And where input is reduced:

- **In wasm**, one Rust reducer that the CLI also links; or **in the shell**,
  with `set_view` into wasm, where the shell would need the scales, layout and
  data to hit-test, and the CLI a second implementation.

A crossing into wasm with two numbers cost 0.027 µs min, 0.049 µs median
(interaction.md, `noop(x, y)` from JS in Node).

## Decision

- Every chart holds its interactive state in one view state of the shape of
  `view.schema.json`; two charts given the same data, options, view and `t`
  draw identical grids (REQ-VIEW-010). Domains are in data units, `null`
  meaning the fitted default (REQ-VIEW-020).
- The core folds input into the view with one reducer, `Chart::input`, used by
  the wasm module and the CLI; it reads no clock and keeps no state outside
  the view and the chart's caches (REQ-INPUT-010).
- There is no input log.
- An animated view change (an eased zoom, a kinetic pan, a reset) is a
  `motion` record in the view state stamped with play time, none under reduced
  motion (REQ-VIEW-050).
- `view` round-trips through JSON as a property and an attribute
  (REQ-VIEW-030); setting it from script fires no reader event (REQ-VIEW-040).

## Consequences

- Linking charts and holding a view in a URL are property sets in data units.
- The contract replays scripted input and checks the view and the frame.
- Indices in `cursor` and `selection` are re-derived from x after a data
  change; pages that save views rely on x values.
- Exports of what a reader did come from the shell recording `(t, view)`.
