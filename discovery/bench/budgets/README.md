# Budgets bench (throwaway)

This directory is throwaway. It holds the code that measured the numbers in
`discovery/budgets.md` and nothing else depends on it. It is not a start on
glyphplot: the next session builds the library from `docs/IMPLEMENTATION_PLAN.md`,
not from this.

- `ladder/` is one standalone crate (an empty `[workspace]`, `target/` inside it)
  whose Cargo features are the steps of the size ladder: `line` (grid, braille
  canvas, line chart over two `Float64Array` columns, box-drawing axes, a ring
  buffer, hover with a crosshair), `scale` (linear scale, nice ticks, SI number
  formatting by hand), `fmtfloat` (the same labels through `core::fmt`, for
  comparison), `time` (UTC calendar ticks and date labels by hand), `serde`
  (serde and serde_json for options and meta), `candle` (a candlestick chart in
  the same module), `downsample` (M4 per dot column). Its release profile and
  wasm-opt flags are unicel's. `src/grid.rs` is the frame buffer and braille
  canvas, written for this bench.
- `ladder/examples/native.rs` times frames, hover and streaming natively.
- `js/bench-core.mjs` runs the same scenarios against the wasm;
  `js/node-bench.mjs` runs them in Node, `js/browser-bench.mjs` in headless
  Chromium (Playwright), `js/load-bench.mjs` times the first frame on a
  throttled network, `js/server.mjs` serves this directory for those (gzip,
  cross-origin isolated), `js/pad.mjs` makes padded copies of a module, `js/page-preload.html` is the
  load page with preload hints.
- `results/` holds the raw output of the runs the report cites.

## Rerunning every measurement

From the glyphplot root, with the toolchain from `mise.toml`:

```sh
B=discovery/bench/budgets
# Size ladder: each step a separate wasm-pack release build into $B/pkg/<step>.
mise exec -- $B/scripts/ladder.sh
# What a module is made of (twiggy into the bench's target dir, then names).
mise exec -- cargo install twiggy --locked --root $B/ladder/target/tools
mise exec -- $B/scripts/breakdown.sh
# Native frame, hover and streaming timings, 5 runs.
mise exec -- cargo run --release --manifest-path $B/ladder/Cargo.toml --example native -- 5
# The same in wasm: Node, then headless Chromium (and Playwright's WebKit).
(cd $B/js && mise exec -- npm install)
mise exec -- node $B/js/node-bench.mjs bench 5
mise exec -- node $B/js/browser-bench.mjs 5 chromium
(cd $B/js && PLAYWRIGHT_BROWSERS_PATH=0 mise exec -- npx playwright install webkit)
PLAYWRIGHT_BROWSERS_PATH=0 mise exec -- node $B/js/browser-bench.mjs 5 webkit
# Memory: wasm pages after loading 1M points.
mise exec -- node $B/js/node-bench.mjs memory
mise exec -- node $B/js/node-bench.mjs memory-copy
# Load: padded modules, then time to first frame on throttled networks.
mise exec -- node $B/js/pad.mjs $B/pkg/6-downsample/ladder_bg.wasm 150 300 1000
mise exec -- node $B/js/load-bench.mjs 7
mise exec -- node $B/js/load-bench.mjs 7 preload   # js/page-preload.html against plain
# Or every timing in turn, one after another, into results/<name>-<tag>.txt:
mise exec -- $B/scripts/run-all.sh <tag>
```

Safari was not measured: `safaridriver` needs "Allow remote automation" in
Safari's Developer settings, which was off and was left off.

Record `uptime` with each timing run: other work on the machine moves the
numbers.
