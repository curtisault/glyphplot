# ADR-110: Scales and formats ported from d3

Status: Proposed (2026-10-18)

## Context

Every chart needs scales, nice domains, ticks, number formats and calendar
ticks with labels, and each module pays for what it links (ADR-005, ADR-008).
`discovery/scales.md` built one wasm-bindgen export per candidate
(wasm-pack release, unicel's profile, `gzip -9`, two clean builds identical):

| Variant | Over a base module |
|---|---|
| Hand port of d3: nice, ticks, SI labels, UTC ticks and labels | +5.5 KB |
| `time` 0.3.55 / `chrono` 0.4.45 date formatting | +8.5 / +13.8 KB |
| jiff 0.2.38 `strftime` / with its tz database | +63.6 / +150.0 KB |
| ICU4X decimal / datetime, locale at run time | +25.4 / +120.9 KB |
| `num-format` one locale / any locale | +8.7 / +16.7 KB |
| `format!("{x:.2}")` | +10.4 KB |
| `Intl.DateTimeFormat` through js-sys, a call per label | +2.0 KB |

- The port matched d3 on all 803 captured cases (scales.md, `tests/d3.rs`);
  a port of d3-array's `ticks` matched bit for bit on 20,651 cases natively
  and in wasm (`discovery/testing.md`, reference tests).
- One `f64::clamp` added 10,351 B, through the `Debug` formatting of its
  assertion (`discovery/budgets.md`, ladder step 1 with and without, traced
  with `twiggy paths`).
- Formatting in JS per label crosses the boundary every frame, depends on the
  browser's ICU and cannot serve the CLI (scales.md).

## Decision

- Scales, ticks, `nice`, a d3-format subset, time intervals and a strftime
  subset are hand-ported from d3 into the core (REQ-SCALE-010, REQ-SCALE-030,
  REQ-TICK-010, REQ-TICK-020, REQ-FMT-010, REQ-FMT-020).
- Each port is compared bitwise with output captured from the pinned original
  by `mise run fixtures`, natively and in wasm (REQ-REF-010). Rounding copies
  JavaScript's `Math.round`.
- No wasm module's tree holds chrono, time, jiff, chrono-tz, num-format or an
  `icu_` crate (REQ-SIZE-060).
- Labels, readouts, legends and table cells come from the core's formatters,
  never `core::fmt` on floats (REQ-FMT-030); `f64::clamp` and `f32::clamp`
  fail lint (REQ-LINT-040).

## Consequences

- About 5.5 KB buys d3's behaviour, with d3 as the test oracle.
- glyphplot owns the port; a new d3 release changes nothing until recaptured.
- Locale-correct output rests on the locale pack of ADR-112.
- jiff MAY serve the CLI natively; it never enters wasm.
