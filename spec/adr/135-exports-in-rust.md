# ADR-135: Exports in Rust

Status: Proposed (2026-10-18)

## Context

A chart exports as text, ANSI, SVG and PNG, from the element and from the CLI.
unicel made its exports in TypeScript for the site, and its Proposed ADR-112
moves txt, ansi and svg into Rust so the CLI writes the same bytes. unicel's
ADR-112 weighed:

- **Everything in Rust, in the wasm**: one implementation, but a rasteriser
  and a font in every module.
- **The CLI drives a headless browser** for png: identical pixels, a huge
  dependency for a terminal tool.
- **The CLI writes txt and ansi only**: no images for READMEs.

A fourth option is unicel's present state: exports in TypeScript, which the
CLI and a Rust server cannot share.

`discovery/surfaces.md` (Exports) takes unicel's ADR-112 plan from the start.
What it measured bears on the shared path: a server-side first frame in Node
from the same wasm module took 0.30 ms to instantiate and 0.96 ms to draw
1,000 points at 80 × 24, median of 5. The size of the export code in a module
was not measured **(unverified)**.

## Decision

- txt, ansi and svg are made in Rust, in `glyphplot::export`, as pure
  functions of a grid and its palette, and are byte-identical between the
  element's `text()`, `toANSI()` and `toSVG()` and the CLI's `render` for the
  same input (REQ-EXPORT-010).
- An export draws the chart's current data, options and view, so a zoomed
  chart exports zoomed (REQ-EXPORT-020).
- png is drawn by the shell's `paint` in the browser and by a `png` feature in
  the CLI that no wasm module enables (REQ-EXPORT-030).
- `glyphplot/node` offers `renderText` and `renderSVG`, whose text equals the
  CLI's `render` (REQ-SSR-010).

## Consequences

- One implementation of each text format serves the element, Node, the CLI
  and Rust servers.
- Every chart module carries the text exporters.
- png from the browser and from the CLI match in size and geometry, not pixel
  for pixel, since glyph anti-aliasing differs (unicel's ADR-112).
- Block shapes exist in TypeScript and in Rust; a test keeps them equal.
