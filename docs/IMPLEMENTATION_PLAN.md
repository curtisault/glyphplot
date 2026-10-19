# glyphplot: implementation plan

For the sessions that build glyphplot. Read it top to bottom once; then work
phase by phase. Every phase ends with acceptance criteria that a named command
verifies. Nothing in this document is a guess unless marked **(unverified)**:
every number says how it was measured, and the measurements are gathered in
[Appendix A](#appendix-a), each pointing at the discovery report and the bench
that produced it. The requirements this plan builds are in
[spec/ROADMAP.md](../spec/ROADMAP.md), by ID; the decisions are in
[spec/adr/](../spec/adr/).

- [Goal](#goal)
- [Decisions already made](#decisions-already-made)
- [Decisions discovery made](#decisions-discovery-made)
- [What the repository holds](#what-the-repository-holds)
- [Architecture](#architecture)
- [The chart contract](#the-chart-contract)
- [The data contract](#the-data-contract)
- [The boundary](#the-boundary)
- [Rendering and colour](#rendering-and-colour)
- [Scales, axes and layout](#scales-axes-and-layout)
- [Theme and accessibility](#theme-and-accessibility)
- [The CLI](#the-cli)
- [The site](#the-site)
- [Domain packs](#domain-packs)
- [Phases](#phases)
  - [0 · Toolchain green (v0.1.0)](#phase-0)
  - [1 · Core (v0.2.0)](#phase-1)
  - [2 · Line end to end (v0.3.0)](#phase-2)
  - [3 · Contract and budgets (v0.4.0)](#phase-3)
  - [4 · Candles, bar, sparkline (v0.5.0)](#phase-4)
  - [5 · Streaming (v0.6.0)](#phase-5)
  - [6 · The site (v0.7.0)](#phase-6)
  - [7 · The CLI (v0.8.0)](#phase-7)
  - [8 · The next tier (v0.9.0)](#phase-8)
  - [9 · Domain packs (v0.10.0)](#phase-9)
  - [10 · Packaging and deploy (v0.11.0)](#phase-10)
  - [11 · Sky and orbits (v0.12.0)](#phase-11)
- [Testing strategy](#testing-strategy)
- [Performance and size budgets](#performance-and-size-budgets)
- [Risks](#risks)
- [Open questions](#open-questions)
- [Appendix A: measurements](#appendix-a)
- [Appendix B: datasets](#appendix-b)
- [Appendix C: unicel as prior art](#appendix-c)

---

## Goal

An interactive charting library whose charts are drawn in Unicode glyphs
(braille, sextants, octants, blocks, box drawing, arrows, geometric shapes),
each cell one code point and one colour. Charts are Rust, compiled to
WebAssembly one module per chart family, played in the browser by a
`<glyph-plot>` custom element over a small TypeScript shell and in the
terminal by a CLI. It is the sibling of unicel (`../animate-unicode`) and
takes its architecture whole. What differs:

1. **The data is the user's.** A chart draws data the page or the terminal
   gives it, once, replaced, or streamed, and stays cheap while it does.
2. **One wasm module per chart family**, so a page pays only for the charts
   it shows.
3. **Interactive**: pan, zoom, crosshair, readouts, brushing, keyboard and
   touch, scrubbing, events out to the page.
4. **A generous size threshold**, measured and set (200 KB gzipped a module
   proposed; a complete line chart measured 17.4 KB without serde and
   49.3 KB with it).
5. **Rich and fast**: scales, ticks, number and date formatting, legends,
   annotations, panes, themes, one ink and paper, a frame budget that holds at
   a million points (measured 2.1–2.4 ms natively, 3.2–3.7 ms in Chromium).

The owner's test: a page shows a live candlestick chart of a market feed in a
`<glyph-plot>` tag, the reader scrubs it with a finger, and the page loaded
one small wasm module to do it. That is v0.6.0's acceptance.

Non-goals until the roadmap says otherwise: a React wrapper (JSX typings
instead, v0.11.0), pages computing in a worker (ADR-142), charts generating
their own data except a demo mode that says so (ADR-006), a hosted CI
(ADR-003).

## Decisions already made

The owner chose these with the trade-offs in view. Each is an Accepted ADR;
do not relitigate them.

| Decision | Choice | Why | ADR |
|---|---|---|---|
| Name | `glyphplot`: crate, npm package, tag prefix `<glyph-plot>` | Says what it is. Free on npm and crates.io on 2026-10-18 (surfaces.md checked `crates.io/api/v1/crates/<name>` and `registry.npmjs.org/<name>`: 404). | [001](../spec/adr/001-name-glyphplot.md) |
| Its own repository | `~/projects/glyphplot`, separate from unicel | Different purpose, users and budgets. It shares no code with unicel ([ADR-013](../spec/adr/013-independent-of-unicel.md)). | [002](../spec/adr/002-its-own-repository.md) |
| Architecture | unicel's: pure Rust core, wasm-bindgen with wasm-pack, TypeScript shell with a custom element, CLI, Elm site, mise, GitButler, local signoff | Proven in unicel; the owner knows it. | [003](../spec/adr/003-unicels-architecture.md) |
| Frame buffer | unicel's grid: `u32` code point and colour indices a cell, typed-array views over wasm memory, `WIDE_TAIL` | No per-frame copying or string building. Colour stays an index ([ADR-102](../spec/adr/102-semantic-palette-of-255.md)). | [004](../spec/adr/004-unicels-frame-buffer.md) |
| One wasm module per chart type | Yes, a Cargo feature each | A page that shows a line chart does not load a histogram. Families with modes ([ADR-101](../spec/adr/101-chart-families-with-modes.md)). | [005](../spec/adr/005-one-wasm-module-per-chart-type.md) |
| Data is injected | Every chart draws the page's or terminal's data; a demo mode says so | The whole point. | [006](../spec/adr/006-data-is-injected.md) |
| Interactive | From the first chart | The contract and budgets include input. | [007](../spec/adr/007-interactive-from-the-first-chart.md) |
| Size threshold | One a module, generous | Discovery proposes 200 KB gzipped ([ADR-133](../spec/adr/133-a-200-kb-module-gate.md)); the owner sets it. | [008](../spec/adr/008-a-generous-size-threshold.md) |
| Determinism | A frame is a pure function of data, options, view state and `t` | Snapshots, seeks and tests at arbitrary times. Streaming appends do not break it. | [009](../spec/adr/009-deterministic-frames.md) |
| Spec format | unicel's `spec/` | The owner builds from it with agents; IDs are what commits and tests name. | [010](../spec/adr/010-unicels-spec-format.md) |
| Licence | MIT | As unicel. | [011](../spec/adr/011-mit-licence.md) |
| Independent of unicel | Nothing copied, ported, vendored or generated from unicel's repository; no dependency on it | A distinctly separate project; unicel is prior art, cited for design as uPlot is | [013](../spec/adr/013-independent-of-unicel.md) |
| No AI attribution | Never, in commits, pull requests, notes or releases | As unicel's `AGENTS.md`. | [012](../spec/adr/012-no-ai-attribution.md) |

## Decisions discovery made

Each has a real alternative and is a Proposed ADR until the owner accepts it.
The ones the owner must settle first are also [open questions](#open-questions).

| Decision | Choice | Why (measured) | ADR |
|---|---|---|---|
| Core code | Rejected by the owner: discovery proposed vendoring unicel's grid, canvases, glyph tables, block shapes and font; glyphplot writes its own (ADR-013) | Vendoring measured 3,043 B gz for grid + braille; crates.io refuses git dependencies | [100](../spec/adr/100-vendor-unicels-core.md) (Rejected) |
| Module split | One module per family; variants by `mode`; `sparkline` a type of the `line` module | ~45 chart names become ~18 modules, six in the first tier; scales + time cost 3.8 KB gz, so a sparkline module of its own saves little | [101](../spec/adr/101-chart-families-with-modes.md) |
| Cell colour | `u8` index into a per-chart palette of 255 with a fixed semantic layout | 128 scale steps fall under a visible step (ΔE76 2.26); a cell stays 6 bytes (RGB: 12); themes and terminal downgrades are table swaps | [102](../spec/adr/102-semantic-palette-of-255.md) |
| Sizing | Cell from the font size (0.6 em by 1.2 em); grid from the box | Square braille dots; text never scaled below legibility | [103](../spec/adr/103-cell-from-font-grid-from-box.md) |
| Data in | Typed `f64` columns plus a JSON descriptor; text parsed outside chart modules | 1M rows: typed arrays 4.3 ms; JSON in wasm 163 ms, +80 MiB never returned, +26.5–34.4 KB gz a module | [104](../spec/adr/104-typed-columns-cross-the-boundary.md) |
| Arrow | A hand-written IPC reader in the core | +3.8 KB gz, 7.4 ms for 1M rows; arrow-rs +262 KB | [105](../spec/adr/105-hand-written-arrow-reader.md) |
| Data changes | Apply at once; no easing | A tween shows values that never existed | [106](../spec/adr/106-data-applies-at-once.md) |
| Live data | A ring allocated once; equal x replaces; late rows dropped and counted | 0.062 µs an append; never grows after the first fill | [107](../spec/adr/107-a-ring-allocated-once.md) |
| Downsampling | M4 per dot column, cached; LTTB opt-in | Exact frames; LTTB missed a column's extremes in 95% of columns | [108](../spec/adr/108-m4-per-dot-column.md) |
| Narrow candles | Resample to calendar intervals | Index-aligned merging changes a candle's meaning as the view pans | [109](../spec/adr/109-candles-resample-to-calendar-intervals.md) |
| Scales and formats | Hand-ported from d3, fixture-tested bitwise | +5.5 KB gz; 0 of 803 cases (scales.md) and 0 of 20,651 tick cases (testing.md) differ; chrono +13.8 KB, ICU4X dates +121 KB | [110](../spec/adr/110-scales-and-formats-ported-from-d3.md) |
| Tick fitting | Lower the count until whole labels fit, in cells | 0 overlaps in 400,000 random axes | [111](../spec/adr/111-ticks-fit-by-lowering-the-count.md) |
| Zones and locales | Resolved by the shell with `Intl`, crossing once as tables | tzdata in wasm +150–161 KB; a year's zone table 0.5–1.7 ms in Node | [112](../spec/adr/112-zones-and-locales-from-the-shell.md) |
| Interaction model | One view state; one reducer in the core; no input log | A log of 512 events forgets old zooms; a page-set view is not an event | [113](../spec/adr/113-view-state-is-the-model.md) |
| Hover | A cached base and an overlay composed in wasm | Hover 0.6–5 µs; a full redraw 1.5–2.3 ms at 1M points | [114](../spec/adr/114-base-and-overlay.md) |
| Input calls | Numeric on the hot path; JSON canonical for tests and the CLI | JSON adds 0.4–0.5 µs an event and serde on the hot path | [115](../spec/adr/115-numeric-input-calls.md) |
| Input defaults | Drag pans, wheel scrolls the page, one finger scrubs, arrows step datum by datum | No scroll trap in an article; the owner's finger test | [116](../spec/adr/116-input-defaults.md) |
| Hit testing | By chart kind, through one cell-mapping module | Two "equal" column formulas disagreed on 1,137 of 12M points | [117](../spec/adr/117-hit-testing-by-chart-kind.md) |
| Element | One tag with `type`, plus `define(tag, {type})` aliases | A type change keeps data, view and listeners; a TSX union types it (tsc 5.9.3) | [118](../spec/adr/118-one-tag-with-a-type.md) |
| Renderer | No (glyph, colour) atlas; `fillText` or a tinted code-point atlas, chosen on the owner's browsers | unicel's atlas: 3.6% hits, 61 ms a frame for a coloured scatter at 80 × 24 | [119](../spec/adr/119-no-glyph-and-colour-atlas.md) |
| Options | `set_options` keeps the chart (reverses unicel's ADR-025) | A chart holds up to a million rows of the page's data | [120](../spec/adr/120-options-change-keeps-the-chart.md) |
| Font | Its own subset of Iosevka with chart text (tier A) | Minus, µ, …, currency missing; +7.2 KB woff2 | [121](../spec/adr/121-extend-the-bundled-font.md) |
| Theme | Tokens; CSS custom properties first; a theme object elsewhere | A page's own dark and light rules re-theme charts with no script | [122](../spec/adr/122-theme-tokens-and-custom-properties.md) |
| Colour defaults | Palettes held to 3:1 and ≥ 15 ΔE00 under four visions; cividis; blue–orange | No well-known palette passes both (Okabe–Ito 10.9; Tableau 10 0.7 for deuteranopes) | [123](../spec/adr/123-colour-defaults-held-to-measured-bars.md) |
| Panics | `panic = "abort"`; the contract is the guard | As unicel ADR-011; unwinding costs size | [124](../spec/adr/124-abort-on-panic.md) |
| Registry | Hand-written, a line a type | As unicel ADR-012 | [125](../spec/adr/125-hand-written-registry.md) |
| Contract layout | Correctness across threads; budgets alone in their own binary | unicel's contract took 32.5 s idle and 61–71 s under load | [126](../spec/adr/126-correctness-and-timing-apart.md) |
| Test profile | `profile.contract`: release without LTO | Rebuild after an edit 1.2–1.3 s (at opt-level 2) against 7.4–11.6 s; the contract no slower | [127](../spec/adr/127-test-profile-without-lto.md) |
| Fixtures | Generated from seeds (SplitMix64); three small real datasets | All 14 bench fixtures in 1.1 ms; the 1M fixtures are 16 MB | [128](../spec/adr/128-fixtures-generated-from-seeds.md) |
| Snapshots | Colour layers as two hex digits an index | unicel's base-64 digit holds 64 colours | [129](../spec/adr/129-snapshot-colours-in-hex.md) |
| Property tests | proptest, lean | Seeded replay; 4.7 s clean dev build | [130](../spec/adr/130-proptest.md) |
| Session gaps | An index time scale | No calendar data needed | [131](../spec/adr/131-index-time-scale-for-session-gaps.md) |
| Semantics | `role="figure"`, a summary on `role="img"`, a slider cursor, a hidden table ≤ 500 rows | 500 rows build in 6 ms, 10,000 in 523 ms | [132](../spec/adr/132-figure-slider-and-hidden-table.md) |
| Size gate | 200 KB gzipped a module, warning at 100 KB | 4× today's full module; ~0.8 s and 2.9 s to first frame on the two load profiles | [133](../spec/adr/133-a-200-kb-module-gate.md) |
| CLI | crossterm, with its own terminal code | 404,800 B against ratatui's 553,968 B; ratatui's `Chart` has no ticks, downsampling or hit tests | [134](../spec/adr/134-cli-on-crossterm.md) |
| Exports | txt, ansi, svg in Rust from the start | Byte-identical element and CLI output | [135](../spec/adr/135-exports-in-rust.md) |
| The site's market | Synthetic, labelled; Wikimedia edits a second as a real feed | No exchange's terms allow display to third parties | [136](../spec/adr/136-the-sites-market-demo.md) |
| Datasets | A manifest with hashes and a licence allow-list | Licences are checked, not remembered | [137](../spec/adr/137-dataset-manifest-and-allow-list.md) |
| Domain packs | Rust crates, their own wasm modules | Indicators 1.5 KB gz, planets 6.5 KB gz; one implementation for web and terminal | [138](../spec/adr/138-domain-packs-are-crates-and-modules.md) |
| Indicators | In the markets pack, incremental | The RSI and MACD panes are not candlestick charts | [139](../spec/adr/139-indicators-in-the-markets-pack.md) |
| Celestial positions | Charts take positions; the astronomy pack computes planets from JPL elements | Within 35″ (Mars) and 123″ (Jupiter) of Horizons, a dot is 2,025″; VSOP87A is 688 KB gz | [140](../spec/adr/140-celestial-positions-in-a-pack.md) |
| serde_json | In every module for options, meta, the descriptor and the view | +33 KB gz; revisit at the 100 KB warning | [141](../spec/adr/141-serde-json-in-chart-modules.md) |
| Thread | The main thread | Frames 2–4 ms at 1M points in Chromium | [142](../spec/adr/142-main-thread-not-a-worker.md) |
| The site's shape | Documentation with live charts; the examples page is the gallery | A chart library is learnt, not browsed | [143](../spec/adr/143-site-docs-with-live-charts.md) |

## What the repository holds

On 2026-10-18, after discovery. Nothing under `crates/`, `web/` or `site/`
exists yet.

```
AGENTS.md, CLAUDE.md        conventions for agents (CLAUDE.md is a copy)
README.md, LICENSE          what glyphplot is; MIT, Curtis Ault
mise.toml                   the toolchain pinned as unicel pins it; a placeholder `check`
rust-toolchain.toml         Rust 1.97, wasm32, rustfmt, clippy
rustfmt.toml                max_width 160
Cargo.toml                  workspace, no members yet, unicel's release profile; excludes discovery/
.gitignore
discovery/                  ten reports and README.md; bench/ holds their throwaway measurement code
docs/IMPLEMENTATION_PLAN.md this file
spec/                       README, SPEC (0.0.0), ROADMAP, adr/, schemas/, conformance.md
```

`mise install` succeeds (checked 2026-10-18). `discovery/bench/*/` are
standalone crates and Node projects with their own `[workspace]` tables;
their README files say how to rerun every number in Appendix A.

## Architecture

```
            ┌──────────────────────────────────────────────────────────────┐
            │ crates/glyphplot  (pure Rust: no DOM, no canvas, no terminal) │
            │  grid · canvases · cell mapping · palette and theme::resolve │
            │  data: Table, columns, ring, M4, Arrow reader                 │
            │  scales · ticks · format · layout · text                     │
            │  view state · input reducer · overlay · hit tests            │
            │  charts/<family>/ (a Cargo feature each) · registry          │
            │  export::{text, ansi, svg} · fixtures · tests/contract.rs    │
            └───────┬─────────────────────────┬───────────────────┬────────┘
                    │                         │                   │
   ┌────────────────▼────────────┐ ┌──────────▼─────────┐ ┌───────▼─────────────┐
   │ crates/wasm (glyphplot-wasm)│ │ crates/cli          │ │ crates/packs/<domain>│
   │  Chart: data, options, view,│ │  (glyphplot-cli,    │ │  markets, telemetry, │
   │  input, resize, frame,      │ │  crossterm) every   │ │  astronomy: columns  │
   │  pointers, text, table, svg │ │  family and pack,   │ │  in, columns out;    │
   │  built once per family      │ │  native             │ │  own wasm modules    │
   └────────────────┬────────────┘ └─────────────────────┘ └──────────────────────┘
                    │ web/pkg/<family>/glyphplot{.js,_bg.wasm}, modules.js,
                    │ budgets.json, sizes.json   (generated)
   ┌────────────────▼─────────────────────────────────────────────────────────┐
   │ web/src  (TypeScript shell, npm package "glyphplot")                      │
   │  wasm.ts     a module per family, loaded on first use, shared             │
   │  chart.ts    chart(type, init): the Chart object; queue before load       │
   │  mount.ts    <pre> or <canvas>; changed cells only; input, gestures       │
   │  theme.ts    custom properties → theme object → resolve_palette           │
   │  locale.ts   Intl → locale pack; zone.ts: Intl → offset table             │
   │  a11y.ts     figure, summary, slider cursor, hidden table, controls       │
   │  element.ts  <glyph-plot type data src options view theme …>              │
   └────────────────┬─────────────────────────────────────────────────────────┘
                    │ Vite alias → ../web/src
   ┌────────────────▼─────────────────────────────────────────────────────────┐
   │ site/  (Elm 0.19.1 + Vite): docs, a page per chart type, examples;        │
   │        test.html, the checklist page (plain HTML, from v0.3.0)            │
   └──────────────────────────────────────────────────────────────────────────┘
```

### Per-frame data flow

1. The shell marks a chart dirty when its data, options, view, size or theme
   changed, or a `motion` record is in flight, and draws it on the next
   animation frame while it is visible (REQ-PERF-020, REQ-MOUNT-020).
2. `chart.frame(t, paper)`. The chart lays out again only if its size,
   options, data extent or domains changed (layout cached); rebuilds the
   base (data marks, axes, legend) only if the data version, view domains,
   size or options changed (M4 aggregates cached by data version, x range and
   width, REQ-SAMPLE-020); composes the overlay (crosshair, readout, brush,
   time cursor, focus) over the base into the grid (REQ-OVERLAY-010).
3. The shell makes `Uint32Array` and `Uint8Array` views at `cells_ptr`,
   `colors_ptr` and `backs_ptr` (fresh: memory may have grown), diffs them
   against the last frame drawn, and draws only changed cells
   (REQ-RENDER-030), each colour looked up in the palette table it resolved
   once for the current theme (REQ-COLOR-030).
4. When the view settles (`final`) or data commits, the shell asks
   `table_json(500)` and `summary()` and rewrites the hidden table and the
   drawing's label, at most once a second (REQ-A11Y-010, REQ-A11Y-020).

In a `<pre>` step 3 is `textContent = chart.text()`.

### Per-event data flow

1. A DOM pointer, wheel, touch or key event reaches `mount`. The shell turns
   pixels into cells with fractions using the box it measured at the last
   resize, and recognises gestures (tap, double-tap, long press, pinch,
   two-finger pan) (REQ-INPUT-030).
2. Discrete events (down, up, leave, tap, key, summed wheel) queue in order;
   only the latest move is kept.
3. On the next animation frame the shell calls `chart.input(kind, x, y, a, b,
   buttons, mods, t)` for each, which returns a bitmask: overlay changed, base
   changed, events pending (REQ-INPUT-020).
4. If events are pending, the shell reads `events_json()` once and dispatches
   each as a `plot-*` `CustomEvent` (REQ-EVENT-010).
5. If the overlay or base changed, the frame of the previous section follows
   in the same animation frame. A hover changes about rows + cols + the
   readout's width in cells (interaction.md measured 8 of 1,920 cells a hover
   at 80 × 24), so step 3 of the frame redraws only those.

### Memory and lifetime

- One instance of each family's module a page, shared by every tag of that
  family (REQ-LOAD-010). Wasm memory never shrinks (the core instruction set
  has `memory.grow` and no shrink), so a page that loads a million points
  twice through a copying path holds twice the memory for its life: data
  enters with at most one copy (REQ-MEM-020).
- A chart is a Rust object owned by JS; `free()` gives its memory back to the
  module's allocator (REQ-WASM-050). The element frees on disconnect and on a
  type change; a load superseded by a later change frees what it made
  (REQ-EL-140).
- Views over memory are re-made after any call that may allocate: `reserve_column`,
  `commit`, `append`, `set_arrow`, `resize`, `frame`. Take the address first,
  then read `memory.buffer` (REQ-DATA-060): evaluating `memory.buffer` before
  `reserve` threw "Cannot perform Construct on a detached ArrayBuffer" in
  data.md's first run.
- A panicking call traps (`panic = "abort"`); the instance lives on; the
  panicked chart refuses every later call, and the element stops it and
  logs (REQ-WASM-040).

## The chart contract

The sibling of unicel's piece contract. A chart family is a folder,
`crates/glyphplot/src/charts/<family>/`, behind a Cargo feature of the same
name; each chart type it plays has a `META` and a `make`, and one line in
the folder's `REGISTRY` (REQ-CHART-010, REQ-REG-010). A chart uses the core
and its own folder only (REQ-ARCH-050).

```rust
pub static META: Meta = Meta {
    slug: "line", name: "line", note: "series over a continuous x, as lines, steps or areas",
    family: "line", modes: &["line", "step", "area"],
    shapes: &["series"],
    options: Some(r#"{"mode":"line","step":"after","spanGaps":false}"#),
    min: Size { cols: 23, rows: 6 }, drop: &[Part::Title, Part::Legend, Part::Axes],
    mono: Mono::Authored, charset: Charset::Extended,
    budget: Budget::FAMILY_DEFAULT,
};
pub fn make(options: &Options) -> Result<Box<dyn Chart>, MakeError>;

pub trait Chart {
    fn set_data(&mut self, data: Data) -> Result<Accepted, DataError>;     // replace all
    fn append(&mut self, batch: Data) -> Result<Appended, DataError>;      // streaming (v0.6.0)
    fn set_options(&mut self, options: &Options) -> Result<(), MakeError>; // keeps data and view
    fn view(&self) -> &View;
    fn set_view(&mut self, patch: &ViewPatch);                             // merge; null resets
    fn input(&mut self, event: &Input) -> Changes;                         // the reducer; events queued
    fn frame(&mut self, t: f64, env: &Env, grid: &mut Grid);               // size from the grid
    fn hit(&self, at: CellPos) -> Option<Hit>;
    fn table(&self, max_rows: usize) -> Table;                             // for the hidden table
}
pub struct Env<'a> { pub paper: bool, pub mono: bool, pub locale: &'a LocalePack, pub zone: &'a ZoneTable }
```

The names are the plan's; the shape is what the requirements fix
(REQ-CHART-040, REQ-CHART-050). `Meta` is serialised by serde with the
fields of [meta.schema.json](../spec/schemas/meta.schema.json); `options`
holds the defaults (unicel's REQ-OPT-030 rule: every key read has one).
`frame` must keep everything it caches keyed by what it depends on, so the
frame is a pure function of data, options, view and `t` whatever order it is
asked in (ADR-009).

The registry, `registry::make(type, options_json)`, returns the meta and a
fresh chart, or `there is no chart named "<type>"`, or
`options are not a JSON object: <message>`.

<a id="contract-rules"></a>
### Contract rules

`crates/glyphplot/tests/contract.rs` enforces rules 1–29 on every registered
chart; `tests/budgets.rs` enforces rule 30 alone (ADR-126). Size set S: 1 × 1
(no panic only), `META.min`, 40 × 12, 80 × 24 and 320 × 120. Times: 0, 1, 2.5
and 6. A "frame" is cells, colours and backs, compared bitwise. From
testing.md, with the milestone each rule lands in.

| # | Rule | Detail | Milestone |
|---|---|---|---|
| 1 | meta | Slug kebab-case and unique; name; note 1–72 characters; modes listed; shapes listed; min size within 1–320 × 1–120; budget given | v0.3.0 |
| 2 | options | Every option read has a default in `META.options`; an unknown mode is an error naming the valid ones | v0.3.0 |
| 3 | shape | At every size in S, `grid.text()` has `rows` lines of `cols` cells | v0.3.0 |
| 4 | no panic | Every fixture × size × mode × env draws without a panic, 1 × 1 included | v0.3.0 |
| 5 | charset | Every code point outside label text is in the font's ranges; label text from data may be any Unicode, wide as `set_wide` + `WIDE_TAIL` | v0.3.0 |
| 6 | colours | Every index below the palette's length, or `NO_BACK` for backs | v0.3.0 |
| 7 | good first frame | Every non-empty fixture inks the plot area at the default view: ≥ 1 cell for `one`, ≥ 2% of plot cells otherwise | v0.3.0 |
| 8 | determinism | Two fresh charts given the same data, options, view and `t` draw the same frame | v0.3.0 |
| 9 | out-of-order calls | One chart asked for t = 6, 1, 2.5, 0 with `view()`, `hit()` and a repeated `frame()` between draws what a fresh chart draws in order | v0.3.0 |
| 10 | one ink | `mono` frames keep rules 3–7 and pass the one-ink checks (ink 1–90%, no slab, glyphs in the font) | v0.4.0 |
| 11 | one ink tells series apart | In `multi-4`, each series draws a glyph or pattern the others do not | v0.4.0 |
| 12 | paper | `paper` frames keep rules 3–6 | v0.4.0 |
| 13 | resize | A chart drawn at A, then B, then A draws its first A frame again; its B frame equals a fresh chart's; its view is unchanged in data units | v0.4.0 |
| 14 | streaming equals replace | Appending in batches of 1, 7, ⌊n/3⌋ and the rest ends on the frame and view of a replace; with ring capacity c < n, of the last c rows | v0.6.0 |
| 15 | replace is whole | Replacing with a, then b, draws what b alone draws | v0.4.0 |
| 16 | view round trip | `set_view(view())` changes nothing; a view through JSON onto a fresh chart draws the same frame | v0.4.0 |
| 17 | input is deterministic | A scripted input list on two fresh charts gives the same events, views and frames; the resulting view on a third gives that frame | v0.4.0 |
| 18 | events validate | Every event serialises to JSON that `events.schema.json` accepts | v0.4.0 |
| 19 | hit names what was drawn | For up to 512 seeded plot cells, a named datum maps into that cell's column by the cell-mapping module; for a mark chart the frame drew its mark there | v0.4.0 |
| 20 | hit and readout agree | The readout for a cursor at a cell names the datum `hit` returns there | v0.4.0 |
| 21 | empty data | `empty` draws axes or "no data", `hit` is none, domains null | v0.4.0 |
| 22 | one point | `one` draws its point in a padded domain; no label reads `NaN`, `inf` or `-0` | v0.4.0 |
| 23 | gaps break lines | In line-like modes no dot column inside a NaN run is inked, unless `spanGaps` | v0.4.0 |
| 24 | tick labels distinct and fitting | No two labels on an axis are equal; none overruns its gutter | v0.4.0 |
| 25 | log-invalid values | Values ≤ 0 on a log scale are skipped, no panic, no NaN cells | v0.4.0 |
| 26 | unsorted and duplicate x | `unsorted-x` draws its sorted copy's frame; `duplicate-x` is deterministic | v0.4.0 |
| 27 | downsampling is exact | A line-like frame of x-sorted data equals the frame from every segment | v0.4.0 |
| 28 | wrong shape | Data of a shape the chart does not accept is an error naming the shape | v0.4.0 |
| 29 | snapshots | Frames match the snapshot files | v0.3.0 |
| 30 | budgets | Release only: frame average and worst by size and visible points, and a pointer move, against `META.budget`; over budget, timed twice more, the best counts | v0.4.0 |

The contract reports every failure in one run, prints a per-chart table with
`--nocapture`, and takes `CHART=<type>[,…]` (REQ-CONTRACT-020). A
`contract-selftest` feature registers three broken charts the contract must
reject (REQ-CONTRACT-060). The milestone column follows the brief's order:
the line chart ships with the rules a first chart needs, and Phase 3
completes the matrix. testing.md estimates rules 3–29 at about 1 s a chart
on one thread and rule 30 at about 2 s **(unverified: no chart exists)**.

<a id="fixtures"></a>
### Fixtures

Generated by `crates/glyphplot/src/fixtures.rs` from SplitMix64 with a fixed
seed each (REQ-FIX-010); `cargo run --example fixtures -- write target/fixtures`
writes them for Node as little-endian `f64`.

| Fixture | Shape | Rows | For |
|---|---|---|---|
| `empty` | series | 0 | no data: no panic, "no data", hit none |
| `one` | series | 1 | a degenerate domain |
| `two` | series | 2 | the smallest line |
| `sine-1k` | series | 1,000 | a smooth reference frame; mode snapshots |
| `random-walk-100k` | series | 100,000 | the 100k budget row |
| `random-walk-1m` | series | 1,000,000 | the 1M budget row; exact downsampling |
| `nan-gaps` | series | 10,000 | every fourth block of 500 NaN |
| `inf` | series | 1,000 | ±Infinity among values |
| `constant` | series | 1,000 | a padded domain; distinct labels |
| `huge-range` | series | 1,000 | 10^-300 to 10^300 |
| `tiny-range` | series | 1,000 | 1 + i·10^-15 |
| `negative` | series | 1,000 | log-invalid values |
| `zero-crossing` | series | 1,000 | symlog, baselines |
| `unsorted-x` | series | 1,000 | a seeded shuffle |
| `duplicate-x` | series | 1,000 | three rows an x |
| `steps-10k` | series | 10,000 | plateaus, the step mode |
| `multi-4` | 4 series | 4 × 1,000 | one ink tells series apart; z-order |
| `labels-wide` | 3 series | 3 × 100 | CJK and combining marks in names |
| `ohlcv-days` | ohlcv | 500 | weekend gaps, a limit-down day, zero volume |
| `ohlcv-ticks-1m` | ohlcv | 1,000,000 | candles resampled from a million rows |
| `categorical-12` | counts | 12 | bars; labels longer than their band |
| `matrix-64` | matrix | 64 × 64 | heatmaps, colour scales |
| `timestamps-dst` | series, time | 2,000 | hourly across a daylight-saving change |

Real datasets, each under 100 KB and public domain, are checked in under
`crates/glyphplot/tests/data/` in v0.5.0 (REQ-FIX-020): a year of EIA WTI
daily prices, the 2026 Treasury par yield curve, a day of USGS earthquakes
([Appendix B](#appendix-b)).

## The data contract

[data.schema.json](../spec/schemas/data.schema.json) is the shape;
data.md has the reasoning. In short:

- **Tables of typed columns, and matrices.** Column types `f64`, `f32`,
  `i64`, `u32`, `bool`, `timestamp`, `duration`, `category`, `string`. A
  column has `name`, `type`, optional `role` (x, y, open, high, low, close,
  volume, lo, hi, value, category, size, color, label, id, source, target,
  weight, group, ra, dec, lon, lat, az, alt, mag, …), `unit`, `scale` and
  `offset` (display only), `time_unit` (`s`, `ms`, `us`, `ns`), `zone`
  (ticks only) and `categories` (REQ-DATA-010).
- **Held as `f64`.** Timestamps in epoch ms; `us` and `ns` as offsets from an
  `i64` origin, exact for spans under 2^53 ns, 104 days at 1 ns (REQ-DATA-020).
- **Missing** is NaN (or `u32::MAX` for categories); a missing y breaks a
  line; a missing x drops the row (REQ-DATA-030).
- **Order**: x is checked non-decreasing on load; unsorted data is drawn as a
  stably sorted copy with one warning (REQ-DATA-040).
- **Channels** come from the `encode` option, then `role`, then the column's
  name, so `{"t": […], "close": […]}` draws with no configuration
  (REQ-DATA-080).
- **Shapes** a chart's `META.shapes` names: `series`, `ohlcv`, `values`,
  `bins`, `counts`, `points`, `matrix`, `graph`, `geo`, `sky`, `orbits`,
  `ephemeris` (data.md, "Domain shapes").
- **Streaming**: `append` takes a row, rows or typed columns; the shell
  coalesces appends a frame; rows live in a ring of `retain.rows` (default
  1,000,000) and optional `retain.age`; an equal x replaces the last row; an
  earlier x is dropped and counted in `plot-data`'s `late` (REQ-STREAM-010 to
  REQ-STREAM-070, ADR-107). Appending and replacing with the same rows end on
  the same frame (rule 14).
- **Data changes apply at once** (REQ-DATA-130, ADR-106).

How data arrives, by surface:

| Surface | Forms | Where text is parsed |
|---|---|---|
| Element | `data` property (data object, columns or rows shorthand, `{descriptor, columns: {name: TypedArray}}`, Arrow `ArrayBuffer` from v0.6.0); `data` attribute (small JSON); `src` URL (v0.6.0); `append()` | the shell (`JSON.parse`, a small CSV reader); Arrow in the core |
| JS API | `chart(type, {data, options, view})`, `c.data = …`, `c.append(…)` | as the element |
| wasm boundary | `reserve_column(i, kind, n)` + JS `set` + `commit(descriptor)`, or `set_column(i, Float64Array)`; `staging(n)` + `append(n)`; `set_arrow(bytes)` | never in chart modules (REQ-DATA-050) |
| CLI | `--from FILE` or stdin (`-`); `--format csv\|tsv\|json\|ndjson\|arrow`; `--x`, `--y`; `--window N`; lines streamed from a pipe | the core's `text` feature, native only |
| Rust crate | `Table::from_columns`, `from_arrow_ipc`, and with `text` `from_csv`, `from_json` | the core |

## The boundary

<a id="the-boundary"></a>
### The wasm `Chart`

Built by `crates/wasm` once per family. The shell is its only consumer
(REQ-ARCH-030). Members (REQ-WASM-010):

| Member | Returns | What | From |
|---|---|---|---|
| `new Chart(type, options_json)` | `Chart` | A chart of a type this module plays, options layered on its defaults. Throws `there is no chart named "<type>"` or `options are not a JSON object: …` | v0.3.0 |
| `set_options(json)` | — | New options; data and view kept (ADR-120) | v0.3.0 |
| `reserve_column(i, kind, n)` | pointer | A buffer of `n` values for column `i` the shell fills through a view | v0.3.0 |
| `set_column(i, values)` | — | The same from a `Float64Array` (one copy by the glue) | v0.3.0 |
| `commit(descriptor_json)` | integer | Applies the reserved columns as the new data; returns the data version | v0.3.0 |
| `set_locale(json)`, `set_zone(json)` | — | The locale pack and zone table ([locale.schema.json](../spec/schemas/locale.schema.json), [zone.schema.json](../spec/schemas/zone.schema.json)) | v0.3.0, v0.5.0 |
| `view_json()`, `set_view(json)` | string, — | The view state, whole or a patch ([view.schema.json](../spec/schemas/view.schema.json)) | v0.3.0 |
| `input(kind, x, y, a, b, buttons, mods, t)` | integer | One input event; returns the bitmask overlay changed (1), base changed (2), events pending (4) | v0.3.0 |
| `events_json()` | string | The events the last inputs produced, as `[{type, detail}]` ([events.schema.json](../spec/schemas/events.schema.json)) | v0.3.0 |
| `resize(cols, rows)` | — | A new grid size; the view kept in data units | v0.3.0 |
| `set_mono(on)` | — | One ink for the frames that follow | v0.3.0 |
| `frame(t, paper)` | — | Draws the frame at `t` into the grid | v0.3.0 |
| `cols()`, `rows()`, `cells_ptr()`, `colors_ptr()`, `backs_ptr()` | integers, pointers | The grid, read as views | v0.3.0 |
| `text()` | string | The frame as text | v0.3.0 |
| `summary()`, `table_json(max_rows)` | string | The drawing's label and the hidden table's rows | v0.3.0 |
| `meta_json()` | string | The type's `Meta` | v0.3.0 |
| `staging(n)`, `append(n)` | pointer, integer | Streaming: a staging buffer of `n` rows, then commit it; returns rows, dropped, late | v0.6.0 |
| `set_arrow(bytes)` | integer | An Arrow IPC stream as the new data | v0.6.0 |
| `svg(theme_json)`, `ansi(theme_json)` | string | The exports, in Rust (ADR-135) | v0.8.0 |
| `free()` | — | Releases the chart | v0.3.0 |

| Module function | What |
|---|---|
| `default(init)` / `initSync` | Instantiates the module (wasm-bindgen `--target web`) |
| `charts_json()` | Every type's meta this module plays |
| `wide_tail()` | `WIDE_TAIL` (0) |
| `resolve_palette(theme_json)` | The 255 RGBA entries for a theme (`theme::resolve`, REQ-COLOR-030) |
| `start()` | Debug builds only: the panic hook |

### The manifest

`crates/glyphplot/examples/manifest.rs` writes `web/pkg/modules.js`,
`modules.d.ts`, `budgets.json` and `sizes.json`
([manifest.schema.json](../spec/schemas/manifest.schema.json)): `charts`
(every type's meta), `modules` (type → module: `line` and `sparkline` →
`line`), `imports`, `fallback` (`null` in release, `"all"` in the debug
build) and `budgets` (REQ-MANIFEST-010). `wasm.ts` imports a module once a
page and shares it (REQ-LOAD-010), as unicel's does.

<a id="element-attributes"></a>
### The element

`<glyph-plot type="candles">`, sized by its container. One tag; `define(tag,
{type})` makes per-type aliases (ADR-118). Light DOM (REQ-EL-130).

| Attribute | Value | On change | From |
|---|---|---|---|
| `type` | a chart type: `line`, `sparkline`, `candles`, `bar`, … | a new chart; data and view carried when accepted | v0.3.0 |
| `data` | JSON text, small data | replaces the data | v0.3.0 |
| `src` | URL of CSV, JSON, NDJSON or Arrow | fetched, replaces the data | v0.6.0 |
| `options` | JSON object text | `set_options`; data and view kept | v0.3.0 |
| `view` | JSON object text | sets the view; fires no event | v0.3.0 |
| `theme` | `auto` (default), `dark`, `light`, `paper` | repaints | v0.3.0 |
| `locale` | a BCP 47 tag | a new locale pack, repaint | v0.3.0 |
| `mono` | present or not | one ink, drawn in a `<pre>` | v0.3.0 |
| `motion` | present or not | view animations play under reduced motion | v0.3.0 |
| `label` | text | the accessible name | v0.3.0 |
| `cols`, `rows` | integers | fix the grid's size (`rows` alone sets the height; default 12 without a CSS height) | v0.3.0 |
| `fit` | `width`, `contain` | with `cols` and `rows`: scales the grid as unicel's `fit` | v0.5.0 |
| `loading` | `lazy` (default), `eager` | `eager` skips the near-screen gate | v0.3.0 |
| `link` | a group name | shares `x.domain` and `cursor.x` with the group | v0.5.0 |
| `paused` | present or not | streamed data is kept, not drawn | v0.6.0 |
| `announce` | seconds, at least 5 | a polite live region of the latest value | v0.6.0 |
| `time` | seconds | holds the frame at `t` (tests, snapshots) | v0.3.0 |
| `debug` | present or not | a frame-cost overlay, its code loaded only then | v0.4.0 |

| Property | What |
|---|---|
| `type`, `options`, `view`, `theme` | as the attributes, as objects; `theme` takes a theme object ([theme.schema.json](../spec/schemas/theme.schema.json)) |
| `data` | set: replace all; get: the descriptor and lengths (REQ-EL-070) |
| `paused` | boolean |
| `chart` | the JS `Chart` once made, else `null` |
| `ready` | a promise resolved after the first frame |
| `cols`, `rows`, `currentTime` | read-only |

| Method | What |
|---|---|
| `append(rows)` | streaming; queued before load |
| `clear()` | empties the data |
| `resetView()` | the fitted view |
| `text()` | the frame as one-ink text |
| `toSVG()`, `toANSI()` | from Rust (v0.8.0) |
| `toPNG(scale?)` | from the shell's `paint` (v0.8.0) |

Events, `CustomEvent`s that bubble and are composed, fired for the reader's
actions and for loading, never for a property set by script (REQ-VIEW-040,
REQ-EVENT-010): `plot-ready`, `plot-error`, `plot-hover`, `plot-select`,
`plot-view` (with `final`), `plot-cursor`, `plot-brush`, `plot-legend`,
`plot-data`. Shapes: [events.schema.json](../spec/schemas/events.schema.json).

<a id="keys"></a>
### Keys

On the focused chart (its slider cursor), the same in the CLI (REQ-KEY-010,
REQ-A11Y-030). Arrows step datum by datum because it is the only way a
keyboard or screen-reader user reads values one at a time (ADR-116).

| Key | Does |
|---|---|
| ← → (↓ ↑) | the cursor to the previous or next datum; past the edge, the view pans |
| Home, End | the first or last datum in view; End also sets `follow` on a stream |
| PageUp, PageDown | the cursor a tenth of the view |
| Shift+← → | pan a tenth of the view |
| + (=), − | zoom about the cursor |
| 0 | reset the view |
| Enter | lock the cursor and select the datum |
| Shift+Enter | start, then commit, a brush from the cursor |
| Escape | hide the readout, clear the brush, unlock; a second Escape blurs |
| Space | pause or resume a stream |
| 1–9 | toggle the nth series |
| ? | show the keys |
| CLI only: h j k l, f, q | ← ↓ ↑ →, follow the live end, quit |

Pointer and touch defaults (REQ-INPUT-040, REQ-TOUCH-010): a drag pans,
Shift+drag brushes, Ctrl or ⌘ with the wheel and a trackpad pinch zoom about
the pointer, the plain wheel scrolls the page, a double-click resets; on
touch one finger scrubs, two pan, a pinch zooms, a double-tap resets, a long
press brushes, and vertical swipes scroll the page (`touch-action: pan-y`).
Options `drag` and `wheel` change the defaults.

### The view state

[view.schema.json](../spec/schemas/view.schema.json): `x {domain, follow}`,
`y {domain, fit}`, `axes`, `cursor {x, y, series, index, locked}`,
`selection {x, y, points}`, `hidden`, `time` (the page's time cursor) and
`motion` (an eased or kinetic view change stamped with play time). Domains
are in data units; `null` is "fit". A page links charts by setting one's
`view` from another's `plot-view`; a URL holds `JSON.stringify(el.view)`
(interaction.md, "Linked views").

## Rendering and colour

<a id="palette"></a>
### Palette

A cell's ink and back are `u8` indices into a palette of 255 entries with a
fixed layout; the frame never holds a colour (ADR-102, REQ-COLOR-020). The
core's `theme::resolve(theme)` turns a theme object into the 255 RGBA
entries; the shell calls it through `resolve_palette` when the theme changes
and repaints from the table, with no new frame (REQ-COLOR-030).

| Index | Token | Used for |
|---|---|---|
| 0 | `ink` | titles, tick labels, readouts (what `Grid::clear` writes) |
| 1 | `muted` | axes, ticks |
| 2 | `rule` | gridlines (decoration, exempt from contrast) |
| 3 | `accent` | crosshair, focus |
| 4 | `ground` | the chart's ground, when it has one |
| 5, 6 | `up`, `down` | candles, bars, signed values |
| 7 | `selection` | brushed ranges, selected data |
| 8–15 | `series-1` … `series-8` | series colours |
| 16–31 | reserved | |
| 32–159 | scale steps 0–127 | a continuous scale, interpolated in Oklab between the theme's stops |
| 160–254 | reserved | a second scale, if a chart needs one |
| 255 | `NO_BACK` | no back colour |

128 steps: viridis's largest step is ΔE76 2.26 at 128 entries and 4.55 at 64,
where about 2.3 is the smallest step people see (rendering.md §2.2).

### Renderer

On a canvas, the shell draws only the cells whose glyph, ink or back changed
(REQ-RENDER-030). Block, quadrant, sextant, octant and wedge glyphs are exact
fills computed by glyphplot's own `blocks.ts` from each glyph's Unicode definition (REQ-RENDER-020). unicel's atlas keyed by
(code point, colour) is dropped: a braille scatter coloured by value hit it
3.6% of the time and took 61 ms a frame at 80 × 24 even with 64 colours
(ADR-119). Two candidates remain, measured in headless Chrome 155:

| Renderer | Braille scatter 80 × 24 | 320 × 120 | Density 320 × 120 |
|---|---|---|---|
| `fillText` per cell, no atlas | 0.9–1.0 ms | 16.6 ms | 21.0 ms |
| atlas keyed by code point, tinted by compositing | 2.4–2.8 ms | 34.0–35.1 ms | 46.5 ms |

The owner measures both on Chrome and Safari with a GPU in Phase 2 (the
bench page `discovery/bench/rendering/canvas/` runs as it is) and the plan
records the choice (REQ-RENDER-050). A full redraw of 320 × 120 braille costs
16–21 ms at best, so a full-screen heatmap that changes every cell holds about
30 frames a second; charts whose data changes few cells (lines, hovers) are
unaffected.

### Cells and size

The cell is `round(0.6 × font-size × devicePixelRatio)` device pixels wide
and exactly twice that tall, so braille and octant dots are square; `cols ×
rows` are the whole cells that fit the content box, the remainder split as
padding (REQ-EL-080, ADR-103). A box without a height takes 12 rows. A resize
lays out again within one animation frame, keeping the view in data units;
width 0 is ignored. Safari has no `devicePixelContentBoxSize` (MDN
compatibility data), so the shell uses `contentBoxSize × devicePixelRatio`
there. Digits at 1× separate by 7 px from an 11 px font and by 1 px at 8 px
(rendering.md §4.3, a proxy for legibility): a floor of 11 px at 1× and 9 px
from 2× is proposed until the owner checks by eye.

Minimum useful sizes with axes (rendering.md §5, design arithmetic,
**(unverified)** by eye): line, area and scatter 23 × 6; candles 28 × 8;
bar (2n + 6) × 6 for n bars (a cell a bar, a cell between, 7 cells of y gutter, 2 rows of x axis); sparkline 8 × 1. Below its minimum a chart sheds parts
in its declared order (title, legend, axes), and it draws without panicking
down to 1 × 1 (REQ-LAYOUT-040).

<a id="font"></a>
### Font

The bundled font is glyphplot's own subset of Iosevka Fixed Extended 34.9.0
(SIL OFL 1.1), family `glyphplot mono`, built by `mise run build:font` with
the repository's own `scripts/font.py` from Iosevka's upstream release, pinned
by SHA-256; no file or script is taken from unicel (ADR-013, ADR-121,
REQ-FONT-010). The sizes below were measured in discovery by subsetting the
same release:

| Tier | Adds | woff2 (measured) | Milestone |
|---|---|---|---|
| base | ASCII, `° · •`, arrows, box drawing, blocks, geometric shapes, braille, octants, legacy computing | 18,128 B rebuilt | v0.2.0 |
| A: numbers and units | U+00A0–00BF, `× ÷`, U+2007–200A, U+2010–2015, U+2018–201F, `…`, U+202F, `‰`, `′ ″`, U+2070–209F, currency U+20A0–20C0 (as Iosevka has them), `№ ™`, `−` U+2212, `≈ ≠ ≤ ≥ ∞`, `Δ Σ Ω µ σ`, `✓ ✗ ★ ☆`, U+FFFD | 25,344 B (+7,216) | v0.2.0 |
| B: Western European letters | U+00C0–00FF, U+0100–017F | 28,180 B (+10,052 over the base) | v0.5.0 |

Every glyph is 600 units wide; Iosevka's thin and narrow spaces are 600 too,
so a formatter's U+2009 or U+202F is one whole cell (REQ-FMT-060). Tier C
(Greek, Cyrillic, maths) brings zero-width marks and is left out.

### Text in cells

Labels are laid out in whole cells: width by `unicode-width` in release
builds (+4,882 B gz), wide glyphs with `WIDE_TAIL`, labels NFC-normalised by
the shell, zero-width code points taking no cell, a multi-scalar grapheme
drawn as its first scalar with the accessible text keeping the original
(REQ-I18N-040, REQ-I18N-050). Draw order is z-order: data, gridlines on empty
cells, axes, labels, annotations, crosshair, readout; a label clears the
cells under it (REQ-TEXT-010, REQ-TEXT-020).

### Terminal colour

The CLI writes 24-bit SGR by default, changed runs only. With `--color 256`
or `16` it maps each palette entry once a palette (REQ-COLOR-050). A
full-screen heatmap frame is 777,760 B in truecolour and 204,601 B in
256-colour, where xterm-256 squeezes viridis into 15 colours (rendering.md
§2.4).

## Scales, axes and layout

From scales.md (ADR-110, ADR-111, ADR-112):

- **Scales** map a domain value to a continuous cell coordinate and back:
  linear and time (v0.2.0); log, symlog, band, index time (v0.5.0); colour
  scales to palette indices (v0.9.0); a wrapping angle scale (v0.12.0).
  Domains are `"data"`, `"view"` (y fits the data visible), `[a, b]` or
  `{follow: seconds}`; `nice`, `zero` and `padding` adjust them; a domain that
  follows data moves in nice steps, without hidden state.
- **Ticks** are d3's (linear: steps of 1, 2, 5 × 10ⁿ; time: 18 calendar
  intervals), fitted by lowering the count from one tick per four cells
  (x) or two rows (y) until whole labels fit with a two-cell or one-row gap.
  Marks go in the nearest cell, gridlines at the nearest braille dot;
  `snap: "rows"` widens a small y domain so every tick lands on a row.
- **Formats** are a d3-format subset (`[sign][$][,][.precision][~][type]`,
  types `f s % e d` and `auto`) and a strftime subset, written by hand; Rust's
  float formatting stays out of labels (it costs 10.4 KB gz).
- **Zones and locales** cross once as data: the shell builds a zone's offset
  transitions over the data's extent with `Intl` (a year of New York in
  0.5–1.7 ms) and a locale pack from `Intl` with the Gregorian calendar
  forced (435–822 bytes); the CLI builds the same from tzdata and a JSON
  file. The core reads no clock, zone or tzdata (REQ-TZ-010).
- **Layout** runs in cells from the outside in: title, legend, x axis, panes
  (by weight, at least three rows), y gutters (aligned across panes), x
  ticks, annotations. Below 16 × 5 a chart draws data only; no title below
  10 rows; no x labels below 6 rows; y labels drop, the right axis first,
  when gutters pass a third of the columns (REQ-LAYOUT-010 to 030).
- **Annotations** (`hline`, `vline`, `band`, `label`, `marker`) are in data
  coordinates, clipped to their pane (REQ-ANNOT-010).

The options shape is [options.schema.json](../spec/schemas/options.schema.json).

## Theme and accessibility

From a11y-theming.md (ADR-122, ADR-123, ADR-132):

- **Theme**: CSS custom properties `--glyphplot-ground`, `-ink`, `-muted`,
  `-rule`, `-accent`, `-series-1` … `-series-8`, `-up`, `-down`, `-scale`,
  inherited from the page, so a page's `[data-theme]` and
  `prefers-color-scheme` rules re-theme every chart; a `theme` object wins
  over them; without a ground, dark or light defaults by the luma of the
  computed text colour. The element re-resolves on a scheme change, on a
  change of `<html>`'s `class`, `style` or `data-theme`, and on `theme`.
- **Defaults** are held to measured bars: series, up, down and accent at
  least 3:1 against the ground, ink and muted 4.5:1; eight categorical
  colours at least 15 CIEDE2000 apart under normal vision and simulated
  deuteranopia, protanopia and tritanopia. Candidates (searched, held near
  Okabe–Ito's hues) are in `discovery/bench/a11y-theming/results-anchored.md`;
  the owner picks by eye (open question 10). Up and down are tokens and differ
  by fill as well as colour.
- **One ink**: line series by dash patterns counted from the x origin
  (so dashes do not crawl on pan), then markers; bars and areas by shade
  fills; heatmaps by dots lit; candles hollow and solid; a legend key for
  each (REQ-MONO-010 to 040).
- **Semantics**: the host is `role="figure"` named by `label`; the drawing is
  `role="img"` with a summary label; a `role="slider"` cursor is the one tab
  stop, its `aria-valuetext` the cursor's values; a visually hidden light-DOM
  `<table>` holds the view's data, at most 500 rows (6 ms to build in
  Chromium; 10,000 rows took 523 ms); a control row (zoom, reset, step) on
  focus and hover gives click alternatives to dragging (WCAG 2.5.7); `paused`
  and Space pause a stream (WCAG 2.2.2); an opt-in `announce` live region,
  at least 5 s apart (REQ-A11Y-010 to 100).
- **Deferred**: forced colours and print (v0.7.0), the visible table
  (v0.7.0), RTL reordering (v0.9.0), Arabic joining and a navigation tree for
  multi-pane charts (later), sonification (not planned).

<a id="the-cli"></a>
## The CLI

Crate `glyphplot-cli`, binary `glyphplot`, on crossterm with its own terminal
code (ADR-134).

```
glyphplot <type> [--from FILE|-] [--x COL] [--y COL[,COL…]] [--format csv|tsv|json|ndjson|arrow]
                 [--window N] [--options JSON] [--view JSON] [--theme FILE] [--locale-file FILE]
                 [--zone NAME] [--mono] [--light] [--color auto|truecolor|256|16|none]
                 [--glyphs braille|octants|sextants|quadrants|ascii] [--cols N] [--rows N]
                 [--fps N] [--seconds N] [--no-mouse] [--table]
glyphplot render <type> --from FILE --format txt|ansi|svg|png [--cols N --rows N] [--options JSON] [--view JSON] [-o FILE]
glyphplot demo <type>        generated data, said so on its first row
glyphplot list               every chart type and the data shape it takes
glyphplot -h | --help | -V | --version
```

`glyphplot candles --from feed.csv` is the owner's example as written.
Without `--x`, the first column is x and the rest are y; columns by name or
1-based index. With stdin not a terminal, data comes from stdin and keys and
mouse from `/dev/tty` (crossterm opens it), so `tail -f feed.csv | glyphplot
line` can be scrubbed; rows are appended at most once a frame. Mouse in the
terminal is a whole cell (crossterm has no pixel mode). Piped, it prints one
plain frame and exits.

| Exit | When |
|---|---|
| 0 | quit by a key, `--seconds`, a signal other than Ctrl+C, end of input when piped; help; version; list; a render written; a closed stdout |
| 2 | a usage error, an unknown type (with up to eight suggestions), bad options, unreadable data (named by line and column), a missing column, a terminal failure; on stderr as `glyphplot: <message>` |
| 130 | Ctrl+C |

<a id="site-examples"></a>
## The site

Elm 0.19.1 with Vite, as unicel's, but documentation with live charts rather
than a gallery alone (ADR-143): a page per chart type (the chart live, a
control per option, its attributes, an event log, snippets that work), doc
pages (getting started, the data contract, view and events, theming,
accessibility, the CLI), and an examples page that is the gallery
(REQ-SITE-010 to 040). `site/test.html`, the plain-HTML checklist page,
exists from v0.3.0 for the browser tests (REQ-TEST-040).

The examples, in order (domains.md):

1. **Live candles (simulated)**: a seeded synthetic market labelled
   "simulated market", volume, SMA 20 and Bollinger 20/2, RSI below,
   scrubbed by finger (the owner's test).
2. **Live candles (real)**: Wikimedia EventStreams edits a second, one candle
   a minute (server-sent events, CORS open).
3. **Service latency**: NAB CloudWatch series and synthetic histograms;
   p50/p90/p99 on a log y, a latency heatmap, an SLO band, an anomaly window.
4. **Earthquakes now**: USGS live GeoJSON as a scatter of longitude and
   latitude and a magnitude histogram.
5. **Yield curve**: the Treasury's daily par curve as a heatmap and a curve
   scrubbed through time.
6. **Oil since 1986**: EIA's daily WTI as a line and weekly candles.
7. **Tonight's sky**, **the solar system**, **a transit** (v0.12.0).
8. **Network throughput**: synthetic, a mirrored area and top talkers.

Every live third-party example names its source and licence and shows a
message when the feed is down (REQ-SITE-050). Datasets follow the manifest
rules ([Appendix B](#appendix-b)).

## Domain packs

A pack is a crate `glyphplot-<domain>` under `crates/packs/<domain>/` that
depends on the core's data contract and on no chart; on the web it is its own
wasm module, in the CLI it links natively; it writes columns a chart then
draws, and never draws (ADR-138, REQ-PACK-010). Handing a column of a million
values from a pack's memory to a chart's costs 0.127 ms (domains.md).

| Pack | Holds | Milestone |
|---|---|---|
| markets | trades to OHLCV, resampling, SMA, EMA, Bollinger, RSI (Wilder), MACD, incremental (ADR-139); depth from order-book snapshots; a seeded synthetic market | v0.10.0 |
| telemetry | Prometheus `query_range` matrices and OTLP/JSON metrics to columns; counter to rate with resets; bucket quantiles; delta to cumulative; presets | v0.10.0 |
| astronomy | planets from JPL's approximate Keplerian elements, RA/Dec to alt/az, precession, Horizons `$$SOE` parsing, light-curve folding (ADR-140) | v0.12.0 |

Network (pcap, flows) and geoscience start as example pages over existing
charts; a network pack waits until a pcap reader's size is measured.

## Phases

Work in order. Each phase is a milestone of [spec/ROADMAP.md](../spec/ROADMAP.md)
§20, on its own GitButler branch named after its version, stacked on the
one before (`but branch new v0.N.0 --above v0.N-1.0`). Each phase lists its
tasks, the requirements it ships (by area; the roadmap has the full list),
and its acceptance: commands an agent runs headless, then the manual checks
it hands the owner. When a phase is done, write "Done <date>" under its
heading with what differed from the tasks, as unicel's plan does.

The brief's order is kept with one change discovery argued for: the contract's
first rules (shape, no panic, charset, colours, first frame, determinism,
out of order, snapshots) ship with the line chart in Phase 2, because
testing.md's matrix is what keeps a first chart honest; Phase 3 completes the
rest of the rules and the budgets.

<a id="phase-0"></a>
### Phase 0 · Toolchain green (v0.1.0)

Tasks:
1. Workspace members: `crates/glyphplot` (the core; features `all` and
   `line`, no charts yet), `crates/wasm` (`glyphplot-wasm`, `publish =
   false`, wasm-bindgen 0.2, features mirroring the core's, `debug`), and
   `crates/cli` (`glyphplot-cli`, a `main` that prints usage and version).
   Add each to `members` in `Cargo.toml` (REQ-PKG-040, REQ-ARCH-040).
2. `crates/glyphplot` starts with no charts and no copied code: glyphplot
   shares nothing with unicel (ADR-013, REQ-INDEP-010). The grid and canvases
   are Phase 1's.
3. `crates/glyphplot/examples/manifest.rs`: `families` prints the families
   with charts; `write <dir>` writes `modules.js`, `modules.d.ts` and
   `budgets.json` (204,800 B default) (REQ-MANIFEST-010 lands in Phase 2;
   Phase 0 needs only `families` and `budgets.json`).
4. `mise.toml` tasks: `install`, `build:wasm`, `lint`, `fmt`, `test`,
   `test:rust`, `size`, `check`, `signoff` (a `scripts/signoff.sh` of its own)
   (REQ-BUILD-010 to 060, REQ-LINT-010, REQ-LINT-030, REQ-SIZE-010 to 040,
   REQ-CI-010).
5. The independence check in `lint`: fail when a manifest, `mise.toml` or
   script names `animate-unicode` or a `unicel` crate or package
   (REQ-INDEP-020).

Acceptance:
- `mise install && mise run check` exits 0.
- `mise run size` prints the `line` module (an empty chart family),
  raw and gzipped, under 204,800 B, and `WASM_GZ_LIMIT=1000 mise run size`
  fails naming it.
- `cargo metadata --format-version 1 | grep -c '"source":"git+'` prints 0, and
  `mise run lint` fails on a test edit that adds `../animate-unicode` as a path
  dependency.

<a id="phase-1"></a>
### Phase 1 · Core (v0.2.0)

Tasks:
1. The grid (REQ-GRID-010 to 070) and the sub-cell canvases
   (REQ-CANVAS-010, 030), written for glyphplot: encodings from the Unicode
   Standard, `scripts/octants.py` generating the octant table from Unicode
   16's `UnicodeData.txt` (downloaded and pinned by SHA-256), unit tests
   pinning glyphs against their Unicode names; then the cell-mapping module
   (column and dot of a value, value of a column), used by everything after
   (REQ-CANVAS-020). Discovery's count of the equivalent code in unicel was
   about 1,240 lines of Rust.
2. `palette` and `theme`: the fixed layout, `theme::resolve` with Oklab
   interpolation of the scale stops, the default dark and light themes from
   the candidates in `discovery/bench/a11y-theming/` (REQ-COLOR-010 to 040).
3. `data`: `Table`, column kinds held as `f64`, timestamps with origin,
   missing values, the descriptor (serde), the order check and stable sort,
   `encode`/`role`/name channel lookup, shape errors (REQ-DATA-010 to 040,
   080, 090, 110, 130).
4. `scale`, `ticks`, `format`, `time`: ports of d3-array `ticks`, d3-scale
   linear and time with `nice` and `tickFormat`, a d3-format subset, d3-time
   UTC intervals and a strftime subset, with JavaScript's rounding where d3
   relies on it (scales.md and testing.md found `Math.round` and the
   non-settling `nice` mattered) (REQ-SCALE-010 to 030, REQ-TICK-010 to 040,
   REQ-FMT-010 to 030, REQ-TZ-010, REQ-I18N-030).
5. `layout` and `text`: outside-in layout in cells, tick fitting by lowering
   the count, label widths by `unicode-width`, labels clearing their cells,
   draw order (REQ-LAYOUT-010 to 030, REQ-TEXT-010, REQ-TEXT-020,
   REQ-I18N-040).
6. `fixtures.rs` (SplitMix64, the 23 fixtures) and
   `examples/fixtures.rs` (REQ-FIX-010).
7. `scripts/fixtures.mjs`, pinned d3-array 3.2.4, d3-scale 4.0.2, d3-time
   3.1.0, d3-format and the LTTB reference, writing
   `crates/glyphplot/tests/fixtures/*.txt`; `mise run fixtures`;
   `tests/reference.rs` comparing bitwise (REQ-REF-010). The discovery
   harnesses `discovery/bench/scales/fixtures/capture.mjs` and
   `discovery/bench/testing/ref/capture.mjs` are the starting point.
8. proptest (lean) properties in `#[cfg(test)]` modules (REQ-PROP-010).
9. `scripts/font.py`, written for glyphplot: download Iosevka's release
   once, check its SHA-256, subset with fonttools (pinned) to the base ranges
   and tier A, rename the family, check every glyph is 600 units and the
   element's `unicode-range` matches; `mise run build:font`; the font checked
   in (REQ-FONT-010).
10. `[profile.contract]`; `clippy.toml` with `f64::clamp` and `f32::clamp`
    disallowed; the `cargo tree` check of forbidden crates in `lint`
    (REQ-BUILD-070, REQ-LINT-040, REQ-SIZE-060).

Acceptance:
- `mise run test:rust` passes, including `reference::*` with 0 cases
  differing and every property.
- `mise run fixtures && git diff --exit-code crates/glyphplot/tests/fixtures`
  (the captures regenerate byte for byte).
- `mise run build:font` writes a font under 32 KB whose cmap covers tier A.
- `mise run lint` passes, and fails when a test edit adds `f64::clamp`.

<a id="phase-2"></a>
### Phase 2 · Line end to end (v0.3.0)

The largest phase: the first chart, the boundary, the shell and the element,
interactive and accessible from the start (ADR-007).

Tasks:
1. `charts/line/`: the `Chart` trait, `META` for `line`, modes `line`, `step`,
   `area`; M4 per dot column cached by data version, x range and width;
   binary-searched window; gaps; the readout of the nearest datum
   (REQ-CHART-*, REQ-LINE-010 to 040, REQ-SAMPLE-010 to 030).
2. View state, the input reducer, the overlay over a cached base, hit tests
   by binary search, events (REQ-VIEW-010 to 050, REQ-INPUT-*, REQ-HIT-010,
   020, REQ-OVERLAY-*, REQ-EVENT-010 to 030).
3. The registry and `tests/contract.rs` with rules 1–9 and 29, snapshots in
   hex, `mise run snap`; `CHART=` (REQ-CONTRACT-010, 020, REQ-SNAP-*).
4. `crates/wasm`: the `Chart` object of [the boundary](#the-boundary), the
   debug feature and `debug-panic`; the manifest in full (REQ-WASM-*,
   REQ-MANIFEST-*).
5. `web/`: `wasm.ts`, `chart.ts` with the load queue,
   `mount.ts` (changed cells, `<pre>` and canvas, block shapes, gestures,
   coalesced input), `theme.ts`, `locale.ts`, `a11y.ts`, `element.ts`;
   package skeleton (REQ-LOAD-*, REQ-EL-*, REQ-MOUNT-*, REQ-JS-010,
   REQ-THEME-010 to 050, 070, REQ-A11Y-010 to 070, REQ-I18N-010, 020, 050).
6. The renderer: implement both candidates behind a flag; `mise run
   bench:render`; the owner measures on Chrome and Safari with a GPU; keep
   the winner, record the numbers here (REQ-RENDER-010, 050).
7. `site/` with Vite and `test.html` (no Elm yet), Playwright and axe:
   `interaction.spec.ts`, `element.spec.ts`, `resize.spec.ts`,
   `a11y.spec.ts`, the smoke set (REQ-TEST-040, REQ-E2E-010 to 040, 070).
8. Node tests: `player`, `data`, `view`, `events`, `snapshot`, `reference`,
   `memory`, `schemas` (REQ-TEST-020, REQ-DATA-050 to 070, REQ-MEM-*).
9. `mise run dev` with a watch-and-reload script of its own (`scripts/dev.sh`)
   (REQ-BUILD-040, 080).

Acceptance:
- `mise run check` exits 0 (it now runs `test:site --smoke`).
- `CHART=line cargo test --profile contract --test contract` passes.
- `mise run test:site` passes interaction, element, resize and axe.
- `node --test web/test/memory.test.mjs`: 1M points within 16 B a point + 1
  MiB; free and remake grows nothing.
- `mise run size`: the `line` module under 204,800 B (about 50 KB expected
  from budgets.md's step 4, **(unverified)** for the real chart).
- Owner: `test.html` in Chrome and Safari on a desktop and on a phone: hover,
  drag, pinch, scrub, keys, VoiceOver on the slider and the table; the
  renderer measurement.

<a id="phase-3"></a>
### Phase 3 · Contract and budgets (v0.4.0)

Tasks:
1. Rules 10–13, 15–28 in `tests/contract.rs`, threads across charts; the
   `contract-selftest` feature with three broken charts (REQ-CONTRACT-030,
   060).
2. `tests/budgets.rs`: rule 30, alone, retime twice, load average printed
   (REQ-CONTRACT-050, REQ-PERF-030 to 050).
3. `mise run bench` (exit 1 over budget), `bench:web`, `bench:load`,
   `bench:render` from the discovery harnesses
   (`discovery/bench/budgets/js/`, `discovery/bench/rendering/canvas/`)
   (REQ-BENCH-*, REQ-PERF-060, 080).
4. `mise run size --charts`, a script of its own that reads an unstripped build's name section
   (REQ-SIZE-050).
5. `signoff` runs everything with `PROPTEST_CASES=4096` (REQ-CI-020).
6. The `debug` overlay (frame and draw time against the budget), as unicel's
   REQ-DEBUG-001.

Acceptance:
- `cargo test --profile contract --test contract --test budgets` passes,
  and `cargo test --profile contract --features contract-selftest --test
  contract selftest` passes.
- `mise run bench` exits 0 and prints every row under budget.
- `scripts/signoff.sh <this branch>` exits 0 locally.
- `mise run check` under five minutes from a warm cache (REQ-BUILD-090).

<a id="phase-4"></a>
### Phase 4 · Candles, bar, sparkline (v0.5.0)

Tasks:
1. `charts/candles/`: `candle`, `hollow`, `ohlc`; glyphs by spacing;
   calendar resampling below a cell; volume pane; magnet crosshair and the
   `o h l c` readout; one-ink rule (REQ-CANDLE-010 to 050, REQ-SAMPLE-040).
2. `charts/bar/`: vertical, horizontal, grouped, stacked, eighth ends; band
   hit test (REQ-BAR-010, 020).
3. `sparkline` in `charts/line/`: no axes, a row band a series, a ring
   (REQ-SPARK-010).
4. Core: log, symlog, band and index time scales; row snapping; log labels;
   the date context row; unit formatters; strftime; zone tables from `Intl`
   and their ticks against d3 under `TZ=`; panes; dual y axes; legend
   placement; annotations (REQ-SCALE-040 to 060, REQ-TICK-050 to 070,
   REQ-FMT-040, 050, REQ-TZ-020, 030, REQ-LAYOUT-050, 060, REQ-AXIS-010,
   REQ-ANNOT-010, REQ-DATA-100).
5. Element: `define` aliases, `type` change carrying data, `fit` with fixed
   `cols` and `rows`, `link`; `plot-brush`, `plot-legend`; one-ink markers
   and fills; font tier B; real datasets in `tests/data/` (REQ-EL-030, 050,
   110, REQ-LINK-010, REQ-MONO-020, 030, REQ-FONT-020, REQ-FIX-020).

Acceptance:
- The contract and snapshots pass for `candles`, `bar` and `sparkline`
  (`CHART=candles,bar,sparkline cargo test --profile contract --test contract`).
- `mise run size`: three modules (`line`, `candles`, `bar`) under budget.
- `mise run test:site` with a linked-charts test.
- Owner: a candlestick of `ohlcv-days` on `test.html`, scrubbed with a finger
  on a phone.

<a id="phase-5"></a>
### Phase 5 · Streaming (v0.6.0)

Tasks:
1. The ring (`retain.rows`, `retain.age`), `staging`/`append`, the equal-x
   and late-row rules, `plot-data`, the shell's per-frame coalescing,
   `x.follow` (REQ-STREAM-*, REQ-VIEW-060, REQ-CANDLE-060).
2. Rule 14 in the contract (REQ-CONTRACT-040).
3. The Arrow IPC reader (from `discovery/bench/data/src/core.rs`, `mod ipc`,
   extended to the types of REQ-DATA-140), `set_arrow`, `src`.
4. `paused`, Space, `announce`, the table's rate limit (REQ-A11Y-080 to 100).
5. `streaming.spec.ts`; the streaming row of `bench:web` (REQ-E2E-050,
   REQ-PERF-070).
6. A seeded synthetic feed in `test.html` (TypeScript, labelled
   "simulated") for the owner's test; the markets pack replaces it later.

Acceptance:
- `cargo test --profile contract --test contract` (rule 14 for every chart).
- `mise run test:site` with `streaming.spec.ts`.
- `mise run bench:web` streaming row at most 500 ms of CPU a second.
- Owner (the one-line test): `test.html`'s live candlestick on a phone over
  a throttled network, scrubbed with a finger; the network panel shows one
  wasm module fetched (`candles`).

<a id="phase-6"></a>
### Phase 6 · The site (v0.7.0)

Tasks:
1. Elm 0.19.1 site with Vite: flags from the manifest, routes
   (`/`, `/<type>`, `/docs/<page>`, `/examples/<name>`), the theme toggle
   (REQ-SITE-010, 060).
2. A page per chart type; the doc pages; the examples of
   [the site](#site-examples) 1–6 and 8 (REQ-SITE-020 to 050).
3. `datasets/manifest.json` and `mise run datasets:check`; HYG fetched at
   build time, never committed (REQ-DATASET-*).
4. "Show data", forced colours, print (REQ-A11Y-110, REQ-THEME-100).
5. `charts.spec.ts` (every type's page, no console error); axe on the index,
   a chart page and an example (REQ-E2E-060, REQ-SITE-070).
6. Install snippets that work before publishing (REQ-PKG-060).

Acceptance:
- `mise run build:site`, `mise run test:site` (all), `mise run
  datasets:check` exit 0.
- Owner: the site in both themes on a desktop and a phone; each live example
  against its feed.

<a id="phase-7"></a>
### Phase 7 · The CLI (v0.8.0)

Tasks:
1. `crates/cli`: `glyphplot <type>`,
   `render`, `demo`, `list`; stdin and `/dev/tty`; `--window`; mouse through
   the core's reducer; `--color`; `--glyphs`; terminal restore on every way
   out (REQ-CLI-*).
2. `glyphplot::export::{text, ansi, svg}` and the `png` feature; the
   element's `toSVG`, `toANSI`, `toPNG` (REQ-EXPORT-*, REQ-CRATE-010).
3. Zone tables from tzdata (jiff, native only), `--locale-file`, `--theme`,
   `--table` (REQ-TZ-040, REQ-I18N-070, REQ-THEME-090, REQ-A11Y-120).
4. `glyphplot/node` with `renderText` and `renderSVG` (REQ-SSR-010).
5. `crates/cli/tests/terminal.rs` (pty, Unix) (REQ-TERM-010).

Acceptance:
- `cargo test -p glyphplot-cli` passes (pty tests on macOS).
- `node --test web/test/exports.test.mjs`: element exports byte-identical to
  `glyphplot render`.
- Owner: `glyphplot candles --from feed.csv` and `tail -f feed.csv |
  glyphplot line` in iTerm2 or Ghostty, with the mouse.

<a id="phase-8"></a>
### Phase 8 · The next tier (v0.9.0)

`charts/scatter/` (points, bubble; data-space index), `charts/heatmap/`
(matrix, calendar), `bar` histogram, `line` band, baseline and stacking,
`candles` Heikin-Ashi, colour scales and the diverging default, RTL labels,
graph data (REQ-SCATTER-*, REQ-HEAT-*, REQ-BAR-030, REQ-LINE-050,
REQ-CANDLE-070, REQ-SCALE-070, REQ-SAMPLE-050, REQ-HIT-030, REQ-THEME-080,
REQ-MONO-050, REQ-I18N-060, REQ-DATA-160).

Acceptance: the contract and snapshots for every new type and mode;
`mise run size` with five modules under budget; `mise run bench` with the
scatter rows; owner: a coloured scatter and a heatmap on the site.

<a id="phase-9"></a>
### Phase 9 · Domain packs (v0.10.0)

`crates/packs/markets` and `crates/packs/telemetry`, each its own wasm
module and linked into the CLI, with presets, fixtures from the probes in
`discovery/bench/domains/` (REQ-PACK-*, REQ-MKT-*, REQ-TELE-*,
REQ-SIZE-070). The site's first example moves from the TypeScript feed to the
markets pack.

Acceptance: `cargo test -p glyphplot-markets -p glyphplot-telemetry`; `mise
run size` lists both pack modules under 32 KB; the pack-to-chart snapshot
tests pass.

<a id="phase-10"></a>
### Phase 10 · Packaging and deploy (v0.11.0)

The npm package `glyphplot` with `.`, `./element`, `./wasm`, `./node` and
`./jsx`; crates published (`cargo publish --dry-run` first); a deploy plan
(`docs/deploy-plan.md`, as unicel's) for the site's host; `glyphplot-ratatui`
(REQ-PKG-*, REQ-CRATE-020). Publishing and hosting are the owner's to
approve.

Acceptance: `node --test web/test/package.test.mjs` (tarball contents);
`cargo publish --dry-run` for each crate; the `tsc` JSX fixture.

<a id="phase-11"></a>
### Phase 11 · Sky and orbits (v0.12.0)

`crates/packs/astronomy`; `charts/sky/` with `sky` and `orbit`; the angle
scale, RA and Dec formatters, Julian-date timestamps; the site's sky,
solar-system and transit examples (REQ-ASTRO-*, REQ-SKY-*, REQ-SCALE-080,
REQ-FMT-070, REQ-DATA-170).

Acceptance: the contract for `sky` and `orbit`; the pack's Horizons fixtures
within 3′; `icrs` to `altaz` within 0.1° for the ten fixture stars.

## Testing strategy

From testing.md (ADR-126 to ADR-130):

| Layer | Tool | Command | In `check` | In `signoff` |
|---|---|---|---|---|
| Core unit and property tests | `cargo test`, proptest (lean) | `test:rust` | yes | yes, `PROPTEST_CASES=4096` |
| Chart contract, rules 1–29 | `tests/contract.rs`, threads over charts | `test:rust` | yes | yes |
| Budgets, rule 30 | `tests/budgets.rs`, alone | `test:rust` | yes | yes |
| Snapshots | the contract; `SNAP=write` | `test:rust`, `snap` | yes | yes |
| Reference fixtures | `tests/reference.rs`; captures by `scripts/fixtures.mjs` | `test:rust`, `fixtures` | yes | yes |
| Wasm | `node --test web/test/*.test.mjs`, release then debug | `test:wasm` | yes | yes |
| Schemas | `spec/schemas/validate.py`; `web/test/schemas.test.mjs` | `test:wasm` | yes | yes |
| Elm site | elm-test | `test:elm` | yes | yes |
| Browser | Playwright, headless Chromium, axe; touch through CDP `Input.dispatchTouchEvent` (Playwright's touch helpers fire no Pointer Events) | `test:site` | smoke | all |
| Terminal | `crates/cli/tests/terminal.rs` (pty, Unix) | `test:rust` | yes | yes |
| Size | gzip a module against `budgets.json` | `size` | yes | yes |
| Native bench | table, exit 1 over budget | `bench` | no | no (rule 30 holds it) |
| Browser benches | the discovery harnesses | `bench:web`, `bench:load`, `bench:render` | no | yes |
| Real devices, Safari, screen readers | the owner | `site/test.html`, the site | — | — |

`test:rust` runs `cargo test --workspace --profile contract` (release
without LTO): an edit rebuilt in 1.2–1.3 s (measured at opt-level 2) instead of 7.4–11.6 s, and the
contract runs no slower (testing.md M2, M3). `test:site` waits for
`test:rust`, so Chromium never shares the machine with the budgets.

## Performance and size budgets

Held in `META.budget` (with a family default) and written by the manifest to
`web/pkg/budgets.json`, so `size`, the contract and the benches read one
number (as unicel's `Category::budget()`).

| Budget | Value | Measured | Held by |
|---|---|---|---|
| Module size, gzip -9 | 200 KB a chart module (204,800 B), warning at 100 KB; 32 KB a pack | line: 17.4 KB without serde, 49.3 KB with; + candles 51.4 KB; + M4 52.6 KB | `mise run size` (REQ-SIZE-010, 040, 070) |
| Frame, native, ≤ 100k visible points | 1 ms average, 8 ms worst | ≤ 0.81 / ≤ 1.03 ms | `tests/budgets.rs` (REQ-PERF-040) |
| Frame, native, ≤ 1M visible points | 4 ms average, 16 ms worst | 2.1–2.4 / 2.5–3.4 ms with M4 | `tests/budgets.rs` (REQ-PERF-030) |
| Frame, Chromium, ≤ 1M visible points | 8 ms average, 16 ms worst | 3.2–3.7 / 3.7–4.2 ms; WebKit 3.8–4.3 / 4.5–5.0 | `bench:web` at signoff (REQ-PERF-060) |
| Pointer move, 1M points | 0.5 ms average, 2 ms worst | 1–8 µs average, ≤ 43 µs worst | `tests/budgets.rs`, `bench:web` (REQ-PERF-050) |
| Streaming 1 kHz into a full 1M ring, 60 Hz | ≤ 500 ms of CPU a second in Chromium | 236 ms/s with M4 | `bench:web` (REQ-PERF-070) |
| Memory a chart | 16 B a point of capacity (two columns) + 1 MiB | 17.7 MB at 1M zero-copy | `memory.test.mjs` (REQ-MEM-010) |
| Memory on replace or free | no growth | pages unchanged after free (522) | `memory.test.mjs` (REQ-MEM-030) |
| First frame, one chart | 1.0 s on 170 ms / 9 Mbps; 3.0 s on Lighthouse slow 4G | 635 ms and 2,082 ms (443 and 1,500 with preload hints) | `bench:load` at signoff (REQ-PERF-080) |
| Font | ≤ 32 KB woff2 | tier A 25,344 B | `build:font` (REQ-FONT-010) |
| `check` | under 5 minutes, warm | unicel's contract alone: 32.5 s idle | (REQ-BUILD-090) |

Every timing above was taken on the reference machine shared with other
agents (load 3–12); [Appendix A](#appendix-a) gives each with its load. The
contract retimes a chart over budget twice and keeps the best, because a slow
chart is slow every time and a loaded machine only now and then (unicel's
REQ-CONTRACT-030).

## Risks

1. **The renderer was measured only in headless Chrome.** `fillText` beat a
   tinted atlas there (16.6 against 34 ms at 320 × 120); a GPU-composited
   Chrome or Safari may order them differently. Settled by the owner's
   measurement in Phase 2 (REQ-RENDER-050).
2. **A full redraw of a large grid is slow.** 16–21 ms for 320 × 120 braille
   in headless Chrome: a full-screen heatmap that changes every cell holds
   30 frames a second. Changed-cell drawing (REQ-RENDER-030) protects lines
   and hovers; heatmaps get `bench:render` rows in Phase 8.
3. **Safari was not measured.** `safaridriver` refused a session (remote
   automation off; left as it was). Playwright's WebKit stood in at 1.1–1.4×
   Chromium. Settled by the owner turning on remote automation or measuring
   by hand.
4. **wasm memory never shrinks.** A page that loads a million points through
   a copying path, or replaces data with larger data, holds the peak for its
   life. Mitigated by one-copy loading and ring reuse (REQ-MEM-*).
5. **serde_json is a third of a small module** (+33 KB). Accepted for parity
   (ADR-141); typed setters remove it if a module passes the 100 KB warning.
6. **Timings move with load.** Native frame times moved up to 1.6× and the
   contract doubled under load 7–12. The retime rule absorbs it; if budgets
   flake, `tests/budgets.rs` skips when the load passes half the cores and
   says so (testing.md open question 1).
7. **Screen-reader behaviour is unverified.** The slider and table follow the
   APG and Chartability, not a run in NVDA, JAWS or VoiceOver. Settled by
   the owner's manual pass in Phase 2.
8. **Touch is tested only through CDP in Chromium.** Real devices are the
   owner's (REQ-TEST-030).
9. **Glyphs outside the font fall back** to system faces whose advance may
   not be 0.6 em, so a `<pre>` row can drift for a label in Greek or
   Cyrillic. The canvas clips; tiers A and B cover numbers, units and Western
   European letters.
10. **Independence costs work.** Phases 0 and 1 write what unicel already has
    (a grid, canvases, glyph tables, block shapes, a font script, about 1,240
    lines of Rust by discovery's count), and sizes measured on unicel's code
    are estimates for glyphplot's own until Phase 1 measures them (ADR-013).
    Glyphs are pinned to Unicode names, so the rewrite is checked against the
    standard, not against unicel.
11. **Market data licences.** No exchange feed checked may be shown to third
    parties (Coinbase, Binance, Kraken terms quoted in domains.md); the site
    uses synthetic data and Wikimedia (ADR-136). Adapter code may ship, data
    may not.
12. **ShareAlike star data.** HYG is CC BY-SA 4.0: never in a crate or the
    npm package; fetched at the site's build (ADR-137).
13. **Ten million points** would take 20–50 ms a frame by linear
    extrapolation **(unverified)**; a min/max pyramid is the answer if a
    domain asks (ADR-108).
14. **The candle rule below four cells in one ink** cannot show rising and
    falling; the rule is to resample instead (REQ-CANDLE-030), which shows
    fewer candles. The owner may prefer `⢸` against `⣿` by eye.

## Open questions

For the owner. Each has the options and a recommendation; the related ADR is
Proposed until the owner answers.

1. **The size threshold a module.** 150 KB (unicel's), 200 KB, or 300 KB
   gzipped. Recommend 200 KB with a warning at 100 KB (ADR-133): four times
   today's full line module, about 0.8 s and 2.9 s to a first frame on the two
   load profiles; 300 KB takes 3.4 s on Lighthouse's slow 4G.
2. **Depend on a published unicel crate or vendor the core.** Settled by the
   owner on 2026-10-18: neither. glyphplot copies, ports and vendors nothing
   from unicel and depends on nothing in its repository (ADR-013; discovery's
   ADR-100 is rejected).
3. **One tag or a tag per chart.** One tag, `<glyph-plot type>`, with
   `define(tag, {type})` aliases for pages that want `<glyph-candles>`
   (ADR-118).
4. **Palette index or RGB a cell.** A `u8` index into a 255-entry semantic
   palette (ADR-102); RGB only if a chart must show three or more continuous
   scales at once.
5. **Celestial positions.** Charts take positions as data in a declared
   frame; the astronomy pack computes planets from JPL's Keplerian elements;
   Horizons for the Moon and precise work (ADR-140).
6. **The Elm site.** Documentation with live charts, a page per chart type,
   and an examples page as the gallery (ADR-143), rather than unicel's
   gallery alone.
7. **The first milestones' charts and the first domain pack.** `line` (line,
   step, area) in v0.3.0; `candles`, `bar` and `sparkline` in v0.5.0;
   streaming in v0.6.0; `scatter`, `heatmap` and the histogram in v0.9.0. The
   first packs are markets and telemetry together in v0.10.0, astronomy with
   the sky charts in v0.12.0.
8. **serde_json in every module (+33 KB) or typed setters.** Keep serde_json
   until a module passes 100 KB (ADR-141).
9. **The renderer.** Measure `fillText` and the tinted atlas on your Chrome
   and Safari in Phase 2 and keep the faster (ADR-119).
10. **The default palettes and up/down colours.** Pick by eye among the
    searched candidates held near Okabe–Ito's hues; up and down blue and
    orange (≥ 50 ΔE00 under every vision) rather than green and red (9.3 for
    deuteranopes); cividis as the sequential default (ADR-123).
11. **Mouse drag and wheel.** Drag pans and the plain wheel scrolls the page
    (no scroll trap); options switch to brushing and wheel zoom (ADR-116).
12. **The smallest font and the font tiers.** Check digits by eye at 8–12 px;
    propose a floor of 11 px at 1× and 9 px from 2×; tier A now, tier B in
    v0.5.0 (ADR-121).
13. **The live market feed on the site.** Synthetic plus Wikimedia edits a
    second; write to an exchange for permission only if a real market matters
    enough to carry a revocable permission (ADR-136).
14. **The minus sign.** U+2212 in the browser, `-` in the CLI, switchable in
    the locale pack (REQ-FMT-060).
15. **The default time zone.** The element defaults to the page's zone,
    resolved to an IANA name and passed in; the CLI and tests to UTC.
16. **Gating budgets on a loaded machine.** Retime twice and keep the best,
    printing the load; skip above half the cores only if it flakes.
17. **The default height** of a chart with no CSS height: 12 rows.

<a id="appendix-a"></a>
## Appendix A: measurements

Every number discovery took, by topic. Machine: Apple M5 Pro, 18 cores,
48 GB, macOS (Darwin 25.5.0), 2026-10-18, toolchain from `mise exec` (Rust
1.97.1, Node 26.5.1, wasm-pack 0.15.0, wasm-bindgen 0.2.129). The machine was
shared by ten agents benchmarking at once; each timing names its load. Sizes
are `gzip -9` of wasm-pack `--target web --release` output with unicel's
release profile and wasm-opt flags unless a row says otherwise. Each row
names its report; the report names its bench, whose README reruns it.

### A.1 Module size (budgets.md, size ladder)

| Step | Adds | wasm raw | gzip | Δ gzip |
|---|---|---|---|---|
| 0 | empty wasm-bindgen module with std | 13,746 | 6,161 | — |
| 1 | grid, braille canvas, line chart over `Float64Array`, box-drawing axes, ring, hover | 27,187 | 12,368 | +6,207 |
| 2 | linear scale, nice ticks, SI labels by hand | 32,456 | 15,062 | +2,694 |
| 2b | step 2 with labels through `core::fmt` floats | 54,426 | 24,382 | +9,320 over 2 |
| 3 | UTC time scale, calendar ticks, date labels by hand | 34,746 | 16,209 | +1,147 |
| 4 | serde + serde_json for options and meta | 106,787 | 49,297 | +33,088 |
| 5 | a candlestick chart in the same module | 111,995 | 51,406 | +2,109 |
| 6 | M4 downsampling and candle merging | 114,823 | 52,647 | +1,241 |
| — | steps 1–3 and 6, no serde or candles | 37,241 | 17,437 | — |
| — | one `f64::clamp` in step 1 | — | 22,719 | +10,351 |

JS glue: 11.3–17.7 KB raw, 2.8–3.7 KB gzipped. Without serde, a module is
about a quarter chart code, a fifth allocator and a fifth wasm-bindgen; with
serde, serde_json and the float formatting and parsing it brings are half
(twiggy 0.7 by owner, `results/breakdown.txt`).

Other sizes:

| What | gzip | Report |
|---|---|---|
| Empty module (other benches) | 5,858 | rendering.md, scales.md |
| Grid + braille, vendored / as a git dependency | 8,901 / 9,045 | rendering.md |
| All four canvases and colour canvases | 13,003 | rendering.md |
| + `Pixels` and blue noise | 19,017 | rendering.md |
| `unicode-width` string width in release | +4,882 | rendering.md |
| Typed-column base (Table, typed paths, bindgen, dlmalloc) | 8,269 | data.md |
| + serde_json options | 19,195 | data.md |
| + JSON data parsing (over base / over options) | +34,354 / +26,527 | data.md |
| + CSV parser | +17,347 | data.md |
| + hand-written Arrow reader | +3,766 | data.md |
| + arrow-rs 60.0.0 / arrow2 0.18.0 | +261,684 / +92,511 | data.md |
| + ring / + min-max, M4, LTTB | +1,202 / +2,276 | data.md |
| + Kepler planets / + VSOP87A | +6,931 / +679,661 | data.md |
| Rust `{:.2}` float formatting | +10.4 KB | scales.md |
| ryu / num-format one locale / any locale | +13.3 / +8.7 / +16.7 KB | scales.md |
| time / chrono / jiff civil / jiff strftime / jiff with tzdb / chrono-tz | +8.5 / +13.8 / +2.2 / +63.6 / +150.0 / +160.9 KB | scales.md |
| ICU4X decimal / datetime | +25.4 / +120.9 KB (scales.md); +24.0 / +219.2 KB without wasm-opt (a11y-theming.md) | scales.md, a11y-theming.md |
| js-sys `Intl.DateTimeFormat` | +2.0 KB, +0.75 KB glue | scales.md |
| Hand-written ticks, nice, SI, UTC ticks and labels | +5.5 KB (linear 3.6, time 4.3) | scales.md |
| d3 array, scale, format, time, time-format minified JS | 18.4 KB | scales.md |
| apache-arrow 21.2.0 JS | 48,477 | data.md |
| Indicator probe (SMA, EMA, Bollinger, RSI, MACD), no bindgen | 1,454 | domains.md |
| Astronomy probe (8 planets, RA/Dec, alt/az), no bindgen | 6,460 | domains.md |

### A.2 Frame time (budgets.md)

300 frames after one warm-up, a view panning 10% on a sine, 5 runs; min /
median of the runs' averages, then of their worst frames, in ms. Native load
7.8 → 6.7; Node 9.0 → 9.5; Chromium 156 headless (cross-origin isolated)
8.6 → 8.3; WebKit 27.2 (Playwright's) 8.3 → 8.5.

| Cells | Points | M4 | Native avg | Chromium avg | WebKit avg | Native worst | Chromium worst |
|---|---|---|---|---|---|---|---|
| 80 × 24 | 1,000 | yes | 0.006 / 0.007 | 0.009 / 0.010 | 0.012 / 0.013 | 0.019 / 0.037 | 0.040 / 0.040 |
| 80 × 24 | 100,000 | yes | 0.248 / 0.259 | 0.374 / 0.379 | 0.473 / 0.477 | 0.301 / 0.481 | 0.495 / 0.515 |
| 80 × 24 | 1,000,000 | no | 2.60 / 2.75 | 2.75 / 2.79 | 3.95 / 4.00 | 3.12 / 3.33 | 3.07 / 3.12 |
| 80 × 24 | 1,000,000 | yes | 2.10 / 2.38 | 3.24 / 3.26 | 3.83 / 3.86 | 2.45 / 5.07 | 3.69 / 3.73 |
| 320 × 120 | 100,000 | yes | 0.409 / 0.415 | 0.571 / 0.581 | 0.710 / 0.728 | 0.568 / 0.629 | 0.710 / 0.715 |
| 320 × 120 | 1,000,000 | no | 5.03 / 5.08 | 5.51 / 5.54 | 7.27 / 7.48 | 5.62 / 5.76 | 6.12 / 6.14 |
| 320 × 120 | 1,000,000 | yes | 2.40 / 2.45 | 3.70 / 3.71 | 4.24 / 4.29 | 2.69 / 3.44 | 4.13 / 4.20 |

budgets.md has every row (1k, 10k, 100k, 1M; Node). charts.md, native, 3
processes × 5 reps, load 6.8–10.1, no axes: 1M points of noise at 320 × 120
took 3.54 / 4.18 ms by per-column min/max, 362.8 / 387.9 ms segment by
segment and 325.2 / 347.1 ms in ratatui-widgets 0.3.2's `Chart`; a random
walk 2.21 / 2.26, 4.15 / 4.35 and 14.40 / 14.71 ms; min/max and every-segment
frames lit equal cells in all 16 configurations; plotext 6.1.0 took 3.4 s
for 1M points.

### A.3 Input (interaction.md, budgets.md)

interaction.md: native and wasm under Node 26, 3 runs × 5 reps, load
9.2–12.1; min / median µs an event, 1M points.

| What | Size | Native | Wasm (Node) |
|---|---|---|---|
| Crossing alone, `noop(x, y)` | any | — | 0.027 / 0.049 |
| Hit test, binary search over 1M sorted x | any | 0.033 / 0.040 | 0.101 / 0.153 from JS |
| Hover: hit, readout, crosshair over the cached base | 80 × 24 | 0.55 / 0.61 | 0.73 / 0.79 |
| Hover | 320 × 120 | 3.8 / 4.9 | 4.1 / 5.0 |
| Hover as JSON | 80 × 24 | 0.69 / 0.80 | 1.16 / 1.22 |
| Hover redrawing the full frame, 1M visible | 80 × 24 | 1,689 / 1,745 | 1,480 / 1,660 |
| Hover redrawing the full frame, 1M visible | 320 × 120 | 1,989 / 2,310 | 2,124 / 2,267 |
| Pan, 1M visible | 320 × 120 | 2,045 / 2,280 | 2,053 / 2,185 |
| Scatter: build a screen-space grid over 1M | 320 × 120 | 5,698 / 8,801 | 5,734 / 9,998 |
| Scatter: nearest by the grid / by a scan | 320 × 120 | 14.4 / 15.9; 1,182 / 1,373 | 21.0 / 21.3; 1,138 / 1,202 |
| Cells changed a hover / a pan | 80 × 24 | 8.2 / 297 of 1,920 | — |

An earlier native run at load 7.4–9.3 gave the full-frame hover at about half
these figures. budgets.md, 5 × 300 moves: 1 µs (80 × 24) and 3–4 µs (320 ×
120) natively, 2 and 5 µs in Chromium, worst 43 µs.

### A.4 Data (data.md)

Wasm in Node, 1M rows of two columns, 5 reps × 3 runs, load 3.67–11.60;
median ms and memory growth.

| Path | ms | Growth |
|---|---|---|
| `Float64Array` as `&[f64]`, copied into a `Vec` | 5.58 | 23.1 MiB |
| `Float64Array` as `Vec<f64>` | 4.27 | 15.4 MiB |
| JS `set` into a chart-owned buffer | 4.99 | 15.4 MiB |
| JSON columns, serde_json in wasm | 163 | 80.0 MiB |
| JSON rows, serde_json in wasm | 265 | 89.5 MiB |
| JSON columns, `JSON.parse` then `Vec<f64>` | 72.9 | 15.4 MiB |
| CSV epoch ms in wasm / in JS | 156 / 155 | 48.4 MiB / JS heap |
| CSV RFC 3339 in wasm / in JS | 193 / 256 | 58.8 MiB / JS heap |
| Arrow IPC, hand-written reader | 7.42 | 33.0 MiB |
| Arrow IPC, arrow-rs | 10.3 | 38.1 MiB |

Ring of 1M (wasm, Node): one append 0.062 µs; a batch of 1,000 3.78 µs;
staging and commit 2.97 µs; allocation 1.39 ms and 15.4 MiB once. Downsampling
1M points in view, wasm min / median ms: min-max 2.33 / 2.71 (160 columns),
2.59 / 2.87 (640); M4 2.35 / 2.52, 2.44 / 2.78; LTTB 1.90 / 2.29, 2.09 / 2.31,
missing a column's true extreme in 153 of 160 and 607 of 640 columns; M4 over
a view of 10k of 1M at 640 columns 0.035 / 0.077.

### A.5 Streaming and memory (budgets.md)

Ten simulated seconds into a full ring of 1M, 320 × 120, 60 Hz redraws, 5
runs; CPU ms a second, min / median.

| Feed | M4 | Native | Node | Chromium | WebKit |
|---|---|---|---|---|---|
| 1 sample every ms | yes | 155 / 157 | 322 / 324 | 236 / 237 | 274 / 281 |
| 1 sample every ms | no | 309 / 324 | 477 / 498 | 346 / 346 | 455 / 464 |
| 1,000 samples every 100 ms | yes | 151 / 151 | 271 / 296 | 233 / 234 | 177 / 278 |

Memory (Node, `memory.buffer.byteLength / 65536`): after init 17 pages; a
chart of 1M points loaded zero-copy 270 pages (17.7 MB); two such charts 522;
both freed 522; one chart through `set_data(Float64Array, Float64Array)` 516
pages (33.8 MB).

### A.6 Load (budgets.md)

Headless Chromium 156, cache disabled, median of 7 cold loads, ms from
navigation start to the end of the first 80 × 24 frame of 1,000 points.

| Module | Transfer B | Brief 4G (170 ms, 9 Mbps) | Lighthouse slow 4G (562.5 ms, 1.44 Mbps, CPU 4×) | No throttling |
|---|---|---|---|---|
| step 1 (12.4 KB) | 15,805 | 596 / 600 | 1,867 / 1,876 | 50 / 55 |
| step 6 (52.6 KB) | 57,102 | 632 / 635 | 2,073 / 2,082 | 53 / 54 |
| step 6 padded to 150 KB | 158,150 | 718 / 722 | 2,609 / 2,616 | 52 / 54 |
| step 6 padded to 300 KB | 311,787 | 855 / 857 | 3,425 / 3,436 | 51 / 55 |
| step 6 padded to 1,000 KB | 1,028,813 | 1,479 / 1,482 | 7,229 / 7,238 | 51 / 55 |
| step 6 with preload hints | 57,102 | 441 / 443 | 1,484 / 1,500 | 31 / 32 |

Each further 100 KB costs about 87 ms on the brief's profile and 530 ms on
Lighthouse's; compile, instantiate and the first frame take 2–4 ms after the
wasm arrives (7–15 ms at CPU 4×).

### A.7 Scales and formatting (scales.md, testing.md)

- The scales port matched d3 on 803 captured cases (0 tick lists, labels or
  nice domains differ); properties over 3 × 200,000 cases: no failure.
- testing.md's `ticks` port matched d3-array on 20,651 cases natively and in
  wasm; its LTTB port matched flot-downsample on 12.
- `fit_x`: 2.6 / 2.7 µs (80-cell linear), 4.0 / 4.4 µs (80-cell time), 14.1 /
  14.7 µs (320 cells), native, 5 runs of 20,000, load 9.2–9.3.
- One wasm call that nices a domain and labels about ten linear and ten UTC
  ticks: 0.84 µs min, 0.87–0.93 µs median, Node, load 7.3–7.7.
- `Intl` offset lookup 1.11 / 1.14–1.26 µs; a year of New York's transitions
  0.48–0.72 ms min, 1.2–1.7 ms median (388 lookups); ten years 4.4 ms; a
  locale object 1.66–1.74 ms min, 435 B JSON for de-DE.
- Two "equal" column formulas, `x*cols/n` and `x*(cols/n)`, disagreed on
  1,137 of 12,006,000 points (testing.md M5).

### A.8 Rendering (rendering.md)

Headless Chrome 155, a full redraw a frame, 10 warm-up and 60 timed frames,
3 runs, min / median of the per-run medians, load 8.5–9.2.

| Size | Workload | Renderer | Distinct (glyph, colour) | Atlas hits | ms |
|---|---|---|---|---|---|
| 80 × 24 | density | atlas, 64 / 255 / RGB | 24 / 87 / 195 | 100% | 3.1–3.3 / 3.2–3.4 |
| 80 × 24 | density | `fillText` | 195 | — | 1.0 / 1.1 |
| 80 × 24 | scatter | atlas, 64 colours | 944 | 3.6% | 60.1 / 61.0 |
| 80 × 24 | scatter | tint | 944–1,209 | 100% | 2.4–2.8 / 2.7–2.8 |
| 80 × 24 | scatter | `fillText` | 1,209 | — | 0.9 / 1.0 |
| 320 × 120 | density | atlas / `fillText` | 24–212 | 100% / — | 46.1–48.0 / 20.7–21.0 |
| 320 × 120 | scatter | atlas | 5,231–17,440 | 0.6–3.6% | 1,130–1,527 |
| 320 × 120 | scatter | tint / `fillText` | — | 100% / — | 34.0–35.1 / 16.6 |
| 320 × 120 | blocks | rects / one image | 85–211 | — | 9.1–9.6 / 6.4–6.8 |

Scale quantisation (largest ΔE76 step of viridis): 16 steps 18.52, 64 steps
4.55, 128 steps 2.26, 255 steps 1.13. Bytes a cell: 6 (index) against 11–12
(RGB). Terminal full frame, 320 × 120 braille heatmap: truecolour 777,760 B
(RGB) and 765,453 B (255 steps), 256-colour 204,601 B; a line chart 47,204 B
either way; viridis in xterm-256 is 15 colours, mean error 37.4. Font: tier A
+7,216 B, tiers A and B +10,052 B woff2. Digits at 1×: 1 px apart at 8 px, 7
px from 11 px; at 2×, 12 px or more from 8 px.

### A.9 Surfaces (surfaces.md)

Hello-chart CLIs, release, stripped: std only 335,728 B; crossterm 404,800 B;
the grid through a ratatui `Widget` 438,112 B; ratatui's `Chart` 553,968 B;
32 against 75 crates. A first frame as text in Node from the wasm: instantiate
0.30 ms, first frame of 1,000 points 0.96 ms, 100,000 points 3.23 ms, a whole
Node process 32.9 ms (median of 5, load 4–8). `tsc` 5.9.3 `--strict` typed
`<glyph-plot>` as a union on `type` and rejected candle data on `type="line"`.

### A.10 Accessibility and theming (a11y-theming.md)

Hidden table build and layout in headless Chromium 156, median of 3: 100 rows
1.5 ms, 500 rows 6.0 ms, 1,000 rows 13.0 ms, 10,000 rows 523.7 ms; 100,000
did not finish in 5 minutes. Palettes, smallest CIEDE2000 between any two
colours (normal, deutan, protan, tritan, Machado 2009 at severity 1.0):
Okabe–Ito 21.7, 11.6, 12.2, 10.9; Tableau 10 18.1, 0.7, 1.0, 6.8; searched
near Okabe–Ito hues on dark 18.5, 16.4, 16.6, 16.5, on light 16.4, 15.7,
15.6, 15.7. Sequential, neighbouring 9-step ΔE00 under the worst vision:
viridis 4.4 (protan), cividis 7.0. Locale pack from `Intl`: 474–822 bytes,
4.23 ms a locale cold; 20 labels with a cached `Intl.NumberFormat` 4.7–6.2
µs a frame.

### A.11 Domains (domains.md)

JPL Table 1 Keplerian elements against Horizons over 2026, daily: Mars
geocentric max 35″, mean 20″; Jupiter max 123″, mean 46″. One braille dot of
a whole-sky chart 320 columns wide: 2,025″. data.md, against VSOP87A on
2026-10-18: Uranus 4.6″ to Saturn 297.1″; eight planets in 1.5–2.0 µs
natively (VSOP87A 0.20–0.48 ms). Indicators over 1M values, wasm in Node:
SMA 1.2 ms, EMA 2.4 ms, Bollinger 6.7 ms (26.0–26.5 ms in another memory
layout, cause not found), RSI 3.6 ms, MACD 1.9 ms. Copying 1M values between
wasm memories 0.127 ms.

### A.12 Testing (testing.md)

unicel's contract today: 32.48 / 32.64 s for the whole binary at load
3.4–3.6; 60.66 / 64.67 s at load 7.5–8.8; 40.19 / 57.19 s without LTO at the
same load. Clean builds of the test targets: dev 0.7 s with no property
crate, 4.7 s with lean proptest, 4.6 s with quickcheck; release 7.0 s, 17.3 s
and 27.2 s. Incremental after an edit: release with LTO 7.4–7.5 s (no
property crate), 11.4–11.6 s (proptest); without LTO 1.2–1.3 s. Fixtures: all
14 bench fixtures in about 1.1 ms; `random-walk-1m` 0.86–0.96 ms.

<a id="appendix-b"></a>
## Appendix B: datasets

From domains.md, checked on 2026-10-18. "Repo" means stored in this MIT
repository; "site" means served by the public site.

| Dataset | Publisher | Licence | Repo | Site | For |
|---|---|---|---|---|---|
| Synthetic OHLCV, depth, flows, Prometheus matrices | glyphplot (seeded) | MIT | yes | yes | candles, streaming, telemetry, network |
| WTI spot price, daily | U.S. EIA | public domain, cite EIA | yes (from EIA, not FRED) | yes | line, weekly candles |
| Daily par yield curve | U.S. Treasury | public domain | yes (16,229 B for 2026) | yes, live (CORS `*`) | heatmap, scrubbed curve |
| 10-year yield, USD/EUR | Federal Reserve (H.15, H.10) | public domain | yes (from the Fed) | yes | long lines |
| NAB `realAWSCloudwatch` | Numenta | MIT | yes | yes | lines, anomaly bands |
| Wikimedia EventStreams `recentchange` | Wikimedia Foundation | not stated; counts are facts | — | yes, live (CORS `*`) | live candles of edits a second |
| Earthquakes, past month | USGS | public domain, credit USGS | an extract under 1 MB | yes, live (CORS `*`) | scatter, histogram |
| K-index; tide gauges | NOAA SWPC; NOAA CO-OPS | public domain | yes | yes, live | bars, line and band |
| HYG v4.1, mag ≤ 6.5 | astronexus | CC BY-SA 4.0 | no: fetched at the site's build | yes, with attribution | sky chart |
| Constellation lines | d3-celestial (Olaf Frohn) | BSD-3-Clause | yes, with notice | yes | sky chart |
| JPL approximate Keplerian elements | NASA/JPL SSD | facts, cite | yes | yes | astronomy pack |
| Horizons vectors, 2026 | NASA/JPL Horizons | not stated, cite (owner to confirm) | as test fixtures | — | pack tests |
| TESS light curve TIC 25155310 s1 | NASA/MAST | public use without restriction | a CSV extract under 1 MB | yes | light curve |
| MIT-BIH Arrhythmia | PhysioNet | ODC-By 1.0 | yes, with attribution | yes | long high-rate line |
| Coinbase, Binance Vision, Kraken | exchanges | display to third parties forbidden or NonCommercial | never | never | — |
| Gaia DR3 | ESA | CC BY-NC 3.0 IGO | never | never | — |
| Wireshark samples, RIPE RIS/Atlas | — | none stated; restricted | never | never | — |

<a id="appendix-c"></a>
## Appendix C: unicel as prior art

`../animate-unicode`. glyphplot takes nothing from it: no code is copied,
ported, vendored or generated from it, and nothing depends on it (ADR-013).
It is cited for design, as uPlot and ratatui are. What its design shows, and
what glyphplot writes for itself:

| Need | What unicel's design shows | glyphplot writes |
|---|---|---|
| Grid | A `u32` code point and two `u8` colour indices a cell, read as typed-array views | `grid.rs`, from REQ-GRID-010 to 070 |
| Canvases and glyph tables | Sub-cell bitmaps encoded as braille, sextants, quadrants, octants | `canvas.rs` from the Unicode Standard; `scripts/octants.py` from `UnicodeData.txt` |
| Terminal glyph fallback | Redrawing dot glyphs for fonts that lack them | `glyphs.rs`, from REQ-CLI-110 |
| One-ink checks | Ink share, rows that differ, glyphs in the font | the contract's rule 10 |
| Block shapes | Filling blocks, quadrants, sextants, octants and wedges as exact shapes on a canvas | `web/src/blocks.ts` from the glyphs' Unicode definitions |
| Loader and manifest | One module per group, imported on first use, listed in a generated `modules.js` | `web/src/wasm.ts`, `examples/manifest.rs` |
| Mount | Views re-made each frame, a font load clearing caches, a ResizeObserver that ignores 0, IntersectionObserver and visibility gates | `web/src/mount.ts` (and no (glyph, colour) atlas, ADR-119) |
| Element | A near-screen gate, superseded loads freed, kept children, `:where()` styles | `web/src/element.ts` |
| Contract | Every failure reported in one run, a filter by name, retiming twice | `tests/contract.rs`, `tests/budgets.rs` |
| CLI | Terminal restore on every exit, changed runs only, pty tests | `crates/cli` and `tests/terminal.rs` |
| Font | A subset of Iosevka with `unicode-range` | `scripts/font.py` from Iosevka's upstream release |
| Signoff | Local CI posting a commit status | `scripts/signoff.sh` |
| Site | Elm with Vite, Playwright, axe | `site/` |
