# ADR-127: A test profile without LTO

Status: Proposed (2026-10-18)

## Context

Budgets are timed only in an optimised build, so unicel runs its tests with
`cargo test --release`. `cargo test --release --no-run -v` shows that builds
with the release profile: `opt-level = "s"`, fat LTO, one codegen unit
(`discovery/testing.md` D3). unicel's `[profile.test]` never applies.

The options:

- **(a) `--release`**, as unicel.
- **(b) A `contract` profile**: release without LTO, 16 codegen units,
  `opt-level = 3`.

Measured in testing.md (M2, M3; fresh target dirs; machine shared at load
6–12):

- After touching the library, the release build relinked in 7.4–7.5 s with no
  property crate and 11.4–11.6 s with proptest (three runs each).
- Without LTO, with 16 codegen units and proptest, it rebuilt in 1.2–1.3 s
  (three runs). That run used `opt-level = 2`; the incremental time at
  `opt-level = 3` was not measured **(unverified)**.
- unicel's contract ran in 60.66–71.35 s with the release profile and
  40.19–57.19 s without LTO at `opt-level = 3`, interleaved under the same
  load (M2): no slower.

## Decision

- The workspace defines `[profile.contract]`: `inherits = "release"`,
  `lto = false`, `codegen-units = 16`, `opt-level = 3`,
  `debug-assertions = false`.
- `mise run test:rust` runs `cargo test --workspace --profile contract`
  (REQ-BUILD-070).
- The shipped wasm keeps the release profile: `opt-level = "s"`, LTO, one
  codegen unit, `panic = "abort"`, `strip` (REQ-BUILD-050). `bench:web` times
  that build.

## Consequences

- An edit to the core reaches the tests in about a second and a half instead
  of seven to twelve.
- Native budgets time code that differs from what ships; native and wasm
  times were never comparable, and the browser budgets time the shipped build.
- A fault that appears only under LTO is caught by the Node and browser tests,
  which run the release modules, not by `test:rust`.
