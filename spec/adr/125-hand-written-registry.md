# ADR-125: A hand-written registry

Status: Proposed (2026-10-18)

## Context

The wasm modules, the CLI and the contract reach charts only through a
registry (REQ-ARCH-020). unicel lists its pieces by hand: each category
folder's `mod.rs` holds a `REGISTRY`, and `pieces/mod.rs` lists `CATEGORIES`,
with no build script or proc macro (unicel's ADR-012). ADR-003 says that
choice is re-decided for glyphplot. The options:

- **(a) A hand-written list** per family folder and a list of families.
- **(b) A build script** that scans the chart folders and writes the list.
- **(c) A macro** on each chart that registers it.

Facts that bear on it:

- glyphplot's charts are families, one folder and one Cargo feature each,
  with a few types and modes in each (ADR-101, `discovery/charts.md` Module
  boundaries): about 18 families at the long tail, not 207 pieces.
- The registry must follow the features, so a module built for one family
  holds that family's entries only.
- unicel's hand-written registry holds 207 pieces with a unit test that each
  entry sits in its category's folder (unicel's ADR-012).
- No measurement decides between the options.

## Decision

- Each family's `mod.rs` holds a hand-written `REGISTRY`, one line a type, and
  `charts/mod.rs` a `FAMILIES` list with each family behind its feature
  (REQ-REG-010).
- `registry::make(type, options_json)` returns a fresh chart, or fails with
  `there is no chart named "<type>"` or `options are not a JSON object:
  <message>` (REQ-REG-010).
- No type is named `render`, `demo` or `list` (REQ-REG-020).
- A unit test checks that each entry sits in its family's folder.

## Consequences

- One file shows every type of a family, in order, and `FAMILIES` order is
  the manifest's order.
- Adding a type is a file, a `pub mod` and one line; a type left out of the
  list is neither tested by the contract nor shipped.
- The build has no generated code to debug.
