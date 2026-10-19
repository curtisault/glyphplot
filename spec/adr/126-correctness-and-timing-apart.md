# ADR-126: Correctness and timing apart

Status: Proposed (2026-10-18)

## Context

unicel's contract is one test that checks its pieces one after another and
times their frames inside the same run. glyphplot's contract is a matrix of
fixtures, sizes, environments, streams, views and scripted input.

Measured in `discovery/testing.md`:

- unicel's contract took 32.5–32.7 s on an idle machine, with 39.0–39.3 s of
  user time: one thread at a time (M1, three runs, `/usr/bin/time -p`).
- Under a load of 7–9 from other agents it took 60.66–71.35 s (M2, three
  runs), twice its idle time.
- glyphplot's matrix is about 2,500 frames a chart, about 1 s a chart on one
  thread for correctness and about 2 s for the timed plays (M6, an estimate
  from measured parts, **(unverified)**).

`discovery/budgets.md` found native timings moved up to 1.6× with load while
Chromium's two runs agreed within 3 %, and unicel found GitHub's runners 3–5×
slower (its plan, risk 9). The options:

- **testing.md D2**: (a) one sequential test with timings inside, as unicel;
  (b) correctness in `tests/contract.rs` across threads, timings in
  `tests/budgets.rs` alone.
- **budgets.md decision 5**: gate native budgets only, browser budgets only,
  or native budgets in `mise run test` and browser budgets in `signoff`.

## Decision

- `tests/contract.rs` checks every registered chart against the contract's
  correctness rules, spreads charts over threads, reports every failing chart
  in one run and takes `CHART=<type>[,…]` (REQ-CONTRACT-010, REQ-CONTRACT-020).
- Time budgets are checked in `tests/budgets.rs`, a test binary of its own,
  only without debug assertions; a chart over budget is timed twice more and
  the lowest average and worst count (REQ-CONTRACT-050).
- Native budgets gate `mise run test` (REQ-PERF-030, REQ-PERF-040,
  REQ-PERF-050). Browser budgets are gated only by `bench:web` and
  `bench:load` in `mise run signoff` (REQ-PERF-060, REQ-PERF-080, REQ-CI-020).

## Consequences

- Correctness scales with cores; cargo runs test binaries one after another,
  so the timings share the machine with no other test.
- Other work on the machine can still slow a timed run; the retime absorbs
  most of it. Skipping budgets under high load is added only if they flake.
- A browser regression shows at signoff, not at every commit.
