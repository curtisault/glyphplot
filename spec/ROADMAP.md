# glyphplot — Roadmap

Version 0.1 (draft) · 2026-10-18

This document holds every requirement of glyphplot not yet built, grouped by
area, each naming the milestone that ships it. When a milestone lands, its
requirements move into [SPEC.md](SPEC.md) with their IDs unchanged, a line
in their old section says where they went, and the milestone's row in §20
says when it was built. v0.1.0 to v0.7.0 have shipped. IDs go up in tens within an area so a requirement
can be added between two others (`REQ-DATA-045`); an ID is never reused,
even after its requirement is removed.

The words MUST, MUST NOT, SHOULD, SHOULD NOT and MAY are used as in RFC 2119.
The terms are defined in [SPEC.md](SPEC.md)'s glossary. Data shapes that cross
a boundary are JSON Schemas in [schemas/](schemas/). Decisions are recorded
in [adr/](adr/): 001–012 were made by the owner (Accepted), 100 and up by
discovery (Proposed). Measurements cited here are in `discovery/` and in
[docs/IMPLEMENTATION_PLAN.md](../docs/IMPLEMENTATION_PLAN.md), Appendix A;
"the reference machine" is the owner's Apple M5 Pro (18 cores, 48 GB,
macOS) on which they were taken.

Each requirement ends with its milestone in parentheses. Milestones are the
plan's phases, one each (§20).

## Contents

