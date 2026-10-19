# Testing and conformance

Discovery agent 9 of 10, 2026-10-18. Brief: `PROMPT.md`, "### 9. Testing and conformance".

## Summary

- glyphplot takes unicel's test layers whole: a contract test over every registered chart, text snapshots, fixture tests against the code a computation was ported from, Node tests against the release and the debug wasm, Playwright in headless Chromium with axe, pseudo-terminal tests for the CLI, and benchmarks that print a table. What changes is the input. unicel's contract plays each piece at its one size from a seed. glyphplot's contract runs each chart over a matrix: every fixture dataset whose shape the chart accepts, at five sizes, in colour, one ink and paper, with data replaced and streamed, with a view set and read back, and with scripted input.
- The chart contract is drafted below as a table of 30 rules, each with how it is checked. Four rules are new to glyphplot and carry most of the risk: streaming equals replacing, the view state round-trips, a hit test names what the frame drew in that cell, and the frame is the same at a new size and back.
- The hit-test rule is not hypothetical. A property test in this bench failed because the test mapped x to a column as `x * cols / n` and the reducer as `x * (cols / n)`. The two formulas are equal in algebra, and they disagree for 1,137 of 12,006,000 points (first case: x = 75, n = 150, cols = 316 gives column 158 one way and 157 the other). A chart must map data to cells with one function, used by both the frame and the hit test. The contract checks that it does.
- Fixture datasets are generated in Rust from a seed (SplitMix64), not checked in. Generating all 14 fixtures in the bench, a million-point random walk among them, takes 1.05–1.16 ms. Reducing a million points to 640 columns takes 1.4–1.8 ms by min-max and 1.06–1.09 ms by LTTB. The data side of the contract is cheap. Its cost is in frames.
- Ports were checked against their originals. A line-for-line port of d3-array 3.2.4's `ticks` matched d3 bit for bit on 20,651 cases, natively and in wasm. A port of the LTTB reference implementation matched on 12 cases (3 series, 4 thresholds), natively and in wasm. The method works. glyphplot's fixture tests capture the original's output with Node into checked-in files, and run the comparison in both `test:rust` and `test:wasm`.
- proptest, not quickcheck. proptest has explicit strategies (ranges by magnitude, NaN mixed in), reproducible seeds (`PROPTEST_RNG_SEED`), regression files to check in, and a release eight days old. Without default features it pulls 13 crates and adds 4.0 s to a clean dev build. Property tests must run optimised: the same four properties took 3.65 s in a debug build and 0.19–0.25 s in release.
- unicel's contract and snapshots take 32.5–32.7 s on an idle machine, and 61–71 s while other agents load it. The contract is one test that checks the pieces one after another (user time 39 s against 32.5 s of wall time). glyphplot should split correctness, which can run charts in parallel threads, from time budgets, which run alone in their own test binary.
- `cargo test --release` builds with the release profile: `opt-level = "s"`, fat LTO and one codegen unit. This was checked with `cargo test --release --no-run -v`. Touching the library then costs a 7.4 s relink of every test binary, and 11.5 s with proptest. A `contract` profile without LTO (`opt-level = 3`, 16 codegen units) rebuilds in 1.2–1.3 s. In the same load window it ran unicel's contract no slower (40–61 s against 61–71 s with LTO). unicel's `[profile.test] opt-level = 2` never applies, because `test:rust` passes `--release`.

## Scope and method

Read in `../animate-unicode`: `crates/unicel/tests/contract.rs` (all 534 lines), the snapshot tree `crates/unicel/tests/snapshots/` (207 files, 7.5 MB, `du -sh`), `crates/unicel/tests/fixtures/`, `crates/unicel/src/mono.rs`, `crates/unicel/examples/bench.rs`, `web/test/load.mjs`, `web/test/snapshot.test.mjs`, the test names in `web/test/player.test.mjs`, `site/e2e/a11y.spec.ts`, `site/e2e/pieces.spec.ts`, the test names in `site/e2e/shell.spec.ts`, `site/playwright.config.ts`, `crates/cli/tests/terminal.rs` (the harness and its five tests), `scripts/signoff.sh`, `scripts/snap.sh`, `mise.toml`, `spec/SPEC.md` §4.4, §11 and §12, `spec/conformance.md` (its preamble, summary table, gaps and the §4.4 rows), ADR-021, ADR-110 and ADR-111, and the plan's piece-contract table (`docs/IMPLEMENTATION_PLAN.md:171-205`).

Read in this repository: `PROMPT.md`. The other discovery reports, where this report's rules depend on theirs: `budgets.md` (the budget table), `interaction.md` (the view state and the input reducer), `charts.md` (module families, REQ-CHART-040 and REQ-CHART-070), `rendering.md` (the 255-entry palette, minimum sizes, REQ-LAYOUT-030), `data.md` (the ring), `scales.md` (REQ-TICK-010) and `domains.md` (licensed datasets).

Built under `discovery/bench/testing/` (throwaway; its `README.md` reruns everything):

- `src/lib.rs`: a seeded fixture generator, a min-max-per-column reducer, a port of LTTB from flot-downsample (MIT, commit af872e0), and a port of d3-array 3.2.4's `ticks`, with plain `extern "C"` exports so Node can call the wasm build without wasm-bindgen.
- `ref/capture.mjs` captures d3's and the reference LTTB's output. `tests/reference.rs` and `ref/wasm-check.mjs` compare the native and wasm ports with that output.
- `tests/props_proptest.rs` and `tests/props_quickcheck.rs` hold the same properties in both crates, plus one false property in each to compare shrinking.
- `examples/cost.rs` times the fixtures and the reductions. `examples/columns.rs` counts the disagreeing column formulas. `compile-cost.sh` times clean builds.

Machine for every measurement: Apple M5 Pro, 18 cores, 48 GB, macOS (Darwin 25.5.0), 2026-10-18, Rust 1.97 and Node 26 from `mise exec`. Other discovery agents were building and benchmarking at the same time, so each timing records `uptime`'s load averages.

Not covered: no chart exists, so no contract was run against a chart. The contract's runtime for glyphplot is an estimate built from measured parts and marked **(unverified)**. Playwright, axe and the pty tests were read in unicel, not run for glyphplot. No d3-time or d3-format fixtures were captured; `scales.md` proposes them. Firefox and WebKit are not considered, as in unicel (ADR-111).

## What unicel does today

