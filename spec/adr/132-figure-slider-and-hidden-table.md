# ADR-132: A figure, a slider and a hidden table

Status: Proposed (2026-10-18)

## Context

An interactive chart must be readable and operable without sight and without
a pointer (WCAG 2.2: 1.1.1, 1.3.1, 2.1.1, 2.2.2 at level A, 2.5.7 at AA).
`discovery/a11y-theming.md` weighed the element's role:

- **`role="img"` only**, as unicel: a focused `img` has presentational
  children, wrong for an interactive chart.
- **`role="application"`**: hands every key to the page and takes the screen
  reader's own reading keys, which Chartability warns against.
- **`role="graphics-document"`** (Graphics-ARIA 1.0), which
  `discovery/interaction.md` proposed: AT support for its children is uneven
  **(unverified)**.
- **`role="figure"`** holding a `role="img"` summary, a hidden table and a
  `role="slider"` cursor.

And where the table lives: light DOM; a shadow root, which ARIA IDREFs do not
cross yet; or a page table named by `aria-describedby`, which flattens it to a
string. Building a visually hidden three-column table and forcing layout in
headless Chromium 156 took, median of 3: 100 rows 1.5 ms, 500 rows 6.0 ms,
1,000 rows 13.0 ms, 10,000 rows 523.7 ms; 100,000 did not finish in five
minutes. Highcharts announces new data every 5,000 ms at most by default.

## Decision

- The host has `role="figure"` and a name from `label`, else `<type>: <series
  names>`; its drawing has `role="img"` and an `aria-label` with the type,
  series, axis titles and visible domains, rewritten at most once a second
  (REQ-A11Y-010).
- A light-DOM, visually hidden `<table>` holds the view, at most 500 rows,
  downsampled keeping each bucket's extremes (REQ-A11Y-020); rebuilt at most
  once a second while streaming and never while focused (REQ-A11Y-090); a
  "show data" control reveals it (REQ-A11Y-110).
- One tab stop, a `role="slider"` cursor whose `aria-valuetext` gives x and up
  to four series (REQ-A11Y-030); single-click alternatives to every drag
  (REQ-A11Y-040); reduced motion jumps (REQ-A11Y-050); a focus ring
  (REQ-A11Y-060); axe finds nothing serious (REQ-A11Y-070).
- `paused` and Space pause a stream (REQ-A11Y-080); a polite live region
  exists only with `announce` of at least 5 s (REQ-A11Y-100).
- This supersedes interaction.md's 100-row table and keyboard live region.

## Consequences

- The slider speaks its value without a live region; a million-point view
  reads as 500 rows, its caption saying how many it holds.
- NVDA, JAWS and VoiceOver behaviour is the owner's check.
