# glyphplot — Specification

Version 0.2.0 · 2026-10-18 · License: MIT

glyphplot is an interactive charting library whose charts are drawn in
Unicode glyphs, one code point and one colour a cell. Charts are Rust,
compiled to one WebAssembly module per chart family, played in the browser
by a `<glyph-plot>` custom element over a small TypeScript shell and in the
terminal by a CLI; every chart draws data the page or the terminal gives it.

This document describes the application exactly as it stands. On
2026-10-18 v0.2.0, the core, is built: a section with nothing built
holds only its heading and the words "nothing is built". Requirements arrive from
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

v0.1.0 (Phase 0, the toolchain) and v0.2.0 (Phase 1, the core) were built
on 2026-10-18: the crates and tasks; the grid, canvases and cell mapping; the
palette and themes; data and fixtures; scales, ticks, formats and time
checked against d3; layout and text; the font. No chart is drawn yet.
Phase 2, the line chart end to end, is next (`docs/IMPLEMENTATION_PLAN.md`).

---

## 2. Architecture

The workspace holds three crates: `crates/glyphplot` (the core, with the
chart families as features), `crates/wasm` (`glyphplot-wasm`, the
wasm-bindgen boundary, built once a family) and `crates/cli`
(`glyphplot-cli`, binary `glyphplot`). The intended layers are drawn in
`docs/IMPLEMENTATION_PLAN.md`, "Architecture".

### 2.1 Layer rules

The core depends on serde, serde_json and unicode-width. Its registry, `glyphplot::FAMILIES`, lists
the families compiled in, each with its budget; the wasm crate exports
`version()` and `families()`, and the CLI prints its usage and version.

- REQ-ARCH-010: The core crate (`crates/glyphplot`) MUST NOT depend on a DOM, a canvas or a terminal; its normal dependencies MUST be `serde`, `serde_json` and `unicode-width` only. Note: (v0.1.0)
- REQ-ARCH-040: Each chart family MUST be one Cargo feature in both `glyphplot` and `glyphplot-wasm`; the feature `all`, on by default, MUST enable every one. Note: (v0.1.0)
- REQ-ARCH-060: The workspace MUST be one version, Rust edition 2021, licence MIT, on the Rust 1.97 toolchain with the `wasm32-unknown-unknown` target, `rustfmt` and `clippy` (`rust-toolchain.toml`, `mise.toml`). Note: (v0.1.0)
- REQ-ARCH-070: The workspace MUST have no git dependencies. Note: (v0.1.0)

### 2.2 Per-frame and per-event data flow

Nothing is built.

---

## 3. Core: grid, glyphs, colour

### 3.1 Grid

`grid::Grid` holds the frame. Text is written by `unicode-width` cells; a
write over either half of a wide glyph blanks the other half.

- REQ-GRID-010: A `Grid` MUST hold `cols × rows` cells row-major, each a `u32` code point, a `u8` ink index and a `u8` back index, with the constants `BLANK = 0x20` (a space), `WIDE_TAIL = 0` (the right half of a double-width glyph, never a real code point) and `NO_BACK = 255` (no colour behind a cell). [ADR-004](adr/004-unicels-frame-buffer.md) Note: (v0.2.0)
- REQ-GRID-020: `Grid::resize(cols, rows)` MUST give a grid of the new size with every cell `BLANK`, colour 0 and `NO_BACK`, reusing its allocation when it is large enough, and MUST panic on a zero width or height. Note: (v0.2.0)
- REQ-GRID-030: A new grid MUST start every cell `BLANK` in colour 0 with `NO_BACK` and MUST panic on a zero width or height; `clear` MUST reset every cell so, and `fill` MUST set every cell's glyph and ink and leave backs as they are. Note: (v0.2.0)
- REQ-GRID-040: Every drawing method (put a glyph, write text, fill a rectangle, draw a Bresenham line including both ends, set a back colour) MUST clip: off-grid coordinates are ignored, so marks may overhang the edges. Note: (v0.2.0)
- REQ-GRID-050: Writing a double-width glyph MUST put it at `x` and `WIDE_TAIL` at `x + 1`, both in the given colour, and only when both cells are on the grid; a debug build MUST assert the glyph is two columns wide. Note: (v0.2.0)
- REQ-GRID-060: `text()` MUST return `rows` lines joined by `\n`, leaving out `WIDE_TAIL` cells and writing any invalid code point as U+FFFD. Note: (v0.2.0)
- REQ-GRID-070: The grid MUST expose its code points, inks and backs as row-major slices for the boundary and for hot loops, and its indexed getters MUST panic off the grid. Note: (v0.2.0)

