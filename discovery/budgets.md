# Performance and size budgets

Discovery agent 6 of 10, 2026-10-18. Brief: `README.md`, "The briefs", topic 6.

## Summary

- A line chart module (grid, braille canvas, line chart over two `Float64Array` columns, box-drawing axes, linear and UTC time scales with nice ticks, SI and date labels by hand, M4 downsampling, hover with a crosshair, a ring buffer) is **17.4 KB gzipped** without serde and **49.3 KB** with serde and serde_json for options and meta. A second chart type (candlestick) in the same module adds 2.1 KB. The empty wasm-bindgen module with std is 6.2 KB.
- serde with serde_json is the largest single cost: **+33.1 KB gzipped**, twice everything else in the module. Core's float formatting is next: **+9.3 KB** when tick labels use `format!("{:.2}")`, and **+10.4 KB** from calling `f64::clamp` anywhere, whose assertion formats both bounds with `Debug`.
- At one million points a frame averages **2.1–2.4 ms natively, 3.2–3.7 ms in Chromium and 3.8–4.3 ms in WebKit** with M4 downsampling, at 80 × 24 and 320 × 120 cells, with no frame over 5 ms; without downsampling up to 5.0 ms natively and 7.5 ms in WebKit. Every measured configuration is inside a 60 Hz frame with room for the shell's draw.
- A pointer move (hit-test by binary search, crosshair over the cached frame, a readout) costs **1 to 8 µs** at one million points: input is not where the time goes, provided a pointer move never re-runs the data pass.
- Streaming at 1 kHz into a full ring of one million points with a 60 Hz redraw costs **about 155 ms of CPU a second natively and 236 ms in Chromium**, all of it the redraws; the appends are negligible.
- One chart holding one million x and y points takes **270 wasm pages (17.7 MB)**; loading the same data through `set_data(Float64Array, Float64Array)` peaks at **516 pages (33.8 MB)**, and wasm memory never shrinks.
- Proposed: a size gate of **200 KB gzipped a chart module**, with a warning at 100 KB (unicel gates at 150 KB), a frame budget of **4 ms average and 16 ms worst natively at up to one million visible points**, an input budget of **0.5 ms a pointer move**, a memory budget of **16 bytes a point plus 1 MB a chart**, and a load budget of **1.0 s to first frame on the brief's 4G profile and 3.0 s on Lighthouse's slow 4G** (measured: 0.64 s and 2.08 s for a 53 KB module; three serial round trips are most of it, and preload hints remove one).

## Scope and method

- Everything was built and measured in `discovery/bench/budgets/` (throwaway; its `README.md` reruns every number). One crate, `ladder/`, whose Cargo features are the steps of the size ladder; each step is a separate `wasm-pack build --target web --release` with unicel's release profile (`opt-level = "s"`, `lto = true`, `codegen-units = 1`, `panic = "abort"`, `strip = true`) and wasm-opt flags (`-Os --enable-bulk-memory --enable-nontrapping-float-to-int`), copied from `../animate-unicode/Cargo.toml` and `../animate-unicode/crates/wasm/Cargo.toml`.
- Size is `gzip -9c x_bg.wasm | wc -c`, as unicel's `mise run size` (`../animate-unicode/mise.toml`, `[tasks.size]`); the JS glue is reported raw and gzipped.
- Machine: Apple M5 Pro, 18 cores, 48 GB, macOS (Darwin 25.5.0). Date: 2026-10-18. Toolchain from `mise exec`: Rust 1.97.1, Node 26.5.1, wasm-pack 0.15.0, wasm-tools 1.261.0, Playwright 1.64.1 with Chromium 156.0.8078.4 headless shell. The machine was shared with other discovery agents benchmarking at the same time; each timing run records its `uptime` load, which was between 6.7 and 12 on 18 cores.
- Each timing is 300 frames after one warm-up frame, repeated 5 times; tables give the minimum and median over the 5 runs of each run's average and of each run's worst frame.
- The chart under test is the bench's `LineChart` (`ladder/src/line.rs`): x is time in ms (one sample a second from 2026-09-21), y a random walk plus a sine, the x axis a UTC time scale. A frame clears the grid, finds the visible range by binary search, computes the y domain from the visible points, makes it nice, draws the line into a braille canvas (Bresenham between consecutive dots, or M4 per dot column), puts the canvas on the grid, and draws axes, ticks and labels. "Moving view": the view is the data's full width, offset by up to 10% of it on a sine over the 300 frames, so 90–100% of the points are visible in every frame and no frame can reuse the last.
- Not measured here: the shell's canvas draw (atlas renderer, agent 4), the element's layout, the font's load, and Safari (see "Safari"); Playwright's WebKit stands in for it.

## Size ladder

Each step adds one feature to the one before; every step is a full release build (`scripts/ladder.sh`, output in `results/ladder.txt`).

| Step | What it adds | wasm raw | wasm gzip -9 | Δ gzip | JS glue raw / gzip |
|---|---|---|---|---|---|
| 0 | Empty wasm-bindgen module with std: one export returning a `String` | 13 746 | 6 161 | — | 5 493 / 1 661 |
| 1 | Grid (`u32` cells, `u8` colours), braille canvas, line chart over `Float64Array` x and y, box-drawing axes, ring buffer, hover with crosshair | 27 187 | 12 368 | +6 207 | 11 269 / 2 782 |
| 2 | Linear scale, nice ticks, SI number formatting by hand, tick labels | 32 456 | 15 062 | +2 694 | 11 269 / 2 782 |
| 2b | Step 2 with labels through `core::fmt` (`write!(s, "{:.*}", d, v)`) instead | 54 426 | 24 382 | +9 320 over 2 | 11 269 / 2 782 |
| 3 | UTC time scale: calendar ticks (1 s to 10 years) and date labels by hand | 34 746 | 16 209 | +1 147 | 11 269 / 2 782 |
| 4 | serde + serde_json: options parsed from JSON, metas written as JSON (as unicel) | 106 787 | 49 297 | +33 088 | 14 745 / 3 486 |
| 5 | A second chart type (candlestick) in the same module | 111 995 | 51 406 | +2 109 | 17 653 / 3 696 |
| 6 | M4 downsampling (line) and candle merging per column | 114 823 | 52 647 | +1 241 | 17 653 / 3 696 |
| — | Steps 1–3 and 6 without serde or candles: the line chart alone | 37 241 | 17 437 | — | 11 269 / 2 782 |

