# glyphplot — Roadmap

Version 0.1 (draft) · 2026-10-18

This document holds every requirement of glyphplot. Nothing is built, so every
requirement starts here, grouped by area, each naming the milestone that
ships it. When a milestone lands, its requirements move into
[SPEC.md](SPEC.md) with their IDs unchanged, and the milestone's row in §20
says when it was built. IDs go up in tens within an area so a requirement
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

- REQ-ARCH-010: The core crate (`crates/glyphplot`) MUST NOT depend on a DOM, a canvas or a terminal; its normal dependencies MUST be `serde`, `serde_json` and `unicode-width` only. (v0.1.0)
- REQ-ARCH-020: The wasm crate (`crates/wasm`) and the CLI (`crates/cli`) MUST reach charts only through the core's registry and define no chart of their own, except the debug charts of REQ-WASM-060. (v0.3.0)
- REQ-ARCH-030: The shell (`web/src`) MUST be the only consumer of the wasm API; the site MUST reach charts only through the shell's exports and `<glyph-plot>`. (v0.3.0)
- REQ-ARCH-040: Each chart family MUST be one Cargo feature in both `glyphplot` and `glyphplot-wasm`; the feature `all`, on by default, MUST enable every one. (v0.1.0)
- REQ-ARCH-050: A chart MUST use only the core's shared modules and its own family folder `crates/glyphplot/src/charts/<family>/`, never another family's; `mise run lint` MUST check every family alone (`cargo check -p glyphplot --no-default-features --features <family>`). (v0.3.0)
- REQ-ARCH-060: The workspace MUST be one version, Rust edition 2021, licence MIT, on the Rust 1.97 toolchain with the `wasm32-unknown-unknown` target, `rustfmt` and `clippy` (`rust-toolchain.toml`, `mise.toml`). (v0.1.0)
- REQ-ARCH-070: The workspace MUST have no git dependencies. (v0.1.0)
- REQ-ARCH-080: The CLI MUST link the core natively with every family and every pack, without wasm. (v0.8.0)

### 1.2 Vendored core (VENDOR)

- REQ-VENDOR-010: The core MUST hold its own copies of unicel's `grid.rs`, `subcell.rs`, `palette.rs`, `bluenoise.rs`, `glyphs.rs` and the braille and dither helpers of `util.rs`, and the shell its copy of `web/src/blocks.ts`, each beginning with a comment that names unicel, its MIT licence and the source revision. [ADR-100](adr/100-vendor-unicels-core.md) (v0.1.0)
- REQ-VENDOR-020: `mise run vendor:check` SHOULD print, for each vendored file, whether it differs from unicel at the revision its header names. (v0.2.0)

### 1.3 Tasks (BUILD)

- REQ-BUILD-010: mise MUST be the only entry point for building, checking and serving; `mise install` MUST install every tool `mise.toml` pins, and `mise run check` MUST exit 0. (v0.1.0)
- REQ-BUILD-020: `mise run check` MUST run `lint`, `test`, `build` and `size` in one task graph, and from v0.3.0 also `test:site --smoke`. (v0.1.0)
- REQ-BUILD-030: `mise run build:wasm` MUST empty `web/pkg`, build one release module per family into `web/pkg/<family>/glyphplot{.js,_bg.wasm}` with `wasm-pack --target web --release --no-pack`, delete the `.gitignore` wasm-pack writes beside each, and write `modules.js`, `modules.d.ts`, `budgets.json` and `sizes.json`. (v0.1.0)
- REQ-BUILD-040: `mise run build:wasm:debug` MUST build one module, `all`, with every family and the `debug` feature, in its own cargo target dir, staged beside `web/pkg` and swapped in whole, so a failed build leaves the last good `web/pkg`. (v0.3.0)
- REQ-BUILD-050: The release profile MUST be `opt-level = "s"`, LTO, one codegen unit, `panic = "abort"` and `strip`, and wasm-opt MUST run with `-Os --enable-bulk-memory --enable-nontrapping-float-to-int`. (v0.1.0)
- REQ-BUILD-060: Generated output (`web/pkg`, `web/dist`, `site/dist`, `target`, the bench outputs under `discovery/bench`) MUST stay out of git; the bundled font is generated and checked in. (v0.1.0)
- REQ-BUILD-070: `mise run test:rust` MUST run `cargo test --workspace --profile contract`, a profile inheriting `release` with LTO off, 16 codegen units and `opt-level = 3`. [ADR-127](adr/127-test-profile-without-lto.md) (v0.2.0)
- REQ-BUILD-080: `mise run dev` MUST serve the debug build with the checklist page and rebuild the debug wasm when a file under `crates/` changes, reloading the page. (v0.3.0)
- REQ-BUILD-090: `mise run check` SHOULD finish in under five minutes from a warm cache on the reference machine. (v0.4.0)

### 1.4 Lint (LINT)

- REQ-LINT-010: `mise run lint` MUST run `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `RUSTDOCFLAGS='-D warnings' cargo doc --no-deps -p glyphplot`, and clippy on `glyphplot-wasm` for `wasm32-unknown-unknown` with and without `debug`. (v0.1.0)
- REQ-LINT-020: From the milestone each lands in, `mise run lint` MUST also run `tsc --noEmit` for `web` (v0.3.0) and `site` (v0.7.0) and `elm-format --validate` on `site/src` and `site/tests` (v0.7.0). (v0.3.0)
- REQ-LINT-030: Rust style MUST follow `rustfmt.toml` (`max_width = 160`, `use_small_heuristics = "Max"`), and every public item of every crate MUST have a doc comment (`#![warn(missing_docs)]`, rustdoc with `-D warnings`). (v0.1.0)
- REQ-LINT-040: `clippy.toml` MUST list `f64::clamp` and `f32::clamp` under `disallowed-methods`, so a call fails `mise run lint`. (v0.2.0)

### 1.5 Local CI (CI)

- REQ-CI-010: There MUST be no hosted CI workflow; `mise run signoff` is CI, as unicel's ADR-021. (v0.1.0)
- REQ-CI-020: `mise run signoff` MUST, for each open pull request (or those named), copy the head commit's tree into `target/ci/tree`, run `npm ci`, `lint`, `test`, `build`, `size`, the whole of `test:site`, `bench:web` and `bench:load` there, with `PROPTEST_CASES=4096`, log to `target/ci/logs/`, and post a `signoff` commit status; it MUST exit 1 when any pull request fails, after checking the rest. (v0.4.0)

---

## 2. Core: grid, glyphs, colour

### 2.1 Grid (GRID)

- REQ-GRID-010: A `Grid` MUST hold `cols × rows` cells row-major, each a `u32` code point, a `u8` ink index and a `u8` back index, with `BLANK = 0x20`, `WIDE_TAIL = 0` and `NO_BACK = 255`, and MUST keep unicel's REQ-GRID-020 to REQ-GRID-090 (clipping draws, `set_wide`, `blit`, `text()`, buffer access). [ADR-004](adr/004-unicels-frame-buffer.md) (v0.2.0)
- REQ-GRID-020: `Grid::resize(cols, rows)` MUST give a grid of the new size with every cell `BLANK`, colour 0 and `NO_BACK`, reusing its allocation when it is large enough, and MUST panic on a zero width or height. (v0.2.0)

### 2.2 Sub-cell canvases and cell mapping (CANVAS)

- REQ-CANVAS-010: The core MUST provide unicel's `Dots` (braille), `Sextants`, `Quadrants` and `Octants` canvases and colour canvases with unicel's REQ-CANVAS-001 to REQ-CANVAS-090 behaviour, unit-tested on the same pinned code points. (v0.2.0)
- REQ-CANVAS-020: The core MUST have one module of cell mappings (data value to column and dot, column and dot to data value) that frames, hit tests, readouts, reducers and the contract all call; a property test MUST show that the x at a column's left edge maps back into that column for 1 to 2,000 points over 40 to 640 columns. (v0.2.0)

### 2.3 Charsets (CHAR)

- REQ-CHAR-010: Every code point a chart draws outside label text MUST be in the bundled font's ranges (REQ-FONT-010); label text that comes from data or options MAY be any Unicode scalar value. (v0.3.0)
- REQ-CHAR-020: A double-width glyph MUST be drawn with `set_wide` and its `WIDE_TAIL`, and a `WIDE_TAIL` MUST follow only a double-width glyph in the same row. (v0.3.0)

### 2.4 Colour (COLOR)

