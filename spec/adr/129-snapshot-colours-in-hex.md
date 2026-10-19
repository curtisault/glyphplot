# ADR-129: Snapshot colours in hex

Status: Proposed (2026-10-18)

## Context

unicel's snapshots write each frame as text, followed by its colour and back
layers run-length encoded, one base-64 digit a palette index (unicel's
REQ-SNAP-002). One base-64 digit holds 64 values, unicel's palette limit.
ADR-102 lifts glyphplot's palette to 255 entries plus `NO_BACK`.

`discovery/testing.md` D4 weighed:

- **(a) Two hex digits an index**, fixed width, `..` for `NO_BACK`.
- **(b) Keep base 64** and cap the palette at 64, which undoes ADR-102 and
  bands a continuous scale (ΔE76 4.55 a step at 64 entries,
  `discovery/rendering.md` §2.2).
- **(c) Variable-width indices** with separators between them.

Sizes, from testing.md (Snapshots):

- unicel's snapshot tree is 7.5 MB for 207 pieces (`du -sh`).
- A whole 80 × 24 braille frame is about 5.8 KB of UTF-8 plus its layers;
  three whole frames a file × 23 fixtures × 6 first-tier families come to
  about 2.4 MB **(unverified: an estimate, no chart exists)**.

## Decision

- A snapshot's colour and back layers write each palette index as two
  lowercase hex digits, `..` for `NO_BACK`, each run followed by its length
  when more than one, as in `3a12 05 ..40` (REQ-SNAP-020).
- The rest of unicel's snapshot format stands: a head line, a block a frame,
  the 320 × 120 frame as an FNV-1a hash (REQ-SNAP-010).

## Consequences

- Every index the palette can hold has a spelling, and fixed width keeps runs
  unambiguous without separators.
- A layer of mixed colours is up to twice as wide as unicel's.
- A diff of a colour change reads as two hex digits.
- unicel's snapshot reader and writer are adapted, not reused.
