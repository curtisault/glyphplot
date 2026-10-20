# glyphplot — Roadmap

Version 0.1 (draft) · 2026-10-18

This document holds every requirement of glyphplot not yet built, grouped by
area, each naming the milestone that ships it. When a milestone lands, its
requirements move into [SPEC.md](SPEC.md) with their IDs unchanged, a line
in their old section says where they went, and the milestone's row in §20
says when it was built. v0.1.0 to v0.3.0 have shipped. IDs go up in tens within an area so a requirement
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
- REQ-BUILD-090: `mise run check` SHOULD finish in under five minutes from a warm cache on the reference machine. (v0.4.0)

### 1.4 Lint (LINT)

- Shipped in v0.1.0 and moved to [SPEC.md](SPEC.md) §16.2: REQ-LINT-010, REQ-LINT-030.
- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §16.2: REQ-LINT-020.
- Shipped in v0.2.0 and moved to [SPEC.md](SPEC.md) §16.2: REQ-LINT-040.

### 1.5 Local CI (CI)

- Shipped in v0.1.0 and moved to [SPEC.md](SPEC.md) §16.5: REQ-CI-010.
- REQ-CI-020: `mise run signoff` MUST, for each open pull request (or those named), copy the head commit's tree into `target/ci/tree`, run `npm ci`, `lint`, `test`, `build`, `size`, the whole of `test:site`, `bench:web` and `bench:load` there, with `PROPTEST_CASES=4096`, log to `target/ci/logs/`, and post a `signoff` commit status; it MUST exit 1 when any pull request fails, after checking the rest. (v0.4.0)

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
- REQ-DATA-100: `unit`, `scale` and `offset` MUST apply to ticks, readouts and the data table and MUST NOT change stored values: a readout of stored 18240 with `scale: 0.01` shows `182.40`. (v0.5.0)
- REQ-DATA-140: The core MUST read an Arrow IPC stream (uncompressed; Float64, Float32, Int32, Int64, UInt32, Timestamp, Bool, Utf8 and Dictionary<Int32, Utf8> columns with validity bitmaps) with its own reader, adding at most 8 KB gzipped to a chart module; the element's `data` property and the CLI MUST accept it. [ADR-105](adr/105-hand-written-arrow-reader.md) (v0.6.0)
- REQ-DATA-150: The element MUST take `src`, a URL of CSV, JSON, NDJSON or Arrow, fetched once per change and parsed in the shell (type from `Content-Type` or the extension), and given as `data`. (v0.6.0)
- REQ-DATA-160: Graph data MUST be `nodes` and `edges` tables whose `source` and `target` are `u32` indices into `nodes` or strings equal to `nodes.id`, and matrices MUST be row-major `rows × cols`; an edge naming no node MUST be dropped with one warning. (v0.9.0)
- REQ-DATA-170: A timestamp column MUST be able to declare a Julian-date epoch with an offset (`BJD - 2457000` days) as well as the Unix epoch; a TESS fixture in that unit MUST draw calendar ticks at the right dates. (v0.12.0)

### 3.2 Streaming (STREAM)

- REQ-STREAM-010: `append()` MUST accept one row object, an array of row objects, or columns of typed arrays, and the shell MUST coalesce every append made within one animation frame into one write to wasm (a Node test counts one commit for 1,000 appends in a frame). (v0.6.0)
- REQ-STREAM-020: A streaming chart MUST keep its rows in a ring allocated once with capacity `retain.rows` (default 1,000,000); appending 3,000,000 rows to a ring of 1,000,000 MUST NOT grow wasm memory after the first 1,000,000. [ADR-107](adr/107-a-ring-allocated-once.md) (v0.6.0)
- REQ-STREAM-030: `retain.age` SHOULD drop rows older than the newest x minus that duration; the oldest kept row MUST be the first with x ≥ newest − age. (v0.6.0)
- REQ-STREAM-040: An appended row whose x equals the last row's MUST replace it; a row whose x is less MUST be dropped and counted in the `late` field of the next `plot-data` event. (v0.6.0)
- REQ-STREAM-050: In Node on the reference machine, appending one row MUST take at most 1 µs and a batch of 1,000 at most 50 µs (median). (v0.6.0)
- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §4.2: REQ-STREAM-060.
- REQ-STREAM-070: After each commit of data the element MUST fire `plot-data` with `{rows, dropped, late, version}`. (v0.6.0)

