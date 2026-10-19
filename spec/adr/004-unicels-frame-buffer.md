# ADR-004: unicel's frame buffer

Status: Accepted (2026-10-18)

## Context

A chart could render to a string, to SVG, to canvas calls, or to a cell
buffer. unicel's grid holds a `u32` code point and colour indices a cell, read
by JS as typed-array views over wasm memory, with `WIDE_TAIL` after a wide
glyph (unicel's ADR-003, ADR-004, ADR-016). It needs no per-frame copying or
string building, and makes astral glyphs (sextants, octants) one cell.

## Decision

glyphplot's frame buffer is unicel's grid: a `u32` code point a cell, colour
for the ink and the back, read by the shell as typed-array views over wasm
memory, re-made after every frame; `WIDE_TAIL` (0) in the cell after a
double-width glyph. Whether a cell's colour stays a palette index or becomes
RGB is discovery's question (`discovery/rendering.md`, [ADR-102](102-semantic-palette-of-255.md)).

## Consequences

- The shell's renderer, the CLI's painter and the exports read one shape.
- Text in charts (labels, ticks, readouts) is glyphs in cells, laid out in
  whole cells; wide glyphs in labels take two.
- The view buffers detach when wasm memory grows (it will, as data streams in),
  so a consumer re-makes its views after every frame.
