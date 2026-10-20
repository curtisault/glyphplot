# ADR-107: A ring allocated once

Status: Proposed (2026-10-18)

## Context

Live feeds append rows without end; a chart must keep a bounded history
cheaply. `discovery/data.md` (decisions 5 and 8) weighed:

- **A ring allocated once**, or **a growable `Vec` trimmed from the front**,
  which moves the whole retained history on every trim.
- For rows earlier than the last: **drop and count them**, or **insert them by
  binary search**, a shift of up to the ring's length. `discovery/charts.md`
  proposed refusing an earlier candle with an error event.

Measured in data.md (wasm in Node, ring of 1M, median a call):

| Operation | Time | Memory growth |
|---|---|---|
| `push(t, v)`, one sample | 0.062 µs | 0 |
| `push_batch`, 1,000 samples | 3.78 µs | 0 |
| staging write then commit, 1,000 | 2.97 µs | 0 |
| allocating the 1M ring | 1.39 ms once | 15.4 MiB once |

`discovery/budgets.md` (Streaming) put 1 kHz into a full 1M ring with a 60 Hz
redraw at 155 ms of CPU a second natively and 236 ms in Chromium, nearly all
of it the redraws. Lightweight Charts' `update` replaces a bar of equal time
and refuses an older one unless `historicalUpdate` is set (charts.md).

## Decision

- A streaming chart keeps its rows in a ring allocated once with capacity
  `retain.rows`, default 1,000,000; appending 3M rows to a 1M ring grows wasm
  memory no further after the first 1M (REQ-STREAM-020).
- `retain.age` drops rows older than the newest x minus that duration
  (REQ-STREAM-030).
- A row whose x equals the last row's replaces it; a row whose x is less is
  dropped and counted in `late` of the next `plot-data` (REQ-STREAM-040,
  REQ-STREAM-070, REQ-CANDLE-060).
- `append()` takes a row, rows or typed columns, and the shell coalesces the
  appends of one animation frame into one write (REQ-STREAM-010), within 1 µs
  for one row and 50 µs for 1,000 in Node (REQ-STREAM-050).

## Consequences

- Memory is fixed by capacity: 16 MB for two `f64` columns of 1M rows; the
  downsamplers read the ring's two segments in place.
- A forming candle updates in place.
- Out-of-order feeds lose their late rows, counted; the page sorts first.
  Revisit when a telemetry example needs out-of-order arrival.