### 3.3 Downsampling (SAMPLE)

- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §4.3: REQ-SAMPLE-010, REQ-SAMPLE-020, REQ-SAMPLE-030.
- REQ-SAMPLE-040: Candlestick charts MUST draw each candle at least one cell wide: when the candles in view do not fit, they MUST be resampled to the smallest interval of the ladder 1 s, 5 s, 15 s, 1 min, 5 min, 15 min, 1 h, 4 h, 1 d, 1 w, 1 mo at which they fit, open the first row, high the maximum, low the minimum, close the last and volume the sum, buckets aligned to the interval in the column's zone. [ADR-109](adr/109-candles-resample-to-calendar-intervals.md) (v0.5.0)
- REQ-SAMPLE-050: Scatter charts MUST set a dot for every visible point; no point may be dropped by sampling. (v0.9.0)
- REQ-SAMPLE-060: LTTB MAY be offered as `sample: "lttb"` and MUST NOT be any chart's default. (v0.5.0)

---

## 4. Scales, ticks, formatting and layout

### 4.1 Scales (SCALE)

- Shipped in v0.2.0 and moved to [SPEC.md](SPEC.md) §5.1: REQ-SCALE-010, REQ-SCALE-020, REQ-SCALE-030.
- REQ-SCALE-040: The core MUST provide log scales (any base above 1) and band scales; a band scale MUST give every category the same whole number of cells. (v0.5.0)
- REQ-SCALE-050: The core SHOULD provide a symlog scale whose ticks are 0 and ± powers of ten within the domain. (v0.5.0)
- REQ-SCALE-060: The core MUST provide an index time scale on which bars sit at consecutive indices whatever the time between them, labelled from each bar's timestamp; a fixture with weekend gaps MUST draw no empty columns. [ADR-131](adr/131-index-time-scale-for-session-gaps.md) (v0.5.0)
- REQ-SCALE-070: The core MUST provide sequential, diverging, categorical and threshold colour scales mapping a value to a palette index. (v0.9.0)
- REQ-SCALE-080: The core MUST provide an angle scale that wraps (right ascension 0–24 h, azimuth 0–360°). (v0.12.0)

### 4.2 Ticks (TICK)

- Shipped in v0.2.0 and moved to [SPEC.md](SPEC.md) §5.2: REQ-TICK-010, REQ-TICK-020, REQ-TICK-030, REQ-TICK-040.
- REQ-TICK-050: A y axis with `snap: "rows"` MUST place every tick on a row exactly with a nice step per row; it MUST be the default for bar and candlestick y axes under 20 rows. (v0.5.0)
- REQ-TICK-060: Log axes MUST label only powers of the base and MAY mark other integer multiples without labels. (v0.5.0)
- REQ-TICK-070: A time axis SHOULD draw a context row with the date at each day boundary when its span is under two days and the layout has the row. (v0.5.0)

### 4.3 Formatting (FMT)

- Shipped in v0.2.0 and moved to [SPEC.md](SPEC.md) §5.3: REQ-FMT-010, REQ-FMT-020, REQ-FMT-030.
- REQ-FMT-040: The core MUST format SI units, IEC bytes (`KiB`, `MiB`), bits a second (`kb/s`, `Mb/s`), durations from nanoseconds to days, prices to a tick size (0.01, 0.25, 1/32) and signed percentages, each covered by a table test. (v0.5.0)
- REQ-FMT-050: Date labels MUST accept the strftime subset `%Y %m %d %e %H %I %M %S %L %p %a %A %b %B %j %Z`, and MUST default to 24-hour multi-scale labels unless the locale's hour cycle is 12-hour. (v0.5.0)
- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §5.3: REQ-FMT-060.
- REQ-FMT-070: The core MUST format right ascension in hours (`18h 54m`) and declination in signed degrees (`−23° 45′`). (v0.12.0)

### 4.4 Time zones and locales (TZ, I18N)

