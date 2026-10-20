# ADR-130: proptest

Status: Proposed (2026-10-18)

## Context

Scales, ticks, reductions, cell mappings and layout have laws that property
tests check better than examples: a property in the bench found the column
formulas that disagree for 1,137 of 12,006,000 points (ADR-117).
`discovery/testing.md` D1 weighed:

- **(a) proptest**, with lean features.
- **(b) quickcheck.**
- **(c) Neither**: hand-rolled loops over a seeded generator, as unicel does,
  with no shrinking and no regression files.

Measured in testing.md M3 (fresh target dirs, three runs, machine shared at
load 6–12; the lean proptest rows ran at a lower load and are flattered by an
unknown amount):

| | proptest 1.12.0, lean | proptest, default | quickcheck 1.1.0 |
|---|---|---|---|
| Crates pulled, with the bench crate | 14 | 27 | 16 |
| Clean dev build of the tests | 4.7–4.8 s | 6.1–6.5 s | 4.5–5.1 s |
| Clean release (LTO) build | 17.0–17.4 s | 19.2–22.4 s | 26.7–27.4 s |
| Replaying a failure | `PROPTEST_RNG_SEED`, regression files | same | no seed variable |
| Shrinking the false property | to 32 elements | same | to 4 elements |

- quickcheck reads only `QUICKCHECK_TESTS`, `_MAX_TESTS`, `_GENERATOR_SIZE`
  and `_MIN_TESTS_PASSED` (`src/tester.rs:22-46`).
- Four properties of 256 cases took 3.65 s in a dev build, 0.19–0.25 s in
  release; in `tests/`, proptest found no source root for regression files.

## Decision

- The core uses proptest with `default-features = false, features = ["std"]`
  as a dev-dependency; quickcheck is rejected.
- Properties live in `#[cfg(test)]` modules of the core, with regression files
  under `proptest-regressions/` checked in (REQ-PROP-010).
- They cover scale inversion, tick order and count, reduction bounds, merged
  reductions equalling the whole, LTTB's ends and order, cell-mapping
  agreement and label non-overlap (REQ-PROP-010).
- They run under ADR-127's profile; `signoff` sets `PROPTEST_CASES=4096`
  (REQ-CI-020).

## Consequences

- A failure replays from its seed and stays as a regression case.
- Float-vector shrinking gave up after 1,024 iterations: strategies stay small.
- A clean dev build costs about 4 s more than with no property crate.
