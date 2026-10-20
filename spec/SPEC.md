# glyphplot — Specification

Version 0.0.0 (nothing is built) · 2026-10-18 · License: MIT

glyphplot is an interactive charting library whose charts are drawn in
Unicode glyphs, one code point and one colour a cell. Charts are Rust,
compiled to one WebAssembly module per chart family, played in the browser
by a `<glyph-plot>` custom element over a small TypeScript shell and in the
terminal by a CLI; every chart draws data the page or the terminal gives it.

This document describes the application exactly as it stands. On
2026-10-18 nothing is built, so every section below holds only its heading
and the words "nothing is built". Requirements arrive from
[ROADMAP.md](ROADMAP.md) when their milestone lands, keeping their IDs and
saying which milestone brought them (`Note: (v0.3.0)`). Where this document
and other docs disagree, this document records what the code does and says
so in a `Note:`, and §21 lists each difference. Requirement IDs (`REQ-*`) are
stable: tests, commits and pull requests name them, and
[conformance.md](conformance.md) maps each to what checks it.

**Conventions.** MUST, MUST NOT, SHOULD, SHOULD NOT and MAY are used as in
RFC 2119. A requirement is one bullet, `- REQ-<AREA>-NNN: <rule>`, testable
as written. IDs go up in tens within an area and are never reused; a
requirement that changes another says so (`changes REQ-…`), and the older
one is marked superseded. A number says how it was measured, or is marked
**(unverified)**. Data shapes that cross a boundary have JSON Schemas in
[schemas/](schemas/); decisions are in [adr/](adr/).

## Glossary

- **chart**: one kind of picture of data, known by its type (`line`,
  `sparkline`, `candles`, `bar`, `scatter`, `heatmap`, …): a `Meta`, a
  `make` and an implementation of the `Chart` trait.
- **chart type**: what the element's `type` attribute and the CLI's first
  argument name.
- **family**: the charts that share scales, data shape and interaction and
  differ in marks (`line` plays `line` and `sparkline`); one Cargo feature
  and, in a release build, one wasm module.
- **mode**: a variant within a chart type, chosen by the `mode` option
  (`line`, `step`, `area`).
- **module**: one family's wasm build, `web/pkg/<family>/glyphplot_bg.wasm`.
- **data**: what a chart draws: named tables of typed columns, and matrices
  ([schemas/data.schema.json](schemas/data.schema.json)).
- **column**: one named, typed run of values; held as `f64`, timestamps as
  epoch milliseconds.
- **shape**: the kind of data a chart takes (`series`, `ohlcv`, `counts`,
  `matrix`, …), listed in its `Meta.shapes`.
- **channel**: a role a column plays for a chart (x, y, open, high, …),
  found by the `encode` option, a column's `role`, or its name.
- **replace**: setting all of a chart's data at once. **append**: adding rows
  to the end of a chart's data. **ring**: the store a streaming chart keeps
  its rows in, allocated once, oldest rows dropped.
- **view state**: everything interactive about a chart: its x and y domains,
  cursor, selection, hidden series, time cursor and any view animation in
  flight ([schemas/view.schema.json](schemas/view.schema.json)).
- **input**: a pointer, wheel, touch or key event in cells, folded into the
  view state by the chart's reducer
  ([schemas/input.schema.json](schemas/input.schema.json)).
- **event**: a `plot-*` `CustomEvent` the element fires for the reader's
  actions and for loading ([schemas/events.schema.json](schemas/events.schema.json)).
- **frame**: the picture a chart draws for its data, options, view state and
  play time `t`; a pure function of the four.
- **base** and **overlay**: the cached part of a frame (data, axes, legend)
  and the part drawn over it at pointer rate (crosshair, readout, brush,
  time cursor, focus).
- **grid**: the frame buffer, `cols × rows` cells. **cell**: one code point,
  an ink index and a back index.
- **palette**: a chart's 255 colours with a fixed layout of theme tokens and a
  scale ramp; a cell names an index, never a colour.
- **theme**: the colours behind the palette's tokens, from CSS custom
  properties or a theme object ([schemas/theme.schema.json](schemas/theme.schema.json)).