- Shipped in v0.2.0 and moved to [SPEC.md](SPEC.md) §5.3: REQ-TZ-010.
- REQ-TZ-020: The element MUST accept `timeZone` as `UTC`, a fixed offset or an IANA name, and MUST give the core a table of offset transitions covering the data's time extent and the view, computed with `Intl.DateTimeFormat`. (v0.5.0)
- REQ-TZ-030: Local-time ticks over the table MUST equal d3-scale's `scaleTime().ticks()` run under `TZ=<zone>` for every captured case, across daylight-saving changes. (v0.5.0)
- REQ-TZ-040: The CLI MUST build the same table from the system's tzdata, and a frame drawn by the CLI and by the element from the same data, options, view and zone MUST be identical. (v0.8.0)
- REQ-TZ-050: The element MUST default its time zone to the page's (`Intl.DateTimeFormat().resolvedOptions().timeZone`), resolved to an IANA name and passed to the core explicitly; the core, the CLI and the tests MUST default to UTC. [ADR-112](adr/112-zones-and-locales-from-the-shell.md) (v0.5.0)
- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §5.3: REQ-I18N-010, REQ-I18N-020, REQ-I18N-050.
- Shipped in v0.2.0 and moved to [SPEC.md](SPEC.md) §5.3: REQ-I18N-030, REQ-I18N-040.
- REQ-I18N-060: Labels with right-to-left runs MUST be reordered by UAX #9 before layout, and an RTL locale MUST put the y axis and the legend on the right. (v0.9.0)
- REQ-I18N-070: The CLI MUST accept a locale pack as a JSON file (`--locale-file`). (v0.8.0)

### 4.5 Layout and text (LAYOUT, TEXT, AXIS, ANNOT)

- Shipped in v0.2.0 and moved to [SPEC.md](SPEC.md) §5.4: REQ-LAYOUT-010, REQ-LAYOUT-020, REQ-LAYOUT-030.
- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §5.4: REQ-LAYOUT-040, REQ-LAYOUT-070.
- REQ-LAYOUT-050: Panes MUST split rows by weight with at least three rows each, share the x scale, and align their plot areas to the same gutters. (v0.5.0)
- REQ-LAYOUT-060: A legend at `position: "auto"` MUST go outside in one row when it fits and rows ≥ 12, else inside the first pane when it takes at most a third of its width and height, else be left out of the frame (it stays in the table and the events). (v0.5.0)
- Shipped in v0.2.0 and moved to [SPEC.md](SPEC.md) §5.4: REQ-TEXT-010, REQ-TEXT-020.
- REQ-AXIS-010: A pane MUST support a left and a right y axis bound to different scales, with gridlines from one axis only. (v0.5.0)
- REQ-ANNOT-010: The core MUST draw `hline`, `vline`, `band`, `label` and `marker` annotations in data coordinates, clipped to their pane; an annotation label on an axis MUST hide the tick labels it overlaps. (v0.5.0)

---

## 5. View, input and interaction

### 5.1 View state (VIEW)

- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §8.1: REQ-VIEW-010, REQ-VIEW-020, REQ-VIEW-030, REQ-VIEW-040, REQ-VIEW-050.
- REQ-VIEW-060: With `x.follow` true, an append MUST keep the x span and move the domain to end at the newest datum; a pan away from the end MUST set it false, and `End` MUST set it true. (v0.6.0)

### 5.2 Input (INPUT)

- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §8.2: REQ-INPUT-010, REQ-INPUT-020, REQ-INPUT-030, REQ-INPUT-040.

### 5.3 Hit testing and the overlay (HIT, OVERLAY)

- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §8.3: REQ-HIT-010, REQ-HIT-020.
- REQ-HIT-030: A scatter chart MUST answer a hover over 1,000,000 points in under 0.1 ms median natively once its spatial index is built, and MUST answer (by scan) before it is. (v0.9.0)
- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §8.3: REQ-OVERLAY-010, REQ-OVERLAY-020.

### 5.4 Events out and linking (EVENT, LINK)

- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §8.4: REQ-EVENT-010, REQ-EVENT-020, REQ-EVENT-030.
- REQ-LINK-010: Charts sharing a `link` attribute value MUST share `x.domain` and `cursor.x` in data units without event loops; a browser test MUST show two linked charts of different widths agreeing after a pan. (v0.5.0)

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
- REQ-CONTRACT-030: The contract MUST also check rules 10–13, 15–28 of the table: one ink and series told apart in it, paper, resize, replace is whole, view round trip, scripted input, events against the schema, hits naming what was drawn, hit and readout agreeing, empty and edge fixtures, gaps, tick labels, log-invalid values, unsorted and duplicate x, exact downsampling and wrong shapes. (v0.4.0)
- REQ-CONTRACT-040: Appending a fixture in the batches 1, 7, ⌊n/3⌋ and the rest, with frames drawn between, MUST end on the frame and view that replacing with the same rows gives; with a ring of capacity c < n, on replacing with the last c rows (rule 14). (v0.6.0)
- REQ-CONTRACT-050: Time budgets (rule 30) MUST be checked in `tests/budgets.rs`, a test binary of its own, only without debug assertions; a chart over budget MUST be timed twice more, and the lowest average and the lowest worst count. [ADR-126](adr/126-correctness-and-timing-apart.md) (v0.4.0)
- REQ-CONTRACT-060: A `contract-selftest` feature MUST register three broken charts (non-deterministic, outside its charset, keeping state across appends), and a test MUST assert that the contract rejects each with the matching message. (v0.4.0)
- Shipped in v0.2.0 and moved to [SPEC.md](SPEC.md) §6.5: REQ-FIX-010, REQ-REF-010, REQ-PROP-010.
- REQ-FIX-020: Real datasets checked in under `crates/glyphplot/tests/data/` MUST each be under 100 KB, public domain or MIT-compatible, named with source and licence in that directory's README. (v0.5.0)

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
- REQ-SPARK-010: The `sparkline` type, played by the `line` module, MUST draw without axes, one series a row band, MUST take appended samples into a ring of set capacity, and MAY show each series' label, latest value, minimum and maximum. (v0.5.0)

### 7.2 Candles (CANDLE)

- REQ-CANDLE-010: The `candles` type MUST draw the modes `candle`, `hollow` and `ohlc`. (v0.5.0)
- REQ-CANDLE-020: It MUST choose glyphs by candle spacing in cells: box-drawing candles at 4 cells or more; braille wick and body with a blank cell between at 2 or 3; braille without the gap at 1; snapshots at spacings 4, 2 and 1. (v0.5.0)
- REQ-CANDLE-030: In one ink it MUST draw rising candles hollow and falling candles solid, and MUST NOT draw candles narrower than four cells, resampling instead (REQ-SAMPLE-040). (v0.5.0)
- REQ-CANDLE-040: It MUST draw volume in a second pane sharing the x axis when the data has a `volume` column, on the index time scale by default (REQ-SCALE-060). (v0.5.0)
- REQ-CANDLE-050: Its crosshair MUST snap to the nearest of open, high, low and close, and its readout MUST show `o h l c`, volume and the signed change. (v0.5.0)
- REQ-CANDLE-060: Streaming into a candlestick chart MUST update the last candle in place when a row's time falls in it (REQ-STREAM-040). (v0.6.0)
- REQ-CANDLE-070: The `candles` type MUST draw the mode `heikin-ashi`. (v0.9.0)

### 7.3 Bar (BAR)

- REQ-BAR-010: The `bar` type MUST draw vertical and horizontal bars, grouped and stacked, with each bar's free end at an eighth of a cell (`▁`–`▇` vertical, `▏`–`▉` horizontal). (v0.5.0)
- REQ-BAR-020: A bar chart MUST hit-test by band in O(1). (v0.5.0)
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
- REQ-EL-030: `define(tag, { type })` MUST define `tag` as a subclass whose chart type is `type` whatever its `type` attribute says, and MUST do nothing for a tag already defined. (v0.5.0)
- REQ-EL-050: A change of `type` MUST make a new chart and give it the old chart's data and view when its meta accepts the data's shape; otherwise it MUST warn once naming the shape and draw the new chart empty. (v0.5.0)
- REQ-EL-110: With `cols` and `rows`, the grid MUST be that size, and `fit` MUST scale its drawing: `width` to the element's content width W (a cell W / cols wide), `contain` to the largest box inside W × H that keeps the cell's 1:2 aspect, centred, acting as `width` when H is 0; any other value MUST act as no fit. (v0.5.0)

### 9.2 Mount and the JS API (MOUNT, JS, SSR)

- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §10.2: REQ-MOUNT-010, REQ-MOUNT-020.
- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §10.2: REQ-JS-010.
- REQ-SSR-010: `glyphplot/node` MUST export `renderText(type, init, cols, rows)` and `renderSVG(type, init, cols, rows)` that load the module from the package's files with no network, and its text MUST equal `glyphplot render <type> --format txt` for the same input. (v0.8.0)