- REQ-COLOR-010: A cell's ink and back MUST be indices 0–254 into the chart's palette, or `NO_BACK` (255) for the back. [ADR-102](adr/102-semantic-palette-of-255.md) (v0.2.0)
- REQ-COLOR-020: The palette MUST have a fixed layout: indices 0–31 are theme tokens in the order of the plan's [palette](../docs/IMPLEMENTATION_PLAN.md#palette) table (ink, muted, rule, accent, ground, up, down, selection, series 1–8, the rest reserved), 32–159 a 128-step scale ramp, and 160–254 reserved; a frame MUST write indices only, never colours. (v0.2.0)
- REQ-COLOR-030: The core MUST resolve a theme object to the 255 RGBA entries in one function, `theme::resolve`, which the shell, the CLI and the exports all call; a change of theme MUST NOT draw a new frame (a test counts `frame` calls: zero). (v0.2.0)
- REQ-COLOR-040: The scale ramp MUST interpolate the theme's scale stops in Oklab over 128 steps, and no two neighbouring steps of a default scale MUST differ by more than ΔE76 2.5. (v0.2.0)
- REQ-COLOR-050: The CLI MUST map the resolved palette to xterm-256 or 16 colours once per palette for `--color 256` and `--color 16`, never per cell. (v0.8.0)

---

## 3. Data

### 3.1 The data contract (DATA)

- REQ-DATA-010: Every chart MUST accept data in the shape of [data.schema.json](schemas/data.schema.json): named tables of columns typed `f64`, `f32`, `i64`, `u32`, `bool`, `timestamp`, `duration`, `category` or `string`, and named matrices; the schema MUST validate the examples in `spec/schemas/examples/`. (v0.2.0)
- REQ-DATA-020: A chart MUST hold numeric, timestamp and duration columns as `f64`; timestamps as epoch milliseconds; and a column whose `time_unit` is `us` or `ns` as offsets from an `i64` origin, so `ns` values spanning under 2^53 ns round-trip exactly (tested with 1792281600000000001 to 1792281600000000004). [ADR-104](adr/104-typed-columns-cross-the-boundary.md) (v0.2.0)
- REQ-DATA-030: JSON `null`, an empty CSV field, NaN and a cleared Arrow validity bit MUST all read as missing: NaN for numbers and times, `u32::MAX` for categories. A missing y MUST break a line or area into separate runs, and a row with a missing x MUST NOT be drawn. (v0.2.0)
- REQ-DATA-040: A chart MUST check on load that its x column is non-decreasing; when it is not, it MUST draw a stably sorted copy and warn once, and its frame MUST equal the frame of the sorted data. (v0.2.0)
- REQ-DATA-050: The wasm boundary MUST take column values only as typed arrays (a `Vec<f64>` argument, or a buffer the chart reserved and the shell filled) plus a descriptor without `values`; no chart module MUST contain a CSV or JSON data parser, which `mise run size --charts` checks by symbol. (v0.3.0)
- REQ-DATA-060: The shell MUST read `memory.buffer` only after the call that returns a buffer's address; a Node test MUST fill a reserved 1,000,000-row column across a memory growth and check its sum. (v0.3.0)
- REQ-DATA-070: Replacing a chart's data with two 1,000,000-row `Float64Array` columns MUST take at most 10 ms (median of 5) in Node on the reference machine and grow wasm memory by at most 1.1 times the columns' bytes. (v0.3.0)
- REQ-DATA-080: A chart MUST find each channel (x, y, open, …) by the option `encode`, then by a column's `role`, then by a column of the channel's name; when none matches it MUST draw its empty-data frame and warn once naming the channel. (v0.2.0)
- REQ-DATA-090: Timestamp values MUST be UTC instants; a column's `zone` MUST decide only tick placement and labels, and no frame MUST depend on the host's time zone (equal frames under `TZ=UTC` and `TZ=Asia/Tokyo`). (v0.2.0)
- REQ-DATA-100: `unit`, `scale` and `offset` MUST apply to ticks, readouts and the data table and MUST NOT change stored values: a readout of stored 18240 with `scale: 0.01` shows `182.40`. (v0.5.0)
- REQ-DATA-110: Data of a shape the chart does not accept MUST give an error naming the shape it expected, never a panic. (v0.2.0)
- REQ-DATA-120: The element MUST accept, as the `data` property or attribute, the data object, the columns shorthand (`{"t": […], "close": […]}`) and the rows shorthand (`[{…}, …]`); the shell MUST parse JSON with `JSON.parse` and infer shorthand types as the schema says. [ADR-104](adr/104-typed-columns-cross-the-boundary.md) (v0.3.0)
- REQ-DATA-130: Data changes MUST take effect at once; a frame MUST be a function of the data as it is, never of an easing between versions. [ADR-106](adr/106-data-applies-at-once.md) (v0.2.0)
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
- REQ-STREAM-060: After `data` is replaced, a chart's frame MUST equal the frame of a new chart given the same data, options, view and `t`. (v0.3.0)
- REQ-STREAM-070: After each commit of data the element MUST fire `plot-data` with `{rows, dropped, late, version}`. (v0.6.0)

### 3.3 Downsampling (SAMPLE)

- REQ-SAMPLE-010: Line-like charts (line, step, area, band, sparkline) MUST draw from M4 aggregates (first, last, minimum, maximum per dot column) of the rows in view whenever those rows outnumber four times the plot's dot columns, and for x-sorted data the frame MUST equal, cell for cell, the frame drawn from every segment. [ADR-108](adr/108-m4-per-dot-column.md) (v0.3.0)
- REQ-SAMPLE-020: A line-like chart MUST find the rows in view by binary search on x, and MUST cache its aggregates by data version, view x range and width; a frame with all three unchanged MUST NOT read the data (a scan counter in the test build stays 0). (v0.3.0)
- REQ-SAMPLE-030: M4 over 1,000,000 rows in view at 640 dot columns MUST take at most 8 ms (median of 7) in wasm in Node on the reference machine. (v0.3.0)
- REQ-SAMPLE-040: Candlestick charts MUST draw each candle at least one cell wide: when the candles in view do not fit, they MUST be resampled to the smallest interval of the ladder 1 s, 5 s, 15 s, 1 min, 5 min, 15 min, 1 h, 4 h, 1 d, 1 w, 1 mo at which they fit, open the first row, high the maximum, low the minimum, close the last and volume the sum, buckets aligned to the interval in the column's zone. [ADR-109](adr/109-candles-resample-to-calendar-intervals.md) (v0.5.0)
- REQ-SAMPLE-050: Scatter charts MUST set a dot for every visible point; no point may be dropped by sampling. (v0.9.0)
- REQ-SAMPLE-060: LTTB MAY be offered as `sample: "lttb"` and MUST NOT be any chart's default. (v0.5.0)

---

## 4. Scales, ticks, formatting and layout

### 4.1 Scales (SCALE)

- REQ-SCALE-010: The core MUST provide linear and time scales that map a domain value to a continuous cell coordinate and back; `invert(map(v))` MUST equal `v` within 1e-9 relative for finite `v` in the domain. [ADR-110](adr/110-scales-and-formats-ported-from-d3.md) (v0.2.0)
- REQ-SCALE-020: A scale's domain MUST be `"data"`, `"view"`, a fixed `[a, b]` or `{"follow": seconds}`, and MUST be a pure function of the data, the options and the view state. (v0.2.0)
- REQ-SCALE-030: `nice` MUST give the domain d3-scale 4.0.2's `linear().nice(count)` gives for every case of the captured fixture file, including leaving unchanged a domain that does not settle in ten rounds. (v0.2.0)
- REQ-SCALE-040: The core MUST provide log scales (any base above 1) and band scales; a band scale MUST give every category the same whole number of cells. (v0.5.0)
- REQ-SCALE-050: The core SHOULD provide a symlog scale whose ticks are 0 and ± powers of ten within the domain. (v0.5.0)
- REQ-SCALE-060: The core MUST provide an index time scale on which bars sit at consecutive indices whatever the time between them, labelled from each bar's timestamp; a fixture with weekend gaps MUST draw no empty columns. [ADR-131](adr/131-index-time-scale-for-session-gaps.md) (v0.5.0)
- REQ-SCALE-070: The core MUST provide sequential, diverging, categorical and threshold colour scales mapping a value to a palette index. (v0.9.0)
- REQ-SCALE-080: The core MUST provide an angle scale that wraps (right ascension 0–24 h, azimuth 0–360°). (v0.12.0)

### 4.2 Ticks (TICK)

