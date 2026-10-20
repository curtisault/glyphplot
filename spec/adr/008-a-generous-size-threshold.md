# ADR-008: A size threshold per module, set generously

Status: Accepted (2026-10-18)

## Context

unicel gates each module at 150 KB gzipped (225 KB for scenes; unicel's
ADR-009), because a page of art should load almost nothing. A chart page is a
tool: its reader came for the chart, and a richer module (scales, formatting,
interaction) is worth more bytes. With no gate at all, modules drift toward
megabytes as dependencies accrete.

## Decision

There is a size threshold per module, and it is generous. Discovery measures
what a real chart module costs and proposes the number
(`discovery/budgets.md`); the owner sets it. Tens of megabytes is wrong; a few
hundred kilobytes may be right.

## Consequences

- `mise run size` fails a module past its threshold, as unicel's does.
- A dependency that would push a module over (a full ICU, an Arrow
  implementation, tzdata) needs a decision of its own.