### 9.3 Rendering (RENDER)

- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §10.3: REQ-RENDER-010, REQ-RENDER-020, REQ-RENDER-030, REQ-RENDER-050.
- REQ-RENDER-040: A frame whose cells carry back colours SHOULD be drawn as one image of `cols × 2·rows` pixels scaled without smoothing, with other glyphs over it. (v0.5.0)

### 9.4 Font (FONT)

- Shipped in v0.2.0 and moved to [SPEC.md](SPEC.md) §3.3: REQ-FONT-010.
- REQ-FONT-020: The font SHOULD also cover U+00C0–00FF and U+0100–017F (tier B). (v0.5.0)
- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §10.4: REQ-FONT-030.

---

## 10. Theme and one ink

### 10.1 Theme (THEME)

- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §11.1: REQ-THEME-010, REQ-THEME-020, REQ-THEME-030, REQ-THEME-040, REQ-THEME-050, REQ-THEME-070.
- REQ-THEME-060: Up and down MUST be theme tokens, never fixed colours, and candles and bars MUST tell them apart by fill (hollow and solid) as well as by colour. (v0.5.0)
- REQ-THEME-065: The default `up` and `down` tokens MUST be a blue and an orange at least 50 CIEDE2000 apart under normal vision and the three simulated deficiencies, overridable like every token. [ADR-123](adr/123-colour-defaults-held-to-measured-bars.md) (v0.5.0)
- REQ-THEME-080: The default sequential scale (cividis) sampled at nine steps MUST be monotonic in L* with neighbours at least 5 ΔE00 apart under each of the four visions, its low end the end nearer the ground's lightness; the default diverging scale MUST run blue to orange with its middle on the ground's side. (v0.9.0)
- REQ-THEME-090: The JS API, the CLI (`--theme <file.json>`), the Rust crate and the SVG and ANSI exports MUST accept a theme object with REQ-THEME-010's token names. (v0.8.0)
- REQ-THEME-100: Under `forced-colors: active` the element MUST draw in one ink in `CanvasText` on `Canvas`, with the accent in `Highlight`; under `@media print` it SHOULD draw on paper. (v0.7.0)

### 10.2 One ink (MONO)

- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §11.2: REQ-MONO-010, REQ-MONO-040.
- REQ-MONO-020: In one ink, series 5–8 MUST repeat patterns 1–4 and add a marker each from `● ○ ■ □ ▲ △ ◆ ◇`, at most one marker a cell. (v0.5.0)
- REQ-MONO-030: In one ink, bar and area series MUST fill with `█ ▓ ▒ ░` by series. (v0.5.0)
- REQ-MONO-050: With up to four series and room, a line chart SHOULD label each series at its last visible point. (v0.9.0)

---

## 11. Accessibility

- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §11.3: REQ-A11Y-010, REQ-A11Y-020, REQ-A11Y-030, REQ-A11Y-040, REQ-A11Y-050, REQ-A11Y-060, REQ-A11Y-070.
- REQ-A11Y-080: A chart receiving streamed data MUST support `paused` (property and attribute) and Space on the cursor: while paused it MUST keep appended data without drawing it, and on resume MUST show the live view. (v0.6.0)
- REQ-A11Y-090: While data streams, the table MUST be rebuilt at most once a second and never while focus is inside it. (v0.6.0)
- REQ-A11Y-100: With `announce="<seconds>"` (at least 5), a polite live region MUST announce the latest value at most once per interval, silent while paused or while the cursor has focus; without it there MUST be no live region. (v0.6.0)
- REQ-A11Y-110: A "show data" control MUST make the table visible. (v0.7.0)
- REQ-A11Y-120: `glyphplot <type> --table` MUST print the data table as text. (v0.8.0)

---

## 12. Budgets

Every budget lives in one place, the chart's `META.budget` (with the family
default), and reaches `mise run size`, the contract and the benches through
`web/pkg/budgets.json`.

### 12.1 Size (SIZE)

- Shipped in v0.1.0 and moved to [SPEC.md](SPEC.md) §16.3: REQ-SIZE-010, REQ-SIZE-020, REQ-SIZE-030, REQ-SIZE-040.
- REQ-SIZE-050: `mise run size --charts` MUST list, for each module, the gzipped share of the chart's own code, the core, serde and serde_json, and the rest (std, alloc, wasm-bindgen), from an unstripped build. (v0.4.0)
- Shipped in v0.2.0 and moved to [SPEC.md](SPEC.md) §16.3: REQ-SIZE-060.
- REQ-SIZE-070: A pack module MUST stay under 32 KB gzipped. (v0.10.0)