- REQ-TICK-010: Linear ticks MUST equal d3-array 3.2.4's `ticks(a, b, count)` exactly for every case of the captured fixture file, natively and in wasm. (v0.2.0)
- REQ-TICK-020: UTC time ticks MUST equal d3-time 3.1.0's `utcTicks(a, b, count)` exactly for every case of the captured fixture file. (v0.2.0)
- REQ-TICK-030: For any axis from 4 to 400 cells and any finite domain, placed ticks MUST lie in the domain, increase strictly in value and never decrease in cell, and their labels MUST lie within the axis and be separated by at least the label gap (default two cells on x, one row on y). [ADR-111](adr/111-ticks-fit-by-lowering-the-count.md) (v0.2.0)
- REQ-TICK-040: A tick mark MUST be drawn in the cell nearest its continuous position, and a gridline at the braille dot nearest it. (v0.2.0)
- REQ-TICK-050: A y axis with `snap: "rows"` MUST place every tick on a row exactly with a nice step per row; it MUST be the default for bar and candlestick y axes under 20 rows. (v0.5.0)
- REQ-TICK-060: Log axes MUST label only powers of the base and MAY mark other integer multiples without labels. (v0.5.0)
- REQ-TICK-070: A time axis SHOULD draw a context row with the date at each day boundary when its span is under two days and the layout has the row. (v0.5.0)

### 4.3 Formatting (FMT)

- REQ-FMT-010: Tick labels MUST default to d3-scale's `tickFormat` behaviour (one format for the axis, precision from the step, one SI prefix from the larger end of the domain for `s`) and MUST equal d3's labels for every case of the captured fixture file. (v0.2.0)
- REQ-FMT-020: The number formatter MUST accept the d3-format subset `[sign][$][,][.precision][~][type]` with types `f`, `s`, `%`, `e`, `d` and `auto`, and MUST reject any other specifier with an error naming the option. (v0.2.0)
- REQ-FMT-030: Tick labels, readouts, legends and table cells MUST be produced by the core's formatters, never by `core::fmt` on floats; no label MUST read `NaN`, `inf` or `-0`. (v0.2.0)
- REQ-FMT-040: The core MUST format SI units, IEC bytes (`KiB`, `MiB`), bits a second (`kb/s`, `Mb/s`), durations from nanoseconds to days, prices to a tick size (0.01, 0.25, 1/32) and signed percentages, each covered by a table test. (v0.5.0)
- REQ-FMT-050: Date labels MUST accept the strftime subset `%Y %m %d %e %H %I %M %S %L %p %a %A %b %B %j %Z`, and MUST default to 24-hour multi-scale labels unless the locale's hour cycle is 12-hour. (v0.5.0)
- REQ-FMT-060: The minus sign MUST be U+2212 in the browser and `-` in the CLI by default, switchable in the locale pack; a group separator given as U+2009, U+202F or U+00A0 MUST be drawn as one plain space cell. (v0.3.0)
- REQ-FMT-070: The core MUST format right ascension in hours (`18h 54m`) and declination in signed degrees (`−23° 45′`). (v0.12.0)

### 4.4 Time zones and locales (TZ, I18N)

- REQ-TZ-010: Time scales MUST default to UTC, and the core MUST NOT read a clock, a zone or tzdata. [ADR-112](adr/112-zones-and-locales-from-the-shell.md) (v0.2.0)
- REQ-TZ-020: The element MUST accept `timeZone` as `UTC`, a fixed offset or an IANA name, and MUST give the core a table of offset transitions covering the data's time extent and the view, computed with `Intl.DateTimeFormat`. (v0.5.0)
- REQ-TZ-030: Local-time ticks over the table MUST equal d3-scale's `scaleTime().ticks()` run under `TZ=<zone>` for every captured case, across daylight-saving changes. (v0.5.0)
- REQ-TZ-040: The CLI MUST build the same table from the system's tzdata, and a frame drawn by the CLI and by the element from the same data, options, view and zone MUST be identical. (v0.8.0)
- REQ-I18N-010: The element's locale MUST be its `locale` attribute, else the `lang` of its nearest ancestor that has one, else `navigator.language`. (v0.3.0)
- REQ-I18N-020: The shell MUST build a locale pack (decimal and group separators, grouping sizes, minus sign, percent pattern, the ten digits, month and weekday names, date and time patterns, direction) from `Intl` with the Gregorian calendar forced, once per locale, and pass it to wasm; no wasm module MUST link a locale database. (v0.3.0)
- REQ-I18N-030: Without a pack, formatting MUST use `.` as the decimal, `,` as the group, groups of three, ASCII digits and `-`. (v0.2.0)
- REQ-I18N-040: Label layout MUST count cells by `unicode-width` 0.2 a code point in release builds and draw width-2 glyphs with `set_wide`. (v0.2.0)
- REQ-I18N-050: The shell MUST NFC-normalise label text before it crosses into wasm; zero-width code points MUST take no cell; a grapheme cluster of several scalars MUST be drawn as its first scalar; the accessible name, summary and table MUST keep the original text. (v0.3.0)
- REQ-I18N-060: Labels with right-to-left runs MUST be reordered by UAX #9 before layout, and an RTL locale MUST put the y axis and the legend on the right. (v0.9.0)
- REQ-I18N-070: The CLI MUST accept a locale pack as a JSON file (`--locale-file`). (v0.8.0)

### 4.5 Layout and text (LAYOUT, TEXT, AXIS, ANNOT)

- REQ-LAYOUT-010: Layout MUST be computed in whole cells in the order title, legend, x axis, panes, y gutters, x ticks, annotations, and MUST be a pure function of cols, rows, options, the data's extent and the view. (v0.2.0)
- REQ-LAYOUT-020: When cols < 16 or rows < 5 a chart MUST draw no axis, title or legend; it MUST draw no title below 10 rows and no x labels below 6 rows. (v0.2.0)
- REQ-LAYOUT-030: Y labels MUST be dropped, the right axis first, when the gutters would take more than a third of the columns. (v0.2.0)
- REQ-LAYOUT-040: Every chart MUST draw a frame without panicking at every size from 1 × 1 to 320 × 120 with every fixture it accepts. (v0.3.0)
- REQ-LAYOUT-050: Panes MUST split rows by weight with at least three rows each, share the x scale, and align their plot areas to the same gutters. (v0.5.0)
- REQ-LAYOUT-060: A legend at `position: "auto"` MUST go outside in one row when it fits and rows ≥ 12, else inside the first pane when it takes at most a third of its width and height, else be left out of the frame (it stays in the table and the events). (v0.5.0)
- REQ-LAYOUT-070: Each chart's meta MUST declare its minimum useful size with axes; the contract and the site MUST use it. (v0.3.0)
- REQ-TEXT-010: A label MUST occupy whole cells; no two labels MUST share a cell; a label MUST clear the cells under it to the ground before it is drawn, so no label shares a cell with data; the contract MUST check this on every fixture at 80 × 24 and 320 × 120. (v0.2.0)
- REQ-TEXT-020: Draw order MUST be the z-order: data, gridlines on empty cells only, axes, labels, annotations, the crosshair, the readout last. (v0.2.0)
- REQ-AXIS-010: A pane MUST support a left and a right y axis bound to different scales, with gridlines from one axis only. (v0.5.0)
- REQ-ANNOT-010: The core MUST draw `hline`, `vline`, `band`, `label` and `marker` annotations in data coordinates, clipped to their pane; an annotation label on an axis MUST hide the tick labels it overlaps. (v0.5.0)

---

## 5. View, input and interaction

### 5.1 View state (VIEW)

- REQ-VIEW-010: Every chart MUST hold its interactive state in one view state of the shape of [view.schema.json](schemas/view.schema.json), and its frame MUST be a function of data, options, view state and `t` only: two charts given the same four MUST draw identical grids. [ADR-009](adr/009-deterministic-frames.md), [ADR-113](adr/113-view-state-is-the-model.md) (v0.3.0)
- REQ-VIEW-020: Every domain in the view state MUST be in data units, and `null` MUST mean the chart's fitted default. (v0.3.0)
- REQ-VIEW-030: The element MUST expose `view` as a property (get: the current view; set: a whole or partial view, merged, a key set to `null` returning to its default) and as an attribute of JSON text; setting `JSON.parse(JSON.stringify(el.view))` back MUST draw the same frame. (v0.3.0)
- REQ-VIEW-040: Setting `view`, `data`, `options` or `time` from script MUST NOT fire `plot-hover`, `plot-select`, `plot-brush`, `plot-view`, `plot-legend` or `plot-cursor`. (v0.3.0)
- REQ-VIEW-050: A view animation (an eased zoom, a kinetic pan, a reset) MUST be a `motion` record in the view state stamped with play time, so the frame at any `t` is determined without replaying input; with reduced motion and no `motion` attribute there MUST be none. (v0.3.0)
- REQ-VIEW-060: With `x.follow` true, an append MUST keep the x span and move the domain to end at the newest datum; a pan away from the end MUST set it false, and `End` MUST set it true. (v0.6.0)

