# ADR-001: The name is glyphplot

Status: Accepted (2026-10-18)

## Context

The library needs one name for the crate, the npm package and the element. unicel
kept a placeholder name until late (its SPEC.md §1.3), which left its install
snippets, its CLI and its package waiting on the decision. Alternatives were a
name that says "unicode" (taken or vague), a name that ties the library to
unicel (which it is not part of), or a name that says what it does.

## Decision

The name is `glyphplot`: the crate (`glyphplot`), the npm package
(`glyphplot`) and the tag prefix (`<glyph-plot>`). On 2026-10-18 the name was
free on npm and crates.io; `glyphplot.dev` and `glyphplot.io` did not resolve.

## Consequences

- Install snippets, the CLI binary and the package exports can be written with
  the real name from the first phase.
- Satellite crates and packages take the name as a prefix (`glyphplot-wasm`,
  `glyphplot-cli`); [ADR-134](134-cli-on-crossterm.md) and REQ-PKG-040 settle the exact list.
- The name must be claimed on npm and crates.io before anyone else takes it;
  that is a publishing step, open until the packaging phase.
