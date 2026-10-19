# ADR-100: Vendor unicel's core

Status: Proposed (2026-10-18)

## Context

glyphplot needs unicel's grid, sub-cell canvases, palette ramp, blue-noise
map, terminal glyph fallback, block shapes and subset font (ADR-002, ADR-004).
`discovery/rendering.md` §1.1 tried three ways to get them:

- **A published unicel crate.** None exists: crates.io answered 404 for
  `unicel` and `unicel-core` on 2026-10-18. unicel would first have to split
  its core from its 207 pieces and serde, name it and release it, and release
  again for every core change glyphplot needs, such as ADR-102's palette.
- **A git dependency pinned by revision.** It builds: 9,045 B gzipped against
  8,901 B vendored for the grid and braille canvas (wasm-pack `--release`,
  `gzip -9`), and a cold build took a median of 3.68 s against 3.48 s (3 runs
  each). crates.io refuses a crate with a git dependency, so it would block
  publishing `glyphplot`. Only pushed commits can be pinned, and cargo's own
  git fetch failed on the owner's machine without
  `CARGO_NET_GIT_FETCH_WITH_CLI=true`.
- **Vendoring with a provenance header.** About 1,240 lines of Rust and 55 of
  TypeScript, taken as they are. The cost is drift between two copies.

`pixels.rs` stretches a one-ink frame between its 2nd and 98th brightness
percentiles and dithers colour by up to ±0.035 a channel, which would misstate
data. `meta.rs` describes a piece's fixed size, fps and category.

## Decision

- The core holds its own copies of unicel's `grid.rs`, `subcell.rs`,
  `palette.rs`, `bluenoise.rs`, `glyphs.rs` and the braille and dither helpers
  of `util.rs`, and the shell its copy of `web/src/blocks.ts`, each beginning
  with a comment naming unicel, its MIT licence and the source revision
  (REQ-VENDOR-010). The font and `scripts/font.py` are copied likewise.
- `pixels.rs` and `meta.rs` are not vendored; `mono.rs`'s checks are adapted
  into the chart contract.
- The workspace has no git dependencies (REQ-ARCH-070).
- `mise run vendor:check` prints which vendored files differ from unicel at
  the revision their header names (REQ-VENDOR-020).

## Consequences

- glyphplot publishes without waiting for unicel, and core changes land here
  without a unicel release.
- A fix in either copy reaches the other only by hand; `vendor:check` shows
  where the copies differ, not which one is right.
- Both projects are MIT with one owner (ADR-011): no licence question arises.
- Revisit once both projects publish: a shared `unicel-core` on crates.io
  would replace the copies.