- **ground**: the colour behind a chart that has one.
- **one ink**: a frame drawn without its palette (a `<pre>`, `--mono`), every
  meaning colour carries told another way. **paper**: dark ink on a light
  ground.
- **dot**: one point of a sub-cell canvas (braille and octants 2 × 4 a cell,
  sextants 2 × 3, quadrants 2 × 2). **dot column**: one column of dots across
  the plot.
- **M4**: the first, last, minimum and maximum of the rows that fall in one dot
  column, the reduction line-like charts draw from.
- **scale**: a map from a domain value to a continuous cell coordinate and
  back. **tick**: a labelled value on an axis.
- **locale pack** and **zone table**: a locale's formatting symbols and a time
  zone's offset transitions, built by the shell from `Intl`
  ([schemas/locale.schema.json](schemas/locale.schema.json),
  [schemas/zone.schema.json](schemas/zone.schema.json)).
- **element**: `<glyph-plot>`. **shell**: the TypeScript in `web/src`.
  **site**: the Elm application in `site/`. **CLI**: the `glyphplot`
  binary. **manifest**: `web/pkg/modules.js`
  ([schemas/manifest.schema.json](schemas/manifest.schema.json)).
- **contract**: the rules every registered chart is checked against by
  `crates/glyphplot/tests/contract.rs` and `tests/budgets.rs`.
- **fixture**: a dataset the contract feeds every chart that takes its shape,
  generated from a seed.
- **domain pack**: a crate and wasm module that turns a domain's data into
  columns (markets, telemetry, astronomy) and never draws.
- **reference machine**: the owner's Apple M5 Pro (18 cores, 48 GB, macOS),
  on which budgets are measured.

## Contents