`f64::clamp` measured separately: the same ladder built with five calls to `f64::clamp` (results in `results/ladder-with-f64-clamp.txt`) made step 1 22 719 bytes gzipped instead of 12 368 (+10 351), and made step 2b cost nothing over step 2, because float formatting was already linked. `twiggy paths` showed the retaining path: `core::fmt::float::float_to_decimal_common_shortest` ⬑ `<f64 as Debug>::fmt` ⬑ the function table, from the assertion message in `f64::clamp`, which prints `min` and `max` with `{:?}`. Integer `clamp` has no such message. Every `f64::clamp` in the bench was replaced by `.max(lo).min(hi)` before the ladder in the table was built.

### What a module is made of

`scripts/breakdown.sh` builds steps unstripped (profile `size-names`, release with names, as unicel's), lists every item's size with twiggy 0.7 (installed into `ladder/target/tools`), and sums by owner (`scripts/classify.mjs`). Shares are of the code and data before wasm-bindgen and wasm-opt, so they are proportions, not bytes of the release module. Output: `results/breakdown.txt`.

| Owner | Step 0 | Step 1 | Step 3 | Step 4 (serde) | Step 6 |
|---|---|---|---|---|---|
| Chart code (`ladder::`, including serde derives from step 4) | 1% | 23% | 32% | 17% | 21% |
| serde, serde_json (incl. its float writer, `zmij`) | — | — | — | 20% | 18% |
| Float formatting and parsing (`core::num::flt2dec`, `dec2flt`) | — | — | — | 12% | 11% |
| `core::fmt` (formatting, panic messages) | 9% | 8% | 8% | 10% | 9% |
| Data section (`.rodata`) | 3% | 4% | 5% | 14% | 13% |
| wasm-bindgen | 28% | 20% | 17% | 7% | 7% |
| Allocator (dlmalloc) | 36% | 20% | 17% | 6% | 6% |
| Rest of std, core, alloc; libm; tables | 23% | 25% | 23% | 15% | 15% |

Reading: without serde, a module is about one quarter chart code, one fifth allocator and one fifth wasm-bindgen. With serde, serde_json, the float formatting and parsing it brings and its tables are half the module (20% + 12% + most of the data section's growth from 2.4 KB to 18 KB).

### Comparison with unicel

unicel's Appendix B (`../animate-unicode/docs/IMPLEMENTATION_PLAN.md`) measured its base, std + serde_json + wasm-bindgen with one small piece, at 33.4 KB gzipped, and its modules at 43–126 KB gzipped against a 150 KB gate (225 KB for scenes; ADR-009, SPEC §11.3 REQ-SIZE-001). The bench's step 4 (49.3 KB) is in line: unicel's base plus about 16 KB of chart, scales and time.

## Frame time

Native: `cargo run --release --example native -- 5` (`ladder/examples/native.rs`), `results/native-1.txt`, load 7.8 → 6.7. Node: `node js/node-bench.mjs bench 5` against `pkg/6-downsample` (zero-copy load), `results/node-1.txt`, load 9.0 → 9.5. Chromium 156 headless: `node js/browser-bench.mjs 5 chromium`, the page served cross-origin isolated so `performance.now()` is precise to 5 µs (`crossOriginIsolated` was true), `results/chromium-a.txt`, load 8.6 → 8.3. WebKit 27.2 (Playwright's build, not Safari): `node js/browser-bench.mjs 5 webkit`, `results/webkit-a.txt`, load 8.3 → 8.5. Each column is the quietest of its runs; the other runs are kept in `results/` (`native-a.txt` at load 10.6 was up to 1.6× slower, `native-b.txt` at load 6.8 → 6.0 up to 1.4× slower at 1M points, `chromium-1.txt` at load 10.8 within 3% of `chromium-a.txt`). `native-1.txt` was built before `f64::clamp` was replaced by `.max().min()` (the only code change; `native-b.txt` is after it). Native timings moved more with load than the browser's did; on a shared machine with performance and efficiency cores, a native run can land on either **(unverified: not pinned)**. ds = M4 downsampling per dot column. Milliseconds a frame; "avg" is a run's mean, "worst" a run's slowest frame; each cell is min / median over 5 runs.

| Cells | Points | ds | Native avg | Node avg | Chromium avg | WebKit avg | Native worst | Node worst | Chromium worst | WebKit worst |
|---|---|---|---|---|---|---|---|---|---|---|
| 80 × 24 | 1 000 | — | 0.007 / 0.007 | 0.008 / 0.009 | 0.008 / 0.010 | 0.013 / 0.014 | 0.009 / 0.051 | 0.014 / 0.064 | 0.030 / 0.055 | 0.060 / 0.060 |
| 80 × 24 | 1 000 | M4 | 0.006 / 0.007 | 0.007 / 0.008 | 0.009 / 0.010 | 0.012 / 0.013 | 0.019 / 0.037 | 0.015 / 0.045 | 0.040 / 0.040 | 0.060 / 0.080 |
| 80 × 24 | 10 000 | — | 0.043 / 0.049 | 0.040 / 0.042 | 0.049 / 0.051 | 0.068 / 0.074 | 0.114 / 0.132 | 0.069 / 0.083 | 0.115 / 0.115 | 0.200 / 0.200 |
| 80 × 24 | 10 000 | M4 | 0.040 / 0.041 | 0.049 / 0.050 | 0.056 / 0.057 | 0.073 / 0.075 | 0.070 / 0.091 | 0.098 / 0.109 | 0.110 / 0.120 | 0.160 / 0.200 |
| 80 × 24 | 100 000 | — | 0.323 / 0.328 | 0.325 / 0.341 | 0.381 / 0.387 | 0.537 / 0.542 | 0.403 / 0.469 | 0.495 / 0.524 | 0.495 / 0.565 | 0.760 / 0.780 |
| 80 × 24 | 100 000 | M4 | 0.248 / 0.259 | 0.308 / 0.319 | 0.374 / 0.379 | 0.473 / 0.477 | 0.301 / 0.481 | 0.428 / 0.444 | 0.495 / 0.515 | 0.700 / 0.740 |
| 80 × 24 | 1 000 000 | — | 2.60 / 2.75 | 2.54 / 2.58 | 2.75 / 2.79 | 3.95 / 4.00 | 3.12 / 3.33 | 3.25 / 3.58 | 3.07 / 3.12 | 4.60 / 4.68 |
| 80 × 24 | 1 000 000 | M4 | 2.10 / 2.38 | 2.45 / 2.50 | 3.24 / 3.26 | 3.83 / 3.86 | 2.45 / 5.07 | 2.84 / 3.26 | 3.69 / 3.73 | 4.50 / 4.62 |
| 320 × 120 | 1 000 | — | 0.051 / 0.053 | 0.057 / 0.058 | 0.073 / 0.073 | 0.083 / 0.087 | 0.081 / 0.083 | 0.076 / 0.103 | 0.135 / 0.145 | 0.180 / 0.220 |
| 320 × 120 | 1 000 | M4 | 0.054 / 0.056 | 0.061 / 0.062 | 0.078 / 0.080 | 0.093 / 0.096 | 0.082 / 0.100 | 0.103 / 0.119 | 0.145 / 0.155 | 0.200 / 0.220 |
| 320 × 120 | 10 000 | — | 0.134 / 0.139 | 0.140 / 0.142 | 0.162 / 0.169 | 0.199 / 0.210 | 0.252 / 0.270 | 0.233 / 0.254 | 0.270 / 0.310 | 0.380 / 0.420 |
| 320 × 120 | 10 000 | M4 | 0.128 / 0.130 | 0.148 / 0.151 | 0.178 / 0.181 | 0.217 / 0.228 | 0.181 / 0.236 | 0.249 / 0.291 | 0.260 / 0.285 | 0.400 / 0.460 |
| 320 × 120 | 100 000 | — | 0.790 / 0.808 | 0.694 / 0.762 | 0.924 / 0.935 | 1.17 / 1.21 | 0.949 / 1.03 | 1.10 / 1.22 | 1.15 / 1.17 | 1.50 / 1.66 |
| 320 × 120 | 100 000 | M4 | 0.409 / 0.415 | 0.466 / 0.474 | 0.571 / 0.581 | 0.710 / 0.728 | 0.568 / 0.629 | 0.596 / 0.645 | 0.710 / 0.715 | 0.940 / 1.02 |
| 320 × 120 | 1 000 000 | — | 5.03 / 5.08 | 4.73 / 4.79 | 5.51 / 5.54 | 7.27 / 7.48 | 5.62 / 5.76 | 5.34 / 5.72 | 6.12 / 6.14 | 8.28 / 8.76 |
| 320 × 120 | 1 000 000 | M4 | 2.40 / 2.45 | 3.91 / 4.80 | 3.70 / 3.71 | 4.24 / 4.29 | 2.69 / 3.44 | 5.52 / 5.72 | 4.13 / 4.20 | 4.82 / 4.96 |

Readings:

- Every configuration in every engine, including one million points without downsampling at 320 × 120, averages under 7.5 ms with no frame over 9 ms: inside a 60 Hz frame. The data pass is the cost: time grows with the points visible, not with the cells.
- wasm is 1.0–1.8× native here: at 1M points with M4 at 320 × 120, 2.4 ms native, 3.7 ms Chromium, 4.3 ms WebKit, 3.9–4.8 ms Node; without M4 the engines are closer (4.7–7.5 ms against 5.0). Chromium was the most repeatable: two runs at loads 10.8 and 8.6 agreed within 3%.
- M4 halves the 1M-point frame at 320 × 120 natively (5.0 → 2.4 ms) and cuts it by a third in Chromium (5.5 → 3.7 ms): it replaces a Bresenham line per point with a bin update per point and one vertical run per dot column, and gives the y domain in the same pass. At 80 × 24 it saves little natively (2.6 → 2.1 ms) and costs in wasm (Chromium 2.8 → 3.2 ms): there the undownsampled line's segments are a dot long and skipped, while the bin update converts every x to a column. Binning by searching each column's index range in the sorted x column, then a min/max over a contiguous slice, would avoid that conversion **(unverified: not built)**.
- The M4 frame and the undownsampled frame of the 100k-point preview (`native -- show`) looked the same by eye; the bench does not compare them cell by cell, so that is **(unverified)** as a rule.
- At one million points the frame reads 16 MB of f64 columns; 2.1 ms is about 7.6 GB/s. A frame that has to stay flat as data grows past 1M needs a min/max pyramid (precomputed per block, O(columns × log n) a frame), not a faster scan. **(unverified: not built)**

## Pointer move

`hover(col, row)` restores the last frame from a copy kept at the end of `frame()`, finds the nearest point by x with a binary search, draws a crosshair (box drawing in empty cells, a colour change on inked ones) and a readout ("02:34:04  -49.89") with the hand-written formatters. 300 moves across the plot, 5 runs, one million points, M4 on.

Microseconds; min / median over 5 runs of 300 moves.

| Cells | Native avg µs | Node avg µs | Chromium avg µs | WebKit avg µs | Native worst µs | Node worst µs | Chromium worst µs | WebKit worst µs |
|---|---|---|---|---|---|---|---|---|
| 80 × 24 | 1 / 1 | 2 / 2 | 2 / 2 | 2 / 2 | 1 / 2 | 24 / 37 | 20 / 60 | 20 / 40 |
| 320 × 120 | 3 / 4 | 7 / 8 | 5 / 5 | 6 / 6 | 8 / 12 | 42 / 43 | 20 / 40 | 40 / 60 |

A pointer move is three orders of magnitude cheaper than a frame because it does not touch the data columns beyond log2(10⁶) = 20 reads. If the crosshair were drawn by a full redraw instead, each pointer move would cost a frame (2–5 ms at 1M points) and a 60 Hz pointer would double the CPU of a static chart. Agent 3 measures this in depth (`discovery/interaction.md`).

## Streaming

Ten simulated seconds into a ring of capacity 1 000 000 that starts full: either one `push(x, y)` a millisecond (1 kHz, one boundary call each) or one `push_batch(Float64Array, Float64Array)` of 1000 every 100 ms (10 Hz), with `set_view` to the newest 1M seconds and `frame()` every 16.67 ms (60 Hz, 600 frames). The loop runs as fast as it can; "busy" is the summed time of the pushes and frames divided by 10, the CPU one real second of the feed would cost. 320 × 120 cells.

Busy ms a second and frame ms are min / median over 5 runs; the frame column gives the median run's average / the median run's worst.

| Feed | ds | Native busy ms/s | Node busy ms/s | Chromium busy ms/s | WebKit busy ms/s | Native frame avg / worst (median) | Node frame avg / worst (median) | Chromium frame avg / worst (median) | WebKit frame avg / worst (median) |
|---|---|---|---|---|---|---|---|---|---|
| 1 sample every 1 ms | M4 | 155 / 157 | 322 / 324 | 236 / 237 | 274 / 281 | 2.62 / 3.44 | 5.39 / 6.33 | 3.94 / 4.32 | 4.68 / 5.42 |
| 1 sample every 1 ms | — | 309 / 324 | 477 / 498 | 346 / 346 | 455 / 464 | 5.39 / 8.65 | 8.31 / 9.52 | 5.77 / 6.24 | 7.74 / 8.76 |
| 1000 samples every 100 ms | M4 | 151 / 151 | 271 / 296 | 233 / 234 | 177 / 278 | 2.52 / 3.28 | 4.94 / 6.01 | 3.90 / 4.34 | 4.63 / 5.36 |
| 1000 samples every 100 ms | — | 302 / 316 | 301 / 416 | 308 / 341 | 392 / 418 | 5.26 / 6.46 | 6.92 / 8.98 | 5.68 / 6.19 | 6.97 / 7.90 |

The appends are negligible: busy time is 60 × the frame time to within a few ms in every row, whether the samples come one boundary call at a time (1000 calls a second) or as one batch copy. A full ring of 1M redrawn at 60 Hz with M4 costs 15% of a core natively and 23–32% in a browser or Node; without M4, 30–50%. The levers are the redraw rate and the frame: a chart redraws only when its data or view changed and at most once per animation frame, and a large live chart may redraw at 30 Hz.

## Memory

Node, `node js/node-bench.mjs memory` and `memory-copy`, step 6 module, `WebAssembly.Memory.buffer.byteLength / 65536`, fresh process each.

| State | Pages | MB |
|---|---|---|
| After `init`, no chart | 17 | 1.1 |
| One 320 × 120 line chart, 1M points loaded zero-copy (`reserve`, write through `Float64Array` views at `x_ptr()`/`y_ptr()`, `commit`), one frame | 270 | 17.7 |
| Two such charts | 522 | 34.2 |
| Both freed | 522 | 34.2 |
| One chart, 1M points through `set_data(Float64Array, Float64Array)` (wasm-bindgen copies each array into wasm memory, the chart copies again) | 516 | 33.8 |

In Chromium and WebKit the memory report ran after the timings in the same page, so the first chart reused memory the timings had grown (292 pages); the second chart added 246 pages (16.1 MB), as in Node (`results/chromium-a.txt`, `results/webkit-a.txt`).

Readings:

- A chart costs 16 bytes a point (two f64 columns) plus its grid (5 bytes a cell in unicel with backgrounds; this bench's 4 + 1), canvas (one byte a cell), base copy and M4 bins (40 bytes a dot column): about 0.4 MB at 320 × 120 beside 16 MB of data.
- wasm memory never shrinks: freeing both charts left 522 pages. A page that loads a million points through a copying API holds twice the memory for the life of the page.
- Zero-copy load (the chart owns the columns; JS writes into views over wasm memory) halves the peak. It needs the views re-created after any allocation, since growth detaches the buffer, as unicel's `mount.ts` already does for the grid (`docs/IMPLEMENTATION_PLAN.md`, "Per-frame data flow").

## Load

Headless Chromium 156, a fresh context per load with the cache disabled, `node js/load-bench.mjs 7`, server sending the wasm and glue gzip -9 (`js/server.mjs`), page `js/page.html?mode=load`: import the glue, `init()` (streaming compile), make an 80 × 24 line chart over 1000 points, `frame()`, read the cells, write `text()` into a `<pre>`. Time is from navigation start to the end of that frame (`performance.now()`), median of 7. Profiles through CDP `Network.emulateNetworkConditions` and `Emulation.setCPUThrottlingRate`:

- "brief 4G": 170 ms latency, 9 Mbps down, 1.5 Mbps up, no CPU throttling (the brief's example).
- "Lighthouse slow 4G": Lighthouse's `mobileSlow4G` in its request-level (DevTools) form, as `core/config/constants.js` of Lighthouse 12.0.0 defines it: 150 ms × 3.75 = 562.5 ms latency, 1.6 Mbps × 0.9 down, 750 kbps × 0.9 up, CPU slowed 4×. Lighthouse's `docs/throttling.md` describes the profile as roughly the bottom quarter of 4G connections.
- "none": localhost, no throttling.

The padded modules are step 6 with a custom section of random bytes appended so the gzipped module reaches 150, 300 and 1000 KB (`js/pad.mjs`); a custom section is downloaded but not compiled, so these rows time transfer only.

`results/load-a.txt` (load 8.5 → 7.6) and `results/load-preload.txt` (load 7.2 → 7.0). Milliseconds from navigation start to the end of the first frame, min / median of 7 loads; "after wasm" is the median first frame minus the median time the wasm finished arriving, which is compile, instantiate, the chart's construction and its first frame. Transfer is the HTML, glue and wasm as sent (gzip).

| Module | Transfer B | Brief 4G | after wasm | Lighthouse slow 4G | after wasm | No throttling |
|---|---|---|---|---|---|---|
| Step 1, line chart, no serde (12.4 KB wasm gz) | 15 805 | 596 / 600 | 2.3 | 1 867 / 1 876 | 7.3 | 50 / 55 |
| Step 4, + serde (49.3 KB) | 53 583 | 629 / 630 | 1.9 | 2 051 / 2 067 | 8.8 | 51 / 55 |
| Step 6, + candles, M4 (52.6 KB) | 57 102 | 632 / 635 | 1.9 | 2 073 / 2 082 | 9.1 | 53 / 54 |
| Step 6 padded to 150 KB gz | 158 150 | 718 / 722 | 2.1 | 2 609 / 2 616 | 8.9 | 52 / 54 |
| Step 6 padded to 300 KB gz | 311 787 | 855 / 857 | 2.6 | 3 425 / 3 436 | 11.0 | 51 / 55 |
| Step 6 padded to 1000 KB gz | 1 028 813 | 1 479 / 1 482 | 3.6 | 7 229 / 7 238 | 15.2 | 51 / 55 |
| Step 6 with `<link rel="modulepreload">` for the glue and `<link rel="preload" as="fetch">` for the wasm (`js/page-preload.html`; separate run, its plain row 615 / 619 and 2 075 / 2 084) | 57 102 | 441 / 443 | 1.9 | 1 484 / 1 500 | 6.8 | 31 / 32 |

Readings:

- Round trips, not bytes, set the floor. The page makes three requests in turn (HTML, then the glue the module script imports, then the wasm the glue fetches), so the first frame cannot come before three request latencies: about 510 ms on the brief's profile and 1 690 ms on Lighthouse's. Preloading the glue and the wasm from the HTML removes one: 619 → 443 ms and 2 084 → 1 500 ms.
- Each further 100 KB gzipped costs about 87 ms on the brief's 4G and 530 ms on Lighthouse's slow 4G (the slope from 57 KB to 1 MB). A 200 KB module would reach its first frame in about 770 ms and 2 890 ms on the two profiles, by interpolating between the 150 and 300 KB rows.
- Compile, instantiate and the first frame of an 80 × 24 chart take 2–4 ms after the wasm arrives, 7–15 ms with the CPU slowed 4×; the padded rows add no code, so they do not show how compile time grows with code size. V8 compiles wasm first with its baseline compiler, Liftoff, built for fast startup (https://v8.dev/blog/liftoff), so compile time is not expected to dominate at these sizes **(unverified for modules with more code)**.
- The shell's own JS and the font are not in these numbers; they are further requests (agents 4 and 8).

## Safari

Safari was not measured. `safaridriver` refused a session: "You must enable 'Allow remote automation' in the Developer section of Safari Settings"; the setting was left as it was. Safari numbers are **(unverified)**. Playwright's WebKit 27.2 (installed into the bench's `node_modules` with `PLAYWRIGHT_BROWSERS_PATH=0`) shares Safari's engine but is not Safari; its numbers are in the tables as the nearest stand-in. It was 1.1–1.4× Chromium at 1M points.

## Measurements

| What | Number | How |
|---|---|---|
| Empty wasm-bindgen module with std, gzip -9 | 6 161 B (13 746 raw); glue 1 661 B gz | `scripts/ladder.sh`, step 0, 2026-10-18 |
| Line chart (grid, braille, axes, ring, hover), gzip -9 | 12 368 B | step 1 |
| + linear scale, nice ticks, SI labels by hand | +2 694 B | step 2 − step 1 |
| Same labels through `core::fmt` floats instead | +9 320 B | step 2b − step 2 |
| One `f64::clamp` in the module | +10 351 B | step 1 with and without, `results/ladder-with-f64-clamp.txt` |
| + UTC time scale and date labels by hand | +1 147 B | step 3 − step 2 |
| + serde, serde_json (options, meta) | +33 088 B | step 4 − step 3 |
| + candlestick chart in the same module | +2 109 B | step 5 − step 4 |
| + M4 downsampling | +1 241 B | step 6 − step 5 |
| Line chart with scales, time, M4, no serde | 17 437 B gz, 37 241 raw | step x |
| Everything (step 6) | 52 647 B gz, 114 823 raw; glue 17 653 raw, 3 696 gz | step 6 |
| serde's share of step 6 (serde + float fmt/parse + tables) | about half | twiggy 0.7 by owner, `results/breakdown.txt` |
| Frame, 1M points, 320 × 120, M4, avg / worst | native 2.40 / 2.69 ms; Chromium 3.70 / 4.13; WebKit 4.24 / 4.82; Node 3.91 / 5.52 | best run of 5 × 300 frames, panning; `results/native-1.txt`, `chromium-a.txt`, `webkit-a.txt`, `node-1.txt` |
| Frame, 1M points, 320 × 120, no downsampling | native 5.03 / 5.62 ms; Chromium 5.51 / 6.12 | same |
| Frame, 1M points, 80 × 24, M4 | native 2.10 / 2.45 ms; Chromium 3.24 / 3.69 | same |
| Frame, 100k points, 320 × 120, M4 | native 0.41 / 0.57 ms; Chromium 0.57 / 0.71 | same |
| Pointer move, 1M points, 320 × 120 | native 3 µs avg, 8 µs worst; Chromium 5 / 20 µs | 5 × 300 moves |
| Streaming 1 kHz singles into a full 1M ring, 60 Hz redraw, M4 | native 155 ms CPU/s; Chromium 236; WebKit 274; Node 322 | 10 simulated s, 5 runs, min |
| Streaming 1000-sample batches at 10 Hz, same | native 151 ms/s; Chromium 233 | same |
| wasm memory, one chart of 1M points, zero-copy | 270 pages (17.7 MB) from 17 at init | Node, `node-bench.mjs memory` |
| wasm memory, one chart of 1M points via `set_data` copies | 516 pages (33.8 MB) | Node, `node-bench.mjs memory-copy` |
| wasm memory after freeing both charts | 522 pages: unchanged | Node |
| First frame, step 6, brief 4G (170 ms, 9 Mbps) | 635 ms median of 7; 443 ms with preload hints | `js/load-bench.mjs`, `results/load-a.txt`, `load-preload.txt` |
| First frame, step 6, Lighthouse slow 4G (562.5 ms, 1.44 Mbps, CPU 4×) | 2 082 ms; 1 500 ms with preload hints | same |
| Cost of 100 KB gzipped more | 87 ms (brief 4G), 530 ms (Lighthouse slow 4G) | slope of the padded rows |
| Compile + instantiate + first 80 × 24 frame after the wasm arrives | 2–4 ms; 7–15 ms at CPU 4× | same |
| Safari | not measured **(unverified)** | safaridriver refused: remote automation is off |

## Recommendation

The budgets below are set from the measurements above with headroom for what a real chart adds (legends, annotations, several series, panes, themes, events, a text layout engine). Each is held by a named check.

| Budget | Proposed | Measured now | Held by |
|---|---|---|---|
| Module size, gzip -9 | 200 KB (204 800 B) a chart module; `mise run size` warns past 100 KB | 17.4 KB (line, no serde), 49.3 KB (line + serde), 52.6 KB (line + candles + serde + M4) | `mise run size`, from `budgets.json` |
| JS glue | Reported, not gated | 11.3–17.7 KB raw, 2.8–3.7 KB gzipped | `mise run size` prints it |
| Frame, native release, ≤ 100k visible points | 1 ms average, 8 ms worst | ≤ 0.81 ms average, ≤ 1.03 ms worst (median of 5) | contract test |
| Frame, native release, ≤ 1M visible points, default options | 4 ms average, 16 ms worst | 2.1–2.4 ms average, 2.5–3.4 ms worst with M4 | contract test |
| Frame, wasm in Chromium, ≤ 1M visible points | 8 ms average, 16 ms worst | 3.2–3.7 ms average, 3.7–4.2 ms worst with M4; WebKit 3.8–4.3 and 4.5–5.0 | `mise run bench:web`, at signoff |
| Pointer move (hit-test, crosshair, readout), 1M points | 0.5 ms average, 2 ms worst, wasm and native | 1–8 µs average, ≤ 43 µs worst | contract test and `bench:web` |
| Streaming, 1 kHz into a full 1M ring, 60 Hz redraw | ≤ 50% of one core in Chromium | 236 ms/s with M4, 346 without | `bench:web` |
| Memory a chart | 16 bytes a point of capacity a column pair, plus 1 MB | 17.7 MB at 1M points (16 MB of columns) | Node test on the release module |
| Memory on replace | Replacing data or freeing and remaking a chart of the same capacity grows no wasm memory | not measured as a rule; pages stayed at 522 after freeing | Node test |
| Load, one chart module, first frame | 1.0 s on the brief 4G profile and 3.0 s on Lighthouse slow 4G, from navigation start, for a page holding one chart (HTML, glue, wasm), measured in headless Chromium | 635 ms and 2 082 ms (step 6, 52.6 KB); 443 ms and 1 500 ms with preload hints | `mise run bench:load`, at signoff |

How the checks hold them, following unicel (`Category::budget()`, `web/pkg/budgets.json`, REQ-SIZE-502, REQ-PIECE-100):

- Each chart is one module, so a chart's budget is its module's. `Chart::budget()` (or the chart's `META`) gives `gzip`, `frame_ms`, `worst_ms`, `input_ms` in one place; the manifest writes them to `web/pkg/budgets.json` and `modules.js`.
- `mise run size` builds every module, prints raw and gzipped wasm and glue a module and the total, appends to `$GITHUB_STEP_SUMMARY`, and fails naming each module over its `gzip`; `WASM_GZ_LIMIT` overrides. `mise run size --charts` lists what a module is made of (chart code, core, serde, std, wasm-bindgen), as unicel's `size-pieces.mjs` does, so a jump like `f64::clamp`'s 10 KB is visible in review.
- `tests/contract.rs` plays every registered chart natively in release over fixtures of 1k, 100k and 1M points at 80 × 24 and 320 × 120, 300 frames with a panning view, and 300 pointer moves at 1M points; over budget it retimes twice and keeps the fastest, as unicel's does, because other work on the machine slows a run now and then and a slow chart is slow every time.
- Browser timings are not gated in CI: unicel found GitHub's runners 3–5× slower (`../animate-unicode/docs/IMPLEMENTATION_PLAN.md`, risk 9). `mise run bench:web` runs this bench's harness in headless Chromium and prints the table with the load average; `mise run signoff` runs it and fails past the browser budgets.
- A frame budget assumes defaults: downsampling is on by default, since without it 1M points at 320 × 120 take 5.0 ms natively and 5.5 ms in Chromium.

## Requirements to propose

- REQ-SIZE-010: `mise run size` MUST fail when any chart module's `glyphplot_bg.wasm` is larger than its budget gzip -9 (204 800 bytes unless the chart's budget says otherwise), naming each module over with its size and its limit. (v0.1.0)
- REQ-SIZE-020: `mise run size` MUST print each module's wasm raw and gzipped bytes, its JS glue raw and gzipped bytes, and the total, and append those lines to `$GITHUB_STEP_SUMMARY` when it is set. (v0.1.0)
- REQ-SIZE-030: `WASM_GZ_LIMIT` MUST override every module's size limit, in bytes. (v0.1.0)
- REQ-SIZE-040: `mise run size` SHOULD print a warning line for each module larger than half its budget. (v0.1.0)
- REQ-SIZE-050: `mise run size --charts` MUST list, for each module, the gzipped share of the chart's own code, the shared core, serde and serde_json, and the rest (std, alloc, wasm-bindgen), from an unstripped build. (v0.2.0)
- REQ-SIZE-060: Chart and core code MUST NOT call `f64::clamp` or `f32::clamp`; clippy's `disallowed-methods` in `clippy.toml` MUST list both, so `mise run check` fails on a call. (v0.1.0)
- REQ-SIZE-070: Tick labels, readouts and legends MUST be formatted by the core's own number and date formatters; a unit test MUST check each formatter's output against fixtures, and a chart module built without serde_json MUST contain no function whose name contains `flt2dec`, checked by `mise run size --charts`. (v0.1.0)
- REQ-PERF-010: In a release build, every chart MUST average at most 4 ms a frame with no frame over 16 ms, over 300 frames of a view panning across a fixture of 1 000 000 points at 80 × 24 and at 320 × 120 cells with default options, measured natively by the contract test. (v0.1.0)
- REQ-PERF-020: In a release build, every chart MUST average at most 1 ms a frame with no frame over 8 ms over the same play with a fixture of 100 000 points. (v0.1.0)
- REQ-PERF-030: Downsampling MUST be on by default for every chart that draws more points than its plot has dot columns. (v0.1.0)
- REQ-PERF-040: A chart's frame SHOULD be the same with and without downsampling at the cell level for a fixture whose visible points number at most twice its plot's dot columns. (v0.2.0)
- REQ-PERF-050: A pointer move (hit-test, crosshair and readout) MUST average at most 0.5 ms with none over 2 ms over 300 moves across a chart holding 1 000 000 points, natively in release and in wasm in headless Chromium. (v0.1.0)
- REQ-PERF-060: A pointer move MUST NOT read more than O(log n) data points: it redraws over the last frame's cells, not by running the frame. (v0.1.0)
- REQ-PERF-070: In headless Chromium on the owner's machine, every chart MUST average at most 8 ms a frame with no frame over 16 ms in the play of REQ-PERF-010, checked by `mise run bench:web` in `mise run signoff`. (v0.2.0)
- REQ-PERF-080: Appending one sample at 1 kHz to a full ring of 1 000 000 points while redrawing at 60 Hz MUST cost at most 500 ms of CPU a second in headless Chromium, measured as the summed time of the appends and frames over 10 simulated seconds. (v0.3.0)
- REQ-PERF-090: A chart MUST redraw only when its data, options, view or size changed since the last frame, and at most once a display frame. (v0.1.0)
- REQ-MEM-010: A chart of capacity n points of two f64 columns MUST hold at most 16 × n bytes + 1 MiB of wasm memory, measured as the growth of `memory.buffer.byteLength` in Node from creating the chart to its first frame. (v0.1.0)
- REQ-MEM-020: The element MUST load a typed-array column into wasm memory with at most one copy (JS writing into a view over memory the chart owns). (v0.1.0)
- REQ-MEM-030: Replacing a chart's data with data of the same or smaller size, or freeing a chart and making another of the same capacity, MUST NOT grow wasm memory. (v0.1.0)
- REQ-LOAD-010: A page holding one chart MUST draw its first frame within 1.0 s of navigation start on a 170 ms latency, 9 Mbps profile and within 3.0 s on Lighthouse 12's `mobileSlow4G` request-level profile (562.5 ms, 1.44 Mbps, CPU 4×), median of 7 cold loads in headless Chromium, checked by `mise run bench:load` in `mise run signoff`. (v0.2.0)

## Decisions with a real alternative

### 1. The size gate per chart module

- Context: the owner wants a gate that is generous but set. A line chart with every feature measured here is 52.6 KB gzipped with serde, 17.4 KB without. unicel gates at 150 KB (225 KB for scenes) and its modules are 43–126 KB.
- Options: (a) 150 KB, unicel's number; (b) 200 KB; (c) 300 KB; (d) a total per page instead of per module.
- Recommendation: (b) 200 KB gzipped a module, with a warning at 100 KB. It is four times today's full module, so a chart can grow legends, panes and annotations without a fight. A 200 KB module reaches its first frame in about 0.8 s on the brief's 4G and 2.9 s on Lighthouse's slow 4G (interpolated from the padded rows), inside the load budget; a 300 KB module takes 3.4 s on the latter and would not be. (d) is not checkable at build time, since the page chooses which charts it shows.

### 2. serde and serde_json in every chart module

- Context: serde + serde_json add 33.1 KB gzipped to a 16.2 KB module (step 3 → 4); every module repeats them. unicel uses them for options and meta.
- Options: (a) serde_json in every module, as unicel; (b) options set from TypeScript through typed setters, meta written natively into the manifest (as unicel's `examples/manifest.rs` already writes `modules.js`), and no JSON in the module; (c) a smaller JSON crate (nanoserde, miniserde) **(unverified: not measured)**.
- Recommendation: (a) for v0.1.0, for parity with unicel and error messages that name the bad field; the 200 KB gate holds it with room. Revisit with (b) if a module passes the 100 KB warning: it is the one change that removes two thirds of a small module.

### 3. Downsampling per frame or a precomputed pyramid

- Context: M4 per dot column scans every visible point each frame: 2.1–2.4 ms natively and 3.2–3.7 ms in Chromium at 1M points, linear in n.
- Options: (a) M4 per frame; (b) a min/max pyramid built once per data change and updated on append, read per frame in O(columns × log n); (c) M4 cached by view, reused when the view moves by whole dot columns.
- Recommendation: (a) for v0.1.0: it holds the budget at 1M points and is a few dozen lines. (b) when 10M points become a goal, since (a) would then take 20–50 ms a frame **(unverified: extrapolated linearly)**.

### 4. How data enters wasm memory

- Context: `set_data(Float64Array, Float64Array)` copies twice (wasm-bindgen into wasm memory, the chart into its columns) and peaked at 33.8 MB for 1M points, against 17.7 MB zero-copy; wasm memory never shrinks.
- Options: (a) wasm-bindgen `&[f64]` arguments; (b) the chart owns its columns and JS writes through `Float64Array` views (`reserve`, `x_ptr`, `commit`); (c) both, (b) for typed arrays and (a) for small JSON data.
- Recommendation: (c). Agent 2 (`discovery/data.md`) owns the data API; this is the memory evidence.

### 5. Where frame budgets are enforced

- Context: native timings are cheap to run in `cargo test`, though on this shared machine they moved up to 1.6× with load, which unicel's retime-twice rule absorbs; wasm in a browser is what readers get, 1.0–1.8× native here, but needs Playwright and is machine-bound.
- Options: (a) native in the contract test only; (b) browser only; (c) native gated in `mise run test`, browser gated in `mise run signoff`.
- Recommendation: (c), as unicel gates native timings and runs signoff locally.

## Open questions for the owner

1. The size gate. Options: 150 KB (unicel), 200 KB, 300 KB gzipped a module. Recommendation: 200 KB with a warning at 100 KB (Decision 1).
2. serde_json in every module (+33 KB each) or options through typed setters and meta in the manifest. Recommendation: serde_json for v0.1.0, revisit at the 100 KB warning (Decision 2).
3. The load profile the budget names. Options: the brief's 4G (170 ms, 9 Mbps), Lighthouse's slow 4G (562.5 ms request latency, 1.44 Mbps, CPU 4×), or both. Recommendation: both, as REQ-LOAD-010 drafts them: the brief's profile is the reader on a good phone network, Lighthouse's the bottom quarter of 4G, and the 200 KB gate meets both (0.8 s and 2.9 s by interpolation). Whether the element preloads its module (one round trip, 0.2–0.6 s here) is agent 8's; the measurement says it should.
4. The ceiling of the frame budget. Options: 1M visible points (measured within budget with M4 per frame), or 10M (needs a pyramid). Recommendation: 1M for v0.1.0; 10M as a v0.3.0 requirement if a domain (market ticks, telemetry) asks for it.
5. Large live charts' redraw rate. Options: 60 Hz always; 30 Hz when a frame passes 4 ms; the page decides. Recommendation: 60 Hz, redrawing only on change (REQ-PERF-090); a `max-fps` attribute for the page that wants less.

## Sources

- `../animate-unicode/Cargo.toml` (release and `size-names` profiles), `../animate-unicode/crates/wasm/Cargo.toml` (wasm-opt flags), `../animate-unicode/mise.toml` (`build:wasm`, `size`).
- `../animate-unicode/docs/IMPLEMENTATION_PLAN.md`: "Per-frame data flow", "Memory and lifetime", "Performance budgets", Appendix B.
- `../animate-unicode/spec/SPEC.md` §11.3 (REQ-SIZE-001 to 030, 501, 502) and REQ-PIECE-100; `../animate-unicode/spec/adr/008-one-wasm-module-per-category.md`, `009-size-budgets.md`.
- `../animate-unicode/crates/unicel/src/grid.rs`, `subcell.rs`, `util.rs` (`braille`), copied in part into `discovery/bench/budgets/ladder/src/grid.rs` for the measurements, since rewritten for the bench (ADR-013).
- `../animate-unicode/crates/wasm/src/lib.rs` (the Player and its pointers), `../animate-unicode/crates/unicel/tests/contract.rs` (timing: `Instant` around each frame, release only, retried twice over budget), `../animate-unicode/crates/unicel/examples/bench.rs`.
- Lighthouse 12.0.0 `core/config/constants.js`, https://unpkg.com/lighthouse@12.0.0/core/config/constants.js (`mobileSlow4G`, `DEVTOOLS_RTT_ADJUSTMENT_FACTOR = 3.75`, `DEVTOOLS_THROUGHPUT_ADJUSTMENT_FACTOR = 0.9`); https://github.com/GoogleChrome/lighthouse/blob/main/docs/throttling.md (150 ms, 1.6 Mbps / 750 kbps, 4× CPU).
- V8, "Liftoff: a new baseline compiler for WebAssembly in V8", https://v8.dev/blog/liftoff
- Chrome DevTools Protocol, `Network.emulateNetworkConditions` and `Emulation.setCPUThrottlingRate`: https://chromedevtools.github.io/devtools-protocol/
- Playwright 1.64.1 (Chromium 156.0.8078.4 headless shell, WebKit 27.2), https://playwright.dev/
- Measurements: `discovery/bench/budgets/results/*.txt`, each with its date, command and `uptime`.
