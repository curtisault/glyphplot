# glyphplot

Interactive charts drawn in Unicode glyphs: braille, sextants, octants,
blocks, box drawing, arrows and geometric shapes, one code point and one
colour a cell. The charts are Rust, compiled to WebAssembly one module per
chart family, played in the browser by a custom element over a small
TypeScript shell and in the terminal by a CLI. Every chart draws data the
page or the terminal gives it, once, replaced or streaming.

```html
<script type="module" src="/glyphplot/element.js"></script>
<glyph-plot type="candles" src="/feed.arrow"></glyph-plot>
```

```sh
glyphplot candles --from feed.csv
tail -f metrics.csv | glyphplot line
```

**Status: v0.6.0.** `line`, `sparkline`, `candles` and `bar` work end to end,
in Rust, in wasm and in `<glyph-plot>`: streamed into a ring at 1 kHz, read
from CSV, JSON, NDJSON or Arrow, with pointer, keyboard and touch input,
linked charts, time zones, a hidden table and a slider cursor. Each is held
to the full chart contract and to time budgets. The CLI does not draw yet,
and nothing is published. The next phase builds the site.

glyphplot is the sibling of [unicel](../animate-unicode) (animated Unicode
art) and follows its architecture, but it is a separate, independent
project: it shares no code with unicel and depends on nothing in its
repository.

## Where things are

| Path | What |
|---|---|
| [`docs/IMPLEMENTATION_PLAN.md`](docs/IMPLEMENTATION_PLAN.md) | The plan, phase by phase, with every decision and measurement |
| [`spec/`](spec/) | The specification: `SPEC.md`, `ROADMAP.md` (every requirement), ADRs, JSON Schemas, the conformance map |
| [`discovery/`](discovery/) | What discovery found, one report a topic; `bench/` holds its throwaway measurement code |
| [`AGENTS.md`](AGENTS.md) | Conventions for anyone, agent or person, working here |

## Getting started

```sh
mise install          # Rust 1.97 (+ wasm32), Node 26, Elm 0.19.1, wasm-pack, uv, …
mise run check        # a placeholder until Phase 0 builds the real one
```

mise is the only entry point; `mise tasks` lists what exists.

## Licence

MIT, © 2026 Curtis Ault. See [LICENSE](LICENSE).
