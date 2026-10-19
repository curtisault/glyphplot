# ADR-119: No glyph-and-colour atlas

Status: Proposed (2026-10-18)

## Context

unicel draws its canvas from an atlas of 1,024 slots keyed by (code point,
colour). A braille scatter coloured by a third variable has independent glyphs
and colours. `discovery/rendering.md` §2.3 timed renderers in headless Chrome
155 (playwright-core 1.59.1, 10 warm-up and 60 timed full redraws, 3 runs,
median of run medians; the machine shared at loads 7.0–9.2):

| Workload | Renderer | 80 × 24 | 320 × 120 |
|---|---|---|---|
| Braille scatter, 64 colours | (code point, colour) atlas, 3.6 % hits | 60.1 ms | 1,130 ms |
| Braille scatter, 4,096-slot atlas | same key | 53.9–55.2 ms | 1,207–1,367 ms |
| Braille scatter, any model | atlas keyed by code point, tinted by compositing | 2.4–2.8 ms | 32.8–34.5 ms |
| Braille scatter, RGB | `fillText` a cell, no atlas | 0.9 ms | 16.1 ms |
| Braille density, 100 % hits | atlas / `fillText` | — | 46.1–48.0 / 20.7 ms |
| Block heatmap | `fillRect` a cell / one image | 0.5 / 0.9 ms | 9.2 / 6.3 ms |

With the GPU off, density at 320 × 120 took 34.1 ms by atlas and 20.6 ms by
`fillText`. Safari and headed Chrome were not measured, and a real GPU may
order the renderers differently.

## Decision

- The (code point, colour) atlas is dropped.
- A frame's draw cost does not depend on the number of distinct (glyph,
  colour) pairs: a 320 × 120 braille scatter with every cell a different
  colour draws within twice the time of the same frame in one colour in
  headless Chrome (REQ-RENDER-010).
- The renderer is `fillText` per changed cell or a code-point atlas tinted by
  compositing, chosen on the owner's Chrome and Safari in v0.3.0 and recorded
  in the plan with the numbers (REQ-RENDER-050).
- Block, quadrant, sextant, octant and wedge glyphs stay exact fills from
  `blocks.ts` (REQ-RENDER-020); back-coloured frames draw as one image
  (REQ-RENDER-040); only changed cells are redrawn (REQ-RENDER-030).

## Consequences

- The colour model (ADR-102) no longer touches draw cost.
- A full redraw of 320 × 120 braille costs 16–21 ms at best, so a streaming
  full-screen heatmap holds about 30 frames a second, not 60.
- With `fillText`, a fallback glyph wider than its cell needs a clip a cell.
- The choice stays open until the owner's measurements; revisit if Safari
  orders the two the other way.