| Layer | Where | What it holds | Source |
|---|---|---|---|
| Contract | `crates/unicel/tests/contract.rs::every_piece_keeps_the_contract` | Meta checks. Frames at t = 0, 1, 2, 3, 4, 6 played from t = 0, four times (colour, colour again, paper, one ink): shape, charset, wide tails, colour indices, first frame, motion, determinism, out-of-order calls (REQ-TEST-502), paper shape, scene colours, time budgets (release only, retimed twice when over), one-ink checks from `mono.rs`. One test, every failing piece reported, a table with `--nocapture`, `PIECE=` to narrow. | `contract.rs:112-317` |
| Snapshots | `contract.rs::every_piece_matches_its_snapshot` | Frames at t = 0, 1, 2.5 as text, with run-length base-64 colour and back layers; a scene's later frames as an FNV-1a hash; `SNAP=write` rewrites. | `contract.rs:319-442`, SPEC REQ-SNAP-001 to 006 |
| Ports | `contract.rs` fixture tests | Ten pieces against frames captured with Node from ascii.rest's TypeScript. | `contract.rs:444-534`, `tests/fixtures/` |
| Wasm | `web/test/*.test.mjs` | The manifest, memory views against `text()`, paper, options, errors, `free()`, clock, one ink, back colours, panics (debug build), the wasm frame 0 against the native snapshot, exports. Run on the release build, then on a debug build. | `mise.toml` `test:wasm`, SPEC §12 |
| Browser | `site/e2e/*.spec.ts` | Playwright, headless Chromium, `vite preview` of the release build; `--smoke` (ten tests, about five seconds) in `check`, all of it in `signoff`; axe in both themes, serious and critical only. | ADR-111, `a11y.spec.ts` |
| Terminal | `crates/cli/tests/terminal.rs` | `libc::openpty`, no crate: a piped frame, a key, Ctrl+C, SIGTERM, pacing after a stall. | `terminal.rs:1-182` |
| Bench | `examples/bench.rs` | 60 frames a piece, a table with `OVER` marked. It exits 0 when a piece is over budget. | `bench.rs:1-50` |
| CI | `scripts/signoff.sh` | No hosted CI. Each open pull request's head runs `npm ci`, lint, test, build, size, `test:seo` and all of `test:site` locally, and a `signoff` status is posted. | ADR-021 |

`spec/conformance.md` names checks as `crate path::test_name` for Rust, `file: "test name"` for Node and Playwright, and `mise run <task>` for tasks. It reports per-area counts of requirements covered, partial and untested (277, 108, 92 and 77 on 2026-10-18), then the gaps that matter, then the map. It counts a requirement whose only check is manual as untested (`spec/conformance.md:1-38`). glyphplot's map should keep that format.

Three lessons from unicel bear on glyphplot's design:

1. The contract tests nothing about itself. REQ-CONTRACT-001 to 030 are "untested (the contract test is the requirement; nothing tests the test)" (`spec/conformance.md:220-223`). glyphplot should give the contract a few deliberately broken charts behind a `contract-selftest` feature: one non-deterministic, one that draws outside its charset, one that keeps state across appends. The contract must then fail each of them, with the right message.
2. Native and wasm maths differ. The wasm snapshot check found starlings, where `hypot` rounded differently by platform (SPEC REQ-SNAP-005 note). glyphplot ports d3's floating-point code, so every fixture test runs in both builds.
3. Frame timings are hostage to the machine. unicel moved CI off GitHub because runners were 3–5 times slower (ADR-021). It keeps Chromium off the machine while the contract times frames (`wait_for = ["test:rust"]`, ADR-111). It retimes an over-budget piece twice (REQ-CONTRACT-030). The measurements below show the contract's wall time doubling under load.

## The chart contract

### Shape of a chart, as the contract sees it

The rules assume this chart surface. Agents 2, 3 and 8 settle the names; the rules do not depend on them.

```rust
pub static META: Meta = Meta { slug, name, note, modes, shapes, options, palette, min: (cols, rows), budget, .. };
pub fn make(options: &Options) -> Result<Box<dyn Chart>, Error>;
trait Chart {
    fn replace(&mut self, data: &Data) -> Result<(), Error>;   // replace-all
    fn append(&mut self, batch: &Data) -> Result<(), Error>;   // streaming
    fn view(&self) -> View;                                      // read
    fn set_view(&mut self, view: &View);                         // set (merge, as interaction.md)
    fn input(&mut self, event: &Input, layout: &Layout) -> Vec<Event>;
    fn frame(&mut self, t: f64, env: &Env, grid: &mut Grid);     // size from the grid
    fn hit(&self, col: usize, row: usize, sub: (u8, u8)) -> Option<Hit>;
}
```

### Fixture datasets

Fixtures are generated by `crates/glyphplot/src/fixtures.rs` (behind a `fixtures` feature that tests, examples and the debug wasm enable) from SplitMix64 with a fixed seed per fixture. SplitMix64 is ten lines and gives the same stream on every target. Nothing large is checked in. Each fixture declares its shape, and the contract feeds a chart every fixture whose shape its `META.shapes` lists. It also feeds one fixture of a shape the chart does not accept, which must be an error and not a panic.

| Fixture | Shape | Rows | What it is for |
|---|---|---|---|
| `empty` | series | 0 | No data: no panic, axes or a "no data" label, hit is none |
| `one` | series | 1 | A degenerate domain: padded, no NaN or inf in labels |
| `two` | series | 2 | The smallest line |
| `sine-1k` | series | 1,000 | A smooth, readable reference frame |
| `random-walk-100k` | series | 100,000 | The ≤ 100k budget row (`budgets.md`) |
| `random-walk-1m` | series | 1,000,000 | The million-point budget and the downsampling equivalence |
| `nan-gaps` | series | 10,000 | Every fourth block of 500 is NaN: gaps break lines |
| `inf` | series | 1,000 | ±Infinity among values: treated as the data contract says, never drawn as a wall |
| `constant` | series | 1,000 | All equal: a padded y domain, distinct tick labels |
| `huge-range` | series | 1,000 | 10^-300 to 10^300: log scale, SI labels that fit the gutter |
| `tiny-range` | series | 1,000 | 1 + i·10^-15: tick labels that stay distinct |
| `negative` | series | 1,000 | All below zero: log-invalid values, handled as the data contract says |
| `zero-crossing` | series | 1,000 | Symlog and baselines |
| `unsorted-x` | series | 1,000 | A seeded shuffle: the frame equals that of the data sorted by x, or a load error, as the data contract says |
| `duplicate-x` | series | 1,000 | Three rows an x: deterministic draw and hit |
| `steps-10k` | series | 10,000 | Plateaus: the step mode, flat runs |
| `multi-4` | series ×4 | 4 × 1,000 | Series told apart in one ink (`a11y-theming.md:180`), z-order (REQ-CHART-080) |
| `labels-wide` | series ×3 | 3 × 100 | Series names in CJK and with combining marks: wide tails in label text |
| `ohlcv-days` | ohlcv | 500 | Trading days with weekend gaps, a limit-down day, zero volume |
| `ohlcv-ticks-1m` | ohlcv | 1,000,000 | Candles aggregated from a million rows |
| `categorical-12` | categorical | 12 | Bars, labels longer than their band |
| `matrix-64` | matrix | 64 × 64 | Heatmaps, colour scales |
| `timestamps-dst` | series, time | 2,000 | Hourly UTC across a daylight-saving change, for time ticks |

Real datasets, small and checked in under `crates/glyphplot/tests/data/` with their licence and source in a README, for snapshots that look like what users see:

- EIA WTI spot, daily, one year, public domain. It includes the negative price day. Source: `domains.md:225`.
- U.S. Treasury par yield curve, 2026, 16,229 B, public domain. Source: `domains.md:226`.
- USGS earthquakes, a day's excerpt, public domain. Source: `domains.md:205`.

None of the market feeds may be redistributed (`domains.md:136-146`).

### The rules