### 12.2 Time (PERF)

- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §16.4: REQ-PERF-010, REQ-PERF-020.
- REQ-PERF-030: In a release build, every chart MUST average at most 4 ms a frame with no frame over 16 ms, over 300 frames of a view panning across a fixture of 1,000,000 points at 80 × 24 and 320 × 120 with default options, measured natively by `tests/budgets.rs`. (v0.4.0)
- REQ-PERF-040: In a release build, every chart MUST average at most 1 ms a frame with no frame over 8 ms over the same play with a fixture of 100,000 points. (v0.4.0)
- REQ-PERF-050: A pointer move (hit test, crosshair and readout) MUST average at most 0.5 ms with none over 2 ms over 300 moves across a chart holding 1,000,000 points, natively in release and in wasm in headless Chromium. (v0.4.0)
- REQ-PERF-060: In headless Chromium on the reference machine, every chart MUST average at most 8 ms a frame with no frame over 16 ms in the play of REQ-PERF-030, checked by `mise run bench:web` in `signoff`. (v0.4.0)
- REQ-PERF-070: Appending one sample at 1 kHz to a full ring of 1,000,000 points while redrawing at 60 Hz MUST cost at most 500 ms of CPU a second in headless Chromium, the summed time of appends and frames over 10 simulated seconds. (v0.6.0)
- REQ-PERF-080: A page holding one chart MUST draw its first frame within 1.0 s of navigation start on a 170 ms, 9 Mbps profile, median of 7 cold loads in headless Chromium; `mise run bench:load` MUST also report, without gating, the time on Lighthouse 12's `mobileSlow4G` request-level profile (562.5 ms, 1.44 Mbps, CPU 4×); both run in `signoff`. [ADR-133](adr/133-a-350-kb-module-gate.md) (v0.4.0)

### 12.3 Memory (MEM)

- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §16.4: REQ-MEM-010, REQ-MEM-020, REQ-MEM-030.

### 12.4 Benches (BENCH)

- REQ-BENCH-010: `mise run bench [type …]` MUST print the load average, then a row per type, fixture size (1k, 100k, 1M) and grid size (80 × 24, 320 × 120) with average, worst, budget and `OVER`, and exit 1 when any row is over. (v0.4.0)
- REQ-BENCH-020: `mise run bench:web`, `bench:load` and `bench:render` MUST run the discovery harnesses' successors in headless Chromium against the release build and print their tables with the load average. (v0.4.0)

---

## 13. Testing

- Shipped in v0.1.0 and moved to [SPEC.md](SPEC.md) §17: REQ-TEST-010.
- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §17: REQ-TEST-020, REQ-TEST-030, REQ-TEST-040.
- Shipped in v0.3.0 and moved to [SPEC.md](SPEC.md) §17: REQ-E2E-010, REQ-E2E-020, REQ-E2E-030, REQ-E2E-040, REQ-E2E-070.
- REQ-E2E-050: `site/e2e/streaming.spec.ts` MUST append at 1 kHz for a second and then find the frame equal to a replace of the same rows. (v0.6.0)
- REQ-E2E-060: Every chart type's page on the site MUST draw a first frame with nothing on the console. (v0.7.0)

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

