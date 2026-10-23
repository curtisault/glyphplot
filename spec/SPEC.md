# glyphplot — Specification

Version 0.5.0 · 2026-10-21 · License: MIT

glyphplot is an interactive charting library whose charts are drawn in
Unicode glyphs, one code point and one colour a cell. Charts are Rust,
compiled to one WebAssembly module per chart family, played in the browser
by a `<glyph-plot>` custom element over a small TypeScript shell and in the
terminal by a CLI; every chart draws data the page or the terminal gives it.

This document describes the application exactly as it stands. On
2026-10-21 v0.5.0, candles, bar and the sparkline, is built: a section with nothing
built
holds only its heading and the words "nothing is built". Requirements arrive
from
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
10. [The element, the shell and
rendering](#10-the-element-the-shell-and-rendering)
11. [Theme, one ink and accessibility](#11-theme-one-ink-and-accessibility)
12. [The CLI](#12-the-cli)
13. [The Rust crate and exports](#13-the-rust-crate-and-exports)
14. [The site and datasets](#14-the-site-and-datasets)
15. [Domain packs](#15-domain-packs)
16. [Build, budgets and tooling](#16-build-budgets-and-tooling)
17. [Testing](#17-testing)
18. [Packaging](#18-packaging)
19. [Decisions](#19-decisions)
20. [Instructions for an implementing
agent](#20-instructions-for-an-implementing-agent)
21. [Appendix: where the docs differ from the
code](#21-appendix-where-the-docs-differ-from-the-code)

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
checked against d3; layout and text; the font. v0.3.0 (Phase 2) was built
on 2026-10-19: the chart framework and the `line` chart, the wasm boundary,
the TypeScript shell and `<glyph-plot>`, the contract test and snapshots,
the Node and Playwright tests, `mise run dev` and the renderer bench. v0.4.0
(Phase 3) was built on 2026-10-20: the contract's other rules and its
self-test, the time budgets, the native and browser benches, the size
breakdown and `signoff` as CI. v0.5.0 (Phase 4) was built on 2026-10-21:
the `candles` and `bar` families, the sparkline, log, symlog, band and index
time scales, zones, units, panes, dual axes, annotations and linked charts.
Phase 5, streaming, is next (`docs/IMPLEMENTATION_PLAN.md`).

---

## 2. Architecture

The workspace holds three crates: `crates/glyphplot` (the core, with the
chart families as features), `crates/wasm` (`glyphplot-wasm`, the
wasm-bindgen boundary, built once a family) and `crates/cli`
(`glyphplot-cli`, binary `glyphplot`). The intended layers are drawn in
`docs/IMPLEMENTATION_PLAN.md`, "Architecture".

### 2.1 Layer rules

The core depends on serde, serde_json and unicode-width. Its registry,
`glyphplot::FAMILIES`, lists
the families compiled in, each with its budget; the wasm crate exports
`version()` and `families()`, and the CLI prints its usage and version.

- REQ-ARCH-010: The core crate (`crates/glyphplot`) MUST NOT depend on a DOM, a canvas or a terminal; its normal dependencies MUST be `serde`, `serde_json` and `unicode-width` only. Note: (v0.1.0)
- REQ-ARCH-040: Each chart family MUST be one Cargo feature in both `glyphplot` and `glyphplot-wasm`; the feature `all`, on by default, MUST enable every one. Note: (v0.1.0)
- REQ-ARCH-060: The workspace MUST be one version, Rust edition 2021, licence MIT, on the Rust 1.97 toolchain with the `wasm32-unknown-unknown` target, `rustfmt` and `clippy` (`rust-toolchain.toml`, `mise.toml`). Note: (v0.1.0)
- REQ-ARCH-070: The workspace MUST have no git dependencies. Note: (v0.1.0)

- REQ-ARCH-020: The wasm crate (`crates/wasm`) and the CLI (`crates/cli`) MUST reach charts only through the core's registry and define no chart of their own, except the debug charts of REQ-WASM-060. Note: (v0.3.0)
- REQ-ARCH-030: The shell (`web/src`) MUST be the only consumer of the wasm API; the site MUST reach charts only through the shell's exports and `<glyph-plot>`. Note: (v0.3.0)
- REQ-ARCH-050: A chart MUST use only the core's shared modules and its own family folder `crates/glyphplot/src/charts/<family>/`, never another family's; `mise run lint` MUST check every family alone (`cargo check -p glyphplot --no-default-features --features <family>`). Note: (v0.3.0)

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

- REQ-CANVAS-040: A colour canvas cell MUST take the colour of the series highest in z-order among its lit dots (REQ-CHART-060). Note: (v0.3.0)

### 3.3 Charsets and the font

`web/fonts/glyphplot-mono.woff2` is 25,896 B with 1,361 code points, every
glyph 600 units (`mise run build:font`, which rebuilds it byte for byte from
Iosevka 34.9.0's release zip, SHA-256 pinned). Iosevka lacks U+2072, U+2073,
U+20A0, U+20A2, U+20B0, U+20B6, U+20B7 and U+20BB of tier A.
`web/fonts/unicode-range.txt` is the range the element's `@font-face` will
use.

- REQ-FONT-010: The bundled font MUST be a subset of Iosevka Fixed Extended 34.9.0 (SIL OFL 1.1), family `glyphplot mono`, covering U+0020–007E, U+00B0, U+00B7, U+2022, U+2190–21FF, U+2500–25FF, U+2800–28FF, U+1CD00–1CDE5, U+1CEA0–1CEAF and U+1FB00–1FBFF (as Iosevka has them) plus tier A (the plan's [font](../docs/IMPLEMENTATION_PLAN.md#font) tier A: U+00A0–00BF, U+00D7, U+00F7, U+2007–200A, U+2010–2015, U+2018–201F, U+2026, U+202F, U+2030, U+2032–2033, U+2070–209F, the currency signs, U+2116, U+2122, U+2212, U+221E, U+2248, U+2260, U+2264–2265, Δ Σ Ω μ σ, U+2713, U+2717, U+2605–2606, U+FFFD), every glyph 0.6 em, at most 32 KB, built only by `mise run build:font` with the repository's own `scripts/font.py` from Iosevka's upstream release, pinned by SHA-256. [ADR-121](adr/121-extend-the-bundled-font.md) Note: (v0.2.0)

- REQ-CHAR-010: Every code point a chart draws outside label text MUST be in the bundled font's ranges (REQ-FONT-010); label text that comes from data or options MAY be any Unicode scalar value. Note: (v0.3.0)
- REQ-CHAR-020: A double-width glyph MUST be drawn with `set_wide` and its `WIDE_TAIL`, and a `WIDE_TAIL` MUST follow only a double-width glyph in the same row. Note: (v0.3.0)

- REQ-FONT-020: The font SHOULD also cover U+00C0–00FF and U+0100–017F (tier B). Note: (v0.5.0)

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

- REQ-DATA-050: The wasm boundary MUST take column values only as typed arrays (a `Vec<f64>` argument, or a buffer the chart reserved and the shell filled) plus a descriptor without `values`; no chart module MUST contain a CSV or JSON data parser, which `mise run size --charts` checks by symbol. Note: (v0.3.0)
- REQ-DATA-060: The shell MUST read `memory.buffer` only after the call that returns a buffer's address; a Node test MUST fill a reserved 1,000,000-row column across a memory growth and check its sum. Note: (v0.3.0)
- REQ-DATA-070: Replacing a chart's data with two 1,000,000-row `Float64Array` columns MUST take at most 10 ms (median of 5) in Node on the reference machine and grow wasm memory by at most 1.1 times the columns' bytes. Note: (v0.3.0)
- REQ-DATA-120: The element MUST accept, as the `data` property or attribute, the data object, the columns shorthand (`{"t": […], "close": […]}`) and the rows shorthand (`[{…}, …]`); the shell MUST parse JSON with `JSON.parse` and infer shorthand types as the schema says. [ADR-104](adr/104-typed-columns-cross-the-boundary.md) Note: (v0.3.0)

- REQ-DATA-100: `unit`, `scale` and `offset` MUST apply to ticks, readouts and the data table and MUST NOT change stored values: a readout of stored 18240 with `scale: 0.01` shows `182.40`. Note: (v0.5.0)

### 4.2 Streaming

`append` concatenates numeric columns, dropping late rows and replacing an
equal x; the ring buffer comes in v0.6.0.

- REQ-STREAM-060: After `data` is replaced, a chart's frame MUST equal the frame of a new chart given the same data, options, view and `t`. Note: (v0.3.0)

### 4.3 Downsampling

`downsample` keeps M4 aggregates (first, last, least, greatest) per dot column
of the rows in view, found by binary search and cached by data version, x
range and width; the line chart reuses the buffers between frames.

- REQ-SAMPLE-010: Line-like charts (line, step, area, band, sparkline) MUST draw from M4 aggregates (first, last, minimum, maximum per dot column) of the rows in view whenever those rows outnumber four times the plot's dot columns, and for x-sorted data the frame MUST equal, cell for cell, the frame drawn from every segment. [ADR-108](adr/108-m4-per-dot-column.md) Note: (v0.3.0)
- REQ-SAMPLE-020: A line-like chart MUST find the rows in view by binary search on x, and MUST cache its aggregates by data version, view x range and width; a frame with all three unchanged MUST NOT read the data (a scan counter in the test build stays 0). Note: (v0.3.0)
- REQ-SAMPLE-030: M4 over 1,000,000 rows in view at 640 dot columns MUST take at most 8 ms (median of 7) in wasm in Node on the reference machine. Note: (v0.3.0)

- REQ-SAMPLE-040: Candlestick charts MUST draw each candle at least one cell wide: when the candles in view do not fit, they MUST be resampled to the smallest interval of the ladder 1 s, 5 s, 15 s, 1 min, 5 min, 15 min, 1 h, 4 h, 1 d, 1 w, 1 mo at which they fit, open the first row, high the maximum, low the minimum, close the last and volume the sum, buckets aligned to the interval in the column's zone. [ADR-109](adr/109-candles-resample-to-calendar-intervals.md) Note: (v0.5.0)
- REQ-SAMPLE-060: LTTB MAY be offered as `sample: "lttb"` and MUST NOT be any chart's default. Note: (v0.5.0)

---

## 5. Scales, ticks, formatting and layout

### 5.1 Scales

`scale::Scale` (linear and time) and `scale::resolve_domain`. JavaScript's
rounding, and V8's own `log10` (fdlibm), are copied where d3 depends on
them.

- REQ-SCALE-010: The core MUST provide linear and time scales that map a domain value to a continuous cell coordinate and back; `invert(map(v))` MUST equal `v` within 1e-9 of the larger magnitude of the domain's ends for finite `v` in the domain (relative to `v` alone it cannot hold near 0). [ADR-110](adr/110-scales-and-formats-ported-from-d3.md) Note: (v0.2.0)
- REQ-SCALE-020: A scale's domain MUST be `"data"`, `"view"`, a fixed `[a, b]` or `{"follow": seconds}`, and MUST be a pure function of the data, the options and the view state. Note: (v0.2.0)
- REQ-SCALE-030: `nice` MUST give the domain d3-scale 4.0.2's `linear().nice(count)` gives for every case of the captured fixture file, including leaving unchanged a domain that does not settle in ten rounds. Note: (v0.2.0)

- REQ-SCALE-040: The core MUST provide log scales (any base above 1) and band scales; a band scale MUST give every category the same whole number of cells. Note: (v0.5.0)
- REQ-SCALE-050: The core SHOULD provide a symlog scale whose ticks are 0 and ± powers of ten within the domain. Note: (v0.5.0)
- REQ-SCALE-060: The core MUST provide an index time scale on which bars sit at consecutive indices whatever the time between them, labelled from each bar's timestamp; a fixture with weekend gaps MUST draw no empty columns. [ADR-131](adr/131-index-time-scale-for-session-gaps.md) Note: (v0.5.0)

### 5.2 Ticks

`ticks::ticks` and `time::utc_ticks` are d3's; `layout::fit_ticks` fits
them to an axis by lowering the count (ADR-111). `layout::fit_scale_ticks`
does the same over a built `Scale`, so its zone, log base, row timestamps
and category names reach the ticks, and places log and symlog marks by the
scale's transform.

- REQ-TICK-010: Linear ticks MUST equal d3-array 3.2.4's `ticks(a, b, count)` exactly for every case of the captured fixture file, natively and in wasm. Note: (v0.2.0)
- REQ-TICK-020: UTC time ticks MUST equal d3-time 3.1.0's `utcTicks(a, b, count)` exactly for every case of the captured fixture file. Note: (v0.2.0)
- REQ-TICK-030: For any axis from 4 to 400 cells and any finite domain, placed ticks MUST lie in the domain, increase strictly in value and never decrease in cell, and their labels MUST lie within the axis and be separated by at least the label gap (default two cells on x, one row on y). [ADR-111](adr/111-ticks-fit-by-lowering-the-count.md) Note: (v0.2.0)
- REQ-TICK-040: A tick mark MUST be drawn in the cell nearest its continuous position, and a gridline at the braille dot nearest it. Note: (v0.2.0)

- REQ-TICK-050: A y axis with `snap: "rows"` MUST place every tick on a row exactly with a nice step per row; it MUST be the default for bar and candlestick y axes under 20 rows. Note: (v0.5.0)
- REQ-TICK-060: Log axes MUST label only powers of the base and MAY mark other integer multiples without labels. Note: (v0.5.0)
- REQ-TICK-070: A time axis SHOULD draw a context row with the date at each day boundary when its span is under two days and the layout has the row. Note: (v0.5.0)

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

- REQ-FMT-060: The minus sign MUST be U+2212 in the browser and `-` in the CLI by default, switchable in the locale pack; a group separator given as U+2009, U+202F or U+00A0 MUST be drawn as one plain space cell. Note: (v0.3.0)
- REQ-I18N-010: The element's locale MUST be its `locale` attribute, else the `lang` of its nearest ancestor that has one, else `navigator.language`. Note: (v0.3.0)
- REQ-I18N-020: The shell MUST build a locale pack (decimal and group separators, grouping sizes, minus sign, percent pattern, the ten digits, month and weekday names, date and time patterns, direction) from `Intl` with the Gregorian calendar forced, once per locale, and pass it to wasm; no wasm module MUST link a locale database. Note: (v0.3.0)
- REQ-I18N-050: The shell MUST NFC-normalise label text before it crosses into wasm; zero-width code points MUST take no cell; a grapheme cluster of several scalars MUST be drawn as its first scalar; the accessible name, summary and table MUST keep the original text. Note: (v0.3.0)

- REQ-FMT-040: The core MUST format SI units, IEC bytes (`KiB`, `MiB`), bits a second (`kb/s`, `Mb/s`), durations from nanoseconds to days, prices to a tick size (0.01, 0.25, 1/32) and signed percentages, each covered by a table test. Note: (v0.5.0)
- REQ-FMT-050: Date labels MUST accept the strftime subset `%Y %m %d %e %H %I %M %S %L %p %a %A %b %B %j %Z`, and MUST default to 24-hour multi-scale labels unless the locale's hour cycle is 12-hour. Note: (v0.5.0)
- REQ-TZ-020: The element MUST accept `timeZone` as `UTC`, a fixed offset or an IANA name, and MUST give the core a table of offset transitions covering the data's time extent and the view, computed with `Intl.DateTimeFormat`. Note: (v0.5.0)
- REQ-TZ-030: Local-time ticks over the table MUST equal d3-scale's `scaleTime().ticks()` run under `TZ=<zone>` for every captured case, across daylight-saving changes. Note: (v0.5.0)
- REQ-TZ-050: The element MUST default its time zone to the page's (`Intl.DateTimeFormat().resolvedOptions().timeZone`), resolved to an IANA name and passed to the core explicitly; the core, the CLI and the tests MUST default to UTC. [ADR-112](adr/112-zones-and-locales-from-the-shell.md) Note: (v0.5.0)

### 5.4 Layout, text, axes and annotations

`layout::layout` places the title, legend, panes and axes in cells;
`text::Frame` draws a frame in stages whose order the types enforce.

- REQ-LAYOUT-010: Layout MUST be computed in whole cells in the order title, legend, x axis, panes, y gutters, x ticks, annotations, and MUST be a pure function of cols, rows, options, the data's extent and the view. Note: (v0.2.0)
- REQ-LAYOUT-020: When cols < 16 or rows < 5 a chart MUST draw no axis, title or legend; it MUST draw no title below 10 rows and no x labels below 6 rows. Note: (v0.2.0)
- REQ-LAYOUT-030: Y labels MUST be dropped, the right axis first, when the gutters would take more than a third of the columns. Note: (v0.2.0)
- REQ-TEXT-010: A label MUST occupy whole cells; no two labels MUST share a cell; a label MUST clear the cells under it to the ground before it is drawn, so no label shares a cell with data; the contract MUST check this on every fixture at 80 × 24 and 320 × 120. Note: (v0.2.0)
- REQ-TEXT-020: Draw order MUST be the z-order: data, gridlines on empty cells only, axes, labels, annotations, the crosshair, the readout last. Note: (v0.2.0)

- REQ-LAYOUT-040: Every chart MUST draw a frame without panicking at every size from 1 × 1 to 320 × 120 with every fixture it accepts. Note: (v0.3.0)
- REQ-LAYOUT-070: Each chart's meta MUST declare its minimum useful size with axes; the contract and the site MUST use it. Note: (v0.3.0)

- REQ-LAYOUT-050: Panes MUST split rows by weight with at least three rows each, share the x scale, and align their plot areas to the same gutters. Note: (v0.5.0)
- REQ-LAYOUT-060: A legend at `position: "auto"` MUST go outside in one row when it fits and rows ≥ 12, else inside the first pane when it takes at most a third of its width and height, else be left out of the frame (it stays in the table and the events). Note: (v0.5.0)
- REQ-AXIS-010: A pane MUST support a left and a right y axis bound to different scales, with gridlines from one axis only. Note: (v0.5.0)
- REQ-ANNOT-010: The core MUST draw `hline`, `vline`, `band`, `label` and `marker` annotations in data coordinates, clipped to their pane; an annotation label on an axis MUST hide the tick labels it overlaps. Note: (v0.5.0)

---

## 6. The chart contract and the registry

### 6.1 Meta and options

`chart::Meta` serialises to meta.schema.json; `options::Options` layers
overrides on `META.options` and records every key a chart reads.

- REQ-CHART-010: A chart family MUST be one folder `crates/glyphplot/src/charts/<family>/` holding, for each chart type it plays, a `pub static META: Meta` and a `make`, and an implementation of the `Chart` trait; one `REGISTRY` line per type in the folder's `mod.rs`; one Cargo feature per family. [ADR-101](adr/101-chart-families-with-modes.md) Note: (v0.3.0)
- REQ-CHART-020: `Meta` MUST have the fields of [meta.schema.json](schemas/meta.schema.json) (slug, name, note, family, modes, shapes, options, min, drop, mono, budget) and satisfy its rules. Note: (v0.3.0)
- REQ-CHART-030: A variant within a family MUST be chosen by the `mode` option, which `META.modes` lists; a mode not listed MUST fail with an error naming the valid modes. Note: (v0.3.0)
- REQ-OPT-010: Options MUST be a JSON object layered on the defaults in `META.options`: a key present in the overrides wins and every other default fills in; an empty or blank string MUST count as no options, and invalid JSON or JSON that is not an object MUST fail with `options are not a JSON object: <message>`; typed getters MUST fall back to the default on a missing key or a wrong type, a list with one wrong element falling back whole; every key a chart reads MUST have a default in `META.options`, and the options MUST record each key read so the contract can check it. Note: (v0.3.0)
- REQ-OPT-020: `set_options` MUST change a live chart's options and keep its data and view; only a change of chart type makes a new chart. [ADR-120](adr/120-options-change-keeps-the-chart.md) Note: (v0.3.0)

### 6.2 The `Chart` trait

`chart::Chart` has the plan's methods; `Env` carries paper, mono, the locale
pack and the zone table. A `ColorCanvas` cell takes the colour of the series
highest in z-order.

- REQ-CHART-040: The `Chart` trait MUST have exactly the methods of the plan's [`Chart` trait](../docs/IMPLEMENTATION_PLAN.md#the-chart-contract) (`set_data`, `append`, `set_options`, `view`, `set_view`, `input`, `frame`, `hit`, `table`), and `frame` MUST take its size from the grid it is given. Note: (v0.3.0)
- REQ-CHART-050: `Env` MUST carry `paper`, `mono`, the locale pack and the zone table, and nothing else from the host. Note: (v0.3.0)
- REQ-CHART-060: When two series draw into one cell, the cell MUST take the colour of the series highest in z-order: the later in the series list, unless the view raises a hovered or selected series. Note: (v0.3.0)

### 6.3 Registry

`registry::make` and `make_with`; each family's `REGISTRY` is written by hand.

- REQ-REG-010: Each family's `mod.rs` MUST hold a hand-written `REGISTRY`, and `charts/mod.rs` a `FAMILIES` list behind each family's feature; `registry::make(type, options_json)` MUST return a fresh chart or fail with `there is no chart named "<type>"` or `options are not a JSON object: <message>`. [ADR-125](adr/125-hand-written-registry.md) Note: (v0.3.0)
- REQ-REG-020: No chart type MUST be named `render`, `demo` or `list` (the registry test checks). Note: (v0.3.0)

### 6.4 The contract test

`tests/contract.rs` checks rules 1–9 and 29 over every fixture, mode, env,
size in S and time, and draws every size from 1 × 1 to 320 × 120 with the
small fixtures and a seeded sample with the large ones. It runs in about 6 s
for `line`.

- REQ-CONTRACT-010: `crates/glyphplot/tests/contract.rs` MUST check every registered chart against rules 1–9 and 29 of the plan's [contract rules](../docs/IMPLEMENTATION_PLAN.md#contract-rules) table (meta, options, shape, no panic, charset, colours, first frame, determinism, out-of-order calls, snapshots), over every fixture whose shape the chart accepts, at 1 × 1, `META.min`, 40 × 12, 80 × 24 and 320 × 120, with no test of the chart's own. Note: (v0.3.0)
- REQ-CONTRACT-020: The contract MUST report every failing chart in one run, spread charts over threads, print a per-chart table with `--nocapture`, and accept `CHART=<type>[,<type>…]`, failing on an unknown type. Note: (v0.3.0)

From v0.4.0 the contract checks rules 1–13 and 15–29; rule 14 (streaming)
comes with v0.6.0, and rule 25 is skipped, saying why, for a chart with no
log scale. Events and views are validated in Rust against their schemas.
The `contract-selftest` feature registers three broken charts the contract
must reject. The whole contract binary runs in about 5.6 s for `line`.

- REQ-CONTRACT-030: The contract MUST also check rules 10–13, 15–28 of the table: one ink and series told apart in it, paper, resize, replace is whole, view round trip, scripted input, events against the schema, hits naming what was drawn, hit and readout agreeing, empty and edge fixtures, gaps, tick labels, log-invalid values, unsorted and duplicate x, exact downsampling and wrong shapes. Note: (v0.4.0)
- REQ-CONTRACT-050: Time budgets (rule 30) MUST be checked in `tests/budgets.rs`, a test binary of its own, only without debug assertions; a chart over budget MUST be timed twice more, and the lowest average and the lowest worst count. [ADR-126](adr/126-correctness-and-timing-apart.md) Note: (v0.4.0)
- REQ-CONTRACT-060: A `contract-selftest` feature MUST register three broken charts (non-deterministic, outside its charset, keeping state across appends), and a test MUST assert that the contract rejects each with the matching message. Note: (v0.4.0)

### 6.5 Fixtures, snapshots, references and properties

The properties run at proptest's default 256 cases in `check` and passed at
4,096 on 2026-10-18.

- REQ-FIX-010: Fixture datasets MUST be generated by `crates/glyphplot/src/fixtures.rs` from SplitMix64 with a fixed seed each, MUST include the 23 fixtures of the plan's [fixtures](../docs/IMPLEMENTATION_PLAN.md#fixtures) table, and two generations MUST be bitwise equal on every target. [ADR-128](adr/128-fixtures-generated-from-seeds.md) Note: (v0.2.0)
- REQ-REF-010: Each port of d3-array, d3-scale, d3-time, d3-format or the LTTB reference MUST be compared bitwise with output captured from the pinned original by `mise run fixtures`, in `tests/reference.rs` and in `web/test/reference.test.mjs`. Note: (v0.2.0)
- REQ-PROP-010: The core MUST hold proptest properties (lean features) for scale inversion, tick order and count, reduction bounds, merged reductions equalling the whole, LTTB's ends and order, cell-mapping agreement and label non-overlap, with regression files checked in. [ADR-130](adr/130-proptest.md) Note: (v0.2.0)

- REQ-SNAP-010: Each chart MUST have one snapshot file a fixture it accepts, `crates/glyphplot/tests/snapshots/<type>/<fixture>.snap`, holding whole frames at 80 × 24, at `META.min` and after a seeded zoom, and the 320 × 120 frame as an FNV-1a hash; non-default modes on `sine-1k` get `<mode>-sine-1k.snap`. Note: (v0.3.0)
- REQ-SNAP-020: A snapshot's colour and back layers MUST write each palette index as two lowercase hex digits (`..` for `NO_BACK`), each run followed by its length when more than one. [ADR-129](adr/129-snapshot-colours-in-hex.md) Note: (v0.3.0)
- REQ-SNAP-030: `mise run snap [type …]` MUST rewrite snapshots; a chart without one MUST fail the contract; `web/test/snapshot.test.mjs` MUST compare every chart's wasm frame for every fixture at 80 × 24 with the native snapshot. Note: (v0.3.0)
- REQ-SNAP-040: A commit that changes a snapshot MUST say why in its message. Note: (v0.3.0)

- REQ-FIX-020: Real datasets checked in under `crates/glyphplot/tests/data/` MUST each be under 100 KB, public domain or MIT-compatible, named with source and licence in that directory's README. Note: (v0.5.0)

---

## 7. Charts

### 7.1 Line and sparkline

`charts/line/` plays `line` with modes `line`, `step` (`before`, `after`,
`middle`) and `area`. Its channels come by `encode`, role or name, else the
first timestamp column, else the first numeric column, so `{"t": […], "close":
[…]}` draws with no configuration. Small plots keep the data domain when it
fills under 0.75 of the niced one. The readout names x and the series in the
axis's format with digits enough to tell neighbouring data apart. Natively a
frame over 1k of 1M points takes 0.034 ms at 80 × 24 against 2.47 ms for all
1M (contract profile, medians).

From v0.5.0 the folder also plays `sparkline` (`spark.rs`, REQ-SPARK-010 in
the roadmap): modes `line`, `area` and `column`, a band of rows a series and
no axes, a text column of each series' label, latest value and extent, and
a ring of `retain.rows` rows (1,000,000 by default; the option was `capacity`
until v0.6.0 shared the core's ring, `ring.rs`). The line chart ticks and
labels time in `Env.zone`'s table, draws the date context row under a time
span of under two days, and writes ticks, readouts and table cells under a
column's `scale`, `offset` and `unit`.

- REQ-LINE-010: The `line` type MUST draw the modes `line`, `step` (`before`, `after`, `middle`) and `area` (to a baseline), each with snapshots. Note: (v0.3.0)
- REQ-LINE-020: A frame over 1,000 visible points of 1,000,000 held MUST take under a tenth of the time of a frame over all 1,000,000. Note: (v0.3.0)
- REQ-LINE-030: In `line`, `step` and `area`, a missing y MUST break the line with no segment drawn across it; the option `spanGaps: true` MAY join across it. Note: (v0.3.0)
- REQ-LINE-040: The readout of a line-like chart MUST name the datum whose x is nearest the cursor's x value, not the minimum or maximum of the dot column under it. Note: (v0.3.0)

- REQ-SPARK-010: The `sparkline` type, played by the `line` module, MUST draw without axes, one series a row band, MUST take appended samples into a ring of set capacity, and MAY show each series' label, latest value, minimum and maximum. Note: (v0.5.0)

### 7.2 Candles

`charts/candles/` plays `candles` in its own module (194,402 B gzipped):
modes `candle`, `hollow` and `ohlc`; glyphs chosen by candle spacing;
calendar resampling up the ladder (with 3 mo and years past 1 mo); a volume
pane on the index time scale; a crosshair that snaps to open, high, low or
close. A table of one value column draws its closes as candles. Natively a
frame over 1M rows takes 0.18 ms at 320 × 120 (`test:budgets`).

- REQ-CANDLE-010: The `candles` type MUST draw the modes `candle`, `hollow` and `ohlc`. Note: (v0.5.0)
- REQ-CANDLE-020: It MUST choose glyphs by candle spacing in cells: box-drawing candles at 4 cells or more; braille wick and body with a blank cell between at 2 or 3; braille without the gap at 1; snapshots at spacings 4, 2 and 1. Note: (v0.5.0)
- REQ-CANDLE-030: In one ink it MUST draw rising candles hollow and falling candles solid, and MUST NOT draw candles narrower than four cells, resampling instead (REQ-SAMPLE-040). Note: (v0.5.0)
- REQ-CANDLE-040: It MUST draw volume in a second pane sharing the x axis when the data has a `volume` column, on the index time scale by default (REQ-SCALE-060). Note: (v0.5.0)
- REQ-CANDLE-050: Its crosshair MUST snap to the nearest of open, high, low and close, and its readout MUST show `o h l c`, volume and the signed change. Note: (v0.5.0)

### 7.3 Bar

`charts/bar/` plays `bar` in its own module (178,564 B gzipped). Orientation
and stacking are options (`orient`, `stack`), not modes: they combine
freely, and modes stay for drawings that change the data (`histogram`,
`waterfall`, later). Several series not stacked are grouped. `signed: auto`
draws one series in the data in up and down. Natively a frame takes
0.35 ms at 320 × 120.

- REQ-BAR-010: The `bar` type MUST draw vertical and horizontal bars, grouped and stacked, with each bar's free end at an eighth of a cell (`▁`–`▇` vertical, `▏`–`▉` horizontal). Note: (v0.5.0)
- REQ-BAR-020: A bar chart MUST hit-test by band in O(1). Note: (v0.5.0)

### 7.4 Scatter and heatmap

Nothing is built.

### 7.5 Sky and orbit

Nothing is built.

---

## 8. View, input and interaction

### 8.1 View state

`view::View` and `ViewPatch` have view.schema.json's shape; a `motion` record
makes the frame at any `t` a function of the view.

- REQ-VIEW-010: Every chart MUST hold its interactive state in one view state of the shape of [view.schema.json](schemas/view.schema.json), and its frame MUST be a function of data, options, view state and `t` only: two charts given the same four MUST draw identical grids. [ADR-009](adr/009-deterministic-frames.md), [ADR-113](adr/113-view-state-is-the-model.md) Note: (v0.3.0)
- REQ-VIEW-020: Every domain in the view state MUST be in data units, and `null` MUST mean the chart's fitted default. Note: (v0.3.0)
- REQ-VIEW-030: The element MUST expose `view` as a property (get: the current view; set: a whole or partial view, merged, a key set to `null` returning to its default) and as an attribute of JSON text; setting `JSON.parse(JSON.stringify(el.view))` back MUST draw the same frame. Note: (v0.3.0)
- REQ-VIEW-040: Setting `view`, `data`, `options` or `time` from script MUST NOT fire `plot-hover`, `plot-select`, `plot-brush`, `plot-view`, `plot-legend` or `plot-cursor`. Note: (v0.3.0)
- REQ-VIEW-050: A view animation (an eased zoom, a kinetic pan, a reset) MUST be a `motion` record in the view state stamped with play time, so the frame at any `t` is determined without replaying input; with reduced motion and no `motion` attribute there MUST be none. Note: (v0.3.0)

### 8.2 Input

`input::Input::decode` reads the plan's numeric encoding; `reduce` folds input
into the view. Between the events of one gesture the reducer keeps a `Gesture`
(a drag's start, an open wheel zoom, a keyboard brush's anchor).

- REQ-INPUT-010: The core MUST fold input into view state with one reducer, `Chart::input`, used by the wasm module and the CLI alike; it MUST read no clock and keep no state outside the view and the chart's caches. [ADR-113](adr/113-view-state-is-the-model.md) Note: (v0.3.0)
- REQ-INPUT-020: Input events MUST have the shape of [input.schema.json](schemas/input.schema.json), positions in cells with fractions inside a cell; the wasm boundary MUST take them as a numeric call returning a bitmask (overlay changed, base changed, events pending). [ADR-115](adr/115-numeric-input-calls.md) Note: (v0.3.0)
- REQ-INPUT-030: The shell MUST send at most one `move` per animation frame (the latest) and MUST send `down`, `up`, `leave`, `tap`, `key` and summed `wheel` deltas, in order, before it. Note: (v0.3.0)
- REQ-INPUT-040: By default a mouse drag MUST pan, Shift+drag MUST brush, Ctrl or ⌘ with the wheel and a trackpad pinch MUST zoom about the pointer, the plain wheel MUST scroll the page, and a double-click MUST reset the view; the options `drag` (`pan`, `brush`, `zoom`) and `wheel` (`scroll`, `zoom`) MUST change the first and the fourth. [ADR-116](adr/116-input-defaults.md) Note: (v0.3.0)

### 8.3 Hit testing and the overlay

A hover redraws only the overlay over a copy of the cached base: 0.010 ms at
320 × 120 over 1M points natively.

- REQ-HIT-010: For x-ordered charts the cursor MUST snap to the datum nearest the pointer's dot column by binary search on x, and for binned charts to the bin under the pointer; tests MUST check both against a brute-force answer over the fixtures. [ADR-117](adr/117-hit-testing-by-chart-kind.md) Note: (v0.3.0)
- REQ-HIT-020: When a dot column holds more than one datum, `plot-hover` MUST carry the column's `aggregate` (count, first, last, minimum, maximum). Note: (v0.3.0)
- REQ-OVERLAY-010: A frame MUST be a cached base (data, axes, legend) and an overlay (crosshair, readout, brush, time cursor, focus) composed in wasm into the grid; a hover that does not change the view MUST NOT redraw the base, and MUST cost under 0.1 ms median natively at 320 × 120 with 1,000,000 points. [ADR-114](adr/114-base-and-overlay.md) Note: (v0.3.0)
- REQ-OVERLAY-020: The readout MUST stay at a fixed place in the chart, not under the pointer, until the pointer or focus leaves, and MUST hide on Escape. Note: (v0.3.0)

### 8.4 Events out and linking

- REQ-EVENT-010: The element MUST dispatch `plot-ready`, `plot-error`, `plot-hover`, `plot-select`, `plot-view`, `plot-cursor` (v0.3.0), `plot-brush`, `plot-legend` (v0.5.0) and `plot-data` (v0.6.0) as bubbling, composed `CustomEvent`s whose `detail` has the shape of [events.schema.json](schemas/events.schema.json); each reader-action event MUST carry `source` (`pointer`, `touch`, `keyboard`, `wheel`). Note: (v0.3.0)
- REQ-EVENT-020: `plot-hover` MUST fire only when the hovered datum changes, at most once an animation frame, with `detail` `null` when the cursor leaves the plot. Note: (v0.3.0)
- REQ-EVENT-030: `plot-view` MUST fire at most once an animation frame during a gesture with `final: false`, and exactly once with `final: true` when the gesture ends (pointer up, wheel idle 150 ms, key up, motion done). Note: (v0.3.0)

- REQ-LINK-010: Charts sharing a `link` attribute value MUST share `x.domain` and `cursor.x` in data units without event loops; a browser test MUST show two linked charts of different widths agreeing after a pan. Note: (v0.5.0)

### 8.5 Keyboard and touch

- REQ-KEY-010: Every interaction (cursor, pan, zoom, reset, brush, select, series toggle, pause) MUST be reachable by the keys of the plan's [keys](../docs/IMPLEMENTATION_PLAN.md#keys) table, acting only while the chart has focus. Note: (v0.3.0)
- REQ-TOUCH-010: On touch, a one-finger horizontal drag MUST move the cursor (scrub), two fingers MUST pan until their spread changes by more than 24 CSS px or 10 % of its start, when they MUST pinch, zooming about their centre, a double-tap MUST reset, a long press MUST start a brush, and a vertical one-finger drag MUST scroll the page (`touch-action: pan-y`). Note: (v0.3.0)
- REQ-TOUCH-020: The shell MUST take touch from Pointer Events only. Note: (v0.3.0)

---

## 9. The wasm boundary and the manifest

### 9.1 The `Chart` object

`crates/wasm` exports the plan's members, matching `web/src/api.ts`. Data
crosses in buffers the chart reserves; us and ns timestamps can carry an i64
`origin`. In Node 26, replacing two 1M columns takes a median 1.58 ms and an
M4 frame over 1M rows 3.4–4.9 ms.

- REQ-WASM-010: The wasm boundary MUST expose exactly the members of the plan's [boundary](../docs/IMPLEMENTATION_PLAN.md#the-boundary) table: a `Chart` object (made from a type and options, with data, options, view, input, resize, frame, the grid's pointers, text, table and free) and the module functions `charts_json`, `wide_tail` and `resolve_palette`. Note: (v0.3.0)
- REQ-WASM-020: Making a chart MUST throw `there is no chart named "<type>"` for a type the module does not play and `options are not a JSON object: …` for bad options, and the module MUST keep working after a throw. Note: (v0.3.0)
- REQ-WASM-030: The memory views at the grid's pointers MUST equal `text()` and the grid after each `frame`; the shell MUST re-create its views after any call that may allocate. Note: (v0.3.0)
- REQ-WASM-050: `free()` MUST give a chart's memory back, so freeing a chart and making another of the same capacity grows no memory. Note: (v0.3.0)

### 9.2 Panics

Release panics trap; the shell restores the shadow stack after a trap and
refuses further calls on that chart.

- REQ-WASM-040: A panicking call MUST trap without taking down the instance: other charts keep drawing and new ones can be made; the panicked chart MUST then refuse every call. [ADR-124](adr/124-abort-on-panic.md) Note: (v0.3.0)
- REQ-WASM-060: With the `debug` feature only, a panic MUST print its message and location with `console.error` before it traps, making the chart `debug-panic` MUST be possible, and the module MUST export `debug_reference(file, line)` and `debug_fixture_digest(name)` for the reference tests; a release build MUST have none of them. Note: (v0.3.0)

### 9.3 Manifest and loading

`web/pkg/modules.js` carries charts, modules, imports, wasm, fallback and
budgets; the debug build maps every type to `all`.

- REQ-MANIFEST-010: `web/pkg/modules.js` MUST export `charts` (every chart's meta, in registry order), `modules` (type → module), `imports` (module → a function importing `./<module>/glyphplot.js`), `wasm` (module → a function returning `new URL("./<module>/glyphplot_bg.wasm", import.meta.url)`, written as that literal so a bundler emits the file and rewrites the URL), `fallback` and `budgets` (module → `{gzip, frameMs, worstMs, inputMs}`), of the shape of [manifest.schema.json](schemas/manifest.schema.json); the same budgets MUST be in `web/pkg/budgets.json` and the gzipped sizes in `web/pkg/sizes.json` ([sizes.schema.json](schemas/sizes.schema.json)). Note: (v0.3.0)
- REQ-MANIFEST-020: In the release build every type MUST map to the module of its family, which plays exactly the types the manifest gives it, and `fallback` MUST be `null`; the debug build MUST map every type to `all`. Note: (v0.3.0)
- REQ-LOAD-010: `chart(type, init)` MUST return its `Chart` at once (so calls can queue, REQ-LOAD-030), load the type's module at most once a page, share it with every later caller, reject its `ready` with `there is no chart named "<type>"` for a type no module plays, and try a failed load again on the next call. Note: (v0.3.0)
- REQ-LOAD-020: The element MUST fetch no wasm module or glue (the manifest ships in the shell, changed in v0.4.0) until it first comes within one screen height of the viewport (IntersectionObserver, `rootMargin: "100% 0px"`), unless `loading="eager"` or there is no IntersectionObserver. Note: (v0.3.0)
- REQ-LOAD-030: A `data`, `options` or `view` set and every `append` made before the chart exists MUST be applied in order once it is made; a `data` set MUST discard the appends queued before it. Note: (v0.3.0)

---

## 10. The element, the shell and rendering

### 10.1 The element

`<glyph-plot>` (web/src/element.ts) in light DOM, loaded lazily, its events
rate-limited.

- REQ-EL-010: Importing `glyphplot/element` MUST define `<glyph-plot>` once, inject one `<style>` with the bundled `@font-face` and the element's rules (wrapped in `:where()`), and do nothing where there is no DOM or no `customElements`. [ADR-118](adr/118-one-tag-with-a-type.md) Note: (v0.3.0)
- REQ-EL-020: The element MUST observe exactly the attributes of the plan's [element attributes](../docs/IMPLEMENTATION_PLAN.md#element-attributes) table. Note: (v0.3.0)
- REQ-EL-040: A change of `options` or `theme` MUST NOT make a new chart: data and view after it MUST equal those before. Note: (v0.3.0)
- REQ-EL-060: The `data`, `options` and `view` properties MUST be defined on the element's class, so `"data" in document.createElement("glyph-plot")` is true before upgrade. Note: (v0.3.0)
- REQ-EL-070: Setting the `data` property MUST replace all data and MUST NOT write the `data` attribute; getting it MUST return the descriptor and lengths, not a copy of the data. Note: (v0.3.0)
- REQ-EL-080: Without `cols` and `rows`, the cell MUST be `round(0.6 × font-size × devicePixelRatio)` device pixels wide and exactly twice that tall, `cols × rows` the whole cells that fit the content box, and the remainder split evenly as padding. [ADR-103](adr/103-cell-from-font-grid-from-box.md) Note: (v0.3.0)
- REQ-EL-085: The element MUST NOT draw a chart in a font smaller than 11 CSS px at a device pixel ratio under 2, or 9 CSS px at 2 or more; a smaller computed `font-size` MUST be raised to the floor before the cell is computed. Note: (v0.3.0)
- REQ-EL-090: A change of the element's box MUST lay the chart out again within one animation frame, keeping its view in data units; a box of width 0 MUST be ignored. Note: (v0.3.0)
- REQ-EL-100: An element with no height of its own MUST take its height from `rows`, default 12. Note: (v0.3.0)
- REQ-EL-120: The element MUST fire `plot-ready` once a chart, after its first frame, and resolve `ready` then; a failed load or first frame MUST fire `plot-error` with the message and keep the element's children. Note: (v0.3.0)
- REQ-EL-130: The element MUST use light DOM; whatever it holds before its chart's first frame (a server-rendered still) MUST stay until the drawn element replaces it. Note: (v0.3.0)
- REQ-EL-140: Disconnecting MUST stop drawing and free the chart; a load superseded by a later change or by removal MUST free what it made and draw nothing. Note: (v0.3.0)
- REQ-EL-150: The element MUST NOT be form-associated. Note: (v0.3.0)

- REQ-EL-030: `define(tag, { type })` MUST define `tag` as a subclass whose chart type is `type` whatever its `type` attribute says, and MUST do nothing for a tag already defined. Note: (v0.5.0)
- REQ-EL-050: A change of `type` MUST make a new chart and give it the old chart's data and view when its meta accepts the data's shape; otherwise it MUST warn once naming the shape and draw the new chart empty. Note: (v0.5.0)
- REQ-EL-110: With `cols` and `rows`, the grid MUST be that size, and `fit` MUST scale its drawing: `width` to the element's content width W (a cell W / cols wide), `contain` to the largest box inside W × H that keeps the cell's 1:2 aspect, centred, acting as `width` when H is 0; any other value MUST act as no fit. Note: (v0.5.0)

### 10.2 Mount and the JS API

`mount` draws changed cells into a `<pre>` or a canvas and turns Pointer
Events into the input encoding, at most one move a frame.

- REQ-MOUNT-010: `mount(el, chart, { input })` MUST draw into a `<pre>` (text) or a `<canvas>` according to the element given, and with `input` MUST turn pointer, wheel, touch and key events on `el` into `chart.input` calls and dispatch the events of REQ-EVENT-010 on `el`; `stop()` MUST remove every listener and observer and cancel the frame loop. Note: (v0.3.0)
- REQ-MOUNT-020: A chart MUST be drawn only while it intersects the viewport and the document is visible; `t` MUST advance only while a `motion` record is in flight. Note: (v0.3.0)
- REQ-JS-010: `chart(type, init)` and the returned `Chart`'s `data`, `append`, `options`, `view`, `input`, `frame`, `resize`, `text`, `table` and `free` MUST work with no DOM; a Node test MUST draw a frame and compare its text with the Rust crate's. Note: (v0.3.0)

### 10.3 Rendering

Both canvas renderers are built behind a flag, fillText by default. Headless
Chromium 156 (`mise run bench:render -- --headless`, medians): a coloured
braille scatter at 320 × 120 takes 30.9 ms with fillText and 61.5 ms with the
atlas; the owner's Chrome and Safari measurement decides (REQ-RENDER-050).

- REQ-RENDER-010: On a canvas, a frame's draw cost MUST NOT depend on the number of distinct (glyph, colour) pairs: a braille scatter of 320 × 120 cells with every cell a different colour MUST draw within twice the time of the same frame in one colour in headless Chrome (`mise run bench:render`). [ADR-119](adr/119-no-glyph-and-colour-atlas.md) Note: (v0.3.0)
- REQ-RENDER-020: Block, quadrant, sextant, octant and wedge glyphs MUST be filled as exact shapes (`blocks.ts`), never drawn from the font. Note: (v0.3.0)
- REQ-RENDER-030: A frame MUST redraw only the cells whose glyph, ink or back changed since the last frame drawn; a crosshair move over an unchanged chart MUST redraw at most the cells of the old and new crosshair and readout. Note: (v0.3.0)
- REQ-RENDER-050: The renderer (`fillText` per changed cell, or an atlas keyed by code point and tinted by compositing) MUST be the one the owner's Chrome and Safari measurements choose in v0.3.0, recorded in the plan with the numbers. Note: (v0.3.0)

- REQ-RENDER-040: A frame whose cells carry back colours SHOULD be drawn as one image of `cols × 2·rows` pixels scaled without smoothing, with other glyphs over it. Note: (v0.5.0)

### 10.4 Font

- REQ-FONT-030: `@font-face` MUST load the font from `new URL("../fonts/glyphplot-mono.woff2", import.meta.url)` with `font-display: swap`, and every surface (the `<pre>`, the canvas, the SVG export) MUST put `"glyphplot mono"` first in its font stack. Note: (v0.3.0)

### 10.5 Server-side rendering

Nothing is built.

---

## 11. Theme, one ink and accessibility

### 11.1 Theme

The element reads the `--glyphplot-*` custom properties into a theme and
resolves it through the module, repainting without a new frame.

- REQ-THEME-010: The element MUST read the custom properties `--glyphplot-ground`, `--glyphplot-ink`, `--glyphplot-muted`, `--glyphplot-rule`, `--glyphplot-accent`, `--glyphplot-series-1` to `--glyphplot-series-8`, `--glyphplot-up`, `--glyphplot-down` and `--glyphplot-scale` from its computed style, each falling back to the default set; a `theme` property set to a theme object ([theme.schema.json](schemas/theme.schema.json): the same tokens, `series` as an array of up to eight) MUST win over them. [ADR-122](adr/122-theme-tokens-and-custom-properties.md) Note: (v0.3.0)
- REQ-THEME-020: Without a ground, a chart MUST draw no ground and MUST use the light default set when its computed `color` has luma `0.2126 r + 0.7152 g + 0.0722 b < 128`, the dark set otherwise. Note: (v0.3.0)
- REQ-THEME-030: The element MUST re-resolve its theme and repaint when `prefers-color-scheme` changes, when `<html>`'s `class`, `style` or `data-theme` changes, and when `theme` is set, without reloading its module or drawing a new frame. Note: (v0.3.0)
- REQ-THEME-040: Every default series, `up`, `down` and `accent` colour MUST have a WCAG 2.2 contrast ratio of at least 3:1 against its default ground, and `ink` and `muted` at least 4.5:1. [ADR-123](adr/123-colour-defaults-held-to-measured-bars.md) Note: (v0.3.0)
- REQ-THEME-050: Each default categorical palette MUST hold eight colours whose smallest pairwise CIEDE2000 difference is at least 15 under normal vision and under deuteranopia, protanopia and tritanopia simulated by Machado 2009 at severity 1.0 on linear sRGB. Note: (v0.3.0)
- REQ-THEME-070: In a debug build, a page-set series colour under 3:1 against the resolved ground MUST log one warning naming the token and the ratio, and MUST still be used. Note: (v0.3.0)

- REQ-THEME-060: Up and down MUST be theme tokens, never fixed colours, and candles and bars MUST tell them apart by fill (hollow and solid) as well as by colour. Note: (v0.5.0)
- REQ-THEME-065: The default `up` and `down` tokens MUST be a blue and an orange at least 50 CIEDE2000 apart under normal vision and the three simulated deficiencies, overridable like every token. [ADR-123](adr/123-colour-defaults-held-to-measured-bars.md) Note: (v0.5.0)

### 11.2 One ink

- REQ-MONO-010: In one ink, line series 1–4 MUST draw with the dash patterns solid, 3 on 1 off, 1 on 1 off, and 3 on 1 off 1 on 1 off, counted in dot columns from the x scale's origin, so a pan by n whole dots shifts the pattern by n dots. [ADR-123](adr/123-colour-defaults-held-to-measured-bars.md) Note: (v0.3.0)
- REQ-MONO-040: The legend MUST draw each series' key beside its name as the series is drawn: a solid line in the series' colour, and in one ink (or with patterns on) its pattern, marker or fill. Note: (v0.3.0)

- REQ-MONO-020: In one ink, series 5–8 MUST repeat patterns 1–4 and add a marker each from `● ○ ■ □ ▲ △ ◆ ◇`, at most one marker a cell. Note: (v0.5.0)
- REQ-MONO-030: In one ink, bar and area series MUST fill with `█ ▓ ▒ ░` by series. Note: (v0.5.0)

### 11.3 Accessibility

The element is a figure with an img drawing, a slider cursor and a hidden
table of at most 500 rows; axe finds no serious or critical violation in light
or dark.

- REQ-A11Y-010: The element MUST have `role="figure"` and an accessible name from `label`, else `<chart type>: <series names>`; its drawing MUST have `role="img"` and an `aria-label` giving the chart type, the series names, each axis's title and visible domain in the resolved locale, rewritten at most once a second. [ADR-132](adr/132-figure-slider-and-hidden-table.md) Note: (v0.3.0)
- REQ-A11Y-020: The element MUST hold, in light DOM and visually hidden, a `<table>` with a caption, `th scope="col"` headers and a row header a row, of the data in the current view, at most 500 rows (downsampled, each bucket's minimum and maximum kept), its caption stating how many points the view holds when it holds more. Note: (v0.3.0)
- REQ-A11Y-030: An interactive chart MUST have exactly one tab stop, a `role="slider"` cursor whose `aria-valuetext` gives the cursor's x and each series' value there (up to four, then `and N more`), moved by the keys of the plan's [keys](../docs/IMPLEMENTATION_PLAN.md#keys) table, which `aria-keyshortcuts` MUST list; its `aria-valuemin` and `aria-valuemax` MUST be the x domain drawn, in data units. Note: (v0.3.0)
- REQ-A11Y-040: Every pan, zoom and brush done by dragging MUST also be possible with single clicks or taps: a control row (zoom out, zoom in, reset, step back, step on) shown on focus and hover, and click-click brushing. Note: (v0.3.0)
- REQ-A11Y-050: Under `prefers-reduced-motion: reduce` and without `motion`, view changes MUST jump with no animated transition. Note: (v0.3.0)
- REQ-A11Y-060: The focused chart MUST show a `:focus-visible` outline at least 2 px wide in `accent`, at least 3:1 against the ground. Note: (v0.3.0)
- REQ-A11Y-070: An axe-core scan of a page holding every chart, in light and dark, MUST find no violation of impact serious or critical. Note: (v0.3.0)

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
- REQ-BUILD-050: The release profile MUST be `opt-level = "z"` (changed from `"s"` in v0.4.0: 177,385 B gzipped against 191,283 B for the line module, frames within budget), LTO, one codegen unit, `panic = "abort"` and `strip`, and wasm-opt MUST run with `-Os --enable-bulk-memory --enable-nontrapping-float-to-int`. Note: (v0.1.0)
- REQ-BUILD-060: Generated output (`web/pkg`, `web/dist`, `site/dist`, `target`, the bench outputs under `discovery/bench`) MUST stay out of git; the bundled font is generated and checked in. Note: (v0.1.0)

- REQ-BUILD-070: `mise run test:rust` MUST run `cargo test --workspace --profile contract`, a profile inheriting `release` with LTO off, 16 codegen units and `opt-level = 3`. [ADR-127](adr/127-test-profile-without-lto.md) Note: (v0.2.0)

- REQ-BUILD-040: `mise run build:wasm:debug` MUST build one module, `all`, with every family and the `debug` feature, in its own cargo target dir, staged beside `web/pkg` and swapped in whole, so a failed build leaves the last good `web/pkg`. Note: (v0.3.0)
- REQ-BUILD-080: `mise run dev` MUST serve the debug build with the checklist page and rebuild the debug wasm when a file under `crates/` changes, reloading the page. Note: (v0.3.0)

- REQ-BUILD-090: `mise run check` SHOULD finish in under five minutes from a warm cache on the reference machine. Note: (v0.4.0)

### 16.2 Lint

`mise run lint` also checks each family alone (`cargo check -p glyphplot
--no-default-features --features <family>`), the core's dependencies
(REQ-ARCH-010), git dependencies (REQ-ARCH-070), unicel's repository
(REQ-INDEP-020) and tracked generated output (REQ-BUILD-060).

- REQ-LINT-010: `mise run lint` MUST run `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `RUSTDOCFLAGS='-D warnings' cargo doc --no-deps -p glyphplot`, and clippy on `glyphplot-wasm` for `wasm32-unknown-unknown` with and without `debug`. Note: (v0.1.0)
- REQ-LINT-030: Rust style MUST follow `rustfmt.toml` (`max_width = 160`, `use_small_heuristics = "Max"`), and every public item of every crate MUST have a doc comment (`#![warn(missing_docs)]`, rustdoc with `-D warnings`). Note: (v0.1.0)

- REQ-LINT-040: `clippy.toml` MUST list `f64::clamp` and `f32::clamp` under `disallowed-methods`, so a call fails `mise run lint`. Note: (v0.2.0)

- REQ-LINT-020: From the milestone each lands in, `mise run lint` MUST also run `tsc --noEmit` for `web` (v0.3.0) and `site` (v0.7.0) and `elm-format --validate` on `site/src` and `site/tests` (v0.7.0). Note: (v0.3.0)

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

`mise run size --charts` splits each module by owner from a build with
names (`scripts/size-charts.mjs`): the chart, the core, serde and
serde_json, the rest, and data. A function's gzipped share is its raw share
times the module's gzipped size. On 2026-10-20 the line module is 175,578 B
gzipped; JSON through serde derives is its largest part.

- REQ-SIZE-050: `mise run size --charts` MUST list, for each module, the gzipped share of the chart's own code, the core, serde and serde_json, and the rest (std, alloc, wasm-bindgen), from an unstripped build. Note: (v0.4.0)

### 16.4 Time and memory budgets

In Node 26 a 1M-point chart grew memory 16.2 MB against its 17.0 MB limit;
freeing and remaking grew 0 B.

- REQ-PERF-010: Downsampling MUST be on by default for every chart that draws more points than its plot has dot columns. Note: (v0.3.0)
- REQ-PERF-020: A chart MUST redraw only when its data, options, view, size or theme changed since the last frame, and at most once a display frame. Note: (v0.3.0)
- REQ-MEM-010: A chart of capacity n points in two f64 columns MUST hold at most 16 × n bytes + 1 MiB of wasm memory, measured as the growth of `memory.buffer.byteLength` in Node from making the chart to its first frame. Note: (v0.3.0)
- REQ-MEM-020: The element MUST load a typed-array column into wasm memory with at most one copy. Note: (v0.3.0)
- REQ-MEM-030: Replacing a chart's data with data of the same or smaller size, or freeing a chart and making another of the same capacity, MUST NOT grow wasm memory, except that a chart first framed after its first replace MAY grow by one column on its second (measured 8,060,928 B at 1,000,000 rows). Note: (v0.3.0)

Measured on 2026-10-20 for `line`:
- natively (`tests/budgets.rs`, contract profile), 1.80 ms a frame over 1M
  points at 320 × 120 and 0.010 ms a pointer move;
- in headless Chromium 156 (`mise run bench:web`), 4.14 ms a frame and
  0.055 ms a move;
- the first frame (`mise run bench:load`, median of 7 cold loads) takes
  796 ms on 170 ms / 9 Mbps and 3,022 ms on slow 4G. The shell ships the
  manifest and fetches the wasm alongside its glue, so a page needs no
  preload hints.

- REQ-PERF-030: In a release build, every chart MUST average at most 4 ms a frame with no frame over 16 ms, over 300 frames of a view panning across a fixture of 1,000,000 points at 80 × 24 and 320 × 120 with default options, measured natively by `tests/budgets.rs`. Note: (v0.4.0)
- REQ-PERF-040: In a release build, every chart MUST average at most 1 ms a frame with no frame over 8 ms over the same play with a fixture of 100,000 points. Note: (v0.4.0)
- REQ-PERF-050: A pointer move (hit test, crosshair and readout) MUST average at most 0.5 ms with none over 2 ms over 300 moves across a chart holding 1,000,000 points, natively in release and in wasm in headless Chromium. Note: (v0.4.0)
- REQ-PERF-060: In headless Chromium on the reference machine, every chart MUST average at most 8 ms a frame with no frame over 16 ms in the play of REQ-PERF-030, checked by `mise run bench:web` in `signoff`. Note: (v0.4.0)
- REQ-PERF-080: A page holding one chart MUST draw its first frame within 1.0 s of navigation start on a 170 ms, 9 Mbps profile, median of 7 cold loads in headless Chromium; `mise run bench:load` MUST also report, without gating, the time on Lighthouse 12's `mobileSlow4G` request-level profile (562.5 ms, 1.44 Mbps, CPU 4×); both run in `signoff`. [ADR-133](adr/133-a-350-kb-module-gate.md) Note: (v0.4.0)
- REQ-BENCH-010: `mise run bench [type …]` MUST print the load average, then a row per type, fixture size (1k, 100k, 1M) and grid size (80 × 24, 320 × 120) with average, worst, budget and `OVER`, and exit 1 when any row is over. Note: (v0.4.0)
- REQ-BENCH-020: `mise run bench:web`, `bench:load` and `bench:render` MUST run the discovery harnesses' successors in headless Chromium against the release build and print their tables with the load average. Note: (v0.4.0)

### 16.5 Local CI (signoff)

`mise run signoff [rev]` checks a commit (HEAD by default) out into
`target/ci/tree`, a detached git worktree removed afterwards, runs
`mise run check` there with a cargo cache in
`target/ci/target`, and logs to `target/ci/logs/<sha>.log`. It posts no
commit status yet; REQ-CI-020 adds that in v0.4.0.

- REQ-CI-010: There MUST be no hosted CI workflow; `mise run signoff` is CI ([ADR-003](adr/003-unicels-architecture.md)). Note: (v0.1.0)

From v0.4.0 `mise run signoff` checks each open pull request's head (or the
PRs named) in its own detached worktree and posts a `signoff` commit
status; a revision argument checks locally and posts nothing. A PR whose
tree has no `check` task is skipped.

- REQ-CI-020: `mise run signoff` MUST, for each open pull request (or those named), copy the head commit's tree into `target/ci/tree`, run `npm ci`, `lint`, `test`, `build`, `size`, the whole of `test:site`, `bench:web` and `bench:load` there, with `PROPTEST_CASES=4096`, log to `target/ci/logs/`, and post a `signoff` commit status; it MUST exit 1 when any pull request fails, after checking the rest. Note: (v0.4.0)

---

## 17. Testing

The tests are the core's unit tests and properties, `tests/reference.rs`
(the d3 captures), and the CLI's (`crates/cli/tests/usage.rs`): 112 on
2026-10-18.

- REQ-TEST-010: `mise run test` MUST pass before every commit, and `mise run check` MUST be green. Note: (v0.1.0)

- REQ-TEST-020: The Node tests (`web/test/*.test.mjs`) MUST run against the release modules in `web/pkg` and against a debug build, find modules with the shell's own compiled `moduleOf`, and run the panic tests against the debug build only. Note: (v0.3.0)
- REQ-TEST-030: Browser behaviour on real devices (pixels, fonts, touch, screen readers, Safari) MUST be checked by the owner; agents check headless and hand the manual checks over as a list. Note: (v0.3.0)
- REQ-TEST-040: `site/test.html` MUST be a plain HTML checklist page (no Elm) with one `<glyph-plot>` per rendering and interaction case and a button that copies the pass as Markdown. Note: (v0.3.0)
- REQ-E2E-010: `mise run test:site` MUST run Playwright in headless Chromium against `vite preview` of the release build of `site/`. Note: (v0.3.0)
- REQ-E2E-020: `site/e2e/interaction.spec.ts` MUST cover pointer hover with its event, drag pan, wheel zoom, keyboard cursor, pan, zoom, reset and escape, a brush, and touch scrub, pan and pinch driven through CDP `Input.dispatchTouchEvent`. Note: (v0.3.0)
- REQ-E2E-030: `site/e2e/element.spec.ts` MUST cover attributes and properties, `view` set and read, a module fetched once for many tags, calls queued before load, and teardown and freeing on attribute changes and removal. Note: (v0.3.0)
- REQ-E2E-040: `site/e2e/resize.spec.ts` MUST show that a change of container width changes `cols`, keeps the view in data units and logs no console error. Note: (v0.3.0)
- REQ-E2E-070: `mise run check` MUST run `test:site --smoke` (a chart's first frame, a hover, a drag, a wheel, a key and the axe scans, under 60 s); `signoff` MUST run all of `test:site`. Note: (v0.3.0)

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