Size set S, which every chart is checked at: 1 × 1 (no panic only, REQ-LAYOUT-030), the chart's `META.min`, 40 × 12, 80 × 24 and 320 × 120. Times T: 0, 1, 2.5 and 6. The "frame" is cells, colours and backs, compared bitwise.

| # | Rule | Detail | How checked |
|---|---|---|---|
| 1 | meta | Slug kebab-case and unique; name; note 1–72 chars; modes listed (REQ-CHART-020); shapes listed; min size within 1–320 × 1–120; budget given; palette 2–255 `#rrggbb` with a ground | `META` fields; registry walk |
| 2 | options | Every option a chart reads has a default in `META.options` (unicel REQ-OPT-030); an unknown mode is an error naming the valid ones | `Options::read_keys` against `keys`, as `contract.rs:162-171` |
| 3 | shape | At every size in S, `grid.text()` has `rows` lines of `cols` cells | Every frame the contract draws |
| 4 | no panic anywhere | Every fixture × size × mode × env draws without a panic, 1 × 1 included | The matrix itself |
| 5 | charset | Every code point the chart draws is in the font's ranges (`mono::FONT_RANGES`); label text that comes from the data may be any Unicode, with wide glyphs as `set_wide` + `WIDE_TAIL` | `allowed()` on cells outside label runs; `labels-wide` for tails |
| 6 | colours | Every ink and back index < palette length; `NO_BACK` (255) otherwise; no back without a palette | Every frame |
| 7 | good first frame | For every non-empty fixture, frame 0 at the default view has ink in the plot area (≥ 1 inked cell for `one`, ≥ 2 % of plot cells otherwise) | Ink count in the layout's plot rectangle |
| 8 | determinism | Two fresh charts given the same data, options, view and t draw the same frame | Two instances, every fixture × size at T |
| 9 | out-of-order calls | Asked for t = 6, 1, 2.5, 0 from one instance, with `view()`, `hit()` and an extra `frame()` at the same t between them, a chart gives the frames a fresh instance gives in order (unicel REQ-TEST-502) | Two instances, compared at each t |
| 10 | one ink | `Env { mono: true }` frames keep rules 3–7, and pass `mono::problems` (ink 1–90 %, no slab, glyphs in the font) | Every fixture at 80 × 24 |
| 11 | one ink tells series apart | In `multi-4` one ink, each series draws a glyph or pattern the others do not | Glyph sets per series from a per-series hit walk |
| 12 | paper | `Env { paper: true }` keeps rules 3–6; its glyphs equal the colour frame's unless the chart declares a ramp | Every fixture at 80 × 24 |
| 13 | resize | One instance drawn at A, then at B, then at A again gives the first A frame. The B frame equals a fresh instance's at B. `view()` is unchanged in data units. | A, B over pairs from S |
| 14 | streaming equals replace | Appending a fixture in seeded batches (1 row, 7 rows, a third, the rest), with frames drawn between appends, ends on the frame and view of a chart given all of it by `replace`. With a ring capacity c < n, it ends on `replace` of the last c rows. | Every series and ohlcv fixture, at 80 × 24 and 320 × 120 |
| 15 | replace is whole | `replace(a)`, `replace(b)` draws what `replace(b)` alone draws: nothing of a survives | Pairs of fixtures |
| 16 | view round trip | `set_view(v)` then `view()` gives v normalised. `set_view(view())` changes no frame. A view serialised to JSON and set on a fresh chart gives the same frame (REQ-VIEW-030). | Seeded views per fixture: zoomed, panned, cursor, selection, hidden series |
| 17 | input is deterministic | A scripted input list (pointer moves, a drag, a wheel, keys) replayed on two fresh instances gives the same events, view and frame. Setting the resulting view on a third instance gives the same frame (REQ-VIEW-010). | Three scripts a chart, at 80 × 24 |
| 18 | events validate | Every event `input` returns serialises to JSON that `spec/schemas/events.schema.json` accepts | Node test with ajv (the Rust side checks the serde round trip) |
| 19 | hit names what was drawn | For up to 512 seeded cells in the plot area, a hit names a datum or nothing. A named datum maps, by the chart's own scale and the core's one cell-mapping function, into that cell's column span. For a mark chart (scatter, candle, bar) the cell is one where the frame drew that datum's mark. | Hit walk against the frame and the mapping |
| 20 | hit and readout agree | The readout drawn for a cursor at a cell names the datum `hit` returns there (REQ-CHART-070) | The cursor set by view, the readout cells read |
| 21 | empty data | `empty` draws axes or a "no data" label, `hit` is none everywhere, and view domains are null | `empty` at every size |
| 22 | one point | `one` draws its point and a padded domain. No label holds `NaN`, `inf` or `-0`. | Text scan of the frame |
| 23 | gaps break lines | In line-like modes no dot column inside a NaN run is inked, unless `spanGaps` (REQ-CHART-060) | `nan-gaps`, columns from the mapping function |
| 24 | tick labels are distinct and fit | Within one axis no two tick labels are equal, and none overruns its gutter | `constant`, `tiny-range`, `huge-range`, `timestamps-dst` |
| 25 | log-invalid values | With a log scale, values ≤ 0 are skipped as the data contract says, with no NaN cells and no panic | `negative`, `zero-crossing` |
| 26 | unsorted and duplicate x | `unsorted-x` draws the frame of its sorted copy, or is refused as the data contract says. `duplicate-x` is deterministic under rule 8. | Both fixtures |
| 27 | downsampling is exact | For x-sorted data, a line-like frame equals, cell for cell, the frame drawn from every segment (REQ-CHART-040) | `sine-1k`, `random-walk-100k`, `random-walk-1m` against a brute-force rasteriser in the test |
| 28 | wrong shape | A fixture of a shape the chart does not accept gives an error naming the shape, not a panic | One per chart |
| 29 | snapshots | Frames match the snapshot files (below) | `every_chart_matches_its_snapshots` |
| 30 | budgets | Release-optimised only: frame average and worst by size and visible points, and a pointer move, against `META.budget` (`budgets.md` table). Over budget, the chart is timed twice more and the best counts (unicel REQ-CONTRACT-030). | `tests/budgets.rs`, alone |

Rules 1–29 are correctness and run in `tests/contract.rs`. The charts are spread over threads with `std::thread::scope`, every failure is collected, and one assertion reports them all with the per-chart table under `--nocapture` and `CHART=<slug>[,…]` to narrow, as unicel's `PIECE=`. Rule 30 runs in `tests/budgets.rs`, a separate test binary. Cargo runs test binaries one after another, so no other test shares the machine with the timings.

### Snapshots

