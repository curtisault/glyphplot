# ADR-006: Data is injected

Status: Accepted (2026-10-18)

## Context

unicel's pieces generate what they draw from a seed; its three data pieces
(candlesticks, bar-chart, sparkline) draw made-up numbers. A chart library
exists to draw someone else's numbers: telemetry, network traffic and
topology, prices, star positions, sensor logs.

## Decision

Every chart draws data the page or the terminal gives it. A chart never
generates its data, except in a demo mode that says so on screen and in its
metadata.

## Consequences

- The boundary carries data in (once, replaced, or streamed) as well as frames
  out; the data contract is a first-class part of the spec, with a schema.
- Empty data is a normal state with a defined frame, not an error.
- Fixture datasets for tests are generated deterministically or checked in,
  never fetched.
