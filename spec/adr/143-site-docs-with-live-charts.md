# ADR-143: The site is documentation with live charts

Status: Proposed (2026-10-18)

## Context

The brief's open question 6: the Elm site as a gallery of examples, as
unicel's is, documentation with live charts, or both. unicel's site is a
gallery of 207 art pieces, each played alone. A chart library's reader comes
to put a chart on a page with their own data, and needs the data contract, the
events, theming and the CLI explained. The options:

- **(a) A gallery**, as unicel's: every chart shown, little explanation.
- **(b) Documentation with live charts**: a page per chart type, doc pages,
  and examples.
- **(c) Both, separately**: a gallery site and a docs site.

What bears on it:

- `discovery/domains.md` (Recommendation 5) lists ten examples in order, from
  the owner's simulated-market candlestick to a TESS transit, each on a
  licensed dataset or a live public feed.
- `discovery/surfaces.md` defines the element's attributes, properties, events
  and the CLI's shape, which a reader needs documented beside working charts,
  and found the package names unpublished on npm and crates.io on 2026-10-18.
- No measurement decides between the options.

## Decision

- The site is Elm 0.19.1 with Vite, its flags from `web/pkg/modules.js`,
  loading no wasm before the page shows (REQ-SITE-010).
- Each chart type has a page with the chart live on a fixture and an example,
  a control per option, its attributes, a log of its events and usage
  snippets that name only installs that work (REQ-SITE-020).
- Documentation pages (getting started, the data contract, view and events,
  theming, accessibility, the CLI) hold live charts (REQ-SITE-030).
- The examples page shows the plan's site examples, the two live candlesticks
  first (REQ-SITE-040, ADR-136); it serves as the gallery.
- Install snippets say the packages are unpublished until they are
  (REQ-PKG-060).

## Consequences

- Every documented behaviour is a live chart the e2e tests load
  (REQ-E2E-060).
- The site has more pages to keep current than unicel's.
- The site depends on the dataset manifest (ADR-137).
- Revisit if the owner wants the gallery as the front page.
