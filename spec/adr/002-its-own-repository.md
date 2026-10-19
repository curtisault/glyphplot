# ADR-002: glyphplot is its own repository

Status: Accepted (2026-10-18)

## Context

glyphplot grows out of unicel's three data pieces (candlesticks, bar-chart,
sparkline) and takes unicel's architecture whole. It could have been a new
category inside unicel. But a chart draws the page's data, answers the pointer
and has budgets of its own, while a unicel piece generates its picture from a
seed and plays.

## Decision

glyphplot lives in its own repository, `~/projects/glyphplot`, separate from
unicel (`~/projects/animate-unicode`). It shares no code with unicel and
depends on nothing in its repository ([ADR-013](013-independent-of-unicel.md),
which settles what this record first left to discovery and rejects
[ADR-100](100-vendor-unicels-core.md)).

## Consequences

- Different purpose, users and budgets do not bend each other's rules: unicel's
  150 KB module gate and four-millisecond frame budget stay unicel's.
- Fixes to shared code (the grid, the canvases, the atlas renderer) must be
  carried across by hand, or through a published crate.
- glyphplot has its own spec, its own requirement IDs and its own milestones.
