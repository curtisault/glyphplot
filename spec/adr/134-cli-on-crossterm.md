# ADR-134: The CLI on crossterm

Status: Proposed (2026-10-18)

## Context

The CLI plays charts in the terminal with mouse and keys, reads data from
files and pipes, and prints frames when piped. unicel's CLI is built on
crossterm. `discovery/surfaces.md` (decision 3) weighed:

- **(a) crossterm**, drawing glyphplot's own grid with its own terminal code (unicel's CLI, also on crossterm, is prior art).
- **(b) ratatui's `Chart` widget.** It takes data as `&[(f64, f64)]` pairs,
  generates no ticks, draws every segment and has no hit testing
  (`ratatui-widgets/src/chart.rs`, code read).
- **(c) ratatui as the terminal layer** over glyphplot's grid, with a second
  buffer and diff.

Measured in surfaces.md (hello-chart CLIs, release and stripped, one build
each; crates by `cargo tree -e normal`):

| Binary | Bytes | Crates |
|---|---|---|
| std only | 335,728 | — |
| (a) crossterm | 404,800 | 32 |
| (c) glyphplot's grid through a ratatui `Widget` | 438,112 | — |
| (b) ratatui's `Chart` | 553,968 | 75 |

`discovery/charts.md` timed ratatui's `Chart` at 325.2 ms min, 347.1 ms median
for 1M points of noise at 320 × 120, against 3.54 / 4.18 ms for a reduction
per dot column (native release, 3 × 5 runs). unicel's CLI shows the behaviour to
match (restoring the terminal on every exit, writing changed runs only,
falling back across glyph sets, pty tests); glyphplot writes it anew
([ADR-013](013-independent-of-unicel.md)).

## Decision

- The CLI is crate `glyphplot-cli`, binary `glyphplot`, built on crossterm
  with its own terminal code, with `glyphplot <type>`, `render`, `demo` and `list`
  (REQ-CLI-010).
- The crates are `glyphplot`, `glyphplot-cli`, `glyphplot-wasm`
  (`publish = false`) and the packs `glyphplot-<domain>` (REQ-PKG-040).
- A `glyphplot-ratatui` crate SHOULD let native TUIs embed a chart as a
  ratatui-core `Widget` (REQ-CRATE-020).

## Consequences

- One chart model and one buffer serve the browser and the terminal.
- ratatui users are served by the adapter, not by the CLI.
- crossterm has no SGR-pixel mouse mode, so the terminal pointer is a whole
  cell, two braille dot columns.
- Windows terminals are not covered (`/dev/tty` for keys when stdin is a pipe).
