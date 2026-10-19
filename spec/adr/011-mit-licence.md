# ADR-011: MIT licence

Status: Accepted (2026-10-18)

## Context

The library is meant to be embedded in other people's pages, tools and
terminals. unicel is MIT (unicel's ADR-022), and ascii.rest, which unicel
follows, is MIT.

## Decision

glyphplot is MIT-licensed, copyright Curtis Ault (`LICENSE`).

## Consequences

- Code vendored from unicel or ported from ascii.rest keeps its attribution
  comment; both are MIT, so no licence conflict arises.
- Third-party dependencies, fonts and datasets must have licences compatible
  with redistribution under MIT (the bundled font is SIL OFL 1.1, as unicel's).
