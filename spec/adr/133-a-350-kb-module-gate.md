# ADR-133: A 350 KB module gate

Status: Accepted (2026-10-18), by the owner

## Context

ADR-008 sets a generous size threshold per module and leaves the number to the
owner, from discovery's measurements. unicel gates at 150 KB gzipped (225 KB
for scenes), and its modules are 43–126 KB.

Module sizes, from `discovery/budgets.md` (Size ladder; wasm-pack release,
wasm-opt `-Os`, `gzip -9`):

- A line chart with scales, time, M4 and no serde: 17,437 B.
- The same with serde and serde_json for options and meta: 49,297 B.
- With a candlestick and M4 as well: 52,647 B.

First frame, from budgets.md (Load; headless Chromium 156, median of 7 cold
loads, from navigation start; modules padded with a custom section to reach
each size):

| Module, gzip | 4G: 170 ms, 9 Mbps | Lighthouse slow 4G: 562.5 ms, 1.44 Mbps, CPU 4× |
|---|---|---|
| 52.6 KB | 635 ms | 2,082 ms |
| 150 KB | 722 ms | 2,616 ms |
| 300 KB | 857 ms | 3,436 ms |
| 1,000 KB | 1,482 ms | 7,238 ms |

Each further 100 KB cost about 87 ms and 530 ms on the two profiles. Discovery
proposed 200 KB, which met a 3.0 s budget on Lighthouse's slow 4G as well as
1.0 s on the 170 ms 4G profile. The options were 150 KB (unicel's), 200 KB,
300 KB, or a total a page, which cannot be checked at build time because the
page chooses its charts.

## Decision

The owner set the gate at 350 KB, building for fast connections and
performant sites.

- `mise run size` fails when a module's `glyphplot_bg.wasm` passes its budget
  gzip -9, 358,400 B unless its budget says otherwise, naming each module over
  (REQ-SIZE-010).
- It warns for each module past half its budget, 179,200 B by default
  (REQ-SIZE-040); `WASM_GZ_LIMIT` overrides every limit (REQ-SIZE-030).
- A page holding one chart draws its first frame within 1.0 s on the 170 ms
  4G profile (REQ-PERF-080). A 350 KB module interpolates to about 0.9 s there
  (857 ms at 300 KB plus about 44 ms) **(unverified: interpolated)**.
- Lighthouse's slow 4G is measured and reported by `bench:load`, not gated: a
  350 KB module interpolates to about 3.7 s there (3,436 ms at 300 KB plus
  about 265 ms) **(unverified: interpolated)**.
- This refines ADR-008.

## Consequences

- A chart can grow to almost seven times today's full module (52.6 KB) before
  the gate, and to more than three times before the warning.
- Readers on the bottom quarter of 4G connections wait about 3.7 s for a
  module at the gate; glyphplot does not promise them a budget.
- The padded modules add no code, so compile time at 350 KB of real code is
  **(unverified)**; V8 compiles wasm first with its baseline compiler, Liftoff.
