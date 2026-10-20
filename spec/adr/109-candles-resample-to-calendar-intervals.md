# ADR-109: Candles resample to calendar intervals

Status: Proposed (2026-10-18)

## Context

A cell has one colour, so a rising and a falling candle can never share one:
below one cell a candle, candles must be combined (`discovery/charts.md`,
Candle glyphs). Two ways were proposed:

- **Merge by index in powers of two** (`discovery/charts.md`, REQ-CHART-110 of
  its draft): groups of 2 to 512 consecutive candles, first open, last close,
  highest high, lowest low, as Lightweight Charts conflates bars below one
  device pixel and caches each level (`src/model/data-conflater.ts`,
  `conflation/constants.ts`, code read). It needs no zone and no calendar,
  works on any spacing, and is proven in a widely used library.
  `discovery/budgets.md` ladder step 6 (M4 for lines and candle merging per
  column) added 1,241 B gzipped (wasm-pack release, `gzip -9`).
- **Resample to a calendar interval** (`discovery/data.md`, Downsampling): the
  smallest interval of the ladder 1 s, 5 s, 15 s, 1 min, 5 min, 15 min, 1 h,
  4 h, 1 d, 1 w, 1 mo at which the candles fit, buckets aligned to time.

What decides it: an index- or pixel-aligned group starts wherever the view or
the data happens to start, so the same group of trades makes a different
candle after a pan or an append. A time-aligned bucket makes the same 5-minute
candle wherever the view is, and the readout can name its period. The cost of
calendar resampling was not measured separately **(unverified)**.

## Decision

- Every candle is at least one cell wide. When the candles in view do not fit,
  they are resampled to the smallest interval of the ladder at which they do:
  open the first row, high the maximum, low the minimum, close the last,
  volume the sum, buckets aligned to the interval in the column's zone
  (REQ-SAMPLE-040).
- Glyphs follow the spacing: box drawing at 4 cells or more, braille with a
  gap at 2 or 3, braille without it at 1 (REQ-CANDLE-020).
- In one ink no candle is narrower than four cells; the chart resamples
  instead (REQ-CANDLE-030).

## Consequences

- A candle always means a calendar period, stable under pan and append.
- Day, week and month buckets need the zone table of ADR-112.
- The ladder steps unevenly (×4 from 1 h to 4 h, ×7 from 1 d to 1 w), so after
  a step candles may be wider than one cell.
- Revisit index merging if a chart must combine bars with no meaningful time
  (a Renko mode, say).
