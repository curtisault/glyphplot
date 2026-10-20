# ADR-139: Indicators in the markets pack

Status: Proposed (2026-10-18)

## Context

A market chart overlays moving averages and Bollinger bands on its prices and
draws RSI and MACD in panes of their own. `discovery/domains.md` (Where
indicators are computed) weighed three places:

- **In the candlestick module**, taking options such as
  `{"overlay": [{"sma": 20}]}`: +1,454 B gzipped in every chart module that
  has them.
- **In the markets pack**, writing columns: 1,454 B once, in a module only
  market pages load, plus one copy of new values into the chart's memory
  (0.032 µs a value, 0.127 ms for 1,000,000, in Node).
- **In TypeScript** in the shell or an example: nothing in wasm, but a second
  implementation that would have to match the Rust CLI bit for bit.

Measured in domains.md (indicator probe built with unicel's release profile,
`gzip -9`; times in Node, mean of 10 after a warm-up, two runs, over 1,000,000
values): SMA 1.2 ms, EMA 2.4 ms, Bollinger 6.7 ms in one memory layout and
26.0–26.5 ms in another (cause not found), RSI 3.6 ms, MACD 1.9 ms.

What decides it: the RSI and MACD panes are line and bar charts, so
indicators in the candlestick module would not serve the panes that draw
them; the CLI needs them too; and appending a sample is O(1) an indicator
with O(window) state.

## Decision

- The markets pack computes SMA, EMA, Bollinger bands, RSI (Wilder) and MACD
  over a column and incrementally a sample, the two agreeing within 1e-9
  relative on every fixture (REQ-MKT-010).
- Indicator values are NaN until their window is full, so no line is drawn
  there (REQ-MKT-020).
- Streaming takes amortised O(1) time a sample (REQ-PACK-050).
- No indicator lives in the candles module or in TypeScript.

## Consequences

- Indicators work the same in the browser, the CLI and Rust.
- A page with indicators loads the markets pack beside its chart modules.
- The fourfold Bollinger variance between memory layouts is unexplained and
  is watched when the pack is built.