- One file per chart per fixture: `crates/glyphplot/tests/snapshots/<chart>/<fixture>.snap`, plus `<chart>/<mode>-<fixture>.snap` for each non-default mode on `sine-1k`.
- Content as unicel's REQ-SNAP-002: a head line (`# <chart> · <fixture> · <cols>×<rows> · default options · default view`), then a `# t=…` block a frame with its rows and the colour and back layers, run-length. Each file holds whole frames at 80 × 24 and at `META.min`, a whole frame after a seeded zoom view, and the 320 × 120 frame as a `hash <16 hex>` line (FNV-1a, as `contract.rs:365-368`).
- The colour layer cannot stay base 64. `rendering.md` lifts the palette to 255 entries, and one base-64 digit holds 64. Each index becomes two hex digits (`..` for `NO_BACK`), each run its index and then its length when more than one (`3a12 05 ..40`). Fixed width keeps it unambiguous.
- Size estimate **(unverified)**: a whole 80 × 24 braille frame is about 5.8 KB of UTF-8 plus its layers. Three whole frames a file × 23 fixtures × 6 first-tier chart families come to about 2.4 MB. That is below unicel's 7.5 MB.
- `mise run snap [chart …]` rewrites them (`SNAP=write`). A chart without snapshots fails the contract. `web/test/snapshot.test.mjs` compares each chart's wasm frame for every fixture at 80 × 24 with the native snapshot (unicel REQ-SNAP-005).

## Property tests

The parts with laws, not pictures, get property tests in the core's unit tests (`#[cfg(test)]` modules). There proptest's regression files land under `proptest-regressions/` and are checked in. In `tests/` it could not find a source root (bench output: "FileFailurePersistence::SourceParallel set, but failed to find lib.rs or main.rs").

| Property | Area |
|---|---|
| `invert(map(v)) == v` within 1e-9 relative for finite v in the domain (REQ-SCALE-010) | scales |
| Ticks ascend strictly, lie inside the domain, and number at most 2·count + 1 | ticks |
| Every non-NaN point lies within its column's min and max, and each column's extremes are attained | downsampling |
| Merging the reductions of a head and a tail equals the reduction of the whole (streaming at the reducer level) | downsampling, ring |
| LTTB keeps the first and last index, keeps `min(threshold, n)` points, in order | downsampling |
| `column_of` and `x_of_column` agree: the x of a column's left edge maps back into that column | cell mapping |
| Laid-out labels never share a cell; every label lies inside the grid | layout |
| A formatted number parses back within the precision it claims | formatting |

proptest against quickcheck, measured (details under Measurements):

| | proptest 1.12.0 | quickcheck 1.1.0 |
|---|---|---|
| Latest release | 2026-10-10 | 2026-02-10 (crates.io API) |
| Crates pulled (normal edges, with the bench crate) | 27 default; 14 with `default-features = false, features = ["std"]` | 16 default (env_logger and regex through the default `use_logging` and `regex` features) |
| Clean dev build of the test targets | 6.1–6.5 s default; 4.7–4.8 s lean | 4.5–5.1 s |
| Clean release (LTO) build of the test targets | 19.2–22.4 s default; 17.0–17.4 s lean | 26.7–27.4 s |
| Runtime, the bench's properties, release | 4 properties × 256 cases, 0.19–0.25 s | 3 properties × 100 cases, under 0.01 s |
| Strategies | Ranges, `prop_oneof!` weighting magnitudes and NaN | `Arbitrary` per type; ranges by hand with `%` and discards |
| Reproducing a failure | `PROPTEST_RNG_SEED`; a regression file per test | No seed variable (`quickcheck-1.1.0/src/tester.rs:22-46` reads only `QUICKCHECK_TESTS`, `_MAX_TESTS`, `_GENERATOR_SIZE`, `_MIN_TESTS_PASSED`) |
| Shrinking the deliberately false property | To a 32-element vector | To a 4-element vector (`[0, 19, -2, 18]`) |

proptest wins on reproducibility and on expressing magnitudes, which are what scales and ticks break on. quickcheck shrinks vectors better. proptest's float-vector shrink gave up after 1,024 iterations, leaving about 1,000 elements. The remedy is bounded strategies (vectors of tens of points, values from a few ranges), not a different crate.

## Fixture tests against reference libraries

Each ported computation is held to the original's output, captured once by a pinned Node script into checked-in files. The method is unicel's `tests/fixtures/` (frames captured with Node from ascii.rest), with exact `f64` comparison by bits.

| Port | Reference | Fixture file (proposed) | Run by |
|---|---|---|---|
| Linear ticks | d3-array 3.2.4 `ticks` | `crates/glyphplot/tests/fixtures/ticks.txt` | `reference::ticks_match_d3_array`, `web/test/reference.test.mjs` |
| Nice domains | d3-scale 4.0.2 `linear().nice()` (REQ-SCALE-030) | `nice.txt` | `reference::nice_matches_d3_scale` |
| UTC time ticks | d3-time 3.1.0 `utcTicks` (REQ-TICK-020) | `time-ticks.txt` | `reference::time_ticks_match_d3_time` |
| Number format subset | d3-format | `format.txt` | `reference::format_matches_d3_format` |
| LTTB | flot-downsample af872e0 (MIT) | `lttb.txt` | `reference::lttb_matches_the_reference` |
| M4 / min-max | no canonical implementation; a brute-force reducer in the test | — | property tests and contract rule 27 |

Measured in the bench:

- `ticks`: 20,651 cases. 651 sit on exact decades (10^-15 to 10^15 × counts 1, 2, 3, 5, 7, 10 and 20, each from 0, symmetric about 0 and reversed), and 20,000 are random over 24 decades with integer and fractional counts. 0 differ from d3, natively and in wasm. The wasm was built for `wasm32-unknown-unknown` with the release profile and run in Node with plain exports.
- d3 rounds with `Math.round`, ties toward +∞, where Rust's `f64::round` sends ties away from zero. The port uses a `js_round`. With `f64::round` instead (feature `naive-round`) 0 cases differed too, because `tickSpec` corrects `i1` and `i2` by comparison after rounding. A port should still copy the rounding, because other d3 code has no such correction.
- LTTB: 12 cases (`sine-1k`, a 10k random walk and a 100k random walk, at thresholds 3, 100, 640 and 999). 0 differ, natively and in wasm.
- Capture: `node ref/capture.mjs` over 20,651 ticks cases and 12 LTTB cases took under a second **(unverified, not timed)**.

## Node tests against the wasm

As unicel's `web/test/`: release modules first, then a debug build. They find modules with the shell's own compiled `moduleOf` (unicel REQ-TEST-501). Fixtures reach Node as files: `cargo run --example fixtures -- write target/fixtures` writes each fixture's columns as little-endian `f64` (16 MB for the million rows), and the tests read them into `Float64Array`s. The Rust generator stays the one source.

Proposed files: `player.test.mjs` (manifest against modules, memory views against `text()`, `free()`, errors), `data.test.mjs` (typed arrays, JSON, CSV, append, replace, ring retention, bad input), `view.test.mjs` (view property semantics across the boundary, rule 16), `events.test.mjs` (scripted input to events, each validated with ajv 2020 against `events.schema.json`), `snapshot.test.mjs` (rule 29 in wasm), `reference.test.mjs` (the fixture tests in wasm), `memory.test.mjs` (replace and free grow no memory, `budgets.md`), `schemas.test.mjs` (every schema in `spec/schemas/` validates its examples), `package.test.mjs` and `exports.test.mjs`.

## Browser tests (Playwright) and axe

`site/e2e/` against `vite preview` of the release build in headless Chromium, as unicel (ADR-111). `@playwright/test` 1.64.1 and `@axe-core/playwright` 4.14.0 are current on npm (registry, 2026-10-18).

