# ADR-007: Interactive from the first chart

Status: Accepted (2026-10-18)

## Context

Interaction could be added after static charts work, as many libraries did.
Added late, it tends to bend the contract: view state, hit testing and input
budgets arrive after the frame loop and the boundary are set.

## Decision

Charts are interactive from the first chart: pan, zoom, crosshair, hover
readouts, brushing, keyboard and touch, with events out to the page. The chart
contract, the boundary and the budgets include input from the first milestone.

## Consequences

- The first chart's milestone is larger than a static chart's would be.
- View state is part of what a frame is a function of (ADR-009).
- Every interaction needs a keyboard path and a test in a headless browser.
