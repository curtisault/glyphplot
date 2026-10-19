# ADR-011: MIT licence

Status: Accepted (2026-10-18)

## Context

The library is meant to be embedded in other people's pages, tools and
terminals. unicel is MIT (unicel's ADR-022), and ascii.rest, which unicel
follows, is MIT.

## Decision

glyphplot is MIT-licensed, copyright Curtis Ault (`LICENSE`).

## Consequences

- No code is taken from unicel or ascii.rest ([ADR-013](013-independent-of-unicel.md)).
  Algorithms ported from permissively licensed projects (d3's, ISC) keep an
  attribution comment.
- Third-party dependencies, fonts and datasets must have licences compatible
  with redistribution under MIT (the bundled font is SIL OFL 1.1, as unicel's).
