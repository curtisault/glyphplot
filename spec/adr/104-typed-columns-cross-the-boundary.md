# ADR-104: Typed columns cross the boundary

Status: Proposed (2026-10-18)

## Context

A chart takes JSON, CSV, typed arrays and Arrow (ADR-006). Chart modules are
one per family, every tag of a family shares one instance, and wasm memory
never shrinks (WebAssembly core spec, memory instructions). The options
(`discovery/data.md`, decision 1): parse text in wasm; parse it in the shell
and send typed columns; or a separate shared data module, which adds an
instance and a copy between the two.

Measured in `discovery/data.md` (wasm in Node, 1M rows, two columns, median of
5 × 3 runs on fresh instances; sizes `gzip -9` of release modules):

| Path | ms | Memory growth | Module, gzip |
|---|---|---|---|
| `Float64Array` as `Vec<f64>` | 4.27 | 15.4 MiB | +0 |
| `Float64Array` as `&[f64]`, copied | 5.58 | 23.1 MiB | +0 |
| JSON columns, serde_json in wasm | 163 | 80.0 MiB | +26.5 to +34.4 KB |
| JSON columns, `JSON.parse` in JS, then `Vec<f64>` | 72.9 | 15.4 MiB | +0 |
| CSV in wasm / in JS (epoch ms) | 156 / 155 | 48.4 MiB / JS heap | +17.3 KB / +0 |

In `discovery/budgets.md` (Memory) a copying `set_data` of 1M points peaked at
516 wasm pages (33.8 MB), against 270 (17.7 MB) writing into chart memory.

## Decision

- Column values cross only as typed arrays, a `Vec<f64>` argument or a buffer
  the chart reserved and the shell filled, with a JSON descriptor without
  `values`; no chart module holds a CSV or JSON data parser, which
  `mise run size --charts` checks by symbol (REQ-DATA-050).
- A typed-array column enters wasm memory with at most one copy (REQ-MEM-020).
- Numbers, timestamps and durations are held as `f64`; timestamps in epoch
  ms; `us` and `ns` columns as offsets from an `i64` origin (REQ-DATA-020).
- The shell parses JSON with `JSON.parse` and infers shorthand types
  (REQ-DATA-120), and CSV in the browser; the core's `text` feature parses CSV
  and JSON for the CLI and native users.
- The shell reads `memory.buffer` only after the call that returns an address
  (REQ-DATA-060); replacing two 1M-row columns takes at most 10 ms in Node
  and grows memory by at most 1.1 times their bytes (REQ-DATA-070).

## Consequences

- 26.5–34.4 KB (JSON) and 16–17 KB (CSV) stay out of every chart module, and
  data-sized spikes stay out of the shared memory.
- Reading `memory.buffer` before `reserve` threw "Cannot perform Construct on
  a detached ArrayBuffer" in data.md's first run.
- `f32` input is widened; revisit `f32` storage when a page needs it.
