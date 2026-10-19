# ADR-101: Chart families with modes

Status: Accepted (2026-10-18), by the owner (open question 7 of the plan)

## Context

ADR-005 makes each chart type its own Cargo feature and wasm module, and lets
a chart that is another chart with a mode share its module.
`discovery/charts.md` counts about 45 chart names in the brief's list, many
differing only in their marks (step and line, histogram and bar, OHLC and
candle). Its options are a module a chart name (about 45 modules), a module a
family with modes inside (about 18, six at the first tier), or a few large
groups, as unicel's categories.

Every module repeats the base. `discovery/budgets.md` measured the empty
wasm-bindgen module with std at 6,161 B gzipped and a line chart with serde at
49,297 B (wasm-pack release, unicel's profile, `gzip -9`). A second chart type
in the same module, a candlestick, added 2,109 B (ladder step 5). The prior
art draws the same line: ratatui's `GraphType`, Lightweight Charts'
`LineType` and textplots' `Shape` switch marks over one series model
(charts.md, code read).

charts.md made `sparkline` its own module only if the axis, tick and date code
it leaves out weighed more than about 10 KB gzipped. budgets.md measured that
code at 2,694 B (linear scale, nice ticks, SI labels, ladder step 2) plus
1,147 B (UTC time scale and date labels, step 3): 3,841 B.

## Decision

- A chart family is one folder `crates/glyphplot/src/charts/<family>/`, one
  Cargo feature in `glyphplot` and `glyphplot-wasm`, and one wasm module
  (REQ-CHART-010, REQ-ARCH-040).
- Variants of a type are chosen by the `mode` option, which `META.modes`
  lists; an unlisted mode fails naming the valid ones (REQ-CHART-030).
- A family may play several types. `sparkline` is its own element type,
  played by the `line` module (REQ-SPARK-010); the manifest maps each type to
  its family's module (REQ-MANIFEST-020).
- The families are `line`, `candles`, `bar`, `scatter`, `heatmap` and later
  `sky`, with the types and modes of ROADMAP.md §7.
- A chart uses the core and its own family folder only (REQ-ARCH-050).
- This refines ADR-005: "a module per chart type" reads "a module per family".

## Consequences

- A page with a line chart and sparklines loads one module, not two.
- A new mode costs its marks and transform, not another copy of the base.
- A sparkline page carries axis and date code it does not draw (3,841 B).
- Revisit a family that passes ADR-133's 175 KB warning by splitting it.
