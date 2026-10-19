# ADR-116: Input defaults

Status: Proposed (2026-10-18)

## Context

The owner's test is a reader scrubbing a live candlestick with a finger
(PROMPT.md). Libraries disagree on defaults, read in code by
`discovery/interaction.md`:

- **Mouse drag**: pans in Lightweight Charts; selects a range and zooms to it
  in uPlot (`src/opts.js`).
- **Plain wheel**: zooms in Lightweight Charts by default, so a page cannot
  scroll past the chart (`chart-options-defaults.ts`); page scroll with
  Ctrl/⌘ to zoom is the alternative.
- **One finger**: pans in Lightweight Charts, with the crosshair behind a long
  press; or scrubs, with two fingers to pan. unicel's interactive pieces use
  `touch-action: none`, which traps an article's scroll.
- **Arrow keys**: `discovery/surfaces.md` proposed ← → to pan and ↑ ↓ to zoom
  in the terminal. interaction.md and `discovery/a11y-theming.md` (keys) have
  the arrows step datum by datum, the APG slider's keys, because that is the
  only way a keyboard or screen-reader user reads values one at a time.

No measurement decides these. They rest on the owner's test, on not trapping
page scroll, and on WCAG 2.1.1 (keyboard).

## Decision

- A mouse drag pans, Shift+drag brushes, Ctrl or ⌘ with the wheel and a
  trackpad pinch zoom about the pointer, the plain wheel scrolls the page, a
  double-click resets; the options `drag` (`pan`, `brush`, `zoom`) and
  `wheel` (`scroll`, `zoom`) change them (REQ-INPUT-040).
- On touch, a one-finger horizontal drag scrubs the cursor, two fingers pan,
  a pinch zooms about its centre, a double-tap resets, a long press starts a
  brush, and a vertical drag scrolls the page through `touch-action: pan-y`
  (REQ-TOUCH-010), from Pointer Events only (REQ-TOUCH-020).
- On every surface, plain arrows step datum by datum and Shift+arrows pan,
  with the keys of the plan's keys table, acting only while the chart has
  focus (REQ-KEY-010, REQ-A11Y-030). This overrules surfaces.md's terminal
  arrows; the CLI adds `h j k l`, `f` and `q` (REQ-CLI-070).

## Consequences

- A chart inside an article never traps the reader's scroll.
- Readers used to wheel zoom hold Ctrl or ⌘, or the page sets `wheel: "zoom"`.
- Pointer Events only leaves out Safari versions without them
  **(unverified which)**.
- Touch and screen-reader behaviour are the owner's device checks.
