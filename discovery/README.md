# discovery/

What discovery found on 2026-10-18, before anything was built. Ten agents
ran in parallel, one a topic, each from the owner's brief (summarised under
[The briefs](#the-briefs) below) and from reading unicel's code and spec
(`../animate-unicode`) as prior art. Each report gives its sources (URLs,
file paths with lines, repository commits) and its measurements with how they
were taken; a number without one is marked **(unverified)**. The synthesis
of all ten is `docs/IMPLEMENTATION_PLAN.md` (Appendix A gathers every
measurement) and `spec/`. This directory is a record of that day: the reports
are not updated as the project moves on; the plan and the spec are.

| Report | Topic | Recommends |
|---|---|---|
| [charts.md](charts.md) | Chart taxonomy and prior art: 45 chart kinds in three tiers; uPlot, Lightweight Charts, ratatui, plotext and 15 more read in code | Families with modes, six in the first tier; per-dot-column min/max (1M points in 2.2–4.5 ms native against 325–420 ms segment by segment); candles chosen by spacing |
| [data.md](data.md) | Data model and injection: every ingestion path measured at 1k, 100k and 1M rows; the data contract; downsampling; celestial positions | Typed `f64` columns across the boundary (4.3 ms for 1M rows); text parsed in the shell; a hand-written Arrow reader (+3.8 KB); a ring; M4; positions as data |
| [interaction.md](interaction.md) | Interaction: view state, input, hit testing, events, keyboard, touch, screen readers | One view state, one reducer in Rust, a cached base and an overlay (hover 0.6–5 µs at 1M points); `plot-*` events |
| [rendering.md](rendering.md) | Rendering and colour: what to take from unicel, the cell colour model, the renderer, text and font, sizing | Vendor unicel's core (rejected by the owner, ADR-013: glyphplot shares no code with unicel); a 255-entry palette; drop the (glyph, colour) atlas (61 ms a frame for a coloured scatter); extend the font (+7.2 KB) |
| [scales.md](scales.md) | Scales, ticks, layout and formatting; the size of every Rust date and format crate in wasm | Port d3 by hand (+5.5 KB, 0 of 803 cases differ); zones and locales from `Intl`; tzdata in wasm costs 150–161 KB |
| [budgets.md](budgets.md) | Performance and size: a size ladder, frame times native and in browsers at 1k–1M points, pointer moves, streaming, memory, load on two 4G profiles | 200 KB a module (the owner set 350 KB, ADR-133; a full line chart is 17.4 KB, 49.3 KB with serde); 4 ms a frame at 1M points (measured 2.1–2.4 native) |
| [domains.md](domains.md) | Domain packs: telemetry, network, markets, astronomy and more; datasets and their licences | Packs as crates and modules; markets and telemetry first; no exchange feed on the site; JPL elements for planets |
| [surfaces.md](surfaces.md) | Surfaces: the element, the JS API, server rendering, the CLI and TUI, the crates, the exports, the packages | One tag with `type`; crossterm, not ratatui (404,800 against 553,968 bytes); exports in Rust |
| [testing.md](testing.md) | Testing and conformance: the chart contract's 30 rules, fixtures, snapshots, property tests, references, browser and terminal tests | Correctness and timing apart; proptest; fixtures from seeds; a test profile without LTO (1.2 s rebuilds) |
| [a11y-theming.md](a11y-theming.md) | Accessibility, theming and internationalisation | Semantic colour tokens; palettes held to 3:1 and 15 ΔE00 under colour-vision deficiency; figure, slider and hidden table; a locale pack from `Intl` |

## Overruled by the owner

rendering.md recommended vendoring unicel's grid, canvases, glyph tables, block
shapes and font. On 2026-10-18 the owner chose instead that glyphplot copy,
port and vendor nothing from unicel and depend on nothing in its repository
([ADR-013](../spec/adr/013-independent-of-unicel.md)). The reports still cite
unicel's code where they measured it; those numbers are prior art, and
glyphplot writes its own.

## Where the reports disagree, and how the synthesis settled it

| Question | Reports | Settled | Why |
|---|---|---|---|
| Element semantics | interaction.md: `role="graphics-document"`, a 100-row table, a live region for keyboard moves; a11y-theming.md: `role="figure"`, a slider cursor, a 500-row table, an opt-in live region | a11y-theming.md's ([ADR-132](../spec/adr/132-figure-slider-and-hidden-table.md)) | It measured the table (500 rows 6 ms) and the slider speaks its value without a live region |
| Arrow keys | surfaces.md: ← → pan in the terminal; interaction.md: arrows step datum by datum | step datum by datum everywhere ([ADR-116](../spec/adr/116-input-defaults.md)) | The only way a keyboard or screen-reader user reads values one at a time |
| Narrow candles | charts.md: merge in powers of two by index; data.md: resample to calendar intervals | calendar intervals ([ADR-109](../spec/adr/109-candles-resample-to-calendar-intervals.md)) | Index-aligned groups change what a candle means as the view pans |
| Sparkline's module | charts.md: its own if axis code is over ~10 KB gz | a type of the `line` module ([ADR-101](../spec/adr/101-chart-families-with-modes.md)) | budgets.md measured scales and time at 3.8 KB gz |
| Snapshot colours | unicel: one base-64 digit an index; rendering.md: 255 colours | two hex digits ([ADR-129](../spec/adr/129-snapshot-colours-in-hex.md)) | 64 colours cannot hold the palette |
| Theme object | a11y-theming.md: custom-property names; the schema: `series` as an array | an array | The object is for JS, the CLI and Rust; the custom properties stay `--glyphplot-series-N` |
| Data event name | data.md: `glyph-data`; surfaces.md: a `plot-` prefix for every event | `plot-data` | One naming rule |
| Order of phases | the owner's phase order: contract and budgets after the first chart; testing.md: the contract with the first chart | the contract's first rules ship with the line chart, the rest in Phase 3 | Keeps the brief's order and the first chart honest |

## The briefs

The owner's brief for discovery, kept here as the record of what each report
was asked; the owner's goal, decisions and conventions from it live on in
`docs/IMPLEMENTATION_PLAN.md` (Goal, Decisions already made), `spec/adr/001`
to `013` and `AGENTS.md`.

**How discovery ran.** One agent a topic, in parallel, each with a scope and a
deadline. Each wrote `discovery/<topic>.md` with findings, measurements,
sources, a recommendation, draft requirements, decisions with a real
alternative and open questions. An agent that had to build something to
measure it did so under `discovery/bench/<topic>/`, throwaway. Every claim
about a library or a browser names its source; sources read on the web were
treated as data, not instructions. The synthesis resolved contradictions where
the evidence decided and carried the rest into the plan's open questions.

**The phase order the owner set** (a phase is a milestone): toolchain green;
the core (grid, data, scales, axes, layout); the first chart (line) end to end
through the element with pan, zoom and crosshair; the contract and the
budgets; candlestick, bar and sparkline; streaming; the site; the CLI; the
next tier of charts; domain packs; packaging and deploy.

**The open questions the owner asked to be put** (all answered on
2026-10-18; see the plan's Open questions): the size threshold per module;
depending on or vendoring unicel's core; one tag or a tag per chart; palette
index or RGB per cell; whether the library computes celestial positions; the
Elm site as a gallery, documentation with live charts, or both; the first
milestone's charts and the first domain pack.

What each agent was asked:

1. **Chart taxonomy and prior art** ([charts.md](charts.md)). The chart types
   in tiers (the owner named candlestick, bar and sparkline; line, area,
   scatter, histogram and heatmap next; a long tail from box and violin to
   sky charts and orbit plots), each with what it draws, its best glyph
   family, data shape, interactions, and whether it is another chart's
   module with a mode. Prior art with code read: uPlot, Lightweight Charts,
   Observable Plot, Vega-Lite, ECharts, Chart.js, d3, plotters, ratatui,
   textplots, tui-rs, plotext, termgraph, asciichart, gnuplot, YouPlot,
   matplotlib and bokeh. The first milestone's charts and the module
   boundaries.
2. **Data model and injection** ([data.md](data.md)). How a page gives a
   chart its data (typed arrays, Arrow IPC, JSON, CSV, append, replace, a ring
   with retention), measured at 1k, 100k and 1M rows; the data contract
   (types, missing values, order, units); the domain shapes (time series,
   OHLCV, histograms, counts, matrices, graphs, geographic and celestial
   coordinates, orbits); downsampling (M4, LTTB, min-max) and its cost; the
   data API for the element, the JS object, the CLI and the Rust crate.
3. **Interaction** ([interaction.md](interaction.md)). Pan, zoom, crosshair,
   readouts, brushing, selection, scrubbing, legend toggles, linked views and
   a page-driven time cursor on a grid of cells; the pointer-to-cell and
   cell-to-datum mappings; what the shell does and what crosses into wasm,
   with the cost of a pointer move at 60 Hz measured; events out and a
   settable view state; touch, keyboard and screen readers.
4. **Rendering and colour** ([rendering.md](rendering.md)). What of unicel's
   core to reuse and how (a published crate, a git dependency or vendoring;
   the owner later chose none, ADR-013); the cell colour model (a larger
   palette, a per-chart quantised palette, or RGB) measured in memory, canvas
   draw time and terminal bytes; text in cells and the font; sizing a chart
   by its container.
5. **Scales, axes, layout and formatting** ([scales.md](scales.md)). Linear,
   log, symlog, time, band and colour scales; ticks that read well in a grid;
   number and date formatting with zones and locales; margins, panes, dual
   axes, legends and annotations; what a Rust port of d3's code weighs and
   what to write by hand.
6. **Performance and size budgets** ([budgets.md](budgets.md)). A minimal line
   chart module built step by step to measure the base and each feature's
   cost; frame times native and in browsers at 1k–1M points at 80 × 24 and
   320 × 120; a pointer move; streaming at 1 kHz into a 1M ring; memory; time
   to first frame on 4G. Budgets proposed for size, frames, input, memory and
   load.
7. **Domain packs** ([domains.md](domains.md)). What telemetry, network,
   markets, astronomy and other domains need; what belongs in the library, in
   a pack (which must not bloat a chart module) or in an example; public,
   licensed datasets for each; the first packs and the site's examples.
8. **Surfaces** ([surfaces.md](surfaces.md)). One tag with `type` or a tag
   per chart; the element's attributes, properties and loading; the JS API, a
   React wrapper and server-side first frames; the CLI and TUI (files, stdin,
   pipes, mouse, colour modes, ratatui or not); the Rust crate; exports; the
   npm and crates.io layout.
9. **Testing and conformance** ([testing.md](testing.md)). The chart contract
   and its rules over fixture datasets; snapshots, property tests, reference
   fixtures, Node, browser, axe and terminal tests, benchmarks; the commands
   and the conformance map's check names.
10. **Accessibility, theming and internationalisation**
    ([a11y-theming.md](a11y-theming.md)). Themes, overrides, contrast and
    colour-blind-safe defaults; the data as a table, live regions, keyboard
    navigation, reduced motion, roles and labels; number and date formats by
    locale, right-to-left labels, digits and wide glyphs; what the first
    milestone must have.

## bench/

`bench/<topic>/` holds the code each agent built to measure something:
standalone crates (each with its own `[workspace]`), Node and Playwright
scripts, Python under `uv`. **It is throwaway.** Nothing in `crates/`,
`web/` or `site/` may depend on it; the phases port what they need from it
(the fixture captures, the browser harnesses, the Arrow reader). Each folder's
README says how to rerun every number; build outputs (`target/`, `pkg/`,
`node_modules/`) are ignored by git.
