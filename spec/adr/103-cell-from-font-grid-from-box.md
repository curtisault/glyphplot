# ADR-103: The cell from the font, the grid from the box

Status: Proposed (2026-10-18)

## Context

unicel inverts what a chart needs: a piece's `META` fixes `cols × rows`, and
the canvas cell is the element's width over `cols` (`mount.ts` `size()`,
`fit.ts`), with rounded per-cell edges that make cells 1 px uneven. A chart's
text must stay readable, so its cell has a floor. The options
(`discovery/rendering.md` §5, `discovery/surfaces.md` Sizing):

- **A fixed grid scaled to the box**, unicel's `fit`. Scaling shrinks text:
  in headless Chrome at 1× pixels the closest pair of digits differs by 1 px
  at an 8 px font and by 7 px from 11 px up (rendering.md §4.3, digits drawn
  into a cell and thresholded at 50 % coverage, one run; a proxy for
  legibility, not legibility).
- **The cell from the font size, the grid from the box**: unicel's planned
  `fit="fill"` made the default. At exactly 1:2 braille and octant dots are
  square, so a scatter keeps its aspect.

Exact device pixels come from `devicePixelContentBoxSize`, which MDN's
compatibility data lists for Chrome 84 and Firefox 108 and not for Safari
(read 2026-10-18). A block element has no height of its own; rendering.md's
open question 6 weighed a `rows` default, an aspect ratio and required CSS.

## Decision

- Without `cols` and `rows`, the cell is `round(0.6 × font-size ×
  devicePixelRatio)` device pixels wide and exactly twice that tall; `cols ×
  rows` are the whole cells that fit the content box, and the remainder is
  split evenly as padding (REQ-EL-080). Safari uses `contentBoxSize ×
  devicePixelRatio`.
- A change of the box lays the chart out again within one animation frame,
  keeping its view in data units; a box of width 0 is ignored (REQ-EL-090).
- An element with no height of its own takes it from `rows`, default 12
  (REQ-EL-100).
- unicel's fixed grid and `fit` (`width`, `contain`) apply only when `cols`
  and `rows` are given (REQ-EL-110).

## Consequences

- Labels stay at the page's font size; a resize re-lays out axes and ticks
  instead of scaling them.
- A chart dropped into a container with no height shows 12 rows.
- Fixed-size exports and server-rendered stills use `cols` and `rows`.
- The smallest usable font is not settled: rendering.md proposes 11 px at 1×
  and 9 px from 2× **(unverified)** until the owner checks digits by eye.
