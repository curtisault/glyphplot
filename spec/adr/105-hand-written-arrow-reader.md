# ADR-105: A hand-written Arrow reader

Status: Proposed (2026-10-18)

## Context

Arrow IPC is the fast path for large data from a server or a file. ADR-008
asks for a decision of its own before an Arrow implementation enters a module.
The options, measured in `discovery/data.md` (wasm in Node, 1M rows of a
timestamp and a nullable `Float64`, median of 5 × 3 runs; sizes are release
modules through `gzip -9`):

| Reader | Adds to a module, gzip | 1M rows | Memory growth |
|---|---|---|---|
| Hand-written, in the core (138 lines) | +3,766 B | 7.42 ms | 33.0 MiB |
| arrow-rs 60.0.0 | +261,684 B | 10.3 ms | 38.1 MiB |
| arrow2 0.18.0 (size only) | +92,511 B | — | — |
| apache-arrow 21.2.0 in the shell | 48,477 B of JS | not timed | — |

- arrow2 is archived upstream: the GitHub API gives `"archived": true`, last
  push 2024-02-27 (data.md).
- Natively the hand-written reader read 1M rows in 1.71 ms min, 3.35 ms
  median, against 3.23 and 4.86 ms for arrow-rs.
- The measured reader handles Float64, Int64 and Timestamp only. The Arrow
  spec aligns IPC buffers to 8 bytes, so a reader could view `Float64`
  columns in place, cutting growth to about 16.5 MiB **(unverified)**.

## Decision

- The core reads an uncompressed Arrow IPC stream with its own reader:
  Float64, Float32, Int32, Int64, UInt32, Timestamp, Bool, Utf8 and
  Dictionary<Int32, Utf8> columns with validity bitmaps, adding at most 8 KB
  gzipped to a chart module (REQ-DATA-140).
- The element's `data` property and the CLI accept it (REQ-DATA-140).
- No wasm module's dependency tree holds arrow or arrow2 (REQ-SIZE-060).
- Its test fixtures are written by arrow-rs in a dev-only test.
- It ships in v0.6.0, after typed arrays and JSON.

## Consequences

- Arrow data reaches a chart at about the speed of typed arrays, in every
  module, for a few KB.
- The 8 KB ceiling for the full type list is a budget, not a measurement:
  the extra types are **(unverified)** in size.
- glyphplot maintains a reader for a format it does not own; Arrow's file
  format, compression, Parquet and nested types are out.
- Revisit if pages ask for compressed streams or nested columns.