1. [Scope](#1-scope)
2. [Architecture](#2-architecture)
3. [Core: grid, glyphs, colour](#3-core-grid-glyphs-colour)
4. [Data](#4-data)
5. [Scales, ticks, formatting and layout](#5-scales-ticks-formatting-and-layout)
6. [The chart contract and the registry](#6-the-chart-contract-and-the-registry)
7. [Charts](#7-charts)
8. [View, input and interaction](#8-view-input-and-interaction)
9. [The wasm boundary and the manifest](#9-the-wasm-boundary-and-the-manifest)
10. [The element, the shell and rendering](#10-the-element-the-shell-and-rendering)
11. [Theme, one ink and accessibility](#11-theme-one-ink-and-accessibility)
12. [The CLI](#12-the-cli)
13. [The Rust crate and exports](#13-the-rust-crate-and-exports)
14. [The site and datasets](#14-the-site-and-datasets)
15. [Domain packs](#15-domain-packs)
16. [Build, budgets and tooling](#16-build-budgets-and-tooling)
17. [Testing](#17-testing)
18. [Packaging](#18-packaging)
19. [Decisions](#19-decisions)
20. [Instructions for an implementing agent](#20-instructions-for-an-implementing-agent)
21. [Appendix: where the docs differ from the code](#21-appendix-where-the-docs-differ-from-the-code)

---

## 1. Scope

### 1.1 Goals

- Charts of the page's or the terminal's data, drawn in Unicode glyphs, one
  code point and one colour a cell.
- One tag, `<glyph-plot type="…">`, that loads only the wasm of the chart
  families a page shows.
- Interactive from the first chart: pan, zoom, crosshair, readouts,
  brushing, keyboard and touch, events out.
- The same charts in a terminal, and as text and SVG from Rust.

### 1.2 Non-goals

- A React wrapper (typings instead), charts computed in a worker, charts that
  generate their own data outside a labelled demo mode, hosted CI, and any
  code shared with unicel (ADR-013). The plan
  and the ADRs say why.

### 1.3 State of delivery

Nothing is built. Discovery, the plan and the roadmap were written on
2026-10-18; Phase 0 is next (`docs/IMPLEMENTATION_PLAN.md`).

---

## 2. Architecture

Nothing is built. The intended layers are drawn in
`docs/IMPLEMENTATION_PLAN.md`, "Architecture".

### 2.1 Layer rules

Nothing is built.

### 2.2 Per-frame and per-event data flow

Nothing is built.

---

## 3. Core: grid, glyphs, colour

### 3.1 Grid

Nothing is built.

### 3.2 Sub-cell canvases and cell mapping

Nothing is built.

### 3.3 Charsets and the font

Nothing is built.

### 3.4 Palette and colour

Nothing is built.

### 3.5 Independence from unicel

Nothing is built.

---

## 4. Data

### 4.1 The data contract

Nothing is built.

### 4.2 Streaming

Nothing is built.

### 4.3 Downsampling

Nothing is built.

---

## 5. Scales, ticks, formatting and layout

### 5.1 Scales

Nothing is built.

### 5.2 Ticks

Nothing is built.

### 5.3 Formatting, time zones and locales

Nothing is built.

### 5.4 Layout, text, axes and annotations

Nothing is built.

---

## 6. The chart contract and the registry

### 6.1 Meta and options

Nothing is built.

### 6.2 The `Chart` trait

Nothing is built.

### 6.3 Registry

Nothing is built.

### 6.4 The contract test

Nothing is built.

### 6.5 Fixtures, snapshots, references and properties

Nothing is built.

---

## 7. Charts

### 7.1 Line and sparkline

Nothing is built.

### 7.2 Candles

Nothing is built.

### 7.3 Bar

Nothing is built.

### 7.4 Scatter and heatmap

Nothing is built.

### 7.5 Sky and orbit

Nothing is built.

---

## 8. View, input and interaction

### 8.1 View state

Nothing is built.

### 8.2 Input

Nothing is built.

### 8.3 Hit testing and the overlay

Nothing is built.

### 8.4 Events out and linking

Nothing is built.

### 8.5 Keyboard and touch

Nothing is built.

---

## 9. The wasm boundary and the manifest

### 9.1 The `Chart` object

Nothing is built.

### 9.2 Panics

Nothing is built.

### 9.3 Manifest and loading

Nothing is built.

---

## 10. The element, the shell and rendering

### 10.1 The element

Nothing is built.

### 10.2 Mount and the JS API

Nothing is built.

### 10.3 Rendering

Nothing is built.

### 10.4 Font

Nothing is built.

### 10.5 Server-side rendering

Nothing is built.

---

## 11. Theme, one ink and accessibility

### 11.1 Theme

Nothing is built.

### 11.2 One ink

Nothing is built.

### 11.3 Accessibility

Nothing is built.

---

## 12. The CLI

Nothing is built.

---

## 13. The Rust crate and exports

Nothing is built.

---

## 14. The site and datasets

### 14.1 The site

Nothing is built.

### 14.2 Examples

Nothing is built.

### 14.3 Datasets

Nothing is built.

---

## 15. Domain packs

Nothing is built.

---

## 16. Build, budgets and tooling

### 16.1 mise tasks

Nothing is built. `mise install` installs the pinned toolchain; `mise run
check` is a placeholder until Phase 0.

### 16.2 Lint

Nothing is built.

### 16.3 Size budgets

Nothing is built.

### 16.4 Time and memory budgets

Nothing is built.

### 16.5 Local CI (signoff)

Nothing is built.

---

## 17. Testing

Nothing is built.

---

## 18. Packaging

Nothing is built.

---

## 19. Decisions

Accepted decisions: the owner's (001–013) and those of discovery's the owner
accepted on 2026-10-18. The rest of ADR-100 to ADR-143 are Proposed (ADR-100
is rejected) and listed in [adr/README.md](adr/README.md); each joins this
table when the owner accepts it.

| ADR | Decision | Date |
|---|---|---|
| [ADR-001](adr/001-name-glyphplot.md) | The name is glyphplot | 2026-10-18 |
| [ADR-002](adr/002-its-own-repository.md) | glyphplot is its own repository | 2026-10-18 |
| [ADR-003](adr/003-unicels-architecture.md) | unicel's architecture, whole | 2026-10-18 |
| [ADR-004](adr/004-unicels-frame-buffer.md) | unicel's frame buffer | 2026-10-18 |
| [ADR-005](adr/005-one-wasm-module-per-chart-type.md) | One wasm module per chart type | 2026-10-18 |
| [ADR-006](adr/006-data-is-injected.md) | Data is injected | 2026-10-18 |
| [ADR-007](adr/007-interactive-from-the-first-chart.md) | Interactive from the first chart | 2026-10-18 |
| [ADR-008](adr/008-a-generous-size-threshold.md) | A size threshold per module, set generously | 2026-10-18 |
| [ADR-009](adr/009-deterministic-frames.md) | A frame is a pure function of data, options, view and t | 2026-10-18 |
| [ADR-010](adr/010-unicels-spec-format.md) | unicel's spec format | 2026-10-18 |
| [ADR-011](adr/011-mit-licence.md) | MIT licence | 2026-10-18 |
| [ADR-012](adr/012-no-ai-attribution.md) | No AI attribution | 2026-10-18 |
| [ADR-013](adr/013-independent-of-unicel.md) | Independent of unicel | 2026-10-18 |
| [ADR-101](adr/101-chart-families-with-modes.md) | Chart families with modes | 2026-10-18 |
| [ADR-102](adr/102-semantic-palette-of-255.md) | A semantic palette of 255 | 2026-10-18 |
| [ADR-103](adr/103-cell-from-font-grid-from-box.md) | The cell from the font, the grid from the box | 2026-10-18 |
| [ADR-112](adr/112-zones-and-locales-from-the-shell.md) | Zones and locales from the shell | 2026-10-18 |
| [ADR-116](adr/116-input-defaults.md) | Input defaults | 2026-10-18 |
| [ADR-118](adr/118-one-tag-with-a-type.md) | One tag with a type | 2026-10-18 |
| [ADR-119](adr/119-no-glyph-and-colour-atlas.md) | No glyph-and-colour atlas | 2026-10-18 |
| [ADR-121](adr/121-extend-the-bundled-font.md) | Extend the bundled font | 2026-10-18 |
| [ADR-123](adr/123-colour-defaults-held-to-measured-bars.md) | Colour defaults held to measured bars | 2026-10-18 |
| [ADR-126](adr/126-correctness-and-timing-apart.md) | Correctness and timing apart | 2026-10-18 |
| [ADR-133](adr/133-a-350-kb-module-gate.md) | A 350 KB module gate | 2026-10-18 |
| [ADR-136](adr/136-the-sites-market-demo.md) | The site's market demo | 2026-10-18 |
| [ADR-140](adr/140-celestial-positions-in-a-pack.md) | Celestial positions in a pack | 2026-10-18 |
| [ADR-141](adr/141-serde-json-in-chart-modules.md) | serde_json in chart modules | 2026-10-18 |
| [ADR-143](adr/143-site-docs-with-live-charts.md) | The site is documentation with live charts | 2026-10-18 |

---

## 20. Instructions for an implementing agent

1. Read `AGENTS.md`, `README.md`, this document, [ROADMAP.md](ROADMAP.md)
   (with its §21), then `docs/IMPLEMENTATION_PLAN.md` and the discovery report
   of the area you work in.
2. Run everything through mise; `mise run check` green before every commit
   once there is code.
3. Build one milestone at a time, in the roadmap's order, on its own
   GitButler branch; name the requirement IDs in every commit; no AI
   attribution.
4. When a milestone lands, move its requirements here with their IDs, under
   the section their area belongs to, each with `Note: (v0.N.0)`; update
   [conformance.md](conformance.md) in the same change.
5. A new chart: one folder under `crates/glyphplot/src/charts/<family>/`, one
   Cargo feature, one registry line, fixtures and snapshots, then
   `mise run test`. It uses the core and its own folder, never another
   family's.
6. Keep the boundary in step: a field added to `Meta`, the view, an event or
   the manifest changes its schema in [schemas/](schemas/) in the same change.
7. Browser and device checks are the owner's: check headless and hand the
   manual checks over as a list.

---

## 21. Appendix: where the docs differ from the code

There is no code yet, so there is no difference to record.
