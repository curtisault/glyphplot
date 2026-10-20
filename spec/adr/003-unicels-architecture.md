# ADR-003: unicel's architecture, whole

Status: Accepted (2026-10-18)

## Context

A charting library compiled to wasm could be built many ways: a JS library
with a wasm kernel, a Rust library that draws through `web-sys`, a canvas
library with a text fallback. unicel has proven one way on 207 pieces: a pure
Rust core crate, a wasm-bindgen boundary built with wasm-pack, a TypeScript
shell with a custom element that renders, a CLI, an Elm site, mise as the only
entry point, GitButler for version control and local signoff instead of hosted
CI. The owner knows it.

## Decision

glyphplot takes unicel's architecture whole:

- a pure Rust core crate with no DOM, canvas or terminal (unicel's REQ-ARCH-001);
- a wasm-bindgen boundary built by wasm-pack with `--target web` (unicel's ADR-002);
- a TypeScript shell that renders and owns a custom element (unicel's ADR-005);
- a CLI linking the core natively;
- an Elm site (unicel's ADR-006);
- mise as the only entry point (unicel's ADR-007);
- GitButler for version control;
- local signoff instead of hosted CI (unicel's ADR-021).

## Consequences

- unicel's design is prior art for every layer (the Player, the manifest,
  the loader, the renderer, the contract test, the signoff script); its code
  is never copied, ported or depended on ([ADR-013](013-independent-of-unicel.md)).
  glyphplot writes each layer itself.
- The decisions unicel made inside that architecture (abort on panic, a
  hand-written registry, the bundled subset font) are not inherited
  automatically; each is re-decided for glyphplot as a Proposed ADR from 100.