### 3.2 Sub-cell canvases and cell mapping

`canvas::Canvas` draws dots in one of four encodings (`Glyphs::Braille`,
`Octants`, `Sextants`, `Quadrants`). `canvas/octants.rs` is generated by
`scripts/octants.py` from UnicodeData 16.0.0 (SHA-256 pinned): 230 masks
from BLOCK OCTANT names, 10 from QUADRANT names and 16 from other block
elements. `cells::Axis` is the one cell mapping; `ticks::mark` places a tick
through it.

- REQ-CANVAS-010: The core MUST provide sub-cell canvases at 2 × 4 dots a cell (braille, octants), 2 × 3 (sextants) and 2 × 2 (quadrants), with dots set, cleared and read, Bresenham lines and midpoint circles in dot coordinates, all clipping; drawing a canvas onto the grid MUST set only cells that have dots, so canvases layer over text and each other. Note: (v0.2.0)
- REQ-CANVAS-020: The core MUST have one module of cell mappings (data value to column and dot, column and dot to data value) that frames, hit tests, readouts, reducers and the contract all call; a property test MUST show that the x at a column's left edge maps back into that column for 1 to 2,000 points over 40 to 640 columns. Note: (v0.2.0)
- REQ-CANVAS-030: Glyphs MUST follow the Unicode Standard: braille by its dot numbering (left column dots 1, 2, 3, 7, right 4, 5, 6, 8; bits 0x01, 0x02, 0x04, 0x40 and 0x08, 0x10, 0x20, 0x80 over U+2800); sextants by the BLOCK SEXTANT names of U+1FB00–1FB3B with the four masks they omit drawn as space, `▌`, `▐` and `█`; quadrants by U+2596–259F and `▀ ▄ ▌ ▐ █`; octants by a 256-entry table that `scripts/octants.py` generates from Unicode 16's `UnicodeData.txt` by character name. A cell with no dots MUST be a space. Unit tests MUST pin code points against their Unicode names. Note: (v0.2.0)

### 3.3 Charsets and the font