### 5.2 Input (INPUT)

- REQ-INPUT-010: The core MUST fold input into view state with one reducer, `Chart::input`, used by the wasm module and the CLI alike; it MUST read no clock and keep no state outside the view and the chart's caches. [ADR-113](adr/113-view-state-is-the-model.md) (v0.3.0)
- REQ-INPUT-020: Input events MUST have the shape of [input.schema.json](schemas/input.schema.json), positions in cells with fractions inside a cell; the wasm boundary MUST take them as a numeric call returning a bitmask (overlay changed, base changed, events pending). [ADR-115](adr/115-numeric-input-calls.md) (v0.3.0)
- REQ-INPUT-030: The shell MUST send at most one `move` per animation frame (the latest) and MUST send `down`, `up`, `leave`, `tap`, `key` and summed `wheel` deltas, in order, before it. (v0.3.0)
- REQ-INPUT-040: By default a mouse drag MUST pan, Shift+drag MUST brush, Ctrl or ⌘ with the wheel and a trackpad pinch MUST zoom about the pointer, the plain wheel MUST scroll the page, and a double-click MUST reset the view; the options `drag` (`pan`, `brush`, `zoom`) and `wheel` (`scroll`, `zoom`) MUST change the first and the fourth. [ADR-116](adr/116-input-defaults.md) (v0.3.0)

### 5.3 Hit testing and the overlay (HIT, OVERLAY)

- REQ-HIT-010: For x-ordered charts the cursor MUST snap to the datum nearest the pointer's dot column by binary search on x, and for binned charts to the bin under the pointer; tests MUST check both against a brute-force answer over the fixtures. [ADR-117](adr/117-hit-testing-by-chart-kind.md) (v0.3.0)
- REQ-HIT-020: When a dot column holds more than one datum, `plot-hover` MUST carry the column's `aggregate` (count, first, last, minimum, maximum). (v0.3.0)
- REQ-HIT-030: A scatter chart MUST answer a hover over 1,000,000 points in under 0.1 ms median natively once its spatial index is built, and MUST answer (by scan) before it is. (v0.9.0)
- REQ-OVERLAY-010: A frame MUST be a cached base (data, axes, legend) and an overlay (crosshair, readout, brush, time cursor, focus) composed in wasm into the grid; a hover that does not change the view MUST NOT redraw the base, and MUST cost under 0.1 ms median natively at 320 × 120 with 1,000,000 points. [ADR-114](adr/114-base-and-overlay.md) (v0.3.0)
- REQ-OVERLAY-020: The readout MUST stay at a fixed place in the chart, not under the pointer, until the pointer or focus leaves, and MUST hide on Escape. (v0.3.0)

### 5.4 Events out and linking (EVENT, LINK)

- REQ-EVENT-010: The element MUST dispatch `plot-ready`, `plot-error`, `plot-hover`, `plot-select`, `plot-view`, `plot-cursor` (v0.3.0), `plot-brush`, `plot-legend` (v0.5.0) and `plot-data` (v0.6.0) as bubbling, composed `CustomEvent`s whose `detail` has the shape of [events.schema.json](schemas/events.schema.json); each reader-action event MUST carry `source` (`pointer`, `touch`, `keyboard`, `wheel`). (v0.3.0)
- REQ-EVENT-020: `plot-hover` MUST fire only when the hovered datum changes, at most once an animation frame, with `detail` `null` when the cursor leaves the plot. (v0.3.0)
- REQ-EVENT-030: `plot-view` MUST fire at most once an animation frame during a gesture with `final: false`, and exactly once with `final: true` when the gesture ends (pointer up, wheel idle 150 ms, key up, motion done). (v0.3.0)
- REQ-LINK-010: Charts sharing a `link` attribute value MUST share `x.domain` and `cursor.x` in data units without event loops; a browser test MUST show two linked charts of different widths agreeing after a pan. (v0.5.0)

### 5.5 Keyboard and touch (KEY, TOUCH)

- REQ-KEY-010: Every interaction (cursor, pan, zoom, reset, brush, select, series toggle, pause) MUST be reachable by the keys of the plan's [keys](../docs/IMPLEMENTATION_PLAN.md#keys) table, acting only while the chart has focus. (v0.3.0)
- REQ-TOUCH-010: On touch, a one-finger horizontal drag MUST move the cursor (scrub), two fingers MUST pan, a pinch MUST zoom about its centre, a double-tap MUST reset, a long press MUST start a brush, and a vertical one-finger drag MUST scroll the page (`touch-action: pan-y`). (v0.3.0)
- REQ-TOUCH-020: The shell MUST take touch from Pointer Events only. (v0.3.0)

---

## 6. The chart contract and the registry

### 6.1 Charts, meta and options (CHART, OPT, REG)

- REQ-CHART-010: A chart family MUST be one folder `crates/glyphplot/src/charts/<family>/` holding, for each chart type it plays, a `pub static META: Meta` and a `make`, and an implementation of the `Chart` trait; one `REGISTRY` line per type in the folder's `mod.rs`; one Cargo feature per family. [ADR-101](adr/101-chart-families-with-modes.md) (v0.3.0)
- REQ-CHART-020: `Meta` MUST have the fields of [meta.schema.json](schemas/meta.schema.json) (slug, name, note, family, modes, shapes, options, min, drop, mono, budget) and satisfy its rules. (v0.3.0)
- REQ-CHART-030: A variant within a family MUST be chosen by the `mode` option, which `META.modes` lists; a mode not listed MUST fail with an error naming the valid modes. (v0.3.0)
- REQ-CHART-040: The `Chart` trait MUST have exactly the methods of the plan's [`Chart` trait](../docs/IMPLEMENTATION_PLAN.md#the-chart-contract) (`set_data`, `append`, `set_options`, `view`, `set_view`, `input`, `frame`, `hit`, `table`), and `frame` MUST take its size from the grid it is given. (v0.3.0)
- REQ-CHART-050: `Env` MUST carry `paper`, `mono`, the locale pack and the zone table, and nothing else from the host. (v0.3.0)
- REQ-CHART-060: When two series draw into one cell, the cell MUST take the colour of the series highest in z-order: the later in the series list, unless the view raises a hovered or selected series. (v0.3.0)
- REQ-OPT-010: Options MUST behave as unicel's REQ-OPT-001 to REQ-OPT-030: a JSON object layered on `META.options`; typed getters falling back on a missing key or wrong type; every key a chart reads MUST have a default in `META.options`. (v0.3.0)
- REQ-OPT-020: `set_options` MUST change a live chart's options and keep its data and view; only a change of chart type makes a new chart. [ADR-120](adr/120-options-change-keeps-the-chart.md) (v0.3.0)
- REQ-REG-010: Each family's `mod.rs` MUST hold a hand-written `REGISTRY`, and `charts/mod.rs` a `FAMILIES` list behind each family's feature; `registry::make(type, options_json)` MUST return a fresh chart or fail with `there is no chart named "<type>"` or `options are not a JSON object: <message>`. [ADR-125](adr/125-hand-written-registry.md) (v0.3.0)
- REQ-REG-020: No chart type MUST be named `render`, `demo` or `list` (the registry test checks). (v0.3.0)

### 6.2 The contract test (CONTRACT, FIX)