- REQ-SITE-010: The site MUST be Elm 0.19.1 with Vite, its flags from `web/pkg/modules.js`, loading no wasm before the page shows. (v0.7.0)
- REQ-SITE-020: The site MUST have a page per chart type with the chart live on a fixture and an example, a control per option, its attributes, a log of its events, and usage snippets that name only installs that work. (v0.7.0)
- REQ-SITE-030: The site MUST have documentation pages (getting started, the data contract, view and events, theming, accessibility, the CLI) with live charts in them. (v0.7.0)
- REQ-SITE-040: The site's examples page MUST be its gallery: every example a card with a live chart, a title, one line on what it shows, and its chart type, modes, domain and features, filterable by each and by milestone; each card MUST link to the example's own page, which shows the chart full size and the page source it runs, copyable. The first two examples MUST be a live candlestick of a synthetic market labelled "simulated market" and a live candlestick of Wikimedia edits a second, one candle a minute. [ADR-143](adr/143-site-docs-with-live-charts.md), [ADR-136](adr/136-the-sites-market-demo.md) (v0.7.0)
- REQ-SITE-050: An example over a live third-party feed MUST name the source and its licence on the page, and MUST show a message, not an error, when the feed is down. (v0.7.0)
- REQ-SITE-060: The site MUST follow `prefers-color-scheme` until the reader chooses, and its theme button MUST set `data-theme` on `<html>` and keep it in `localStorage`, every chart following it (REQ-THEME-030). (v0.7.0)
- REQ-SITE-070: An axe scan of the index, a chart page and an example, in both themes, MUST find no violation of impact serious or critical. (v0.7.0)
- REQ-SITE-080: `site/examples/manifest.json` MUST list every example with its slug, title, chart types, modes, domain, features, datasets and milestone, and `mise run examples:check` MUST fail when a chart type that is built has fewer than three examples, a mode or a feature of the plan's feature list that is built has none, an example names a dataset missing from `datasets/manifest.json`, or an example in the manifest has no page. (v0.7.0)
- REQ-SITE-090: The gallery MUST hold at least 50 examples at v0.7.0 and at least 85 at v0.12.0, `examples:check` counting them. (v0.7.0)
- REQ-SITE-100: Every milestone from v0.8.0 that ships a chart type, a mode, a pack or a feature of the plan's feature list MUST add its examples in the same milestone, so `examples:check` passes on its branch. (v0.8.0)
- REQ-SITE-110: An example drawn from generated data MUST say so on its card and its page ("simulated", with the seed), as ADR-006's demo mode requires. (v0.7.0)
- REQ-SITE-120: Each gallery card MUST show a first frame rendered as text at build time (with the JS API in Node) until its element comes near the screen, so the gallery loads no wasm before the reader scrolls and each family's module is fetched once however many cards use it. (v0.7.0)
- REQ-DATASET-010: Every dataset committed or served by the site MUST have an entry in `datasets/manifest.json` (publisher, URL, licence name, licence URL, retrieval date, size, SHA-256), and a check MUST fail when a file under `datasets/` or the site's data folder has no entry or a different hash. [ADR-137](adr/137-dataset-manifest-and-allow-list.md) (v0.7.0)
- REQ-DATASET-020: No committed dataset MUST exceed 1 MB or carry a NonCommercial, NoDerivatives or proprietary licence; the check MUST read the manifest's licence names against an allow-list (public domain, CC0, CC BY, ODC-By, MIT, BSD). (v0.7.0)
- REQ-DATASET-030: ShareAlike datasets MUST NOT be in any published crate or npm package; the check MUST list the package contents against the manifest. (v0.7.0)

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
- REQ-PKG-060: Until the package is on npm and the CLI on crates.io, every install snippet MUST say so and show how to build from the repository. (v0.7.0)

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
| v0.4.0 | 3 · Contract and budgets | CONTRACT-030, 050, 060; PERF-030–060, 080; SIZE-050; BENCH; CI-020; BUILD-090 | `cargo test --profile contract --test contract --test budgets`; `mise run bench`; `mise run signoff` on the branch |
| v0.5.0 | 4 · Candles, bar, sparkline | CANDLE-010–050; BAR-010–020; SPARK-010; SCALE-040–060; TICK-050–070; FMT-040–050; TZ-020–030, 050; LAYOUT-050–060; AXIS; ANNOT; LINK; EL-030, 050, 110; EVENT-010 (`plot-brush`, `plot-legend`); SAMPLE-040, 060; DATA-100; THEME-060, 065; MONO-020–030; RENDER-040; FONT-020; FIX-020 | contract and snapshots for the three types; the owner's candlestick on the checklist page |
| v0.6.0 | 5 · Streaming | STREAM-010–050, 070; VIEW-060; CANDLE-060; CONTRACT-040; DATA-140–150; A11Y-080–100; PERF-070; E2E-050; EVENT-010 (`plot-data`) | `streaming.spec.ts`; `bench:web` streaming row; the owner's one-line test on a phone |
| v0.7.0 | 6 · The site | SITE (all but 100); DATASET; A11Y-110; THEME-100; E2E-060; LINT-020 (site); PKG-060 | `mise run build:site`; `test:site` in full; `datasets:check` |
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
