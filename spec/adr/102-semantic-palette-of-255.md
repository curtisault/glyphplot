# ADR-102: A semantic palette of 255

Status: Accepted (2026-10-18), by the owner (open question 4 of the plan)

## Context

unicel's grid holds a `u8` ink and a `u8` back a cell, indexing 2 to 64
colours; 255 is `NO_BACK`. The 64 is a contract rule, not a property of the
buffer. A heatmap or a scatter coloured by value wants a continuous scale.
`discovery/rendering.md` §2 weighed three models:

- **(a) unicel's 64 colours.** Viridis quantised to 64 steps has a largest
  neighbouring step of ΔE76 4.55, about twice the 2.3 people can see
  (`terminal/steps.mjs`, deterministic arithmetic).
- **(b) A `u8` index into a per-chart palette of up to 255.** At 128 steps
  the largest step is 2.26; the cell stays 6 bytes (layout arithmetic).
- **(c) RGB a cell.** 11 to 12 bytes a cell. Truecolour terminal output is
  within 2 % of (b) (777,760 against 765,453 B for a 320 × 120 heatmap,
  `terminal/sgr.mjs`), but the CLI must map every cell to 256 colours every
  frame instead of the palette once.

On a canvas the two cost the same once the renderer's key ignores colour
(ADR-119). `discovery/a11y-theming.md` (Themes) adds that indices naming roles
let a theme change be a repaint, while RGB carries the theme into wasm and
makes every theme change a new frame.

## Decision

- A cell's ink and back are indices 0–254 into the chart's palette, or
  `NO_BACK` (255) for the back (REQ-COLOR-010).
- The palette layout is fixed: 0–31 theme tokens (ink, muted, rule, accent,
  ground, up, down, selection, series 1–8, the rest reserved), 32–159 a
  128-step scale ramp, 160–254 reserved. A frame writes indices only, never
  colours (REQ-COLOR-020).
- `theme::resolve` in the core is the one function that turns a theme into
  the 255 RGBA entries; the shell, the CLI and the exports call it. A theme
  change repaints without a frame (REQ-COLOR-030, REQ-THEME-030).
- The ramp interpolates the theme's stops in Oklab, neighbours at most ΔE76
  2.5 apart (REQ-COLOR-040). The CLI maps the palette to 256 or 16 colours
  once a palette (REQ-COLOR-050).
- RGB a cell is rejected.

## Consequences

- Frames do not depend on the page's CSS, and the buffer keeps ADR-004's shape.
- Dark, light, paper and terminal downgrades are table swaps in the shell.
- One ramp holds one continuous scale; a second (a diverging scale beside a
  sequential one) needs the reserved range. Revisit (c) if a chart must show
  more than about two continuous scales at once (rendering.md).
- The index layout is a design rule, not a measurement.
