# discovery/

What discovery found on 2026-10-18, before anything was built. Ten agents
ran in parallel, one a topic, each from the owner's brief (`PROMPT.md`) and
unicel's code and spec (`../animate-unicode`). Each report gives its sources
(URLs, file paths with lines, repository commits) and its measurements with
how they were taken; a number without one is marked **(unverified)**. The
synthesis of all ten is `docs/IMPLEMENTATION_PLAN.md` (Appendix A gathers
every measurement) and `spec/`.

| Report | Topic | Recommends |
|---|---|---|
| [charts.md](charts.md) | Chart taxonomy and prior art: 45 chart kinds in three tiers; uPlot, Lightweight Charts, ratatui, plotext and 15 more read in code | Families with modes, six in the first tier; per-dot-column min/max (1M points in 2.2–4.5 ms native against 325–420 ms segment by segment); candles chosen by spacing |
| [data.md](data.md) | Data model and injection: every ingestion path measured at 1k, 100k and 1M rows; the data contract; downsampling; celestial positions | Typed `f64` columns across the boundary (4.3 ms for 1M rows); text parsed in the shell; a hand-written Arrow reader (+3.8 KB); a ring; M4; positions as data |
| [interaction.md](interaction.md) | Interaction: view state, input, hit testing, events, keyboard, touch, screen readers | One view state, one reducer in Rust, a cached base and an overlay (hover 0.6–5 µs at 1M points); `plot-*` events |
| [rendering.md](rendering.md) | Rendering and colour: what to take from unicel, the cell colour model, the renderer, text and font, sizing | Vendor unicel's core (rejected by the owner, ADR-013: glyphplot shares no code with unicel); a 255-entry palette; drop the (glyph, colour) atlas (61 ms a frame for a coloured scatter); extend the font (+7.2 KB) |
| [scales.md](scales.md) | Scales, ticks, layout and formatting; the size of every Rust date and format crate in wasm | Port d3 by hand (+5.5 KB, 0 of 803 cases differ); zones and locales from `Intl`; tzdata in wasm costs 150–161 KB |
| [budgets.md](budgets.md) | Performance and size: a size ladder, frame times native and in browsers at 1k–1M points, pointer moves, streaming, memory, load on two 4G profiles | 200 KB a module (a full line chart is 17.4 KB, 49.3 KB with serde); 4 ms a frame at 1M points (measured 2.1–2.4 native) |
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
| Order of phases | the brief: contract and budgets after the first chart; testing.md: the contract with the first chart | the contract's first rules ship with the line chart, the rest in Phase 3 | Keeps the brief's order and the first chart honest |

## bench/

`bench/<topic>/` holds the code each agent built to measure something:
standalone crates (each with its own `[workspace]`), Node and Playwright
scripts, Python under `uv`. **It is throwaway.** Nothing in `crates/`,
`web/` or `site/` may depend on it; the phases port what they need from it
(the fixture captures, the browser harnesses, the Arrow reader). Each folder's
README says how to rerun every number; build outputs (`target/`, `pkg/`,
`node_modules/`) are ignored by git.