`web/fonts/glyphplot-mono.woff2` is 25,896 B with 1,361 code points, every
glyph 600 units (`mise run build:font`, which rebuilds it byte for byte from
Iosevka 34.9.0's release zip, SHA-256 pinned). Iosevka lacks U+2072, U+2073,
U+20A0, U+20A2, U+20B0, U+20B6, U+20B7 and U+20BB of tier A.
`web/fonts/unicode-range.txt` is the range the element's `@font-face` will
use.

- REQ-FONT-010: The bundled font MUST be a subset of Iosevka Fixed Extended 34.9.0 (SIL OFL 1.1), family `glyphplot mono`, covering U+0020–007E, U+00B0, U+00B7, U+2022, U+2190–21FF, U+2500–25FF, U+2800–28FF, U+1CD00–1CDE5, U+1CEA0–1CEAF and U+1FB00–1FBFF (as Iosevka has them) plus tier A (the plan's [font](../docs/IMPLEMENTATION_PLAN.md#font) tier A: U+00A0–00BF, U+00D7, U+00F7, U+2007–200A, U+2010–2015, U+2018–201F, U+2026, U+202F, U+2030, U+2032–2033, U+2070–209F, the currency signs, U+2116, U+2122, U+2212, U+221E, U+2248, U+2260, U+2264–2265, Δ Σ Ω μ σ, U+2713, U+2717, U+2605–2606, U+FFFD), every glyph 0.6 em, at most 32 KB, built only by `mise run build:font` with the repository's own `scripts/font.py` from Iosevka's upstream release, pinned by SHA-256. [ADR-121](adr/121-extend-the-bundled-font.md) Note: (v0.2.0)

### 3.4 Palette and colour

`palette` names the layout; `theme::resolve` and `theme::resolve_json` give
the 255 entries. The default dark and light themes are provisional until the
owner picks by eye (open question 10). Their scale is cividis at nine stops,
largest neighbouring step ΔE76 2.14 at 128 entries
(`scale_ramp_steps_within_delta_e_2_5`). Reserved entries are transparent.

- REQ-COLOR-010: A cell's ink and back MUST be indices 0–254 into the chart's palette, or `NO_BACK` (255) for the back. [ADR-102](adr/102-semantic-palette-of-255.md) Note: (v0.2.0)
- REQ-COLOR-020: The palette MUST have a fixed layout: indices 0–31 are theme tokens in the order of the plan's [palette](../docs/IMPLEMENTATION_PLAN.md#palette) table (ink, muted, rule, accent, ground, up, down, selection, series 1–8, the rest reserved), 32–159 a 128-step scale ramp, and 160–254 reserved; a frame MUST write indices only, never colours. Note: (v0.2.0)
- REQ-COLOR-030: The core MUST resolve a theme object to the 255 RGBA entries in one function, `theme::resolve`, which the shell, the CLI and the exports all call; a change of theme MUST NOT draw a new frame (a test counts `frame` calls: zero). Note: (v0.2.0)
- REQ-COLOR-040: The scale ramp MUST interpolate the theme's scale stops in Oklab over 128 steps, and no two neighbouring steps of a default scale MUST differ by more than ΔE76 2.5. Note: (v0.2.0)

### 3.5 Independence from unicel

`scripts/deps.sh`, run by `mise run lint`, fails on any non-comment line of
a `Cargo.toml`, `package.json`, `mise.toml` or script that names
`animate-unicode` or `unicel`.

- REQ-INDEP-010: No file in the repository MUST be copied, ported or generated from unicel's repository (`../animate-unicode`), and no crate, package, script or task MUST depend on it or read from it; glyphplot's grid, canvases, glyph tables, block shapes, font build, loader, element, CLI and scripts are written for glyphplot. unicel is prior art, cited for design as other libraries are. [ADR-013](adr/013-independent-of-unicel.md) Note: (v0.1.0)
- REQ-INDEP-020: `mise run lint` MUST fail when any `Cargo.toml`, `package.json`, `mise.toml` or file under `scripts/` names `animate-unicode`, or a `unicel` crate or package, as a dependency, a path or an input. Note: (v0.1.0)

---

## 4. Data

### 4.1 The data contract

`data::Data` holds named tables and matrices; `Data::from_json` reads the
data object and its columns and rows shorthands. `Table::channel` finds a
channel by `encode`, role, then name. `fixtures` generates the 23 fixtures,
bitwise equal natively and in wasm32 (checked once by hand in Node 26 on
2026-10-18).

- REQ-DATA-010: Every chart MUST accept data in the shape of [data.schema.json](schemas/data.schema.json): named tables of columns typed `f64`, `f32`, `i64`, `u32`, `bool`, `timestamp`, `duration`, `category` or `string`, and named matrices; the schema MUST validate the examples in `spec/schemas/examples/`. Note: (v0.2.0)
- REQ-DATA-020: A chart MUST hold numeric, timestamp and duration columns as `f64`; timestamps as epoch milliseconds; and a column whose `time_unit` is `us` or `ns` as offsets from an `i64` origin, so `ns` values spanning under 2^53 ns round-trip exactly (tested with 1792281600000000001 to 1792281600000000004). [ADR-104](adr/104-typed-columns-cross-the-boundary.md) Note: (v0.2.0)
- REQ-DATA-030: JSON `null`, an empty CSV field, NaN and a cleared Arrow validity bit MUST all read as missing: NaN for numbers and times, `u32::MAX` for categories. A missing y MUST break a line or area into separate runs, and a row with a missing x MUST NOT be drawn. Note: (v0.2.0)
- REQ-DATA-040: A chart MUST check on load that its x column is non-decreasing; when it is not, it MUST draw a stably sorted copy and warn once, and its frame MUST equal the frame of the sorted data. Note: (v0.2.0)
- REQ-DATA-080: A chart MUST find each channel (x, y, open, …) by the option `encode`, then by a column's `role`, then by a column of the channel's name; when none matches it MUST draw its empty-data frame and warn once naming the channel. Note: (v0.2.0)
- REQ-DATA-090: Timestamp values MUST be UTC instants; a column's `zone` MUST decide only tick placement and labels, and no frame MUST depend on the host's time zone (equal frames under `TZ=UTC` and `TZ=Asia/Tokyo`). Note: (v0.2.0)
- REQ-DATA-110: Data of a shape the chart does not accept MUST give an error naming the shape it expected, never a panic. Note: (v0.2.0)
- REQ-DATA-130: Data changes MUST take effect at once; a frame MUST be a function of the data as it is, never of an easing between versions. [ADR-106](adr/106-data-applies-at-once.md) Note: (v0.2.0)

### 4.2 Streaming

Nothing is built.

### 4.3 Downsampling

Nothing is built.

---

## 5. Scales, ticks, formatting and layout

### 5.1 Scales

`scale::Scale` (linear and time) and `scale::resolve_domain`. JavaScript's
rounding, and V8's own `log10` (fdlibm), are copied where d3 depends on
them.

- REQ-SCALE-010: The core MUST provide linear and time scales that map a domain value to a continuous cell coordinate and back; `invert(map(v))` MUST equal `v` within 1e-9 of the larger magnitude of the domain's ends for finite `v` in the domain (relative to `v` alone it cannot hold near 0). [ADR-110](adr/110-scales-and-formats-ported-from-d3.md) Note: (v0.2.0)
- REQ-SCALE-020: A scale's domain MUST be `"data"`, `"view"`, a fixed `[a, b]` or `{"follow": seconds}`, and MUST be a pure function of the data, the options and the view state. Note: (v0.2.0)
- REQ-SCALE-030: `nice` MUST give the domain d3-scale 4.0.2's `linear().nice(count)` gives for every case of the captured fixture file, including leaving unchanged a domain that does not settle in ten rounds. Note: (v0.2.0)

### 5.2 Ticks

`ticks::ticks` and `time::utc_ticks` are d3's; `layout::fit_ticks` fits
them to an axis by lowering the count (ADR-111).

- REQ-TICK-010: Linear ticks MUST equal d3-array 3.2.4's `ticks(a, b, count)` exactly for every case of the captured fixture file, natively and in wasm. Note: (v0.2.0)
- REQ-TICK-020: UTC time ticks MUST equal d3-time 3.1.0's `utcTicks(a, b, count)` exactly for every case of the captured fixture file. Note: (v0.2.0)
- REQ-TICK-030: For any axis from 4 to 400 cells and any finite domain, placed ticks MUST lie in the domain, increase strictly in value and never decrease in cell, and their labels MUST lie within the axis and be separated by at least the label gap (default two cells on x, one row on y). [ADR-111](adr/111-ticks-fit-by-lowering-the-count.md) Note: (v0.2.0)
- REQ-TICK-040: A tick mark MUST be drawn in the cell nearest its continuous position, and a gridline at the braille dot nearest it. Note: (v0.2.0)

### 5.3 Formatting, time zones and locales

`format::NumberFormat` writes digits by hand (`format::js` copies
JavaScript's `toFixed`, `toExponential`, `toPrecision` and `toString`);
`time::strftime` formats UTC times. `crates/glyphplot/tests/fixtures/` holds
32,264 cases captured from the pinned d3 packages by `mise run fixtures`;
`tests/reference.rs` finds 0 differing.

- REQ-FMT-010: Tick labels MUST default to d3-scale's `tickFormat` behaviour (one format for the axis, precision from the step, one SI prefix from the larger end of the domain for `s`) and MUST equal d3's labels for every case of the captured fixture file. Note: (v0.2.0)
- REQ-FMT-020: The number formatter MUST accept the d3-format subset `[sign][$][,][.precision][~][type]` with types `f`, `s`, `%`, `e`, `d` and `auto`, and MUST reject any other specifier with an error naming the option. Note: (v0.2.0)
- REQ-FMT-030: Tick labels, readouts, legends and table cells MUST be produced by the core's formatters, never by `core::fmt` on floats; no label MUST read `NaN`, `inf` or `-0`. Note: (v0.2.0)
- REQ-TZ-010: Time scales MUST default to UTC, and the core MUST NOT read a clock, a zone or tzdata. [ADR-112](adr/112-zones-and-locales-from-the-shell.md) Note: (v0.2.0)
- REQ-I18N-030: Without a pack, formatting MUST use `.` as the decimal, `,` as the group, groups of three, ASCII digits and `-`. Note: (v0.2.0)
- REQ-I18N-040: Label layout MUST count cells by `unicode-width` 0.2 a code point in release builds and draw width-2 glyphs with `set_wide`. Note: (v0.2.0)

### 5.4 Layout, text, axes and annotations

`layout::layout` places the title, legend, panes and axes in cells;
`text::Frame` draws a frame in stages whose order the types enforce.

- REQ-LAYOUT-010: Layout MUST be computed in whole cells in the order title, legend, x axis, panes, y gutters, x ticks, annotations, and MUST be a pure function of cols, rows, options, the data's extent and the view. Note: (v0.2.0)
- REQ-LAYOUT-020: When cols < 16 or rows < 5 a chart MUST draw no axis, title or legend; it MUST draw no title below 10 rows and no x labels below 6 rows. Note: (v0.2.0)
- REQ-LAYOUT-030: Y labels MUST be dropped, the right axis first, when the gutters would take more than a third of the columns. Note: (v0.2.0)
- REQ-TEXT-010: A label MUST occupy whole cells; no two labels MUST share a cell; a label MUST clear the cells under it to the ground before it is drawn, so no label shares a cell with data; the contract MUST check this on every fixture at 80 × 24 and 320 × 120. Note: (v0.2.0)
- REQ-TEXT-020: Draw order MUST be the z-order: data, gridlines on empty cells only, axes, labels, annotations, the crosshair, the readout last. Note: (v0.2.0)

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

The properties run at proptest's default 256 cases in `check` and passed at
4,096 on 2026-10-18.

- REQ-FIX-010: Fixture datasets MUST be generated by `crates/glyphplot/src/fixtures.rs` from SplitMix64 with a fixed seed each, MUST include the 23 fixtures of the plan's [fixtures](../docs/IMPLEMENTATION_PLAN.md#fixtures) table, and two generations MUST be bitwise equal on every target. [ADR-128](adr/128-fixtures-generated-from-seeds.md) Note: (v0.2.0)
- REQ-REF-010: Each port of d3-array, d3-scale, d3-time, d3-format or the LTTB reference MUST be compared bitwise with output captured from the pinned original by `mise run fixtures`, in `tests/reference.rs` and in `web/test/reference.test.mjs`. Note: (v0.2.0)
- REQ-PROP-010: The core MUST hold proptest properties (lean features) for scale inversion, tick order and count, reduction bounds, merged reductions equalling the whole, LTTB's ends and order, cell-mapping agreement and label non-overlap, with regression files checked in. [ADR-130](adr/130-proptest.md) Note: (v0.2.0)

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

| Task | What it does |
|---|---|
| `install` | `mise install`: the pinned toolchain |
| `build:wasm` | `scripts/build-wasm.sh`: one release module a family into `web/pkg/<family>/`, then `modules.js`, `modules.d.ts` and `budgets.json` (`crates/glyphplot/examples/manifest.rs`) and `sizes.json` |
| `build` | `build:wasm` |
| `fmt` | `cargo fmt --all` |
| `lint` | rustfmt, clippy, rustdoc, each family alone, `scripts/octants.py --check`, `scripts/deps.sh`, `scripts/generated.sh` (§16.2) |
| `test:rust`, `test` | `cargo test --workspace --profile contract` |
| `fixtures` | `scripts/fixtures.mjs`: the d3 captures (§5.3) |
| `build:font` | `scripts/font.py` (§3.3) |
| `size` | `build:wasm`, then `scripts/size.mjs` (§16.3) |
| `check` | `lint`, `test`, `build` and `size` |
| `signoff` | `scripts/signoff.sh` (§16.5) |

- REQ-BUILD-010: mise MUST be the only entry point for building, checking and serving; `mise install` MUST install every tool `mise.toml` pins, and `mise run check` MUST exit 0. Note: (v0.1.0)
- REQ-BUILD-020: `mise run check` MUST run `lint`, `test`, `build` and `size` in one task graph, and from v0.3.0 also `test:site --smoke`. Note: (v0.1.0)
- REQ-BUILD-030: `mise run build:wasm` MUST empty `web/pkg`, build one release module per family into `web/pkg/<family>/glyphplot{.js,_bg.wasm}` with `wasm-pack --target web --release --no-pack`, delete the `.gitignore` wasm-pack writes beside each, and write `modules.js`, `modules.d.ts`, `budgets.json` and `sizes.json`. Note: (v0.1.0)
- REQ-BUILD-050: The release profile MUST be `opt-level = "s"`, LTO, one codegen unit, `panic = "abort"` and `strip`, and wasm-opt MUST run with `-Os --enable-bulk-memory --enable-nontrapping-float-to-int`. Note: (v0.1.0)
- REQ-BUILD-060: Generated output (`web/pkg`, `web/dist`, `site/dist`, `target`, the bench outputs under `discovery/bench`) MUST stay out of git; the bundled font is generated and checked in. Note: (v0.1.0)

- REQ-BUILD-070: `mise run test:rust` MUST run `cargo test --workspace --profile contract`, a profile inheriting `release` with LTO off, 16 codegen units and `opt-level = 3`. [ADR-127](adr/127-test-profile-without-lto.md) Note: (v0.2.0)

### 16.2 Lint

`mise run lint` also checks each family alone (`cargo check -p glyphplot
--no-default-features --features <family>`), the core's dependencies
(REQ-ARCH-010), git dependencies (REQ-ARCH-070), unicel's repository
(REQ-INDEP-020) and tracked generated output (REQ-BUILD-060).

- REQ-LINT-010: `mise run lint` MUST run `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `RUSTDOCFLAGS='-D warnings' cargo doc --no-deps -p glyphplot`, and clippy on `glyphplot-wasm` for `wasm32-unknown-unknown` with and without `debug`. Note: (v0.1.0)
- REQ-LINT-030: Rust style MUST follow `rustfmt.toml` (`max_width = 160`, `use_small_heuristics = "Max"`), and every public item of every crate MUST have a doc comment (`#![warn(missing_docs)]`, rustdoc with `-D warnings`). Note: (v0.1.0)

- REQ-LINT-040: `clippy.toml` MUST list `f64::clamp` and `f32::clamp` under `disallowed-methods`, so a call fails `mise run lint`. Note: (v0.2.0)

### 16.3 Size budgets

`scripts/size.mjs` reads each module's limit from `web/pkg/budgets.json` and
measures with `gzip -9` of the file on stdin, as `gzip -9 < file | wc -c`
counts it. On 2026-10-18 the empty `line` module is 15,742 B, 7,052 B
gzipped, and its JS glue 7,016 B, 2,016 B gzipped (`mise run size`).

- REQ-SIZE-010: `mise run size` MUST fail when any module's `glyphplot_bg.wasm` passes its budget gzip -9 (358,400 bytes unless its budget says otherwise), naming each module over with its size and its limit. [ADR-008](adr/008-a-generous-size-threshold.md), [ADR-133](adr/133-a-350-kb-module-gate.md) Note: (v0.1.0)
- REQ-SIZE-020: `mise run size` MUST print each module's wasm and JS glue, raw and gzipped, and the total, and append those lines to `$GITHUB_STEP_SUMMARY` when it is set. Note: (v0.1.0)
- REQ-SIZE-030: `WASM_GZ_LIMIT` MUST override every module's limit, in bytes. Note: (v0.1.0)
- REQ-SIZE-040: `mise run size` SHOULD print a warning for each module past half its budget (179,200 bytes by default). Note: (v0.1.0)

- REQ-SIZE-060: No wasm module's dependency tree MUST hold chrono, time, jiff, chrono-tz, num-format, any `icu_` crate, arrow, arrow2 or vsop87, which `mise run lint` checks with `cargo tree -e normal --target wasm32-unknown-unknown`. Note: (v0.2.0)

### 16.4 Time and memory budgets

Nothing is built.

### 16.5 Local CI (signoff)

`mise run signoff [rev]` checks a commit (HEAD by default) out into
`target/ci/tree`, a detached git worktree removed afterwards, runs
`mise run check` there with a cargo cache in
`target/ci/target`, and logs to `target/ci/logs/<sha>.log`. It posts no
commit status yet; REQ-CI-020 adds that in v0.4.0.

- REQ-CI-010: There MUST be no hosted CI workflow; `mise run signoff` is CI ([ADR-003](adr/003-unicels-architecture.md)). Note: (v0.1.0)

---

## 17. Testing

The tests are the core's unit tests and properties, `tests/reference.rs`
(the d3 captures), and the CLI's (`crates/cli/tests/usage.rs`): 112 on
2026-10-18.

- REQ-TEST-010: `mise run test` MUST pass before every commit, and `mise run check` MUST be green. Note: (v0.1.0)

---

## 18. Packaging

Nothing is published. The crates are named; the packs come with v0.10.0.

- REQ-PKG-040: The crates MUST be `glyphplot` (library), `glyphplot-cli` (binary `glyphplot`), `glyphplot-wasm` (`publish = false`) and the packs `glyphplot-<domain>`. Note: (v0.1.0)

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

On 2026-10-18:
- The plan's fixtures table gives `labels-wide` as 3 series × 100; the code
  holds it in long form (300 rows, the names in a `series` category column),
  because column names are ASCII. `multi-4` is wide (four `y` columns).
- d3-format writes `NaN` and `Infinity`; the formatter writes the locale's
  `nan`, empty by default (REQ-FMT-030). Sign `+` on a negative that rounds
  to zero writes `+0.00` where d3 writes `-0.00`.
- A domain given high to low is laid out low to high; reversed axes are not
  built.
- The font holds four code points beyond REQ-FONT-010's list (U+208F,
  U+209D–209F, as Iosevka has them), draws Iosevka's 27 composite symbols as
  whole outlines, and drops the GDEF, GPOS and GSUB tables.
- `Data::from_json` is in every build until the `text` feature of
  REQ-DATA-050 lands (v0.3.0).
