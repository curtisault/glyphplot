# ADR-122: Theme tokens and custom properties

Status: Proposed (2026-10-18)

## Context

A page must be able to set a chart's colours, and dark and light themes must
follow the page. `discovery/a11y-theming.md` (decision 2) weighed:

- **(a) CSS custom properties**, inherited from the page, with the same token
  names as an object for surfaces without CSS. unicel's element already reads
  custom properties (`--unicel-font`, unicel's ADR-118).
- **(b) A theme object or attribute only.** Simpler to test, but it ignores
  the page's own `[data-theme]` and `prefers-color-scheme` rules.
- **(c) Named presets only** (`theme="dark"`).

Facts that bear on it, from a11y-theming.md:

- unicel's `mount` picks paper from the ground's luma, or with no ground from
  the element's computed text colour, `0.2126 r + 0.7152 g + 0.0722 b < 128`
  (`web/src/mount.ts:95-101`, code read).
- Under `forced-colors: active` the browser forces CSS colours but not a
  canvas's pixels (MDN's list of forced properties), so a chart must switch
  itself to one ink. Forced colours were not tried on Windows
  **(unverified)**.
- No measurement decides between the options.

## Decision

- The tokens are `ground`, `ink`, `muted`, `rule`, `accent`, `series-1` to
  `series-8`, `up`, `down` and `scale`. The element reads `--glyphplot-<token>`
  from its computed style, each falling back to the default set, and a
  `theme` property set to an object with the same names wins (REQ-THEME-010).
- Without a ground a chart draws none, and uses the light set when its
  computed `color` has luma under 128, the dark set otherwise (REQ-THEME-020).
- The element re-resolves and repaints when `prefers-color-scheme` changes,
  when `<html>`'s `class`, `style` or `data-theme` changes, and when `theme`
  is set, without reloading its module or drawing a frame (REQ-THEME-030).
- The JS API, the CLI (`--theme <file.json>`), the Rust crate and the SVG and
  ANSI exports take the same object (REQ-THEME-090).
- Under forced colours the element draws one ink in `CanvasText` on `Canvas`,
  with the accent in `Highlight`; under print it SHOULD draw on paper
  (REQ-THEME-100).

## Consequences

- A page that switches its theme in CSS re-themes every chart with no script.
- Every surface resolves through `theme::resolve` (ADR-102).
- The element watches `<html>` with a MutationObserver for its life.