- REQ-CONTRACT-010: `crates/glyphplot/tests/contract.rs` MUST check every registered chart against rules 1–9 and 29 of the plan's [contract rules](../docs/IMPLEMENTATION_PLAN.md#contract-rules) table (meta, options, shape, no panic, charset, colours, first frame, determinism, out-of-order calls, snapshots), over every fixture whose shape the chart accepts, at 1 × 1, `META.min`, 40 × 12, 80 × 24 and 320 × 120, with no test of the chart's own. (v0.3.0)
- REQ-CONTRACT-020: The contract MUST report every failing chart in one run, spread charts over threads, print a per-chart table with `--nocapture`, and accept `CHART=<type>[,<type>…]`, failing on an unknown type. (v0.3.0)
- REQ-CONTRACT-030: The contract MUST also check rules 10–13, 15–28 of the table: one ink and series told apart in it, paper, resize, replace is whole, view round trip, scripted input, events against the schema, hits naming what was drawn, hit and readout agreeing, empty and edge fixtures, gaps, tick labels, log-invalid values, unsorted and duplicate x, exact downsampling and wrong shapes. (v0.4.0)
- REQ-CONTRACT-040: Appending a fixture in the batches 1, 7, ⌊n/3⌋ and the rest, with frames drawn between, MUST end on the frame and view that replacing with the same rows gives; with a ring of capacity c < n, on replacing with the last c rows (rule 14). (v0.6.0)
- REQ-CONTRACT-050: Time budgets (rule 30) MUST be checked in `tests/budgets.rs`, a test binary of its own, only without debug assertions; a chart over budget MUST be timed twice more, and the lowest average and the lowest worst count. [ADR-126](adr/126-correctness-and-timing-apart.md) (v0.4.0)
- REQ-CONTRACT-060: A `contract-selftest` feature MUST register three broken charts (non-deterministic, outside its charset, keeping state across appends), and a test MUST assert that the contract rejects each with the matching message. (v0.4.0)
- REQ-FIX-010: Fixture datasets MUST be generated by `crates/glyphplot/src/fixtures.rs` from SplitMix64 with a fixed seed each, MUST include the 23 fixtures of the plan's [fixtures](../docs/IMPLEMENTATION_PLAN.md#fixtures) table, and two generations MUST be bitwise equal on every target. [ADR-128](adr/128-fixtures-generated-from-seeds.md) (v0.2.0)
- REQ-FIX-020: Real datasets checked in under `crates/glyphplot/tests/data/` MUST each be under 100 KB, public domain or MIT-compatible, named with source and licence in that directory's README. (v0.5.0)

### 6.3 Snapshots, references and properties (SNAP, REF, PROP)

- REQ-SNAP-010: Each chart MUST have one snapshot file a fixture it accepts, `crates/glyphplot/tests/snapshots/<type>/<fixture>.snap`, holding whole frames at 80 × 24, at `META.min` and after a seeded zoom, and the 320 × 120 frame as an FNV-1a hash; non-default modes on `sine-1k` get `<mode>-sine-1k.snap`. (v0.3.0)
- REQ-SNAP-020: A snapshot's colour and back layers MUST write each palette index as two lowercase hex digits (`..` for `NO_BACK`), each run followed by its length when more than one. [ADR-129](adr/129-snapshot-colours-in-hex.md) (v0.3.0)
- REQ-SNAP-030: `mise run snap [type …]` MUST rewrite snapshots; a chart without one MUST fail the contract; `web/test/snapshot.test.mjs` MUST compare every chart's wasm frame for every fixture at 80 × 24 with the native snapshot. (v0.3.0)
- REQ-SNAP-040: A commit that changes a snapshot MUST say why in its message. (v0.3.0)
- REQ-REF-010: Each port of d3-array, d3-scale, d3-time, d3-format or the LTTB reference MUST be compared bitwise with output captured from the pinned original by `mise run fixtures`, in `tests/reference.rs` and in `web/test/reference.test.mjs`. (v0.2.0)
- REQ-PROP-010: The core MUST hold proptest properties (lean features) for scale inversion, tick order and count, reduction bounds, merged reductions equalling the whole, LTTB's ends and order, cell-mapping agreement and label non-overlap, with regression files checked in. [ADR-130](adr/130-proptest.md) (v0.2.0)

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

- REQ-LINE-010: The `line` type MUST draw the modes `line`, `step` (`before`, `after`, `middle`) and `area` (to a baseline), each with snapshots. (v0.3.0)
- REQ-LINE-020: A frame over 1,000 visible points of 1,000,000 held MUST take under a tenth of the time of a frame over all 1,000,000. (v0.3.0)
- REQ-LINE-030: In `line`, `step` and `area`, a missing y MUST break the line with no segment drawn across it; the option `spanGaps: true` MAY join across it. (v0.3.0)
- REQ-LINE-040: The readout of a line-like chart MUST name the datum whose x is nearest the cursor's x value, not the minimum or maximum of the dot column under it. (v0.3.0)
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

- REQ-WASM-010: The wasm boundary MUST expose exactly the members of the plan's [boundary](../docs/IMPLEMENTATION_PLAN.md#the-boundary) table: a `Chart` object (made from a type and options, with data, options, view, input, resize, frame, the grid's pointers, text, table and free) and the module functions `charts_json`, `wide_tail` and `resolve_palette`. (v0.3.0)
- REQ-WASM-020: Making a chart MUST throw `there is no chart named "<type>"` for a type the module does not play and `options are not a JSON object: …` for bad options, and the module MUST keep working after a throw. (v0.3.0)
- REQ-WASM-030: The memory views at the grid's pointers MUST equal `text()` and the grid after each `frame`; the shell MUST re-create its views after any call that may allocate. (v0.3.0)
- REQ-WASM-040: A panicking call MUST trap without taking down the instance: other charts keep drawing and new ones can be made; the panicked chart MUST then refuse every call. [ADR-124](adr/124-abort-on-panic.md) (v0.3.0)
- REQ-WASM-050: `free()` MUST give a chart's memory back, so freeing a chart and making another of the same capacity grows no memory. (v0.3.0)
- REQ-WASM-060: With the `debug` feature only, a panic MUST print its message and location with `console.error` before it traps, and making the chart `debug-panic` MUST be possible; a release build MUST have neither. (v0.3.0)

### 8.2 Manifest and loading (MANIFEST, LOAD)

