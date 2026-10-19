# ADR-133: A 200 KB module gate

Status: Proposed (2026-10-18)

## Context

ADR-008 sets a generous size threshold per module and leaves the number to the
owner, from discovery's measurements. unicel gates at 150 KB gzipped (225 KB
for scenes), and its modules are 43–126 KB.

Module sizes, from `discovery/budgets.md` (Size ladder; wasm-pack release
with unicel's profile and wasm-opt flags, `gzip -9`):

- A line chart with scales, time, M4 and no serde: 17,437 B.
- The same with serde and serde_json for options and meta: 49,297 B.
- With a candlestick and M4 as well: 52,647 B.

First frame, from budgets.md (Load; headless Chromium 156, median of 7 cold
loads, from navigation start; modules padded with a custom section to reach
each size):

| Module, gzip | Brief 4G: 170 ms, 9 Mbps | Lighthouse slow 4G: 562.5 ms, 1.44 Mbps, CPU 4× |
|---|---|---|
| 52.6 KB | 635 ms | 2,082 ms |
| 150 KB | 722 ms | 2,616 ms |
| 300 KB | 857 ms | 3,436 ms |
| 1,000 KB | 1,482 ms | 7,238 ms |

Each further 100 KB cost about 87 ms and 530 ms on the two profiles; a 200 KB
module interpolates to about 0.8 s and 2.9 s. The options were 150 KB
(unicel's), 200 KB, 300 KB, or a total a page, which cannot be checked at
build time because the page chooses its charts.

## Decision

- `mise run size` fails when a module's `glyphplot_bg.wasm` passes its budget
  gzip -9, 204,800 B unless its budget says otherwise, naming each module over
  (REQ-SIZE-010).
- It warns for each module past half its budget, 100 KB by default
  (REQ-SIZE-040); `WASM_GZ_LIMIT` overrides every limit (REQ-SIZE-030).
- A page holding one chart draws its first frame within 1.0 s and 3.0 s on
  the two profiles (REQ-PERF-080).
- This refines ADR-008. The owner sets the number; this is the proposal.

## Consequences

- A chart can grow to four times today's full module before the gate.
- 300 KB would miss the 3.0 s load budget on slow 4G (3,436 ms).
- The padded modules add no code, so compile time at 200 KB of real code is
  **(unverified)**.
