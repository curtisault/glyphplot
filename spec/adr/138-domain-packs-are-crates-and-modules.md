# ADR-138: Domain packs are crates and modules

Status: Proposed (2026-10-18)

## Context

Domains need adapters and arithmetic that charts do not: counter rates,
histogram quantiles, OHLCV aggregation, indicators, planet positions. The
owner forbids a pack from bloating a chart module, and the CLI needs the same
code as the browser. `discovery/domains.md` weighed:

- **(a) Cargo features of the chart modules.**
- **(b) TypeScript beside the shell**: nothing in wasm, but the CLI and the
  Rust crate would need a second implementation.
- **(c) A Rust crate per domain**, its own wasm module on the web, linked
  natively in the CLI.

Measured in domains.md (probes built with unicel's release profile, no
wasm-bindgen and no wasm-opt, `gzip -9`; copies in Node):

- An astronomy probe (eight planets, RA and Dec, alt and az): 6,460 B.
- An indicator probe (SMA, EMA, Bollinger, RSI, MACD): 1,454 B.
- Both together: 7,051 B.
- Copying a 1,000,000-value `Float64Array` from one wasm memory to another:
  0.127 ms, mean of 50; one value 0.032 µs.

So size does not decide where a pack's code lives; separation, one
implementation for web and terminal, and the cost of crossing do.

## Decision

- A pack is a crate `crates/packs/<domain>/`, named `glyphplot-<domain>`, that
  depends on the core's data contract and on no chart, which `cargo tree`
  shows (REQ-PACK-010).
- It builds to its own wasm module, and no chart module holds its exports
  (REQ-PACK-020); it stays under 32 KB gzipped (REQ-SIZE-070).
- Its output is columns in the data contract that the chart its preset names
  accepts unchanged (REQ-PACK-030), with the same bits natively and in wasm
  (REQ-PACK-040), and its streaming functions take amortised O(1) time a
  sample (REQ-PACK-050).
- The CLI links every pack natively (REQ-ARCH-080).
- TypeScript in examples only wires transport.

## Consequences

- Chart modules stay chart-only; a page without a domain loads no pack.
- A page using a pack loads two modules with two memories, and copies each
  new value across once.
- A pack never draws; a domain's look is a preset of options.