1. [Architecture and build](#1-architecture-and-build)
2. [Core: grid, glyphs, colour](#2-core-grid-glyphs-colour)
3. [Data](#3-data)
4. [Scales, ticks, formatting and layout](#4-scales-ticks-formatting-and-layout)
5. [View, input and interaction](#5-view-input-and-interaction)
6. [The chart contract and the registry](#6-the-chart-contract-and-the-registry)
7. [Charts](#7-charts)
8. [The wasm boundary and the manifest](#8-the-wasm-boundary-and-the-manifest)
9. [The element, the shell and rendering](#9-the-element-the-shell-and-rendering)
10. [Theme and one ink](#10-theme-and-one-ink)
11. [Accessibility](#11-accessibility)
12. [Budgets](#12-budgets)
13. [Testing](#13-testing)
14. [The CLI](#14-the-cli)
15. [The Rust crate and exports](#15-the-rust-crate-and-exports)
16. [The site and datasets](#16-the-site-and-datasets)
17. [Domain packs](#17-domain-packs)
18. [Long tail: sky and orbits](#18-long-tail-sky-and-orbits)
19. [Packaging](#19-packaging)
20. [Milestones](#20-milestones)
21. [Instructions for the implementing agent](#21-instructions-for-the-implementing-agent)

---

## 1. Architecture and build

### 1.1 Layers (ARCH)

- Shipped in v0.1.0 and moved to [SPEC.md](SPEC.md) §2.1: REQ-ARCH-010, REQ-ARCH-040, REQ-ARCH-060, REQ-ARCH-070.
- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §2.1: REQ-ARCH-020, REQ-ARCH-030, REQ-ARCH-050.
- REQ-ARCH-080: The CLI MUST link the core natively with every family and every pack, without wasm. (v0.8.0)

### 1.2 Independence (INDEP)

- Shipped in v0.1.0 and moved to [SPEC.md](SPEC.md) §3.5: REQ-INDEP-010, REQ-INDEP-020.

REQ-VENDOR-010 and REQ-VENDOR-020 were withdrawn on 2026-10-18 by ADR-013, before anything was built; their IDs are not reused.

### 1.3 Tasks (BUILD)

- Shipped in v0.1.0 and moved to [SPEC.md](SPEC.md) §16.1: REQ-BUILD-010, REQ-BUILD-020, REQ-BUILD-030, REQ-BUILD-050, REQ-BUILD-060.
- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §16.1: REQ-BUILD-040, REQ-BUILD-080.
- Shipped in v0.2.0 and moved to [SPEC.md](SPEC.md) §16.1: REQ-BUILD-070.
- Shipped in v0.4.0 and moved to [SPEC.md](SPEC.md) §16.1: REQ-BUILD-090.

### 1.4 Lint (LINT)

- Shipped in v0.1.0 and moved to [SPEC.md](SPEC.md) §16.2: REQ-LINT-010, REQ-LINT-030.
- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §16.2: REQ-LINT-020.
- Shipped in v0.2.0 and moved to [SPEC.md](SPEC.md) §16.2: REQ-LINT-040.

### 1.5 Local CI (CI)

- Shipped in v0.1.0 and moved to [SPEC.md](SPEC.md) §16.5: REQ-CI-010.
- Shipped in v0.4.0 and moved to [SPEC.md](SPEC.md) §16.5: REQ-CI-020.

---

## 2. Core: grid, glyphs, colour

### 2.1 Grid (GRID)

- Shipped in v0.2.0 and moved to [SPEC.md](SPEC.md) §3.1: REQ-GRID-010, REQ-GRID-020, REQ-GRID-030, REQ-GRID-040, REQ-GRID-050, REQ-GRID-060, REQ-GRID-070.

### 2.2 Sub-cell canvases and cell mapping (CANVAS)

- Shipped in v0.2.0 and moved to [SPEC.md](SPEC.md) §3.2: REQ-CANVAS-010, REQ-CANVAS-020, REQ-CANVAS-030.
- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §3.2: REQ-CANVAS-040.

### 2.3 Charsets (CHAR)

- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §3.3: REQ-CHAR-010, REQ-CHAR-020.

### 2.4 Colour (COLOR)

- Shipped in v0.2.0 and moved to [SPEC.md](SPEC.md) §3.4: REQ-COLOR-010, REQ-COLOR-020, REQ-COLOR-030, REQ-COLOR-040.
- REQ-COLOR-050: The CLI MUST map the resolved palette to xterm-256 or 16 colours once per palette for `--color 256` and `--color 16`, never per cell. (v0.8.0)

---

## 3. Data

### 3.1 The data contract (DATA)

- Shipped in v0.2.0 and moved to [SPEC.md](SPEC.md) §4.1: REQ-DATA-010, REQ-DATA-020, REQ-DATA-030, REQ-DATA-040, REQ-DATA-080, REQ-DATA-090, REQ-DATA-110, REQ-DATA-130.
- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §4.1: REQ-DATA-050, REQ-DATA-060, REQ-DATA-070, REQ-DATA-120.
- Shipped in v0.5.0 and moved to [SPEC.md](SPEC.md) §4.1: REQ-DATA-100.
- Shipped in v0.6.0 and moved to [SPEC.md](SPEC.md) §4.1: REQ-DATA-140, REQ-DATA-150.
- REQ-DATA-160: Graph data MUST be `nodes` and `edges` tables whose `source` and `target` are `u32` indices into `nodes` or strings equal to `nodes.id`, and matrices MUST be row-major `rows × cols`; an edge naming no node MUST be dropped with one warning. (v0.9.0)
- REQ-DATA-170: A timestamp column MUST be able to declare a Julian-date epoch with an offset (`BJD - 2457000` days) as well as the Unix epoch; a TESS fixture in that unit MUST draw calendar ticks at the right dates. (v0.12.0)

### 3.2 Streaming (STREAM)

- Shipped in v0.6.0 and moved to [SPEC.md](SPEC.md) §4.2: REQ-STREAM-010, REQ-STREAM-020, REQ-STREAM-030, REQ-STREAM-040, REQ-STREAM-050, REQ-STREAM-070.
- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §4.2: REQ-STREAM-060.

### 3.3 Downsampling (SAMPLE)

- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §4.3: REQ-SAMPLE-010, REQ-SAMPLE-020, REQ-SAMPLE-030.
- Shipped in v0.5.0 and moved to [SPEC.md](SPEC.md) §4.3: REQ-SAMPLE-040, REQ-SAMPLE-060.
- REQ-SAMPLE-050: Scatter charts MUST set a dot for every visible point; no point may be dropped by sampling. (v0.9.0)

---

## 4. Scales, ticks, formatting and layout

### 4.1 Scales (SCALE)

- Shipped in v0.2.0 and moved to [SPEC.md](SPEC.md) §5.1: REQ-SCALE-010, REQ-SCALE-020, REQ-SCALE-030.
- Shipped in v0.5.0 and moved to [SPEC.md](SPEC.md) §5.1: REQ-SCALE-040, REQ-SCALE-050, REQ-SCALE-060.
- REQ-SCALE-070: The core MUST provide sequential, diverging, categorical and threshold colour scales mapping a value to a palette index. (v0.9.0)
- REQ-SCALE-080: The core MUST provide an angle scale that wraps (right ascension 0–24 h, azimuth 0–360°). (v0.12.0)

### 4.2 Ticks (TICK)

- Shipped in v0.2.0 and moved to [SPEC.md](SPEC.md) §5.2: REQ-TICK-010, REQ-TICK-020, REQ-TICK-030, REQ-TICK-040.
- Shipped in v0.5.0 and moved to [SPEC.md](SPEC.md) §5.2: REQ-TICK-050, REQ-TICK-060, REQ-TICK-070.

### 4.3 Formatting (FMT)

- Shipped in v0.2.0 and moved to [SPEC.md](SPEC.md) §5.3: REQ-FMT-010, REQ-FMT-020, REQ-FMT-030.
- Shipped in v0.5.0 and moved to [SPEC.md](SPEC.md) §5.3: REQ-FMT-040, REQ-FMT-050.
- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §5.3: REQ-FMT-060.
- REQ-FMT-070: The core MUST format right ascension in hours (`18h 54m`) and declination in signed degrees (`−23° 45′`). (v0.12.0)

### 4.4 Time zones and locales (TZ, I18N)

- Shipped in v0.2.0 and moved to [SPEC.md](SPEC.md) §5.3: REQ-TZ-010.
- Shipped in v0.5.0 and moved to [SPEC.md](SPEC.md) §5.3: REQ-TZ-020, REQ-TZ-030, REQ-TZ-050.
- REQ-TZ-040: The CLI MUST build the same table from the system's tzdata, and a frame drawn by the CLI and by the element from the same data, options, view and zone MUST be identical. (v0.8.0)
- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §5.3: REQ-I18N-010, REQ-I18N-020, REQ-I18N-050.
- Shipped in v0.2.0 and moved to [SPEC.md](SPEC.md) §5.3: REQ-I18N-030, REQ-I18N-040.
- REQ-I18N-060: Labels with right-to-left runs MUST be reordered by UAX #9 before layout, and an RTL locale MUST put the y axis and the legend on the right. (v0.9.0)
- REQ-I18N-070: The CLI MUST accept a locale pack as a JSON file (`--locale-file`). (v0.8.0)

### 4.5 Layout and text (LAYOUT, TEXT, AXIS, ANNOT)

- Shipped in v0.2.0 and moved to [SPEC.md](SPEC.md) §5.4: REQ-LAYOUT-010, REQ-LAYOUT-020, REQ-LAYOUT-030.
- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §5.4: REQ-LAYOUT-040, REQ-LAYOUT-070.
- Shipped in v0.5.0 and moved to [SPEC.md](SPEC.md) §5.4: REQ-LAYOUT-050, REQ-LAYOUT-060.
- Shipped in v0.2.0 and moved to [SPEC.md](SPEC.md) §5.4: REQ-TEXT-010, REQ-TEXT-020.
- Shipped in v0.5.0 and moved to [SPEC.md](SPEC.md) §5.4: REQ-AXIS-010.
- Shipped in v0.5.0 and moved to [SPEC.md](SPEC.md) §5.4: REQ-ANNOT-010.

---

## 5. View, input and interaction

### 5.1 View state (VIEW)

- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §8.1: REQ-VIEW-010, REQ-VIEW-020, REQ-VIEW-030, REQ-VIEW-040, REQ-VIEW-050.
- Shipped in v0.6.0 and moved to [SPEC.md](SPEC.md) §8.1: REQ-VIEW-060.

### 5.2 Input (INPUT)

- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §8.2: REQ-INPUT-010, REQ-INPUT-020, REQ-INPUT-030, REQ-INPUT-040.

### 5.3 Hit testing and the overlay (HIT, OVERLAY)

- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §8.3: REQ-HIT-010, REQ-HIT-020.
- REQ-HIT-030: A scatter chart MUST answer a hover over 1,000,000 points in under 0.1 ms median natively once its spatial index is built, and MUST answer (by scan) before it is. (v0.9.0)
- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §8.3: REQ-OVERLAY-010, REQ-OVERLAY-020.

### 5.4 Events out and linking (EVENT, LINK)

- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §8.4: REQ-EVENT-010, REQ-EVENT-020, REQ-EVENT-030.
- Shipped in v0.5.0 and moved to [SPEC.md](SPEC.md) §8.4: REQ-LINK-010.

### 5.5 Keyboard and touch (KEY, TOUCH)

- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §8.5: REQ-KEY-010.
- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §8.5: REQ-TOUCH-010, REQ-TOUCH-020.

---

## 6. The chart contract and the registry

### 6.1 Charts, meta and options (CHART, OPT, REG)

- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §6.1: REQ-CHART-010, REQ-CHART-020, REQ-CHART-030.
- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §6.2: REQ-CHART-040, REQ-CHART-050, REQ-CHART-060.
- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §6.1: REQ-OPT-010, REQ-OPT-020.
- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §6.3: REQ-REG-010, REQ-REG-020.

### 6.2 The contract test (CONTRACT, FIX)

- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §6.4: REQ-CONTRACT-010, REQ-CONTRACT-020.
- Shipped in v0.4.0 and moved to [SPEC.md](SPEC.md) §6.4: REQ-CONTRACT-030, REQ-CONTRACT-050, REQ-CONTRACT-060.
- Shipped in v0.6.0 and moved to [SPEC.md](SPEC.md) §6.4: REQ-CONTRACT-040.
- Shipped in v0.2.0 and moved to [SPEC.md](SPEC.md) §6.5: REQ-FIX-010, REQ-REF-010, REQ-PROP-010.
- Shipped in v0.5.0 and moved to [SPEC.md](SPEC.md) §6.5: REQ-FIX-020.

### 6.3 Snapshots, references and properties (SNAP, REF, PROP)

- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §6.5: REQ-SNAP-010, REQ-SNAP-020, REQ-SNAP-030, REQ-SNAP-040.

---

## 7. Charts

The first tier is six families. Each is a module; its types are what the
element's `type` names.

| Family (module) | Types | Modes | Milestone |
|---|---|---|---|
| `line` | `line`, `sparkline` | line: `line`, `step`, `area` (v0.3.0); `band`, `baseline` (v0.9.0) · sparkline: `line`, `area`, `column` | v0.3.0, sparkline v0.5.0 |
| `candles` | `candles` | `candle`, `hollow`, `ohlc` (v0.5.0); `heikin-ashi` (v0.9.0) | v0.5.0 |
| `bar` | `bar` | `bar` (v0.5.0); `histogram` (v0.9.0) | v0.5.0 |
| `scatter` | `scatter` | `point`; `bubble` | v0.9.0 |
| `heatmap` | `heatmap` | `matrix`; `calendar` | v0.9.0 |
| `sky` | `sky`, `orbit` | `equatorial`, `horizon` · orbit | v0.12.0 |

### 7.1 Line (LINE, SPARK)

- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §7.1: REQ-LINE-010, REQ-LINE-020, REQ-LINE-030, REQ-LINE-040.
- REQ-LINE-050: The `line` type MUST draw the modes `band` (between two columns) and `baseline` (coloured above and below a value), and `area` MUST take `stack` (`none`, `stack`, `percent`). (v0.9.0)
- Shipped in v0.5.0 and moved to [SPEC.md](SPEC.md) §7.1: REQ-SPARK-010.

### 7.2 Candles (CANDLE)

- Shipped in v0.5.0 and moved to [SPEC.md](SPEC.md) §7.2: REQ-CANDLE-010, REQ-CANDLE-020, REQ-CANDLE-030, REQ-CANDLE-040, REQ-CANDLE-050.
- Shipped in v0.6.0 and moved to [SPEC.md](SPEC.md) §7.2: REQ-CANDLE-060.
- REQ-CANDLE-070: The `candles` type MUST draw the mode `heikin-ashi`. (v0.9.0)

### 7.3 Bar (BAR)

- Shipped in v0.5.0 and moved to [SPEC.md](SPEC.md) §7.3: REQ-BAR-010, REQ-BAR-020.
- REQ-BAR-030: The `histogram` mode MUST bin unbinned values by a bin count or width, MUST accept pre-binned edges and counts, and its counts MUST equal a reference binning of the fixture. (v0.9.0)

### 7.4 Scatter and heatmap (SCATTER, HEAT)

- REQ-SCATTER-010: The `scatter` type MUST draw points as braille dots and resolve the pointer to the nearest datum within a radius given in cells, or to none, through a data-space index built once a data change. (v0.9.0)
- REQ-SCATTER-020: The `scatter` type SHOULD take `size` and `color` channels (mode `bubble`, `· • ● ⬤` by size). (v0.9.0)
- REQ-HEAT-010: The `heatmap` type MUST draw one value a cell as `█`, or two a cell as `▀` in ink and back when `half: true`, coloured by a colour scale; in one ink it MUST shade by braille dots lit (0–8 a cell). (v0.9.0)
- REQ-HEAT-020: The `heatmap` type MAY draw the mode `calendar` (days in weeks). (v0.9.0)

---

## 8. The wasm boundary and the manifest

### 8.1 The boundary object (WASM)

- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §9.1: REQ-WASM-010, REQ-WASM-020, REQ-WASM-030.
- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §9.2: REQ-WASM-040.
- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §9.1: REQ-WASM-050.
- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §9.2: REQ-WASM-060.

### 8.2 Manifest and loading (MANIFEST, LOAD)

- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §9.3: REQ-MANIFEST-010, REQ-MANIFEST-020.
- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §9.3: REQ-LOAD-010, REQ-LOAD-020, REQ-LOAD-030.

---

## 9. The element, the shell and rendering

### 9.1 The element (EL)

- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §10.1: REQ-EL-010, REQ-EL-020, REQ-EL-040, REQ-EL-060, REQ-EL-070, REQ-EL-080, REQ-EL-085, REQ-EL-090, REQ-EL-100, REQ-EL-120, REQ-EL-130, REQ-EL-140, REQ-EL-150.
- Shipped in v0.5.0 and moved to [SPEC.md](SPEC.md) §10.1: REQ-EL-030, REQ-EL-050, REQ-EL-110.

### 9.2 Mount and the JS API (MOUNT, JS, SSR)

- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §10.2: REQ-MOUNT-010, REQ-MOUNT-020.
- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §10.2: REQ-JS-010.
- REQ-SSR-010: `glyphplot/node` MUST export `renderText(type, init, cols, rows)` and `renderSVG(type, init, cols, rows)` that load the module from the package's files with no network, and its text MUST equal `glyphplot render <type> --format txt` for the same input. (v0.8.0)

### 9.3 Rendering (RENDER)

- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §10.3: REQ-RENDER-010, REQ-RENDER-020, REQ-RENDER-030, REQ-RENDER-050.
- Shipped in v0.5.0 and moved to [SPEC.md](SPEC.md) §10.3: REQ-RENDER-040.

### 9.4 Font (FONT)

- Shipped in v0.2.0 and moved to [SPEC.md](SPEC.md) §3.3: REQ-FONT-010.
- Shipped in v0.5.0 and moved to [SPEC.md](SPEC.md) §3.3: REQ-FONT-020.
- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §10.4: REQ-FONT-030.

---

## 10. Theme and one ink

### 10.1 Theme (THEME)

- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §11.1: REQ-THEME-010, REQ-THEME-020, REQ-THEME-030, REQ-THEME-040, REQ-THEME-050, REQ-THEME-070.
- Shipped in v0.5.0 and moved to [SPEC.md](SPEC.md) §11.1: REQ-THEME-060, REQ-THEME-065.
- REQ-THEME-080: The default sequential scale (cividis) sampled at nine steps MUST be monotonic in L* with neighbours at least 5 ΔE00 apart under each of the four visions, its low end the end nearer the ground's lightness; the default diverging scale MUST run blue to orange with its middle on the ground's side. (v0.9.0)
- REQ-THEME-090: The JS API, the CLI (`--theme <file.json>`), the Rust crate and the SVG and ANSI exports MUST accept a theme object with REQ-THEME-010's token names. (v0.8.0)
- Shipped in v0.7.0 and moved to [SPEC.md](SPEC.md) §11.1: REQ-THEME-100.

### 10.2 One ink (MONO)

- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §11.2: REQ-MONO-010, REQ-MONO-040.
- Shipped in v0.5.0 and moved to [SPEC.md](SPEC.md) §11.2: REQ-MONO-020, REQ-MONO-030.
- REQ-MONO-050: With up to four series and room, a line chart SHOULD label each series at its last visible point. (v0.9.0)

---

## 11. Accessibility

- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §11.3: REQ-A11Y-010, REQ-A11Y-020, REQ-A11Y-030, REQ-A11Y-040, REQ-A11Y-050, REQ-A11Y-060, REQ-A11Y-070.
- Shipped in v0.6.0 and moved to [SPEC.md](SPEC.md) §11.3: REQ-A11Y-080, REQ-A11Y-090, REQ-A11Y-100.
- Shipped in v0.7.0 and moved to [SPEC.md](SPEC.md) §11.3: REQ-A11Y-110.
- REQ-A11Y-120: `glyphplot <type> --table` MUST print the data table as text. (v0.8.0)

---

## 12. Budgets

Every budget lives in one place, the chart's `META.budget` (with the family
default), and reaches `mise run size`, the contract and the benches through
`web/pkg/budgets.json`.

### 12.1 Size (SIZE)

- Shipped in v0.1.0 and moved to [SPEC.md](SPEC.md) §16.3: REQ-SIZE-010, REQ-SIZE-020, REQ-SIZE-030, REQ-SIZE-040.
- Shipped in v0.4.0 and moved to [SPEC.md](SPEC.md) §16.3: REQ-SIZE-050.
- Shipped in v0.2.0 and moved to [SPEC.md](SPEC.md) §16.3: REQ-SIZE-060.
- REQ-SIZE-070: A pack module MUST stay under 32 KB gzipped. (v0.10.0)

### 12.2 Time (PERF)

- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §16.4: REQ-PERF-010, REQ-PERF-020.
- Shipped in v0.4.0 and moved to [SPEC.md](SPEC.md) §16.4: REQ-PERF-030, REQ-PERF-040, REQ-PERF-050, REQ-PERF-060, REQ-PERF-080.
- Shipped in v0.6.0 and moved to [SPEC.md](SPEC.md) §16.4: REQ-PERF-070.

### 12.3 Memory (MEM)

- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §16.4: REQ-MEM-010, REQ-MEM-020, REQ-MEM-030.

### 12.4 Benches (BENCH)

- Shipped in v0.4.0 and moved to [SPEC.md](SPEC.md) §16.4: REQ-BENCH-010, REQ-BENCH-020.

---

## 13. Testing

- Shipped in v0.1.0 and moved to [SPEC.md](SPEC.md) §17: REQ-TEST-010.
- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §17: REQ-TEST-020, REQ-TEST-030, REQ-TEST-040.
- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §17: REQ-E2E-010, REQ-E2E-020, REQ-E2E-030, REQ-E2E-040, REQ-E2E-070.
- Shipped in v0.6.0 and moved to [SPEC.md](SPEC.md) §17: REQ-E2E-050.
- Shipped in v0.7.0 and moved to [SPEC.md](SPEC.md) §17: REQ-E2E-060.

---

## 14. The CLI

- REQ-CLI-010: The CLI (crate `glyphplot-cli`, binary `glyphplot`) MUST accept `glyphplot <type>`, `render <type>`, `demo <type>`, `list`, `-h` and `-V` with the flags of the plan's [CLI](../docs/IMPLEMENTATION_PLAN.md#the-cli), and reject an unknown flag, a second type and a missing or out-of-range number as usage errors. [ADR-134](adr/134-cli-on-crossterm.md) (v0.8.0)
- REQ-CLI-020: The CLI MUST exit 0, 2 or 130 as the plan's [CLI](../docs/IMPLEMENTATION_PLAN.md#the-cli) exit table says, with every error on stderr as `glyphplot: <message>`; a data error MUST name the line and column. (v0.8.0)
- REQ-CLI-030: An unknown type MUST be an error naming up to eight suggestions: types containing the input first, then types within an edit distance (a swap of neighbours counting one) of a third of the input's length, at least 1. (v0.8.0)
- REQ-CLI-040: With stdin not a terminal and no `--from`, or `--from -`, the CLI MUST read data from stdin and keys and mouse from the controlling terminal (`/dev/tty`). (v0.8.0)
- REQ-CLI-050: Rows arriving on stdin while playing MUST be appended at most once a frame, and `--window N` MUST keep only the last N rows. (v0.8.0)
- REQ-CLI-060: With stdout not a terminal, the CLI MUST read to the end of input and print one frame as text with no escapes; a closed stdout MUST end it with exit 0 and nothing on stderr. (v0.8.0)
- REQ-CLI-070: Playing MUST enable mouse capture unless `--no-mouse`, and MUST send mouse and keys through the core's reducer (REQ-INPUT-010) with the keys of the plan's [keys](../docs/IMPLEMENTATION_PLAN.md#keys) plus `h j k l`, `f` (follow) and `q`. (v0.8.0)
- REQ-CLI-080: `--color auto` MUST be `none` when `NO_COLOR` is set and not empty, `truecolor` when `COLORTERM` is `truecolor` or `24bit`, `256` when `TERM` contains `256color`, else `16`. (v0.8.0)
- REQ-CLI-090: The CLI MUST restore the terminal (alternate screen, cursor, wrapping, raw mode, mouse capture) on every way out: the end, a key, Ctrl+C, SIGTERM, SIGHUP and a panic. (v0.8.0)
- REQ-CLI-100: `glyphplot demo <type>` MUST say, on the chart's first row or above it, that the data is generated. (v0.8.0)
- REQ-CLI-110: The CLI MUST take `--glyphs` (`braille` by default, `octants`, `sextants`, `quadrants`, `ascii`) and convert every frame with the core's `glyphs::convert`, live and piped, keeping each cell's colour: `braille` redraws octants and sextants as braille dots, `sextants` redraws octants, `quadrants` redraws every dot glyph, `ascii` shades dot glyphs by dots lit (` .:-=+*#%@`) and draws box drawing as `-|+`, and `octants` changes nothing. (v0.8.0)
- REQ-CLI-120: Each frame MUST write only changed runs of cells. (v0.8.0)
- REQ-TERM-010: `crates/cli/tests/terminal.rs` MUST check, in a pseudo-terminal, the piped plain frame, SGR mouse pan and zoom, a SIGWINCH redraw at the new size, stdin streaming, and the terminal restored after a key, Ctrl+C and SIGTERM. (v0.8.0)

---

## 15. The Rust crate and exports

- REQ-CRATE-010: The `glyphplot` crate MUST render any chart to text and to SVG with no wasm, DOM or terminal dependency in its default features. (v0.8.0)
- REQ-CRATE-020: A `glyphplot-ratatui` crate SHOULD implement ratatui-core's `Widget` for a chart, writing every grid cell into the `Buffer` within the widget's area and mapping mouse events relative to that area onto the chart's input. (v0.11.0)
- REQ-EXPORT-010: txt, ansi and svg MUST be made in Rust (`glyphplot::export`) and MUST be byte-identical between the element's `text()`, `toANSI()` and `toSVG()` and the CLI's `render` for the same input. [ADR-135](adr/135-exports-in-rust.md) (v0.8.0)
- REQ-EXPORT-020: An export MUST draw the chart's current data, options and view, so a test that zooms and then exports gets the zoomed frame. (v0.8.0)
- REQ-EXPORT-030: png MUST be drawn by the shell's `paint` in the browser and by a `png` feature in the CLI that no wasm module enables. (v0.8.0)

---

## 16. The site and datasets

- Shipped in v0.7.0 and moved to [SPEC.md](SPEC.md) §14.1: REQ-SITE-010, REQ-SITE-020, REQ-SITE-030.
- Shipped in v0.7.0 and moved to [SPEC.md](SPEC.md) §14.2: REQ-SITE-040, REQ-SITE-050.
- Shipped in v0.7.0 and moved to [SPEC.md](SPEC.md) §14.1: REQ-SITE-060, REQ-SITE-070.
- Shipped in v0.7.0 and moved to [SPEC.md](SPEC.md) §14.2: REQ-SITE-080, REQ-SITE-090, REQ-SITE-110, REQ-SITE-120.
- REQ-SITE-100: Every milestone from v0.8.0 that ships a chart type, a mode, a pack or a feature of the plan's feature list MUST add its examples in the same milestone, so `examples:check` passes on its branch. (v0.8.0)
- Shipped in v0.7.0 and moved to [SPEC.md](SPEC.md) §14.3: REQ-DATASET-010, REQ-DATASET-020, REQ-DATASET-030.

---

## 17. Domain packs

- REQ-PACK-010: A domain pack MUST be a crate under `crates/packs/<domain>/` (`glyphplot-<domain>`) that depends on the core's data contract and on no chart, which `cargo tree` shows. [ADR-138](adr/138-domain-packs-are-crates-and-modules.md) (v0.10.0)
- REQ-PACK-020: A pack MUST build to its own wasm module, and no chart module MUST contain a pack's exports. (v0.10.0)
- REQ-PACK-030: A pack's output MUST be columns in the data contract, accepted unchanged by the chart its preset names; a test per preset MUST feed the output to that chart and compare a snapshot. (v0.10.0)
- REQ-PACK-040: A pack's functions MUST give the same output bits natively and in wasm on every fixture. (v0.10.0)
- REQ-PACK-050: A pack's streaming functions MUST take amortised O(1) time a sample: appending 1,000,000 samples at windows 10 and 1,000 MUST differ in time a sample by under 2×. (v0.10.0)
- REQ-MKT-010: The markets pack MUST compute SMA, EMA, Bollinger bands, RSI (Wilder) and MACD over a column and incrementally a sample, the two agreeing within 1e-9 relative on every fixture. [ADR-139](adr/139-indicators-in-the-markets-pack.md) (v0.10.0)
- REQ-MKT-020: Indicator values MUST be NaN until their window is full. (v0.10.0)
- REQ-MKT-030: The markets pack MUST aggregate trades (time, price, size) into OHLCV of a period aligned to a UTC origin: open the first trade, close the last, high and low the extremes, volume the sum. (v0.10.0)
- REQ-MKT-040: The markets pack MUST generate synthetic OHLCV and order books from a seed, the same for the same seed, and every chart drawing them MUST show "simulated" in its title or legend. (v0.10.0)
- REQ-TELE-010: The telemetry pack MUST read a Prometheus `query_range` matrix response, `"NaN"`, `"+Inf"` and `"-Inf"` included, into one column a series named by its labels. (v0.10.0)
- REQ-TELE-020: The telemetry pack MUST turn a counter into a per-second rate that treats a drop as a reset, never a negative rate. (v0.10.0)
- REQ-TELE-030: The telemetry pack MUST compute quantiles from classic `le` buckets, native histogram buckets and OTLP exponential histograms by linear interpolation within the bucket, agreeing with Prometheus's documented `histogram_quantile` examples. (v0.10.0)
- REQ-TELE-040: A sample marked stale or "no recorded value" MUST become NaN, so the line breaks. (v0.10.0)

---

## 18. Long tail: sky and orbits

- REQ-ASTRO-010: The astronomy pack MUST compute the geocentric RA and Dec of the eight planets from JPL's approximate Keplerian elements, within 3′ of Horizons fixtures for every day of a committed year. [ADR-140](adr/140-celestial-positions-in-a-pack.md) (v0.12.0)
- REQ-ASTRO-020: The astronomy pack MUST convert RA and Dec to altitude and azimuth for a latitude, longitude and UTC time, within 0.1° of a committed reference. (v0.12.0)
- REQ-ASTRO-030: The astronomy pack MUST precess J2000 positions to a given date, fold a light curve at a period and epoch into phase −0.5 to 0.5, and parse a Horizons `$$SOE … $$EOE` CSV block into columns. (v0.12.0)
- REQ-SKY-010: The `sky` type MUST take positions as data in frame `icrs`, `ecliptic` or `altaz` (degrees), and draw them in equirectangular, stereographic or azimuthal projection; given an observer and a time it MUST convert `icrs` to `altaz` within 0.1° of the reference for ten fixture stars. (v0.12.0)
- REQ-SKY-020: Chart modules MUST NOT compute planetary or lunar positions; a symbol check over each chart module shows none of the pack's. (v0.12.0)
- REQ-SKY-030: The `orbit` type MUST draw each orbit's ellipse from the `orbits` shape and each body's position at the view's time by Kepler's equation, with equal aspect in dots. (v0.12.0)

---

## 19. Packaging

- REQ-PKG-010: The npm package MUST be named `glyphplot`, ESM, with exports `.`, `./element`, `./wasm` and `./node`, each with types, `files` `dist`, `pkg` and `fonts`, and `sideEffects` only `./dist/element.js`. [ADR-001](adr/001-name-glyphplot.md) (v0.11.0)
- REQ-PKG-020: The packed tarball MUST hold every export target, every module's `glyphplot.js` and `glyphplot_bg.wasm`, the font and its licence, and no `.gitignore`. (v0.11.0)
- REQ-PKG-030: The package SHOULD export JSX typings as `glyphplot/jsx` typing `<glyph-plot>` as a union discriminated on `type`; a `tsc` fixture MUST reject candle data on `type="line"`. (v0.11.0)
- Shipped in v0.1.0 and moved to [SPEC.md](SPEC.md) §18: REQ-PKG-040.
- REQ-PKG-050: `prepublishOnly` MUST run `mise run build:web`, which builds the wasm first. (v0.11.0)
- Shipped in v0.7.0 and moved to [SPEC.md](SPEC.md) §18: REQ-PKG-060.

---

## 20. Milestones

Each milestone is one phase of [docs/IMPLEMENTATION_PLAN.md](../docs/IMPLEMENTATION_PLAN.md)
and one GitButler branch named after its version, each stacked on the one
before. A milestone's acceptance is the plan's phase acceptance, which a named
command verifies; real-browser checks are the owner's (REQ-TEST-030).

| Version | Phase | Areas it ships | Acceptance (plan) |
|---|---|---|---|
| v0.1.0 (built 2026-10-18) | 0 · Toolchain green | ARCH-010, 040, 060, 070; INDEP-010–020; BUILD-010–030, 050, 060; LINT-010, 030; CI-010; SIZE-010–040; TEST-010; PKG-040 | `mise run check` exits 0 on an empty family; `mise run size` prints the empty module |
| v0.2.0 (built 2026-10-18) | 1 · Core | GRID, CANVAS, COLOR-010–040, DATA-010–040, 080–090, 110, 130, SCALE-010–030, TICK-010–040, FMT-010–030, TZ-010, I18N-030–040, LAYOUT-010–030, TEXT, FIX-010, REF-010, PROP-010, FONT-010, BUILD-070, LINT-040, SIZE-060 | `mise run test:rust`: reference fixtures 0 differ; properties pass; `fixtures` regenerates bitwise |
| v0.3.0 (built 2026-10-19) | 2 · Line end to end | the `line` type; VIEW-010–050; INPUT; HIT-010–020; OVERLAY; EVENT-010–030; KEY; TOUCH; CHART; OPT; REG; CONTRACT-010–020; SNAP; WASM; MANIFEST; LOAD; EL (most); MOUNT; JS; RENDER-010–030, 050; FONT-030; THEME-010–050, 070; MONO-010, 040; A11Y-010–070; PERF-010–020; MEM; DATA-050–070, 120; STREAM-060; SAMPLE-010–030; I18N-010–020, 050; FMT-060; CANVAS-040; EL-085; LAYOUT-040, 070; CHAR; TEST-020–040; E2E-010–040, 070; BUILD-040, 080; LINT-020; ARCH-020–050 | `mise run check`; `mise run test:site` (interaction, element, resize, axe) |
| v0.4.0 (built 2026-10-20) | 3 · Contract and budgets | CONTRACT-030, 050, 060; PERF-030–060, 080; SIZE-050; BENCH; CI-020; BUILD-090 | `cargo test --profile contract --test contract --test budgets`; `mise run bench`; `mise run signoff` on the branch |
| v0.5.0 (built 2026-10-21) | 4 · Candles, bar, sparkline | CANDLE-010–050; BAR-010–020; SPARK-010; SCALE-040–060; TICK-050–070; FMT-040–050; TZ-020–030, 050; LAYOUT-050–060; AXIS; ANNOT; LINK; EL-030, 050, 110; EVENT-010 (`plot-brush`, `plot-legend`); SAMPLE-040, 060; DATA-100; THEME-060, 065; MONO-020–030; RENDER-040; FONT-020; FIX-020 | contract and snapshots for the three types; the owner's candlestick on the checklist page |
| v0.6.0 (built 2026-10-22) | 5 · Streaming | STREAM-010–050, 070; VIEW-060; CANDLE-060; CONTRACT-040; DATA-140–150; A11Y-080–100; PERF-070; E2E-050; EVENT-010 (`plot-data`) | `streaming.spec.ts`; `bench:web` streaming row; the owner's one-line test on a phone |
| v0.7.0 (built 2026-10-23) | 6 · The site | SITE (all but 100); DATASET; A11Y-110; THEME-100; E2E-060; LINT-020 (site); PKG-060 | `mise run build:site`; `test:site` in full; `datasets:check` |
| v0.8.0 | 7 · The CLI | SITE-100; CLI; TERM; CRATE-010; EXPORT; SSR; TZ-040; I18N-070; COLOR-050; THEME-090; A11Y-120; ARCH-080 | `cargo test -p glyphplot-cli`; `glyphplot candles --from feed.csv` by the owner |
| v0.9.0 | 8 · The next tier | SCATTER; HEAT; BAR-030; LINE-050; CANDLE-070; SCALE-070; SAMPLE-050; HIT-030; THEME-080; MONO-050; I18N-060; DATA-160 | contract and snapshots for the new types and modes |
| v0.10.0 | 9 · Domain packs | PACK; MKT; TELE; SIZE-070 | `cargo test -p glyphplot-markets -p glyphplot-telemetry`; pack sizes |
| v0.11.0 | 10 · Packaging and deploy | PKG-010–030, 050; CRATE-020 | `npm pack` contents test; `cargo publish --dry-run` for each crate |
| v0.12.0 | 11 · Sky and orbits | ASTRO; SKY; SCALE-080; FMT-070; DATA-170 | contract for `sky` and `orbit`; Horizons fixtures within 3′ |

Counts are kept in [conformance.md](conformance.md).

## 21. Instructions for the implementing agent

1. Read `AGENTS.md`, `README.md`, [SPEC.md](SPEC.md), this document, then `docs/IMPLEMENTATION_PLAN.md` and the discovery report of the area you work in.
2. One milestone at a time, in §20's order, each on its own GitButler branch named after its version, stacked on the one before (`but branch new v0.N.0 --above v0.N-1.0`). Review fixes go into the milestone they belong to (`but amend`). No git write commands; no push or pull request unless asked.
3. Every commit message starts with the requirement ID it implements (`REQ-DATA-020: hold ns timestamps as offsets`), says what changed and why, and carries no AI attribution.
4. `mise run check` is green on every commit once there is code. A change to a chart's frames comes with its snapshot diff and a reason (REQ-SNAP-040).
5. Write the test with the behaviour, and update [conformance.md](conformance.md) for that requirement in the same commit. When a milestone ships, move its requirements into SPEC.md with their IDs and mark its row in §20 with the date.
6. A boundary shape changed (a `Meta` field, the manifest, the view, an event) changes its schema in `schemas/` with the code, and the schema test (`web/test/schemas.test.mjs`) still validates its examples.
7. A number in a commit, a doc or a requirement says how it was measured, or is marked **(unverified)**.
8. Browser and device checks are the owner's: check headless and hand the manual checks over as a list at the end of each milestone.
9. Where this document and the code disagree, stop and ask; where a decision here turns out wrong, propose a change to its ADR rather than working round it.
