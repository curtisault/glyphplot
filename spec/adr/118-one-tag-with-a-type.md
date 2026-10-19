# ADR-118: One tag with a type

Status: Accepted (2026-10-18), by the owner (open question 3 of the plan)

## Context

Open question 3 of the plan, which the owner put: `<glyph-plot type="candles">` or a tag per chart
such as `<glyph-candles>`. `discovery/surfaces.md` weighed:

- **(a) One tag with a `type` attribute**, plus `define` aliases, as unicel's
  `<unicel-art piece>` and `define(tag)`, and Vaadin's `<vaadin-chart type>`.
- **(b) A tag per chart.** Ignite UI uses a tag per family and still picks the
  leaf type with a `chartType` property; no surveyed library makes a tag for
  each leaf type (surfaces.md, package tarballs and docs read).
- **(c) A host tag with plugin elements**, as Perspective registers each chart
  as its own custom element.

What does not decide it: module loading and bundle splitting are the same
either way, since each module is a dynamic `import()` from the manifest.

What does:

- Typing. `tsc` 5.9.3 `--strict` accepted a `glyph-plot` typed as a union
  discriminated on `type` and rejected candle data on `type="line"` and an
  unknown type (surfaces.md, one run of a TSX fixture).
- The upgrade path. Changing `type` on one element keeps the element, its
  data, its listeners and its view; with a tag per chart the page replaces
  the element and re-sends its data, up to a million rows.
- Chart-specific settings live in `options` JSON, so the element's own
  attributes are the same for every chart, as `<input type>` does.

## Decision

- Importing `glyphplot/element` defines `<glyph-plot>` once, injects one
  `<style>` with the font face, and does nothing where there is no DOM or no
  `customElements` (REQ-EL-010).
- `define(tag, { type })` defines `tag` as a subclass whose type is fixed and
  whose `type` attribute is ignored, and does nothing for a tag already
  defined (REQ-EL-030).
- A change of `type` makes a new chart and gives it the old data and view when
  its meta accepts the data's shape, else warns once and draws it empty
  (REQ-EL-050).
- `glyphplot/jsx` types the tag as a union on `type` (REQ-PKG-030).

## Consequences

- One documentation page and one attribute list cover every chart.
- A dashboard can switch chart types in place.
- Plain-HTML editors cannot complete per-type options from one tag; a page
  that wants that uses an alias.