| Spec | Covers |
|---|---|
| `interaction.spec.ts` | Pointer move → crosshair cells and a hover event whose `detail` names the datum; drag → pan; wheel → zoom about the pointer; double-click → reset; keyboard (Tab to focus, arrows pan, `+`/`-` zoom, `Home`/`End`, `Esc`); touch pan and pinch; a brush selection |
| `element.spec.ts` | Attributes and properties; `view` set and read; two charts linked through `view`; a chart module fetched once for many tags; teardown and freeing |
| `streaming.spec.ts` | Appends at 1 kHz for a second, then the frame equals a `replace` of the same rows |
| `resize.spec.ts` | Container width changed → cols change, the view is kept in data units, no console error |
| `charts.spec.ts` | Every chart's page draws a first frame with no console error (unicel's `pieces.spec.ts`) |
| `a11y.spec.ts` | axe on the index, a chart page and the element test page, light and dark, no serious or critical violation; the data table present and labelled |

Touch needs care. Playwright's touch page, titled "Touch events (legacy)", emulates pan and pinch by `locator.dispatchEvent('touchstart', …)` (https://playwright.dev/docs/touch-events). Those are synthetic `TouchEvent`s and fire no Pointer Events. If the element listens for `pointerdown` with `pointerType === "touch"`, as `interaction.md` proposes, the tests must instead drive Chromium's `Input.dispatchTouchEvent` over a CDP session (https://chromedevtools.github.io/devtools-protocol/tot/Input/), which produces real touch input with two touch points for a pinch. That makes the touch tests Chromium-only, which `test:site` already is.

The smoke set in `check` is about ten tests: one chart's first frame, a hover, a drag, a wheel, a key, the four axe scans. Everything else runs in `signoff` (ADR-111).

## Terminal tests

`crates/cli/tests/terminal.rs` keeps unicel's harness: `libc::openpty`, the child on the slave side, output read with timestamps (`terminal.rs:22-103`). No crate is needed. expectrl 0.9.0, rexpect 0.7.1 and portable-pty 0.9.0 exist (crates.io API, 2026-10-18), but unicel's 182 lines already do the job. New cases for glyphplot:

- Piped output prints one plain frame of the data given on stdin.
- SGR mouse reports (`\x1b[<0;10;5M` press, `\x1b[<32;…M` drag, `\x1b[<64;…M` wheel) pan and zoom, seen as a changed frame.
- `TIOCSWINSZ` on the pty, then SIGWINCH, redraws at the new size.
- Lines streamed into stdin appear in the frame without a restart.
- A key, Ctrl+C and SIGTERM give the terminal back (`ENTER`/`LEAVE` sequences and modes, as `terminal.rs:119-161`).

## Benchmarks

- `mise run bench [chart …]`: native release. One row per chart × fixture (1k, 100k, 1M) × size (80 × 24, 320 × 120), 300 frames with a panning view, then 300 pointer moves. Columns: average, worst, budget, `OVER`. It exits 1 when any row is over, which unicel's `bench.rs` does not (it exits 0 and marks `OVER`). It prints `uptime`'s load first, so a reader can discount a loaded run.
- `mise run bench:web` (`budgets.md`): the same in headless Chromium, run by `signoff`. It is not part of `check`, for the reason of ADR-021.

## Measurements

All on Apple M5 Pro, 18 cores, 48 GB, macOS, 2026-10-18. Load averages are `uptime`'s 1, 5 and 15 minute figures, taken before each run.

### M1. unicel's contract and snapshots today

`cd ../animate-unicode && mise exec -- cargo test -q --release -p unicel --test contract`, after a release build of the tests (19.15 s, partly cached). Wall time is `/usr/bin/time -p` real; "finished in" is libtest's.

| What | Runs (s) | Min | Median | Load |
|---|---|---|---|---|
| Whole binary (8 tests, in parallel) | 32.72, 32.64, 32.48 | 32.48 | 32.64 | 3.44–3.64 |
| `every_piece_keeps_the_contract` alone | 32.36, 32.50, 34.18 | 32.36 | 32.50 | 3.02–3.82 |
| `every_piece_matches_its_snapshot` alone | 6.87, 7.10, 7.21 | 6.87 | 7.10 | 6.87–7.58 |

- The whole binary's user time was 39.0–39.3 s against 32.5–32.7 s real. The contract is one thread checking 207 pieces in turn, and the binary takes as long as it does.
- The per-piece table (`--nocapture`, one run) reported 207/207 ok. The pieces' average frame times sum to 48.58 ms, the largest being holo-koi at 5.29 ms (320 × 80). The contract draws several hundred frames a piece (four plays to t = 6 at 15 fps, the out-of-order pairs, the one-ink check), so drawing is about half its time **(estimate from the table, not measured apart)**.

### M2. The same contract under load, with and without LTO

Interleaved runs, three of each, while other agents were building. "noLTO" is a separate target dir with `--config 'profile.release.lto=false' --config 'profile.release.codegen-units=16' --config 'profile.release.opt-level=3'`. Its clean build of unicel's tests took 16.5 s (one run, load 9.00).

| Build | finished in (s) | Min | Median | Load |
|---|---|---|---|---|
| release (LTO, `opt-level = "s"`) | 71.35, 64.67, 60.66 | 60.66 | 64.67 | 7.52–8.80 |
| no LTO, `opt-level = 3` | 57.19, 60.63, 40.19 | 40.19 | 57.19 | 7.21–8.54 |

Under a load of 7–9 the contract took twice its idle time. Without LTO it was no slower.

### M3. Build costs of the property-test crates

`discovery/bench/testing/compile-cost.sh`: `cargo build -q --tests [--release] [--features …]` into a fresh target dir each run. The release profile is unicel's (`opt-level = "s"`, `lto = true`, `codegen-units = 1`).

| Build | Runs (s) | Min | Median | Load |
|---|---|---|---|---|
| dev, no property crate | 0.8, 0.7, 0.7 | 0.7 | 0.7 | 10.80 |
| dev, proptest (default features) | 6.5, 6.1, 6.1 | 6.1 | 6.1 | 10.39–10.82 |
| dev, proptest (`default-features = false, features = ["std"]`) | 4.7, 4.8, 4.7 | 4.7 | 4.7 | 6.03–6.81 |
| dev, quickcheck (default features) | 4.6, 4.5, 5.1 | 4.5 | 4.6 | 10.25–10.79 |
| release, no property crate | 6.8, 7.0, 7.3 | 6.8 | 7.0 | 10.89–12.19 |
| release, proptest (default) | 22.4, 19.2, 22.0 | 19.2 | 22.0 | 11.41–12.14 |
| release, proptest (lean) | 17.3, 17.4, 17.0 | 17.0 | 17.3 | 6.90–8.75 |
| release, quickcheck (default) | 27.4, 27.2, 26.7 | 26.7 | 27.2 | 10.16–10.77 |

The lean proptest rows ran at a lower load than the others, so they flatter it by an unknown amount.

Incremental: `touch src/lib.rs`, then the same build.

| Build | Runs (s) | Load |
|---|---|---|
| release (LTO), no property crate | 7.5, 7.4, 7.4 | 10.07–10.57 |
| release (LTO), proptest | 11.6, 11.5, 11.4 | 10.69–10.84 |
| no LTO, `opt-level = 2`, 16 codegen units, proptest | 1.2, 1.3, 1.3 (first, clean: 8.0) | 9.99–10.05 |

`cargo test --release --no-run -v --test reference` passes `-C opt-level=s -C lto -C codegen-units=1` to rustc for the test target. `cargo test --release` uses the release profile.

Property test runtime: `cargo test -q --features proptest --test props_proptest`, 4 properties × 256 cases, took 3.65 s in a dev build (one run, load about 8) and 0.19–0.25 s in release (three runs across the session).

### M4. Fixture and reduction costs

`cargo run -q --release --example cost`: 7 timings a step, min and median. Three runs at load 5.72.

| Step | Min (ms), runs 1/2/3 | Median (ms), runs 1/2/3 |
|---|---|---|
| Generate all 14 fixtures (sum of medians) | 1.053, 1.158, 1.096 | — |
| Generate `random-walk-1m` | 0.862, 0.962, 0.867 | 0.937, 1.036, 0.967 |
| Generate `random-walk-100k` | 0.086, 0.089, 0.087 | 0.087, 0.092, 0.088 |
| Min-max 1M → 640 columns | 1.443, 1.521, 1.564 | 1.493, 1.589, 1.827 |
| Min-max 1M → 80 columns | 1.111, 1.114, 1.139 | 1.137, 1.149, 1.168 |
| LTTB 1M → 640 | 1.076, 1.058, 1.075 | 1.087, 1.070, 1.094 |
| LTTB 100k → 640 | 0.108, 0.108, 0.110 | 0.112, 0.108, 0.110 |
| 10,000 `ticks` calls | 0.346, 0.359, 0.355 | 0.363, 0.365, 0.371 |
| Clone 1M points | 0.230, 0.247, 0.232 | 0.241, 0.257, 0.277 |

### M5. Two equal column formulas

`cargo run -q --release --example columns`: for n = 1–2,000 points with x = 0..n and cols ∈ {40, 80, 160, 316, 320, 640}, `((x * cols / n) as usize)` and `((x * (cols / n)) as usize)` disagree for 1,137 of 12,006,000 points. The first is x = 75, n = 150, cols = 316: 158 against 157. The proptest property `every_point_lies_within_its_columns_min_and_max` found it at cols = 316 before the bench gave the reducer and the test one shared `column_of`.

### M6. Contract runtime for glyphplot (estimate)

**(unverified)**: no chart exists. Built from M4, `budgets.md`'s frame times (≤ 0.81 ms average at ≤ 100k visible points, 2.1–2.4 ms at 1M at 320 × 120, native) and unicel's mean of 0.23 ms a frame (48.58 ms / 207 pieces, M1).

Rules 3–29 for one chart take about 23 fixtures × 5 sizes × about 22 frames (determinism 2, one ink, paper, out-of-order 8, resize 3, streaming 5, view 2), about 2,500 frames. Of those, about 110 frames are at 1M points (2.4 ms each), about 110 at 100k (0.8 ms), and the rest at about 0.25 ms. That is about 1 s a chart on one thread. Rule 30 adds 300 frames × 2 sizes × 3 fixture sizes, about 2 s a chart. Six families come to about 18 s single-threaded, or under 5 s for rules 1–29 spread across threads plus 12 s for budgets.

## Recommendation

1. **Layers.** Keep unicel's, with the matrix contract above:

| Layer | Tool | Command | In `check` | In `signoff` |
|---|---|---|---|---|
| Core unit and property tests | `cargo test`, proptest (lean) | `test:rust` | yes | yes, with `PROPTEST_CASES=4096` |
| Chart contract, rules 1–29 | `tests/contract.rs`, threads over charts | `test:rust` | yes | yes |
| Budgets, rule 30 | `tests/budgets.rs`, alone | `test:rust` | yes | yes |
| Snapshots | the contract; `SNAP=write` | `test:rust`, `snap` | yes | yes |
| Reference fixtures | `tests/reference.rs`; capture by `scripts/fixtures.mjs` | `test:rust`, `fixtures` (rewrites them) | yes | yes |
| Wasm | `node --test web/test/*.test.mjs`, release then debug | `test:wasm` | yes | yes |
| Elm site | elm-test | `test:elm` | yes | yes |
| Browser | Playwright, Chromium, axe | `test:site` (`--smoke` in check) | smoke | all |
| Terminal | `crates/cli/tests/terminal.rs` (pty, Unix) | `test:rust` | yes | yes |
| Size | gzip per module against `budgets.json` | `size` | yes | yes |
| Native bench | table, exit 1 over budget | `bench` | no | no (the contract's rule 30 holds the budget) |
| Browser bench | `budgets.md` harness | `bench:web` | no | yes |

2. **Profile.** Add `[profile.contract]` (`inherits = "release"`, `lto = false`, `codegen-units = 16`, `opt-level = 3`, `debug-assertions = false`), and run `test:rust` as `cargo test --workspace --profile contract`. Budgets are skipped when `cfg!(debug_assertions)`, as now. The shipped wasm keeps the release profile with LTO, and `bench:web` times that.
3. **Split correctness from timing.** `tests/contract.rs` checks rules 1–29 with charts in parallel. `tests/budgets.rs` times alone, retimes twice when over, and prints `uptime`'s load in its table. `test:site` keeps `wait_for = ["test:rust"]`.
4. **One mapping function.** The core exposes the data-to-cell and cell-to-data mappings once. Frames, hit tests, readouts, the reducer and the contract all call them. Rule 19 exists to catch a chart that does not.
5. **proptest**, with `default-features = false, features = ["std"]`, in `#[cfg(test)]` modules of the core, with regression files checked in.
6. **Fixtures in Rust from seeds**; three small public-domain datasets checked in with their licence. Reference captures (d3, LTTB) checked in, rewritten only by `mise run fixtures`, compared bitwise in both native and wasm.
7. **A self-test for the contract**: three broken charts behind a `contract-selftest` feature that the contract must reject, run by `test:rust`. This closes unicel's "nothing tests the test" gap.
8. **Tasks.** `check` runs `lint`, `test` (`test:rust`, `test:wasm`, `test:elm`), `build`, `size` and `test:site --smoke`, in one task graph. `signoff` runs `npm ci`, those, all of `test:site`, `bench:web`, and the property tests with `PROPTEST_CASES=4096`.

### Draft check names for `spec/conformance.md`

Rust (`mise run test:rust`):

- `contract::every_chart_keeps_the_contract` (rules 1–13, 21–26, 28)
- `contract::streaming_equals_replace` (14)
- `contract::replace_is_whole` (15)
- `contract::view_round_trips` (16)
- `contract::input_is_deterministic` (17)
- `contract::hits_name_what_was_drawn` (19, 20)
- `contract::downsampling_is_exact` (27)
- `contract::every_chart_matches_its_snapshots` (29)
- `contract::selftest_rejects_broken_charts` (feature `contract-selftest`)
- `budgets::every_chart_keeps_its_budgets` (30)
- `reference::ticks_match_d3_array`, `reference::nice_matches_d3_scale`, `reference::time_ticks_match_d3_time`, `reference::format_matches_d3_format`, `reference::lttb_matches_the_reference`
- `glyphplot scale::tests::invert_undoes_map`, `ticks::tests::ticks_are_ordered_inside_and_about_count`, `downsample::tests::every_point_within_its_column`, `downsample::tests::merged_equals_whole`, `downsample::tests::lttb_keeps_ends_and_order`, `layout::tests::labels_never_share_a_cell`, `format::tests::formatted_numbers_parse_back`
- `terminal` (`crates/cli/tests/terminal.rs`): `piped_it_prints_one_plain_frame`, `mouse_pans_and_zooms`, `sigwinch_redraws_at_the_new_size`, `stdin_streams_into_the_chart`, `a_key_stops_it_and_gives_the_terminal_back`, `ctrl_c_stops_it_with_130_and_gives_the_terminal_back`, `sigterm_stops_it_and_gives_the_terminal_back`

Node (`mise run test:wasm`): `web/test/player.test.mjs`, `data.test.mjs`, `view.test.mjs`, `events.test.mjs`, `snapshot.test.mjs`, `reference.test.mjs`, `memory.test.mjs`, `schemas.test.mjs`, `package.test.mjs`, `exports.test.mjs`.

Browser (`mise run test:site`): `site/e2e/interaction.spec.ts`, `element.spec.ts`, `streaming.spec.ts`, `resize.spec.ts`, `charts.spec.ts`, `a11y.spec.ts`.

Tasks: `mise run lint`, `build`, `size`, `bench:web`, `signoff`.

## Requirements to propose

Milestones follow `charts.md:144`: v0.1.0 is the first chart (line) end to end with the contract, v0.2.0 adds candlestick, bar and sparkline, v0.3.0 brings streaming. The synthesis may renumber.

- REQ-CONTRACT-010: `crates/glyphplot/tests/contract.rs` MUST check every registered chart against rules 1–29 of the chart-contract table, over every fixture whose shape the chart's `META.shapes` lists, at 1 × 1, `META.min`, 40 × 12, 80 × 24 and 320 × 120, with no test of the chart's own. (v0.1.0)
- REQ-CONTRACT-020: The contract MUST report every failing chart in one run, print a per-chart table with `--nocapture`, and accept `CHART=<slug>[,<slug>…]`, failing on an unknown slug. (v0.1.0)
- REQ-CONTRACT-030: Time budgets MUST be checked in `tests/budgets.rs`, a test binary of their own, only in a build without debug assertions; a chart over budget MUST be timed twice more and the lowest average and the lowest worst count. (v0.1.0)
- REQ-CONTRACT-040: Two fresh charts given the same data, options, view and `t` MUST draw identical cells, colours and backs; one chart asked for t = 6, 1, 2.5, 0 with `view()`, `hit()` and a repeated `frame()` between MUST draw the frames a fresh chart draws in order. (v0.1.0)
- REQ-CONTRACT-050: A chart drawn at size A, then B, then A MUST draw its first A frame again; its B frame MUST equal a fresh chart's at B; and its view MUST be unchanged in data units. (v0.1.0)
- REQ-CONTRACT-060: Appending a fixture in the batches 1, 7, ⌊n/3⌋ and the rest, with frames drawn between appends, MUST end on the frame and view that `replace` of the same rows gives; with a ring of capacity c < n, on `replace` of the last c rows. (v0.3.0)
- REQ-CONTRACT-070: `set_view(view())` MUST change no frame, and a view serialised to JSON and set on a fresh chart with the same data MUST draw the same frame. (v0.1.0)
- REQ-CONTRACT-080: A scripted input list replayed on two fresh charts MUST give identical events, views and frames, and setting the resulting view on a third MUST give the same frame. (v0.1.0)
- REQ-CONTRACT-090: For up to 512 seeded cells of the plot area, a datum `hit` names MUST map, through the core's cell-mapping function, into that cell's column span; for a mark chart, the frame MUST have drawn that datum's mark in that cell. (v0.1.0)
- REQ-CONTRACT-100: Every chart MUST draw `empty`, `one`, `nan-gaps`, `constant`, `huge-range`, `tiny-range`, `negative`, `unsorted-x` and `duplicate-x` without a panic, with no tick label equal to another on the same axis, and with no `NaN`, `inf` or `-0` in any label. (v0.1.0)
- REQ-CONTRACT-110: A fixture of a shape a chart does not accept MUST give an error that names the shape, not a panic. (v0.1.0)
- REQ-CONTRACT-120: A feature `contract-selftest` MUST register three broken charts (non-deterministic, outside its charset, keeping state across appends), and a test MUST assert that the contract rejects each with the matching message. (v0.2.0)
- REQ-FIX-010: Fixture datasets MUST be generated by `crates/glyphplot/src/fixtures.rs` from SplitMix64 with a fixed seed each, and MUST include at least the 23 fixtures named in `discovery/testing.md`; two generations MUST be bitwise equal on every target. (v0.1.0)
- REQ-FIX-020: Real datasets checked in under `crates/glyphplot/tests/data/` MUST each be under 100 KB, public domain or MIT-compatible, and named with source and licence in that directory's README. (v0.2.0)
- REQ-SNAP-010: Each chart MUST have one snapshot file per accepted fixture at `crates/glyphplot/tests/snapshots/<chart>/<fixture>.snap`, holding whole frames at 80 × 24, at `META.min` and after a seeded zoom, and the 320 × 120 frame as an FNV-1a hash. (v0.1.0)
- REQ-SNAP-020: A snapshot's colour and back layers MUST write each palette index as two lowercase hex digits (`..` for no back), each run followed by its length when more than one. (v0.1.0)
- REQ-SNAP-030: `mise run snap [chart …]` MUST rewrite snapshots; a chart without a snapshot MUST fail the contract; `web/test/snapshot.test.mjs` MUST compare every chart's wasm frame for every fixture at 80 × 24 with the native snapshot. (v0.1.0)
- REQ-REF-010: Each port of d3-array, d3-scale, d3-time, d3-format or the LTTB reference MUST be compared bitwise with output captured from the pinned original by `mise run fixtures`, in `tests/reference.rs` and in `web/test/reference.test.mjs`. (v0.1.0)
- REQ-PROP-010: The core MUST hold proptest properties for scale inversion, tick order and count, reduction bounds, merged reductions equalling the whole, LTTB's ends and order, cell-mapping agreement and label non-overlap, with regression files checked in. (v0.1.0)
- REQ-PROP-020: `mise run signoff` MUST run the property tests with `PROPTEST_CASES=4096`. (v0.2.0)
- REQ-TEST-010: `mise run test:rust` MUST run `cargo test --workspace --profile contract`, a profile inheriting release with LTO off and `opt-level = 3`. (v0.1.0)
- REQ-TEST-020: `mise run check` MUST run `lint`, `test`, `build`, `size` and `test:site --smoke` in one task graph and finish in under five minutes from a warm cache on the owner's machine. (v0.1.0)
- REQ-E2E-010: `site/e2e/interaction.spec.ts` MUST cover pointer hover with its event, drag pan, wheel zoom, keyboard pan, zoom, reset and escape, and touch pan and pinch driven through CDP `Input.dispatchTouchEvent`. (v0.1.0)
- REQ-E2E-020: An axe scan of the index, a chart page and the element test page, in both themes, MUST find no violation of impact serious or critical. (v0.1.0)
- REQ-TERM-010: `crates/cli/tests/terminal.rs` MUST check, in a pseudo-terminal, a piped plain frame, SGR mouse pan and zoom, a SIGWINCH redraw at the new size, stdin streaming, and the terminal restored after a key, Ctrl+C and SIGTERM. (v0.5.0)
- REQ-BENCH-010: `mise run bench [chart …]` MUST print a row per chart, fixture size and grid size with average, worst, budget and `OVER`, print the load average first, and exit 1 when any row is over. (v0.2.0)

## Decisions with a real alternative

### D1. proptest or quickcheck

Context: scales, ticks, reducers and layout have laws. One crate should check them.
Options:
- (a) proptest, lean features. Strategies by range and magnitude, seeds and regression files. 14 crates, 4.7 s clean dev.
- (b) quickcheck. 16 crates, 4.5–5.1 s clean dev, 26.7–27.4 s release with its default logging and regex. Better vector shrinking. No seed to replay a failure.
- (c) Neither: hand-rolled loops over a seeded `Rng`, as unicel does. No new crates, no shrinking, no regression files.

Recommendation: (a).

### D2. One contract test or two binaries

Context: unicel's contract is one sequential test that also times frames. glyphplot's matrix is about 2,500 frames a chart.
Options:
- (a) As unicel: one test, sequential, timings inside.
- (b) Correctness in `tests/contract.rs`, parallel over charts; timings in `tests/budgets.rs`, alone.

Recommendation: (b). Correctness then scales with cores, and nothing shares the machine with the timings except other `mise` tasks.

### D3. The test build profile

Context: `cargo test --release` uses fat LTO. A library edit costs 7.4–11.6 s of relinking before any test runs. The shipped wasm needs LTO; the native tests do not.
Options:
- (a) As unicel: `--release`.
- (b) `--profile contract`: release without LTO, `opt-level = 3`, 16 codegen units.

Recommendation: (b). Incremental rebuilds took 1.2–1.3 s, and the contract ran no slower (M2). Budgets stay native and are not comparable with wasm in either case.

### D4. Snapshot colour encoding

Context: REQ-SNAP-002 writes an index as one base-64 digit (64 colours). `rendering.md` proposes 255.
Options:
- (a) Two hex digits an index, fixed width.
- (b) Keep base 64 and cap the palette at 64.
- (c) Variable-width indices with separators.

Recommendation: (a).

### D5. Fixtures generated or checked in

Context: the million-point fixtures are 16 MB as `f64`.
Options:
- (a) Generated from seeds in Rust, written to `target/` for Node.
- (b) Checked in, perhaps with Git LFS.
- (c) Generated in both Rust and JS from the same PRNG.

Recommendation: (a), plus three small real datasets checked in. Generation of all 14 bench fixtures took about 1.1 ms.

## Open questions for the owner

1. **Budget gating when the machine is loaded.** The contract's wall time doubled under load (M1 against M2). Options:
   - (a) Retime twice and keep the best, as unicel.
   - (b) Skip budgets when the one-minute load is above the core count ÷ 2, and say so in the table.
   - (c) Gate budgets only in `signoff`.

   Recommendation: (a) in `check`, with the load printed. Add (b) only if it flakes.
2. **How big `check` may grow.** unicel's contract alone is 32.5 s idle. Options:
   - (a) A target of under five minutes for `check` from a warm cache (REQ-TEST-020).
   - (b) No target.

   Recommendation: (a).
3. **Touch tests on real devices.** CDP touch covers Chromium only. Options:
   - (a) The owner's manual pass on an iPhone and an Android device, listed in the conformance map as manual.
   - (b) Add Playwright's WebKit project for touch.

   Recommendation: (a), as unicel's REQ-TEST-040.
4. **Whether the contract checks wasm frames for every fixture or frame 0 only.** Every fixture at 80 × 24 is about 23 frames a chart in Node. Recommendation: every fixture at 80 × 24; the ports' floating-point code is what differs between targets.

## Sources

- `../animate-unicode/crates/unicel/tests/contract.rs:1-534` (the contract, snapshots and port fixtures)
- `../animate-unicode/crates/unicel/src/mono.rs:1-159` (one-ink checks, `FONT_RANGES`)
- `../animate-unicode/crates/unicel/examples/bench.rs:1-50` (exit codes, `OVER`)
- `../animate-unicode/crates/unicel/src/meta.rs:80-124` (`Category::budget`)
- `../animate-unicode/crates/cli/tests/terminal.rs:1-182`
- `../animate-unicode/web/test/load.mjs`, `web/test/snapshot.test.mjs`, `web/test/player.test.mjs` (test names)
- `../animate-unicode/site/e2e/a11y.spec.ts`, `pieces.spec.ts`, `shell.spec.ts`; `site/playwright.config.ts`
- `../animate-unicode/mise.toml` (tasks `test`, `test:rust`, `test:wasm`, `test:site`, `lint`, `size`, `check`, `signoff`, `snap`, `bench`)
- `../animate-unicode/scripts/signoff.sh`, `scripts/snap.sh`
- `../animate-unicode/Cargo.toml:20-35` (release and test profiles)
- `../animate-unicode/spec/SPEC.md` §4.4 (lines 315-349), §11 (799-886), §12 (887-933)
- `../animate-unicode/spec/conformance.md:1-131, 220-231`
- `../animate-unicode/spec/adr/021-local-ci-signoff.md`, `110-frame-snapshots.md`, `111-browser-tests.md`
- `../animate-unicode/docs/IMPLEMENTATION_PLAN.md:171-205` (the piece-contract table)
- `discovery/budgets.md` (budget table), `interaction.md` (view state, input), `charts.md:129-145, 553-581`, `rendering.md` (palette, minimum sizes, REQ-LAYOUT-030), `data.md` (ring), `scales.md` (REQ-SCALE-*, REQ-TICK-*), `domains.md:136-146, 205, 225-226`, `a11y-theming.md:180`
- d3-array 3.2.4 `src/ticks.js`, ISC, installed from npm into `discovery/bench/testing/ref/node_modules`
- flot-downsample `jquery.flot.downsample.js`, MIT, https://github.com/sveinn-steinarsson/flot-downsample, commit af872e0 (2024-06-25, GitHub API)
- proptest 1.12.0 `src/test_runner/config.rs:26-40` and quickcheck 1.1.0 `src/tester.rs:22-46`, `Cargo.toml` features (cargo registry sources)
- crates.io API, 2026-10-18: proptest 1.12.0 (2026-10-10), quickcheck 1.1.0 (2026-02-10), expectrl 0.9.0, rexpect 0.7.1, portable-pty 0.9.0, insta 1.49.0, criterion 0.8.2
- npm registry, 2026-10-18: @playwright/test 1.64.1, @axe-core/playwright 4.14.0, d3-array 3.2.4, d3-scale 4.0.2, fast-check 4.10.2
- https://playwright.dev/docs/touch-events ("Touch events (legacy)": pan and pinch by `dispatchEvent`)
- https://chromedevtools.github.io/devtools-protocol/tot/Input/ (`Input.dispatchTouchEvent`)
- Measurements M1–M5: `discovery/bench/testing/` (README.md reruns them)