- REQ-MANIFEST-010: `web/pkg/modules.js` MUST export `charts` (every chart's meta, in registry order), `modules` (type → module), `imports` (module → a function importing `./<module>/glyphplot.js`), `fallback` and `budgets` (module → `{gzip, frameMs, worstMs, inputMs}`), of the shape of [manifest.schema.json](schemas/manifest.schema.json); the same budgets MUST be in `web/pkg/budgets.json` and the gzipped sizes in `web/pkg/sizes.json` ([sizes.schema.json](schemas/sizes.schema.json)). (v0.3.0)
- REQ-MANIFEST-020: In the release build every type MUST map to the module of its family, which plays exactly the types the manifest gives it, and `fallback` MUST be `null`; the debug build MUST map every type to `all`. (v0.3.0)
- REQ-LOAD-010: `chart(type, init)` MUST load the type's module at most once a page, share it with every later caller, reject with `there is no chart named "<type>"` for a type no module plays, and try a failed load again on the next call. (v0.3.0)
- REQ-LOAD-020: The element MUST load nothing until it first comes within one screen height of the viewport (IntersectionObserver, `rootMargin: "100% 0px"`), unless `loading="eager"` or there is no IntersectionObserver. (v0.3.0)
- REQ-LOAD-030: A `data`, `options` or `view` set and every `append` made before the chart exists MUST be applied in order once it is made; a `data` set MUST discard the appends queued before it. (v0.3.0)

---

## 9. The element, the shell and rendering

### 9.1 The element (EL)

- REQ-EL-010: Importing `glyphplot/element` MUST define `<glyph-plot>` once, inject one `<style>` with the bundled `@font-face` and the element's rules (wrapped in `:where()`), and do nothing where there is no DOM or no `customElements`. [ADR-118](adr/118-one-tag-with-a-type.md) (v0.3.0)
- REQ-EL-020: The element MUST observe exactly the attributes of the plan's [element attributes](../docs/IMPLEMENTATION_PLAN.md#element-attributes) table. (v0.3.0)
- REQ-EL-030: `define(tag, { type })` MUST define `tag` as a subclass whose chart type is `type` whatever its `type` attribute says, and MUST do nothing for a tag already defined. (v0.5.0)
- REQ-EL-040: A change of `options` or `theme` MUST NOT make a new chart: data and view after it MUST equal those before. (v0.3.0)
- REQ-EL-050: A change of `type` MUST make a new chart and give it the old chart's data and view when its meta accepts the data's shape; otherwise it MUST warn once naming the shape and draw the new chart empty. (v0.5.0)
- REQ-EL-060: The `data`, `options` and `view` properties MUST be defined on the element's class, so `"data" in document.createElement("glyph-plot")` is true before upgrade. (v0.3.0)
- REQ-EL-070: Setting the `data` property MUST replace all data and MUST NOT write the `data` attribute; getting it MUST return the descriptor and lengths, not a copy of the data. (v0.3.0)
- REQ-EL-080: Without `cols` and `rows`, the cell MUST be `round(0.6 × font-size × devicePixelRatio)` device pixels wide and exactly twice that tall, `cols × rows` the whole cells that fit the content box, and the remainder split evenly as padding. [ADR-103](adr/103-cell-from-font-grid-from-box.md) (v0.3.0)
- REQ-EL-090: A change of the element's box MUST lay the chart out again within one animation frame, keeping its view in data units; a box of width 0 MUST be ignored. (v0.3.0)
- REQ-EL-100: An element with no height of its own MUST take its height from `rows`, default 12. (v0.3.0)
- REQ-EL-110: With `cols` and `rows`, the grid MUST be that size, and `fit` (`width`, `contain`) MUST scale it as unicel's REQ-FIT-001 to REQ-FIT-006 do. (v0.5.0)
- REQ-EL-120: The element MUST fire `plot-ready` once a chart, after its first frame, and resolve `ready` then; a failed load or first frame MUST fire `plot-error` with the message and keep the element's children. (v0.3.0)
- REQ-EL-130: The element MUST use light DOM; whatever it holds before its chart's first frame (a server-rendered still) MUST stay until the drawn element replaces it. (v0.3.0)
- REQ-EL-140: Disconnecting MUST stop drawing and free the chart; a load superseded by a later change or by removal MUST free what it made and draw nothing. (v0.3.0)
- REQ-EL-150: The element MUST NOT be form-associated. (v0.3.0)

### 9.2 Mount and the JS API (MOUNT, JS, SSR)

- REQ-MOUNT-010: `mount(el, chart, { input })` MUST draw into a `<pre>` (text) or a `<canvas>` according to the element given, and with `input` MUST turn pointer, wheel, touch and key events on `el` into `chart.input` calls and dispatch the events of REQ-EVENT-010 on `el`; `stop()` MUST remove every listener and observer and cancel the frame loop. (v0.3.0)
- REQ-MOUNT-020: A chart MUST be drawn only while it intersects the viewport and the document is visible; `t` MUST advance only while a `motion` record is in flight. (v0.3.0)
- REQ-JS-010: `chart(type, init)` and the returned `Chart`'s `data`, `append`, `options`, `view`, `input`, `frame`, `resize`, `text`, `table` and `free` MUST work with no DOM; a Node test MUST draw a frame and compare its text with the Rust crate's. (v0.3.0)
- REQ-SSR-010: `glyphplot/node` MUST export `renderText(type, init, cols, rows)` and `renderSVG(type, init, cols, rows)` that load the module from the package's files with no network, and its text MUST equal `glyphplot render <type> --format txt` for the same input. (v0.8.0)

### 9.3 Rendering (RENDER)

- REQ-RENDER-010: On a canvas, a frame's draw cost MUST NOT depend on the number of distinct (glyph, colour) pairs: a braille scatter of 320 × 120 cells with every cell a different colour MUST draw within twice the time of the same frame in one colour in headless Chrome (`mise run bench:render`). [ADR-119](adr/119-no-glyph-and-colour-atlas.md) (v0.3.0)
- REQ-RENDER-020: Block, quadrant, sextant, octant and wedge glyphs MUST be filled as exact shapes (`blocks.ts`), never drawn from the font. (v0.3.0)
- REQ-RENDER-030: A frame MUST redraw only the cells whose glyph, ink or back changed since the last frame drawn; a crosshair move over an unchanged chart MUST redraw at most the cells of the old and new crosshair and readout. (v0.3.0)
- REQ-RENDER-040: A frame whose cells carry back colours SHOULD be drawn as one image of `cols × 2·rows` pixels scaled without smoothing, with other glyphs over it. (v0.5.0)
- REQ-RENDER-050: The renderer (`fillText` per changed cell, or an atlas keyed by code point and tinted by compositing) MUST be the one the owner's Chrome and Safari measurements choose in v0.3.0, recorded in the plan with the numbers. (v0.3.0)

### 9.4 Font (FONT)

- REQ-FONT-010: The bundled font MUST be a subset of Iosevka Fixed Extended 34.9.0 (SIL OFL 1.1), family `glyphplot mono`, covering unicel's ranges plus tier A (the plan's [font](../docs/IMPLEMENTATION_PLAN.md#font) tier A: U+00A0–00BF, U+00D7, U+00F7, U+2007–200A, U+2010–2015, U+2018–201F, U+2026, U+202F, U+2030, U+2032–2033, U+2070–209F, the currency signs, U+2116, U+2122, U+2212, U+221E, U+2248, U+2260, U+2264–2265, Δ Σ Ω μ σ, U+2713, U+2717, U+2605–2606, U+FFFD), every glyph 0.6 em, at most 32 KB, rebuilt only by `mise run build:font`. [ADR-121](adr/121-extend-the-bundled-font.md) (v0.2.0)
- REQ-FONT-020: The font SHOULD also cover U+00C0–00FF and U+0100–017F (tier B). (v0.5.0)
- REQ-FONT-030: `@font-face` MUST load the font from `new URL("../fonts/glyphplot-mono.woff2", import.meta.url)` with `font-display: swap`, and every surface (the `<pre>`, the canvas, the SVG export) MUST put `"glyphplot mono"` first in its font stack. (v0.3.0)

---

## 10. Theme and one ink

### 10.1 Theme (THEME)

- REQ-THEME-010: The element MUST read the custom properties `--glyphplot-ground`, `--glyphplot-ink`, `--glyphplot-muted`, `--glyphplot-rule`, `--glyphplot-accent`, `--glyphplot-series-1` to `--glyphplot-series-8`, `--glyphplot-up`, `--glyphplot-down` and `--glyphplot-scale` from its computed style, each falling back to the default set; a `theme` property set to a theme object ([theme.schema.json](schemas/theme.schema.json): the same tokens, `series` as an array of up to eight) MUST win over them. [ADR-122](adr/122-theme-tokens-and-custom-properties.md) (v0.3.0)
- REQ-THEME-020: Without a ground, a chart MUST draw no ground and MUST use the light default set when its computed `color` has luma `0.2126 r + 0.7152 g + 0.0722 b < 128`, the dark set otherwise. (v0.3.0)
- REQ-THEME-030: The element MUST re-resolve its theme and repaint when `prefers-color-scheme` changes, when `<html>`'s `class`, `style` or `data-theme` changes, and when `theme` is set, without reloading its module or drawing a new frame. (v0.3.0)
- REQ-THEME-040: Every default series, `up`, `down` and `accent` colour MUST have a WCAG 2.2 contrast ratio of at least 3:1 against its default ground, and `ink` and `muted` at least 4.5:1. [ADR-123](adr/123-colour-defaults-held-to-measured-bars.md) (v0.3.0)
- REQ-THEME-050: Each default categorical palette MUST hold eight colours whose smallest pairwise CIEDE2000 difference is at least 15 under normal vision and under deuteranopia, protanopia and tritanopia simulated by Machado 2009 at severity 1.0 on linear sRGB. (v0.3.0)
- REQ-THEME-060: Up and down MUST be theme tokens, never fixed colours, and candles and bars MUST tell them apart by fill (hollow and solid) as well as by colour. (v0.5.0)
- REQ-THEME-070: In a debug build, a page-set series colour under 3:1 against the resolved ground MUST log one warning naming the token and the ratio, and MUST still be used. (v0.3.0)
- REQ-THEME-080: The default sequential scale sampled at nine steps MUST be monotonic in L* with neighbours at least 5 ΔE00 apart under each of the four visions, its low end the end nearer the ground's lightness; the default diverging scale MUST run blue to orange with its middle on the ground's side. (v0.9.0)
- REQ-THEME-090: The JS API, the CLI (`--theme <file.json>`), the Rust crate and the SVG and ANSI exports MUST accept a theme object with REQ-THEME-010's token names. (v0.8.0)
- REQ-THEME-100: Under `forced-colors: active` the element MUST draw in one ink in `CanvasText` on `Canvas`, with the accent in `Highlight`; under `@media print` it SHOULD draw on paper. (v0.7.0)

### 10.2 One ink (MONO)

- REQ-MONO-010: In one ink, line series 1–4 MUST draw with the dash patterns solid, 3 on 1 off, 1 on 1 off, and 3 on 1 off 1 on 1 off, counted in dot columns from the x scale's origin, so a pan by n whole dots shifts the pattern by n dots. [ADR-123](adr/123-colour-defaults-held-to-measured-bars.md) (v0.3.0)
- REQ-MONO-020: In one ink, series 5–8 MUST repeat patterns 1–4 and add a marker each from `● ○ ■ □ ▲ △ ◆ ◇`, at most one marker a cell. (v0.5.0)
- REQ-MONO-030: In one ink, bar and area series MUST fill with `█ ▓ ▒ ░` by series. (v0.5.0)
- REQ-MONO-040: The legend MUST draw each series' one-ink key (pattern, marker or fill) beside its name, in colour and in one ink. (v0.3.0)
- REQ-MONO-050: With up to four series and room, a line chart SHOULD label each series at its last visible point. (v0.9.0)

---

## 11. Accessibility

- REQ-A11Y-010: The element MUST have `role="figure"` and an accessible name from `label`, else `<chart type>: <series names>`; its drawing MUST have `role="img"` and an `aria-label` giving the chart type, the series names, each axis's title and visible domain in the resolved locale, rewritten at most once a second. [ADR-132](adr/132-figure-slider-and-hidden-table.md) (v0.3.0)
- REQ-A11Y-020: The element MUST hold, in light DOM and visually hidden, a `<table>` with a caption, `th scope="col"` headers and a row header a row, of the data in the current view, at most 500 rows (downsampled, each bucket's minimum and maximum kept), its caption stating how many points the view holds when it holds more. (v0.3.0)
- REQ-A11Y-030: An interactive chart MUST have exactly one tab stop, a `role="slider"` cursor whose `aria-valuetext` gives the cursor's x and each series' value there (up to four, then `and N more`), moved by the keys of the plan's [keys](../docs/IMPLEMENTATION_PLAN.md#keys) table, which `aria-keyshortcuts` MUST list. (v0.3.0)
- REQ-A11Y-040: Every pan, zoom and brush done by dragging MUST also be possible with single clicks or taps: a control row (zoom out, zoom in, reset, step back, step on) shown on focus and hover, and click-click brushing. (v0.3.0)
- REQ-A11Y-050: Under `prefers-reduced-motion: reduce` and without `motion`, view changes MUST jump with no animated transition. (v0.3.0)
- REQ-A11Y-060: The focused chart MUST show a `:focus-visible` outline at least 2 px wide in `accent`, at least 3:1 against the ground. (v0.3.0)
- REQ-A11Y-070: An axe-core scan of a page holding every chart, in light and dark, MUST find no violation of impact serious or critical. (v0.3.0)
- REQ-A11Y-080: A chart receiving streamed data MUST support `paused` (property and attribute) and Space on the cursor: while paused it MUST keep appended data without drawing it, and on resume MUST show the live view. (v0.6.0)
- REQ-A11Y-090: While data streams, the table MUST be rebuilt at most once a second and never while focus is inside it. (v0.6.0)
- REQ-A11Y-100: With `announce="<seconds>"` (at least 5), a polite live region MUST announce the latest value at most once per interval, silent while paused or while the cursor has focus; without it there MUST be no live region. (v0.6.0)
- REQ-A11Y-110: A "show data" control MUST make the table visible. (v0.7.0)
- REQ-A11Y-120: `glyphplot <type> --table` MUST print the data table as text. (v0.8.0)

---

## 12. Budgets

Every budget lives in one place, the chart's `META.budget` (with the family
default), and reaches `mise run size`, the contract and the benches through
`web/pkg/budgets.json`, as unicel's `Category::budget()` does.

### 12.1 Size (SIZE)

- REQ-SIZE-010: `mise run size` MUST fail when any module's `glyphplot_bg.wasm` passes its budget gzip -9 (204,800 bytes unless its budget says otherwise), naming each module over with its size and its limit. [ADR-008](adr/008-a-generous-size-threshold.md), [ADR-133](adr/133-a-200-kb-module-gate.md) (v0.1.0)
- REQ-SIZE-020: `mise run size` MUST print each module's wasm and JS glue, raw and gzipped, and the total, and append those lines to `$GITHUB_STEP_SUMMARY` when it is set. (v0.1.0)
- REQ-SIZE-030: `WASM_GZ_LIMIT` MUST override every module's limit, in bytes. (v0.1.0)
- REQ-SIZE-040: `mise run size` SHOULD print a warning for each module past half its budget. (v0.1.0)
- REQ-SIZE-050: `mise run size --charts` MUST list, for each module, the gzipped share of the chart's own code, the core, serde and serde_json, and the rest (std, alloc, wasm-bindgen), from an unstripped build. (v0.4.0)
- REQ-SIZE-060: No wasm module's dependency tree MUST hold chrono, time, jiff, chrono-tz, num-format, any `icu_` crate, arrow, arrow2 or vsop87, which `mise run lint` checks with `cargo tree -e normal --target wasm32-unknown-unknown`. (v0.2.0)
- REQ-SIZE-070: A pack module MUST stay under 32 KB gzipped. (v0.10.0)

### 12.2 Time (PERF)

- REQ-PERF-010: Downsampling MUST be on by default for every chart that draws more points than its plot has dot columns. (v0.3.0)
- REQ-PERF-020: A chart MUST redraw only when its data, options, view, size or theme changed since the last frame, and at most once a display frame. (v0.3.0)
- REQ-PERF-030: In a release build, every chart MUST average at most 4 ms a frame with no frame over 16 ms, over 300 frames of a view panning across a fixture of 1,000,000 points at 80 × 24 and 320 × 120 with default options, measured natively by `tests/budgets.rs`. (v0.4.0)
- REQ-PERF-040: In a release build, every chart MUST average at most 1 ms a frame with no frame over 8 ms over the same play with a fixture of 100,000 points. (v0.4.0)
- REQ-PERF-050: A pointer move (hit test, crosshair and readout) MUST average at most 0.5 ms with none over 2 ms over 300 moves across a chart holding 1,000,000 points, natively in release and in wasm in headless Chromium. (v0.4.0)
- REQ-PERF-060: In headless Chromium on the reference machine, every chart MUST average at most 8 ms a frame with no frame over 16 ms in the play of REQ-PERF-030, checked by `mise run bench:web` in `signoff`. (v0.4.0)
- REQ-PERF-070: Appending one sample at 1 kHz to a full ring of 1,000,000 points while redrawing at 60 Hz MUST cost at most 500 ms of CPU a second in headless Chromium, the summed time of appends and frames over 10 simulated seconds. (v0.6.0)
- REQ-PERF-080: A page holding one chart MUST draw its first frame within 1.0 s of navigation start on a 170 ms, 9 Mbps profile and within 3.0 s on Lighthouse 12's `mobileSlow4G` request-level profile (562.5 ms, 1.44 Mbps, CPU 4×), median of 7 cold loads in headless Chromium, checked by `mise run bench:load` in `signoff`. (v0.4.0)

### 12.3 Memory (MEM)

- REQ-MEM-010: A chart of capacity n points in two f64 columns MUST hold at most 16 × n bytes + 1 MiB of wasm memory, measured as the growth of `memory.buffer.byteLength` in Node from making the chart to its first frame. (v0.3.0)
- REQ-MEM-020: The element MUST load a typed-array column into wasm memory with at most one copy. (v0.3.0)
- REQ-MEM-030: Replacing a chart's data with data of the same or smaller size, or freeing a chart and making another of the same capacity, MUST NOT grow wasm memory. (v0.3.0)

### 12.4 Benches (BENCH)

- REQ-BENCH-010: `mise run bench [type …]` MUST print the load average, then a row per type, fixture size (1k, 100k, 1M) and grid size (80 × 24, 320 × 120) with average, worst, budget and `OVER`, and exit 1 when any row is over. (v0.4.0)
- REQ-BENCH-020: `mise run bench:web`, `bench:load` and `bench:render` MUST run the discovery harnesses' successors in headless Chromium against the release build and print their tables with the load average. (v0.4.0)

---

## 13. Testing

- REQ-TEST-010: `mise run test` MUST pass before every commit, and `mise run check` MUST be green. (v0.1.0)
- REQ-TEST-020: The Node tests (`web/test/*.test.mjs`) MUST run against the release modules in `web/pkg` and against a debug build, find modules with the shell's own compiled `moduleOf`, and run the panic tests against the debug build only. (v0.3.0)
- REQ-TEST-030: Browser behaviour on real devices (pixels, fonts, touch, screen readers, Safari) MUST be checked by the owner; agents check headless and hand the manual checks over as a list. (v0.3.0)
- REQ-TEST-040: `site/test.html` MUST be a plain HTML checklist page (no Elm) with one `<glyph-plot>` per rendering and interaction case and a button that copies the pass as Markdown. (v0.3.0)
- REQ-E2E-010: `mise run test:site` MUST run Playwright in headless Chromium against `vite preview` of the release build of `site/`. (v0.3.0)
- REQ-E2E-020: `site/e2e/interaction.spec.ts` MUST cover pointer hover with its event, drag pan, wheel zoom, keyboard cursor, pan, zoom, reset and escape, a brush, and touch scrub, pan and pinch driven through CDP `Input.dispatchTouchEvent`. (v0.3.0)
- REQ-E2E-030: `site/e2e/element.spec.ts` MUST cover attributes and properties, `view` set and read, a module fetched once for many tags, calls queued before load, and teardown and freeing on attribute changes and removal. (v0.3.0)
- REQ-E2E-040: `site/e2e/resize.spec.ts` MUST show that a change of container width changes `cols`, keeps the view in data units and logs no console error. (v0.3.0)
- REQ-E2E-050: `site/e2e/streaming.spec.ts` MUST append at 1 kHz for a second and then find the frame equal to a replace of the same rows. (v0.6.0)
- REQ-E2E-060: Every chart type's page on the site MUST draw a first frame with nothing on the console. (v0.7.0)
- REQ-E2E-070: `mise run check` MUST run `test:site --smoke` (a chart's first frame, a hover, a drag, a wheel, a key and the axe scans, under 60 s); `signoff` MUST run all of `test:site`. (v0.3.0)

---

## 14. The CLI

- REQ-CLI-010: The CLI (crate `glyphplot-cli`, binary `glyphplot`) MUST accept `glyphplot <type>`, `render <type>`, `demo <type>`, `list`, `-h` and `-V` with the flags of the plan's [CLI](../docs/IMPLEMENTATION_PLAN.md#the-cli), and reject an unknown flag, a second type and a missing or out-of-range number as usage errors. [ADR-134](adr/134-cli-on-crossterm.md) (v0.8.0)
- REQ-CLI-020: The CLI MUST exit 0, 2 or 130 as the plan's [CLI](../docs/IMPLEMENTATION_PLAN.md#the-cli) exit table says, with every error on stderr as `glyphplot: <message>`; a data error MUST name the line and column. (v0.8.0)
- REQ-CLI-030: An unknown type MUST be an error naming up to eight suggestions, as unicel's REQ-CLI-010. (v0.8.0)
- REQ-CLI-040: With stdin not a terminal and no `--from`, or `--from -`, the CLI MUST read data from stdin and keys and mouse from the controlling terminal (`/dev/tty`). (v0.8.0)
- REQ-CLI-050: Rows arriving on stdin while playing MUST be appended at most once a frame, and `--window N` MUST keep only the last N rows. (v0.8.0)
- REQ-CLI-060: With stdout not a terminal, the CLI MUST read to the end of input and print one frame as text with no escapes; a closed stdout MUST end it with exit 0 and nothing on stderr. (v0.8.0)
- REQ-CLI-070: Playing MUST enable mouse capture unless `--no-mouse`, and MUST send mouse and keys through the core's reducer (REQ-INPUT-010) with the keys of the plan's [keys](../docs/IMPLEMENTATION_PLAN.md#keys) plus `h j k l`, `f` (follow) and `q`. (v0.8.0)
- REQ-CLI-080: `--color auto` MUST be `none` when `NO_COLOR` is set and not empty, `truecolor` when `COLORTERM` is `truecolor` or `24bit`, `256` when `TERM` contains `256color`, else `16`. (v0.8.0)
- REQ-CLI-090: The CLI MUST restore the terminal (alternate screen, cursor, wrapping, raw mode, mouse capture) on every way out: the end, a key, Ctrl+C, SIGTERM, SIGHUP and a panic. (v0.8.0)
- REQ-CLI-100: `glyphplot demo <type>` MUST say, on the chart's first row or above it, that the data is generated. (v0.8.0)
- REQ-CLI-110: The CLI MUST take `--glyphs` (`braille` by default, `octants`, `sextants`, `quadrants`, `ascii`) and convert every frame with the vendored `glyphs::convert`, live and piped. (v0.8.0)
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
- REQ-SITE-040: The site MUST show the examples of the plan's [site examples](../docs/IMPLEMENTATION_PLAN.md#site-examples), the first two being a live candlestick of a synthetic market labelled "simulated market" and a live candlestick of Wikimedia edits a second, one candle a minute. [ADR-136](adr/136-the-sites-market-demo.md) (v0.7.0)
- REQ-SITE-050: An example over a live third-party feed MUST name the source and its licence on the page, and MUST show a message, not an error, when the feed is down. (v0.7.0)
- REQ-SITE-060: The site MUST follow `prefers-color-scheme` until the reader chooses, and its theme button MUST set `data-theme` on `<html>` and keep it in `localStorage`, every chart following it (REQ-THEME-030). (v0.7.0)
- REQ-SITE-070: An axe scan of the index, a chart page and an example, in both themes, MUST find no violation of impact serious or critical. (v0.7.0)
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
- REQ-PKG-040: The crates MUST be `glyphplot` (library), `glyphplot-cli` (binary `glyphplot`), `glyphplot-wasm` (`publish = false`) and the packs `glyphplot-<domain>`. (v0.1.0)
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
| v0.1.0 | 0 · Toolchain green | ARCH-010, 040, 060, 070; VENDOR-010; BUILD-010–030, 050, 060; LINT-010, 030; CI-010; SIZE-010–040; TEST-010; PKG-040 | `mise run check` exits 0 on an empty family; `mise run size` prints the empty module |
| v0.2.0 | 1 · Core | GRID, CANVAS, COLOR-010–040, DATA-010–040, 080–090, 110, 130, SCALE-010–030, TICK-010–040, FMT-010–030, TZ-010, I18N-030–040, LAYOUT-010–030, TEXT, FIX-010, REF-010, PROP-010, FONT-010, VENDOR-020, BUILD-070, LINT-040, SIZE-060 | `mise run test:rust`: reference fixtures 0 differ; properties pass; `fixtures` regenerates bitwise |
| v0.3.0 | 2 · Line end to end | the `line` type; VIEW-010–050; INPUT; HIT-010–020; OVERLAY; EVENT-010–030; KEY; TOUCH; CHART; OPT; REG; CONTRACT-010–020; SNAP; WASM; MANIFEST; LOAD; EL (most); MOUNT; JS; RENDER-010–030, 050; FONT-030; THEME-010–050, 070; MONO-010, 040; A11Y-010–070; PERF-010–020; MEM; DATA-050–070, 120; STREAM-060; SAMPLE-010–030; I18N-010–020, 050; FMT-060; LAYOUT-040, 070; CHAR; TEST-020–040; E2E-010–040, 070; BUILD-040, 080; LINT-020; ARCH-020–050 | `mise run check`; `mise run test:site` (interaction, element, resize, axe) |
| v0.4.0 | 3 · Contract and budgets | CONTRACT-030, 050, 060; PERF-030–060, 080; SIZE-050; BENCH; CI-020; BUILD-090 | `cargo test --profile contract --test contract --test budgets`; `mise run bench`; `mise run signoff` on the branch |
| v0.5.0 | 4 · Candles, bar, sparkline | CANDLE-010–050; BAR-010–020; SPARK-010; SCALE-040–060; TICK-050–070; FMT-040–050; TZ-020–030; LAYOUT-050–060; AXIS; ANNOT; LINK; EL-030, 050, 110; EVENT-010 (`plot-brush`, `plot-legend`); SAMPLE-040, 060; DATA-100; THEME-060; MONO-020–030; RENDER-040; FONT-020; FIX-020 | contract and snapshots for the three types; the owner's candlestick on the checklist page |
| v0.6.0 | 5 · Streaming | STREAM-010–050, 070; VIEW-060; CANDLE-060; CONTRACT-040; DATA-140–150; A11Y-080–100; PERF-070; E2E-050; EVENT-010 (`plot-data`) | `streaming.spec.ts`; `bench:web` streaming row; the owner's one-line test on a phone |
| v0.7.0 | 6 · The site | SITE; DATASET; A11Y-110; THEME-100; E2E-060; LINT-020 (site); PKG-060 | `mise run build:site`; `test:site` in full; `datasets:check` |
| v0.8.0 | 7 · The CLI | CLI; TERM; CRATE-010; EXPORT; SSR; TZ-040; I18N-070; COLOR-050; THEME-090; A11Y-120; ARCH-080 | `cargo test -p glyphplot-cli`; `glyphplot candles --from feed.csv` by the owner |
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
