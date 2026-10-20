# Surfaces

## Summary

glyphplot reaches its users through six surfaces: the `<glyph-plot>` element,
a JavaScript `Chart` object under it, a first frame rendered on a server, a
CLI that also plays charts interactively in the terminal, the Rust crate (for
native TUIs and for servers), and exports (text, ANSI, SVG, PNG).

The recommendation is one tag, `<glyph-plot type="candles">`, with an opt-in
`define("glyph-candles", { type: "candles" })` for pages that want a tag per
chart. The tag keeps unicel's shape: its attributes are for HTML authoring
and small values, and its properties and methods carry data, options and view
state as objects. Data is never only an attribute. The element loads one wasm
module per chart type through a manifest, as unicel's `modules.js` does. It
holds data set before the module arrives and applies it once the chart is
made. The element is sized by its container: cols and rows come from its box,
which is unicel's `fit="fill"` (unicel ROADMAP REQ-BACK-001) made the default.

The CLI stays on crossterm, as unicel's does, not on ratatui. Measured on
2026-10-18, a hello-chart CLI is 404,800 bytes on crossterm and 553,968 bytes
on ratatui's own `Chart` widget, release and stripped. ratatui's `Chart` has
nothing glyphplot needs: no tick generation, no downsampling, no hit testing,
and data as `&[(f64, f64)]` pairs. glyphplot SHOULD still ship a small
`glyphplot-ratatui` adapter that draws a chart's grid into a ratatui
`Buffer`, so that native TUIs can embed charts. A grid-through-ratatui hello
CLI is 438,112 bytes.

A first frame as text, rendered in Node from the same wasm module, takes 0.3 ms
to instantiate and 1 ms to draw for 1,000 points. A whole Node process takes
33 ms, most of it Node's own start. The Rust crate does the same natively.

## Scope and method

Read in unicel (`../animate-unicode`): `spec/SPEC.md` §6–§9 and §10.8,
`spec/ROADMAP.md` §4.5, §4.7, §5.6, §6.2, §7.6 and the v0.13.0 backdrops
requirements, ADRs 005, 008, 025, 100, 112 and 115, `web/src/{element,mount,wasm,index}.ts`,
`web/package.json`, `crates/cli/src/{main,paint,play}.rs`, `crates/cli/Cargo.toml`,
`crates/wasm/Cargo.toml`, `crates/unicel/examples/manifest.rs` and
`spec/schemas/manifest.schema.json`.

Read the code of ratatui (shallow clone, commit `0297560`, 2026-10-17;
ratatui 0.30.2, ratatui-core 0.1.2, ratatui-widgets 0.3.2), crossterm (commit
`ab8681d`, 2026-10-16; 0.29.0), Perspective (commit `0857a7d`, 2026-10-17)
and the published `igniteui-webcomponents-charts` 7.1.0 tarball. Read the
docs of Vaadin Charts, TradingView Lightweight Charts v5 and React's
custom-element support.

Built a throwaway bench, `discovery/bench/surfaces/` (README there): a hello
chart in braille with axes, shown by four CLIs and by a wasm module run from
Node. Checked with `tsc` that a single tag can be typed per chart in TSX.

Not covered: Windows terminals, which need crossterm's WinAPI path and
`CONIN$` instead of `/dev/tty`. Also not covered: an `npx glyphplot` bin's terminal
input parsing, Web Workers and OffscreenCanvas, Vue and Svelte bindings, and
any browser timing. The budgets agent owns frame times, and this report
measures only what decides a surface.

## The element: one tag or a tag per chart

### What established libraries do

| Library | Shape | Source |
|---|---|---|
| Vaadin Charts | One tag, `<vaadin-chart type="column">`; series are `<vaadin-chart-series>` children whose values are a property (`.values=${[…]}`); a series may override `type`. | https://vaadin.com/docs/latest/components/charts |
| Perspective (FINOS; Rust and wasm) | One host tag, `<perspective-viewer>`; each chart is a plugin custom element (`perspective-viewer-charts-<tag>`) that registers itself with the host (`Viewer.registerPlugin(tagName)`); data goes in by a method (`load`), state by `restore`/`save`. | `packages/viewer-charts/src/ts/index.ts:30-50`, `rust/perspective-viewer/src/rust/lib.rs:131-136`, `rust/perspective-viewer/src/rust/custom_elements/viewer.rs:488,1078,1367` |
| Ignite UI Web Components | A tag per chart family: `igc-category-chart`, `igc-financial-chart`, `igc-data-chart`, `igc-pie-chart`, `igc-doughnut-chart`, `igc-funnel-chart`, `igc-data-pie-chart`, each with its own module class to register (`IgcCategoryChartModule` …); a `chartType` property picks the variant inside a family. | `igniteui-webcomponents-charts@7.1.0` tarball, `htmlTagName` strings; `lib/igc-category-chart-component.d.ts` |
| Lightweight Charts v5 (JS API, no element) | One chart object; series by `chart.addSeries(CandlestickSeries, options)`, each series definition imported separately "for better tree-shaking". | https://tradingview.github.io/lightweight-charts/docs/migrations/from-v4-to-v5 |
| unicel | One tag, `<unicel-art piece="donut">`; `define(tag)` defines another tag name with a subclass. | `web/src/element.ts:336-343`, SPEC REQ-EL-010 |

No surveyed library makes a tag for each leaf chart type. The families that
use several tags (Ignite UI) still choose the leaf type with a property.

### The criteria

- **Module loading.** This does not decide it. Either way the shell defines
  small classes up front and loads wasm on first use. With one tag the manifest
  maps `type` to a module. With a tag per chart it maps each tag to a module.
  Both cost one manifest import.
- **Bundle tree-shaking.** This does not decide it either. The wasm is loaded by
  dynamic `import()` per module (unicel `web/src/wasm.ts:24-36`,
  `manifest.rs:57-60`), so a bundler splits it per chart in both designs. The
  JS of the element is one class either way.
- **Typing in TSX.** One tag can be typed per chart as a discriminated union
  of props on `type`. `tsc` 5.9.3 `--strict` accepts
  `<glyph-plot type="candles" data={{t,o,h,l,c}} />` and rejects candle data
  on `type="line"` and an unknown `type`. Measured: `scratchpad/surfaces/tsx/t.tsx`,
  2026-10-18, with `@ts-expect-error` on the two bad lines and no other
  errors. A tag per chart types more simply, and a `define` alias types the
  same way (`"glyph-candles": Omit<Candles, "type">` in the same file).
- **Discoverability.** One tag gives one documentation page, one `observedAttributes`
  list and one place to look. A tag per chart lets editors complete attribute
  names per tag through HTML custom data. A union does the same in TSX but
  not in plain HTML.
- **Attribute sets that differ by chart.** The chart-specific settings live in
  `options` (a JSON object validated by the chart's options schema), as in
  unicel (SPEC §4.2). The element's own attributes (`type`, `data`, `options`,
  `view`, `theme`, `mono`, `motion`, `label`, `fit`, `loading`) are the same
  for every chart. So the differing sets do not leak into the tag. This is the
  `<input type>` trade the HTML platform made.
- **The upgrade path.** This is the deciding criterion. Changing `type` on one
  element turns a line into an area, or a candlestick into OHLC bars, and
  keeps the element, its data, its listeners and its view. With a tag per
  chart the page must replace the element, re-feed the data (a million rows,
  perhaps) and re-attach its listeners. A dashboard that offers a chart-type
  switch, as Perspective's does, needs the single host.
- **Selectors and styling.** `glyph-plot[type=candles]` and `glyph-candles`
  both select. This is neutral.

### Recommendation

One tag, `<glyph-plot>`, whose `type` attribute names the chart. The shell
exports `define(tag, { type })`, unicel's `define(tag)` (REQ-EL-010) with a
fixed type. With it, a page that prefers `<glyph-candles>` gets a subclass
whose `type` is fixed and whose `type` attribute is ignored. This answers the
brief's open question 3.

## The element's API

The rules carried from unicel are these. Attributes are strings for HTML
authoring and small values. Anything large or structured is a property or a
method. A change to `time` or `fit` does not reload the chart (REQ-EL-080,
REQ-FIT-005). The element loads nothing until it is within one screen of the
viewport (REQ-EL-030). Whatever the element holds before its chart is ready,
such as a server-rendered still, stays until the chart replaces it
(REQ-EL-060). Light DOM, with the drawn element's look set inline
(REQ-MONO-001), so the server-rendered fallback is plain children.

One rule differs from unicel's. A change of options does not re-create the
chart. unicel re-creates the player (ADR-025) because a piece holds nothing
the page gave it. A glyphplot chart holds the page's data, up to a million
rows, so re-creating it would mean re-sending all of it. Options go through
`set_options`, and only `type` makes a new chart. The data is carried across
when the new type accepts its shape.

### Attributes

| Attribute | Value | Effect | On change |
|---|---|---|---|
| `type` | a chart's slug: `line`, `candles`, `bar` … | The chart to draw. | new chart; data and view carried when the new type accepts them |
| `data` | JSON text, small data only | The data, in the chart's data shape (data.md). Read once per change; a `data` property set later wins. | replaces data |
| `src` | URL of CSV, JSON or Arrow IPC | Fetched and given as `data` (v0.2.0 or later). | replaces data |
| `options` | JSON object text | Overrides the chart's option defaults. | `set_options`, keeps data and view |
| `view` | JSON object text | The view state (domain, zoom, cursor, selection), so that a URL can hold a view (interaction.md). | sets view |
| `theme` | a theme name | The palette and ground (a11y-theming.md). | `set_options` |
| `mono` | present or not | One ink, drawn as text in a `<pre>` (unicel REQ-EL-050). | redraws |
| `motion` | present or not | Transitions play even under reduced motion (unicel REQ-MOUNT-120). | none |
| `label` | text | The accessible name; the chart's name otherwise. | relabels |
| `fit` | `width` or `contain` | Only with fixed `cols` and `rows`: scales that grid as unicel's `fit` does (ADR-100). Without `cols` and `rows`, the grid fills the box. | refits |
| `cols`, `rows` | integers | Fix the grid's size instead of taking it from the box. | resizes |
| `loading` | `lazy` (default) or `eager` | `eager` skips the near-screen gate, as `<img loading>` does (https://html.spec.whatwg.org/multipage/urls-and-fetching.html#lazy-loading-attributes). | none |
| `time` | seconds | Holds the frame at `t`, the animation time, for tests and snapshots (unicel's `time`). The data's time cursor is part of `view`, not this. | seeks |
| `debug` | present or not | The frame-cost overlay, loaded only then (unicel REQ-DEBUG-001–002). | reloads overlay |

### Properties

| Property | Type | What |
|---|---|---|
| `type` | string | Reflects the attribute. |
| `data` | the chart's data shape (columns of typed arrays, Arrow, rows) | Set: replaces all data. Get: the last value set. Not reflected to the attribute. |
| `options` | object | Set: `set_options`; get: the options in force, defaults filled in. |
| `view` | object | Set: moves the view and fires no event. Get: the current view, after any pan or zoom. |
| `chart` | `Chart` or `null` | The JS chart object (below), once made. |
| `ready` | `Promise<void>` | Resolves after the first frame of the current chart is drawn. |
| `cols`, `rows` | number, read-only when not set | The grid's size now. |
| `currentTime` | number, read-only | The animation time of the frame shown (unicel §7). |

The property names are deliberately present on the class. React 19 then
passes values as properties, not as stringified attributes. React's docs say
that React "will … recognize an custom element's property as one that it may
pass arbitrary values to if the property name shows up on the class during
construction" (https://react.dev/reference/react-dom/components#custom-html-elements).

### Methods

| Method | What |
|---|---|
| `append(batch)` | Appends rows (one sample or a batch of columns) to the data; the chart's retention policy trims (data.md). Before the chart loads, batches are queued in order. |
| `resetView()` | Returns the view to the chart's fitted default. |
| `text()` | The frame shown, as one-ink text. |
| `toSVG()`, `toANSI()` | `Promise<string>`: the frame shown, from Rust (`glyphplot::export`), the same bytes as the CLI's `render`. |
| `toPNG(scale?)` | `Promise<Blob>`: the frame drawn by the shell's `paint` at `scale` × layout size. |

### Events

Events are `CustomEvent`s that bubble and are composed. They fire for the
reader's actions and for loading, never for a property the page set, so that
two linked charts cannot echo each other. Native `input` events follow the
same rule: a script that sets `value` does not fire them. Names carry a
`plot-` prefix, so they never collide with the native `select` or `change`.
React 19 binds them as written, dashes included: `onplot-hover={…}` (react.dev,
same page).

| Event | `detail` | When |
|---|---|---|
| `plot-ready` | `{ type, cols, rows }` | First frame of a chart drawn. |
| `plot-error` | `{ type, message }` | Load or first frame failed. |
| `plot-hover` | `{ series, index, datum, x, y }` or `null` | The datum under the pointer or keyboard cursor changed. |
| `plot-view` | the view state | The reader panned, zoomed or scrubbed. |
| `plot-select` | `{ series, index, datum }` | The reader clicked or pressed Enter on a datum. |
| `plot-brush` | `{ x: [a, b], y?: [a, b] }` or `null` | The reader brushed or cleared a range. |
| `plot-legend` | `{ series, visible }` | The reader toggled a series. |

This is a naming rule and a starting list. The vocabulary and the `detail`
shapes are interaction.md's to settle.

### Not form-associated

A chart is not a form control. A form-associated custom element
(`static formAssociated = true` and `attachInternals()`) would add a value,
validity and form reset that no chart needs. A page that wants a brushed
range in a form copies `plot-brush`'s `detail` into an input.

### Sizing

The element is sized by its container. The shell measures the content box W
× H on a ResizeObserver. It derives the cell from the font, 0.6 em wide as
unicel's bundled font is (REQ-FONT-001), and two widths tall. Then
`cols = floor(W / cw)` and `rows = floor(H / ch)`, the formula unicel's
roadmap gives `fit="fill"` (ROADMAP REQ-BACK-001, line 420). The chart is
then re-laid-out at that size: axes, ticks and labels are recomputed, not
scaled.

A box with no height of its own gets its height from the `rows` attribute or
a default number of rows. rendering.md owns the default and the minimum useful
size. unicel debounces its backdrops' resize by 150 ms (ROADMAP REQ-BACK-002).
A chart may re-lay-out at most once an animation frame instead, once
budgets.md shows a re-layout fits in a frame.

## Module loading

This is unicel's mechanism, renamed. unicel's `manifest.rs` writes
`web/pkg/modules.js` with `pieces` (every meta), `modules` (category →
module), `imports` (module → `() => import("./<module>/unicel.js")`),
`fallback` and `budgets` (`manifest.rs:41-73`, schema
`spec/schemas/manifest.schema.json`). `wasm(name)` imports and instantiates a
module once per page and retries after a failure (`web/src/wasm.ts:24-36`).

For glyphplot:

- `charts`: every chart's meta (slug, name, data shapes accepted,
  interactions), so a page can list charts and validate `type` with no wasm.
- `modules`: chart slug → module name. It is many to one where charts.md puts
  two charts in one module with a mode (line and area, say), and one to one
  otherwise. In the debug build every chart maps to `all`.
- `imports`, `fallback`, `budgets`: as unicel's, `budgets` per module.
- The element waits for the near-screen gate (`rootMargin: "100% 0px"`, unicel
  `element.ts:321-334`) unless `loading="eager"`. It then calls
  `chart(type, options)`, which loads the module once per page.
- The `data`, `options`, `view` and `append` calls made before the chart exists
  are kept in order and applied once it is made. A `data` assignment drops
  the queued appends before it.

The manifest can also give each module's URL. A page can then preload a
chart it shows above the fold with `<link rel="modulepreload">`. This is a
convenience, not a requirement.

## The JS API without the element

unicel's index exports `player(slug, options)`, `mount(el, exports, player, options)`
and `paint(ctx, …)` (`web/src/index.ts:9-13`). The element is built from them.
glyphplot keeps that layering, so a page, a worker or Node can use a chart
with no element:

```ts
import { chart, mount, paint, charts } from "glyphplot";
const c = await chart("candles", { options, data });   // loads the module once
c.append({ t, o, h, l, c: close });
c.view = { x: [from, to] };
const playing = mount(canvas, c, { input: true });       // draws, and wires pointer and keys
c.text(); await c.svg(); c.free();
```

- `Chart` wraps the wasm boundary object (the Player's successor). It has
  `data`, `append`, `options`, `view`, `input(event)`, `frame(t)`, `resize(cols, rows)`,
  `text()`, `svg()`, `ansi()`, `free()`, and the grid views over wasm memory.
- `mount(el, chart, options)` draws into a `<pre>` or a `<canvas>`, as
  unicel's does. With `input`, it also turns pointer, wheel, touch and keyboard
  events into `chart.input(…)` calls and dispatches the events above on `el`.
  The element is `mount` plus sizing, loading and attributes.
- `chart()` and its frames need no DOM. That is what makes server rendering
  and Node tests possible.

### React and other frameworks

As in unicel, there is no wrapper. React 19 sets properties that exist on the
class and binds custom events by their exact names (react.dev, cited above).
`<glyph-plot type="line" data={columns} onplot-hover={f} />` therefore works
with no wrapper. The package SHOULD ship JSX typings as `glyphplot/jsx`, as
unicel plans (ROADMAP REQ-PKG-500), using the discriminated union above. A
React wrapper is deferred until a framework is found where properties or
events do not reach the element.

## Server-side rendering of a first frame

There are three routes, and all of them draw the same frame because the chart
is a pure function of its data, options, view and `t`:

1. **Node runs the chart's wasm module.** `--target web` output needs its
   bytes handed to `initSync({ module })`, because Node's fetch takes no
   `file:` URL (unicel SPEC REQ-TEST-503's note). Measured: instantiate 0.30 ms,
   first frame of 1,000 points 0.96 ms, and a whole `node` process 32.9 ms
   (median of 5).
2. **The Rust crate natively**, for a Rust server: the same frame, no wasm.
3. **The CLI**: `glyphplot render line --from cpu.csv --format txt --cols 80 --rows 20`.

The server does not know the reader's box, so it renders at a fixed size. The
element then replaces that still at the measured size (REQ-EL-060). A text
still inside a `<pre>` shifts layout when the real chart arrives. An SVG still
with an `aspect-ratio` does not, since it scales to the box. The package
SHOULD export a Node entry, `glyphplot/node`, that loads a module's bytes from
the package and offers `renderText` and `renderSVG`.

## The CLI and TUI

### Building on ratatui or on unicel's crossterm CLI

| | ratatui 0.30 `Chart` | crossterm 0.29, unicel's style |
|---|---|---|
| Data | `Dataset.data: &'a [(f64, f64)]`, pairs not columns (`ratatui-widgets/src/chart.rs:325-340`) | glyphplot's columns |
| Ticks | none: labels are given by the caller and spaced evenly, `width_between_ticks = graph_area.width / labels_len` (`chart.rs:886-930`); bounds are fixed by the caller (`Axis.bounds`, `chart.rs:40-52`) | glyphplot's scales |
| Large data | draws every segment of every dataset through `Canvas` (`chart.rs:1046-1100`); no downsampling | glyphplot's downsampling |
| Interaction | none: no hit test, no view state | glyphplot's |
| Glyphs | `Marker::{Dot, Block, Bar, Braille, HalfBlock, Quadrant, Sextant, Octant}` (`ratatui-core/src/symbols/marker.rs:8-48`) | the core's sub-cell canvases |
| Cell | `symbol: Option<CompactString>`, `fg`, `bg`, `modifier` (`ratatui-core/src/buffer/cell.rs:38-60`), diffed per frame | unicel's grid, changed runs written (unicel REQ-CLI-070) |
| Hello CLI, release, stripped | 553,968 bytes (Chart widget); 438,112 bytes (glyphplot grid through a ratatui `Widget`) | 404,800 bytes |
| Crates in the tree | 75 | 32 |

glyphplot's core already does everything ratatui's `Chart` does, and more.
Built on ratatui, the CLI would carry a second chart model and a second
double buffer. That costs 149 KB over crossterm for its widget, or 33 KB
for its terminal layer alone. Neither gives glyphplot anything its core lacks.
unicel's CLI already has the restore-on-every-exit handling, the
changed-run writer, half-block pairing, glyph fallback (`--glyphs`) and pty
tests (`crates/cli/src/play.rs`, `paint.rs`, SPEC REQ-CLI-030–100,
REQ-TEST-505). Its writer moves into Rust's `export::ansi` by unicel's own
roadmap (ADR-115, REQ-TERM-002).

So the CLI is built on crossterm, after unicel's CLI. A help overlay and a
legend are drawn by the core into the grid, like any label.

### Mouse and keyboard in the terminal

- crossterm's `EnableMouseCapture` turns on modes 1000, 1002, 1003 (any
  motion), 1015 and 1006 (SGR) (`crossterm/src/event/commands.rs:44-60`).
  `MouseEvent` carries `column` and `row` as `u16`, a whole cell
  (`src/event/mouse.rs:24-33`). crossterm has no SGR-pixel mode (1016): a
  grep of its source for `1016` finds nothing. So the terminal pointer is one
  cell, two braille dot columns, and hover snaps to the nearest datum to the
  cell's centre. The browser keeps sub-cell precision.
- `MouseEventKind` has `Down`, `Up`, `Drag`, `Moved` and the four scroll
  directions (`mouse.rs:73-91`). These give hover (Moved), pan (Drag), zoom
  (ScrollUp and ScrollDown, with Ctrl as a modifier where the terminal reports
  it), and brush (Drag with Shift). crossterm notes that some terminals report
  no button for Up and Drag (`mouse.rs:8-16`).
- Mouse capture takes the terminal's own text selection. `--no-mouse` leaves it
  to the terminal.
- Keys, at least: ← → pan, ↑ ↓ or + − zoom, Home resets the view, Tab cycles
  series, `f` follows the live end of a stream, space pauses following, `?`
  shows the keys, q or Esc quits, and Ctrl+C quits with exit 130 as unicel's
  does.

### Data on stdin and streaming from a pipe

- With `--from -`, or with no `--from` and stdin not a terminal, the data
  comes from stdin. crossterm then reads keys and mouse from `/dev/tty`: its
  `tty_fd()` opens `/dev/tty` when stdin is not a tty
  (`crossterm/src/terminal/sys/file_descriptor.rs:48-56`). So
  `tail -f feed.csv | glyphplot line` can be scrubbed with the mouse. This is
  Unix only. Windows is not covered.
- A reader thread parses lines as they arrive. The play loop appends what
  arrived once a frame, never a line at a time, so a fast pipe costs one
  append a frame. `--window N` keeps the last N rows (data.md's ring buffer).
  The chart follows the live end until the reader pans, and `f` returns to it.
- With stdout not a terminal, the CLI reads to the end of input and prints one
  frame as text, as unicel's piped frame does (REQ-CLI-020). A closed stdout
  (`| head`) ends it quietly.

### Colour modes

unicel always writes 24-bit SGR (REQ-CLI-040). glyphplot's charts colour by
value (heatmaps, gradients), so the mode matters more. `--color auto|truecolor|256|16|none`.
`auto` is `none` when `NO_COLOR` is set and not empty (https://no-color.org).
It is `truecolor` when `COLORTERM` is `truecolor` or `24bit`, `256` when `TERM`
contains `256color`, and `16` otherwise. `--mono` is one ink as in unicel,
and `--light` is paper. rendering.md decides the quantisation to 256 and 16
colours.

### The CLI's shape

Binary `glyphplot`, crate `glyphplot-cli`.

```
glyphplot <type> [--from FILE|-] [--x COL] [--y COL[,COL…]] [--format csv|tsv|json|ndjson|arrow]
                 [--window N] [--options JSON] [--view JSON] [--theme NAME] [--mono] [--light]
                 [--color auto|truecolor|256|16|none] [--glyphs braille|octants|sextants|quadrants|ascii]
                 [--cols N] [--rows N] [--fps N] [--seconds N] [--no-mouse]
glyphplot render <type> --from FILE --format txt|ansi|svg|png [--cols N --rows N] [--options JSON] [--view JSON] [-o FILE]
glyphplot demo <type>        plays the chart on generated data, and says so on its first line
glyphplot list               every chart type, with its data shape
glyphplot -h | --help | -V | --version
```

`candles` is a slug, so `glyphplot candles --from feed.csv` is the owner's
example as written. `render`, `demo` and `list` are reserved and no chart may
take them (unicel ROADMAP REQ-CLI-500's note). Columns are chosen by name or
1-based index. Without `--x`, the first column is x and the rest are y.

| Exit | When |
|---|---|
| 0 | Quit by a key, `--seconds`, a signal other than Ctrl+C, end of input when piped; help; version; list; a render written; a closed stdout. |
| 2 | A usage error, an unknown chart (with up to eight suggestions, REQ-CLI-010), bad options, unreadable or unparseable data (named by line and column), a column the chart needs missing, or a terminal failure, said on stderr as `glyphplot: <message>`. |
| 130 | Ctrl+C. |

These are unicel's codes (SPEC §9), extended to data. A separate data-error
code would break a script that tests only for 0 and non-zero. Nothing calls for one.

## The Rust crate

- `glyphplot`: the pure core, with a Cargo feature per chart as unicel's
  `unicel` has one per category (ADR-008). It covers data, view and input,
  frames into the grid, and `export::{text, ansi, svg}` (pure functions of a
  grid and its palette, small enough for every wasm module as unicel's ADR-112
  argues). `export::png` sits behind a `png` feature that no wasm module
  enables. A server renders with `Chart::new(type, options)`,
  `chart.set_data(…)`, `chart.frame(cols, rows, t)` and
  `export::svg(&grid, …)`, with no wasm or DOM.
- `glyphplot-ratatui` (later): `impl Widget for &glyphplot::Chart`, or a
  `StatefulWidget` holding the view. Its `render` copies the grid into the
  `Buffer` one `set_char` and `set_fg` per cell, as the bench's
  `hello_ratatui_grid.rs` does. It also maps crossterm mouse events, minus the
  widget's area origin, onto `chart.input`. It depends on `ratatui-core`
  (0.1.2), which holds `Widget` (`ratatui-core/src/widgets/widget.rs:70`) and
  `Buffer`, not on a backend.

## Exports

These are as unicel has them (SPEC §10.8), with unicel's planned move to Rust
(ADR-112, ROADMAP REQ-RENDER-001–005) taken from the start:

| Export | Made by | Used by |
|---|---|---|
| txt | `Chart.text()` / `export::text` | element `text()`, CLI `render --format txt`, piped CLI, SSR |
| ansi | `export::ansi` (the CLI's writer, ADR-115) | element `toANSI()`, CLI `render --format ansi`, CLI play |
| svg | `export::svg`, self-contained (unicel REQ-RENDER-004) | element `toSVG()`, CLI, SSR, README images |
| png | TS `paint` in the browser; Rust `png` feature in the CLI | element `toPNG()`, CLI |

An export draws the chart's current data, options and view at the frame's
`t`. So what the reader zoomed to is what they export.

## Package layout

### npm: one package, `glyphplot`

| Export | File | What |
|---|---|---|
| `.` | `dist/index.js` | `chart`, `mount`, `paint`, `fitBox`, `charts`, `moduleOf`, `wasm`, `budgets`, the constants and the types (unicel REQ-PKG-030's list, renamed). |
| `./element` | `dist/element.js` | Defines `<glyph-plot>`, exports `define`; the one side effect (`sideEffects: ["./dist/element.js"]`). |
| `./wasm` | `pkg/modules.js` | The manifest. |
| `./node` | `dist/node.js` | Loads a module's bytes from the package for Node; `renderText`, `renderSVG`. |
| `./jsx` | `dist/jsx.d.ts` | JSX typings for React 19, Preact, Solid (later). |

`files` are `dist`, `pkg` and `fonts`, as unicel's `web/package.json`. Every
chart's module ships in this one package as `pkg/<module>/glyphplot.js` and
`glyphplot_bg.wasm`. A page fetches only the modules it shows, so splitting the
npm package per chart would save no bytes on the wire and would add a
version-matching problem. A `bin` (`npx glyphplot`, running wasm in Node as
unicel's ROADMAP REQ-TERM-001 plans) is a later milestone. It needs terminal
input parsing outside crossterm.

### crates.io

| Crate | Published | What |
|---|---|---|
| `glyphplot` | yes | The core library (above). |
| `glyphplot-cli` | yes | Binary `glyphplot`: `cargo install glyphplot-cli`. |
| `glyphplot-wasm` | no (`publish = false`, as `unicel-wasm` is: `crates/wasm/Cargo.toml:8`) | The wasm-bindgen boundary; built by wasm-pack into the npm package. |
| `glyphplot-ratatui` | yes, later | The ratatui adapter. |

All four names returned 404 from `https://crates.io/api/v1/crates/<name>` on
2026-10-18, as did `glyphplot` and `glyphplot-react` from
`https://registry.npmjs.org/<name>`. Whether the npm organisation `@glyphplot`
is free could not be read: `npmjs.com/org/glyphplot` answered 403.

## Measurements

Machine: Apple M5 Pro, 18 cores, 48 GB, macOS. Date: 2026-10-18. Toolchain:
`mise exec --` in the repository (Rust 1.97, Node 26, wasm-pack 0.15.0).
Other agents were benchmarking at the same time: `uptime` load averages were
8.04, 5.61 and 4.06 before the Node runs and 7.73, 5.66 and 4.11 before the
process timings. The bench and the commands are in
`discovery/bench/surfaces/README.md`. The release profile is unicel's.

**Binary size**, hello-chart CLIs, release and stripped, one build each (sizes
are deterministic):

| Binary | Bytes | gzip -9 | Over `hello-std` |
|---|---|---|---|
| `hello-std` (std only, text out) | 335,728 | 163,130 | — |
| `hello-crossterm` | 404,800 | 201,353 | +69,072 |
| `hello-ratatui-grid` (glyphplot grid via a ratatui `Widget`) | 438,112 | 219,400 | +102,384 |
| `hello-ratatui` (ratatui's `Chart`) | 553,968 | 276,337 | +218,240 |

Crates in each tree: `cargo tree -e normal --prefix none | sort -u | wc -l`,
32 lines with `crossterm-cli` and 75 with `ratatui-cli`.

**Server-side first frame in Node.** `node ssr.mjs <n>`, 5 runs per n, an
80 × 24 frame, the hello chart's wasm (43,623 bytes, 19,457 gzip -9). Times
are in ms, given as min / median. "First frame" includes copying the
`Float64Array` into wasm memory and has no downsampling:

| n | instantiate | first frame | warm frame (mean of 100) |
|---|---|---|---|
| 1,000 | 0.27 / 0.30 | 0.94 / 0.96 | 0.033 / 0.034 |
| 100,000 | 0.28 / 0.32 | 3.09 / 3.23 | 0.83 / 0.84 |
| 1,000,000 | 0.31 / 0.31 | 20.8 / 21.3 | 8.01 / 8.11 |

**Process wall time, piped** (Python `subprocess.run` with
`time.perf_counter`, 5 runs, ms, min / median): `hello-crossterm` on 100,000
lines of stdin 6.6 / 6.9; `hello-ratatui` on the same 7.5 / 8.2; `node ssr.mjs 1000`
(a whole Node process) 31.3 / 32.9.

**TSX typing**: `npx -p typescript@5.9.3 tsc --noEmit --strict --jsx preserve t.tsx`
passed. The file types `glyph-plot` as a union discriminated on `type` and
`glyph-candles` as a fixed alias. The two `@ts-expect-error` lines (candle
data on `type="line"`, `type="pie"`) were errors as expected. Run once.

## Recommendation

- One element, `<glyph-plot type="…">`, plus `define(tag, { type })` for
  per-chart aliases. Its attributes, properties, methods and events are as
  tabled above. Data is a property or `append`, and the `data` attribute is
  only for small JSON. An options change keeps data and view. A `type` change
  keeps them when the new chart accepts them.
- The grid is sized by the container (unicel's `fill`). `cols` and `rows`
  attributes fix it, and `fit` then scales it as unicel's `fit` does.
- Modules load through a `modules.js` manifest (`charts`, `modules`,
  `imports`, `fallback`, `budgets`) behind unicel's near-screen gate, with
  `loading="eager"`. Calls made before load are queued.
- The JS layering is `chart()`, `mount()` and `paint()` under the element, with
  no DOM needed for `chart()`. There is no React wrapper, only JSX typings.
  `glyphplot/node` serves server-side first frames.
- The CLI is on crossterm after unicel's, with `glyphplot <type>`, `render`,
  `demo` and `list`. Data comes from files or stdin, streaming from a pipe
  with input from `/dev/tty`. Mouse and keys are as listed. Colour is
  `--color auto` with `NO_COLOR`. Exit codes are 0, 2 and 130.
- Crates: `glyphplot`, `glyphplot-cli`, `glyphplot-wasm` (unpublished) and
  later `glyphplot-ratatui` on `ratatui-core`. npm: one `glyphplot` package.
- Exports: txt, ansi and svg in Rust from the start. png is TS `paint` in the
  browser and Rust behind a feature in the CLI.

## Requirements to propose

These assume the brief's phase order maps to milestones as follows: v0.1.0
the first chart (line) end to end through the element; v0.2.0 contract and
budgets; v0.3.0 candlestick, bar, sparkline; v0.4.0 streaming; v0.5.0 the
site; v0.6.0 the CLI; v0.7.0 the next tier; v0.8.0 domain packs; v0.9.0
packaging. The synthesis may renumber them.

**Element**

- REQ-EL-010: Importing `glyphplot/element` MUST define `<glyph-plot>` once and do nothing where there is no DOM or no `customElements`. (v0.1.0)
- REQ-EL-020: `define(tag, { type })` MUST define `tag` as a subclass whose chart type is `type` whatever its `type` attribute says, and MUST do nothing for a tag already defined. (v0.3.0)
- REQ-EL-030: The element MUST observe exactly `type`, `data`, `options`, `view`, `theme`, `mono`, `motion`, `label`, `fit`, `cols`, `rows`, `loading`, `time` and `debug`. (v0.1.0; `src` added in REQ-EL-150)
- REQ-EL-040: The element MUST load nothing until it first comes within one screen height of the viewport (IntersectionObserver, `rootMargin: "100% 0px"`), unless `loading="eager"` or there is no IntersectionObserver. (v0.1.0)
- REQ-EL-050: A `data`, `options` or `view` set and every `append` made before the chart exists MUST be applied in order once it is made, a `data` set discarding the appends queued before it; a Node or Playwright test sets data and appends three batches before load and reads them all back from the chart. (v0.1.0)
- REQ-EL-060: A change of `options` or `theme` MUST NOT re-create the chart: data and view after the change MUST equal those before it. (v0.1.0)
- REQ-EL-070: A change of `type` MUST make a new chart and MUST give it the old chart's data and view when the new chart's meta accepts the data's shape; otherwise it MUST warn once on the console naming the shape and draw the new chart empty. (v0.3.0)
- REQ-EL-080: The `data`, `options` and `view` properties MUST be defined on the element's class, so that `"data" in document.createElement("glyph-plot")` is true before upgrade-time attributes are read. (v0.1.0)
- REQ-EL-090: Setting the `data` property MUST replace all data and MUST NOT write the `data` attribute; getting it MUST return the last value set. (v0.1.0)
- REQ-EL-100: Without `cols` and `rows`, the grid MUST be `cols = floor(W / cw)`, `rows = floor(H / ch)` of the element's content box, `cw` 0.6 × the element's font size and `ch` 2 × `cw`, and MUST follow every change of the box within one animation frame of the ResizeObserver's callback. (v0.1.0)
- REQ-EL-110: With `cols` and `rows`, the grid MUST be that size and `fit` MUST size it as unicel's REQ-FIT-001–006 do. (v0.2.0)
- REQ-EL-120: Events MUST be `CustomEvent`s named `plot-<name>` that bubble and are composed, fired for the reader's input and for loading only; setting `view`, `data` or `options` from script MUST fire no `plot-view`, `plot-hover`, `plot-select`, `plot-brush` or `plot-legend`. (v0.1.0)
- REQ-EL-130: The element MUST fire `plot-ready` once per chart made, after its first frame is drawn, and resolve `ready` then; a failed load or first frame MUST fire `plot-error` with the message and keep the element's children (unicel REQ-EL-060, REQ-EL-070). (v0.1.0)
- REQ-EL-140: `text()` MUST equal the CLI's piped frame for the same type, data, options, view and size. (v0.6.0)
- REQ-EL-150: The element SHOULD take `src`, a URL of CSV, JSON or Arrow IPC, fetched once per change and given as `data`. (v0.4.0)
- REQ-EL-160: The element MUST NOT be form-associated (`formAssociated` absent). (v0.1.0)

**Loading and manifest**

- REQ-MANIFEST-010: `modules.js` MUST export `charts` (every chart's meta, in registry order), `modules` (chart slug → module name), `imports` (module name → a function importing `./<module>/glyphplot.js`), `fallback` and `budgets` (module → `{gzip, frameMs, worstMs}`). (v0.1.0)
- REQ-MANIFEST-020: In the release build every chart MUST map to a module that plays exactly the charts the manifest maps to it, and `fallback` MUST be `null`; the debug build MUST map every chart to `all`. (v0.1.0)
- REQ-LOAD-010: `chart(type, init)` MUST load the type's module at most once per page, share it with every later caller, and reject with `there is no chart named "<type>"` for a type no module plays; a failed load MUST be tried again on the next call. (v0.1.0)

**JS API and SSR**

- REQ-JS-010: `chart(type, init)` and the returned `Chart`'s `data`, `append`, `options`, `view`, `frame`, `resize`, `text` and `free` MUST work with no DOM; a Node test draws a frame and compares its text to the Rust crate's. (v0.1.0)
- REQ-JS-020: `mount(el, chart, { input: true })` MUST turn pointer, wheel, touch and key events on `el` into `chart.input` calls and dispatch REQ-EL-120's events on `el`; `stop()` MUST remove every listener. (v0.1.0)
- REQ-SSR-010: `glyphplot/node` MUST export `renderText(type, init, cols, rows)` and `renderSVG(type, init, cols, rows)` that load the module from the package's files with no network, and its text MUST equal `glyphplot render <type> --format txt` for the same input. (v0.6.0)

**CLI**

- REQ-CLI-010: The CLI MUST accept `glyphplot <type>`, `render`, `demo`, `list`, `-h` and `-V` with the flags in this report and reject an unknown flag or a second type as a usage error; no chart slug may be `render`, `demo` or `list` (the registry test checks). (v0.6.0)
- REQ-CLI-020: The CLI MUST exit 0, 2 or 130 as this report's table says, with every error on stderr as `glyphplot: <message>`; a data error MUST name the line and column. (v0.6.0)
- REQ-CLI-030: With stdin not a terminal and no `--from`, or `--from -`, the CLI MUST read data from stdin and read keys and mouse from the controlling terminal; a pty test feeds lines to stdin and a key to the tty and sees the chart redraw and quit. (v0.6.0)
- REQ-CLI-040: Lines arriving on stdin while playing MUST be appended at most once a frame, and `--window N` MUST keep only the last N rows. (v0.6.0)
- REQ-CLI-050: With stdout not a terminal, the CLI MUST read to the end of input and print one frame as text with no escapes; a closed stdout MUST end it with exit 0 and nothing on stderr. (v0.6.0)
- REQ-CLI-060: Playing MUST enable mouse capture unless `--no-mouse`, and MUST map Moved to hover, Drag to pan, Shift+Drag to brush and scroll to zoom, with the keys listed in this report. (v0.6.0)
- REQ-CLI-070: `--color auto` MUST be `none` when `NO_COLOR` is set and not empty, `truecolor` when `COLORTERM` is `truecolor` or `24bit`, `256` when `TERM` contains `256color`, else `16`. (v0.6.0)
- REQ-CLI-080: The CLI MUST restore the terminal on every way out as unicel's REQ-CLI-030 lists, mouse capture included. (v0.6.0)
- REQ-CLI-090: `glyphplot demo <type>` MUST print, on the chart's first row or above it, that the data is generated. (v0.6.0)

**Rust crate and exports**

- REQ-CRATE-010: The `glyphplot` crate MUST render any chart to text and to SVG with no wasm, DOM or terminal dependency in its default features. (v0.6.0)
- REQ-CRATE-020: `glyphplot-ratatui` SHOULD implement ratatui-core's `Widget` for a chart, writing every grid cell into the `Buffer` within the widget's area and mapping mouse events relative to that area onto the chart's input. (v0.9.0)
- REQ-RENDER-010: txt, ansi and svg MUST be made in Rust (`glyphplot::export`) and MUST be byte-identical between the element's `toSVG()`/`toANSI()`/`text()` and the CLI's `render` for the same input. (v0.6.0)
- REQ-RENDER-020: An export MUST draw the chart's current data, options and view, so a test that zooms then exports gets the zoomed frame. (v0.6.0)

**Packages**

- REQ-PKG-010: The npm package MUST be named `glyphplot`, ESM, with exports `.`, `./element`, `./wasm` and `./node`, each with types, `files` `dist`, `pkg` and `fonts`, and `sideEffects` only `./dist/element.js`. (v0.9.0)
- REQ-PKG-020: The package SHOULD export JSX typings as `glyphplot/jsx` that type `<glyph-plot>` as a union discriminated on `type`; a `tsc` fixture MUST reject candle data on `type="line"`. (v0.9.0)
- REQ-PKG-030: The crates MUST be `glyphplot` (library), `glyphplot-cli` (binary `glyphplot`) and `glyphplot-wasm` (`publish = false`). (v0.1.0)

## Decisions with a real alternative

1. **One tag with `type`, or a tag per chart.** Context: the brief's open
   question 3. Options: (a) `<glyph-plot type>` with `define` aliases; (b) a
   tag per chart; (c) a host tag and plugin elements, as Perspective does.
   Recommendation: (a). Loading and tree-shaking are equal. TSX typing is
   solved by a union (measured). A type change keeps data, view and listeners.
   No surveyed library tags each leaf type. (c) adds a second element per chart
   for nothing a module boundary does not already give.
2. **Options change: re-create the chart, or `set_options`.** Context:
   unicel's ADR-025 re-creates. Options: (a) re-create and re-send the data
   from a JS copy; (b) `set_options` on the live chart. Recommendation: (b). A
   chart holds up to a million rows of the page's data, and a JS copy doubles
   memory. It reverses ADR-025 for glyphplot and should say so.
3. **CLI on crossterm, or on ratatui.** Options: (a) crossterm with unicel's
   writer; (b) ratatui's `Chart`; (c) ratatui as the terminal layer over
   glyphplot's grid. Recommendation: (a). 405 KB against 554 KB (b) or 438 KB
   (c), measured. (b) lacks ticks, downsampling and hit testing. (c) adds a
   second buffer and diff. ratatui serves users through an adapter crate
   instead.
4. **Grid size: from the box, or fixed by the chart.** Options: (a) cols and
   rows from the container (unicel's `fill`) by default, with `cols`/`rows`
   to fix them; (b) a fixed grid scaled by `fit`, as unicel's pieces.
   Recommendation: (a). A chart re-lays-out its axes and labels, and scaling a
   fixed grid shrinks its text below legibility.
5. **SSR artifact: text or SVG.** Options: (a) a text `<pre>`; (b) an SVG with
   `aspect-ratio`. Recommendation: offer both from `glyphplot/node`, and
   document (b) for pages, since it scales to the unknown box with no layout
   shift. Use (a) for terminals and plain-text contexts.
6. **A React wrapper, or JSX typings only.** Options: (a) `glyphplot/react`;
   (b) typings only. Recommendation: (b). React 19 passes properties and
   custom events natively (react.dev).

## Open questions for the owner

1. **One tag or a tag per chart** (brief's question 3). Options: `<glyph-plot type>`;
   `<glyph-candles>` and others; both, with aliases opt-in. Recommendation:
   `<glyph-plot type>` with `define(tag, { type })` aliases.
2. **The CLI's crate name.** Options: `glyphplot-cli`, installed as
   `cargo install glyphplot-cli`; or a `cli` feature and `[[bin]]` in the
   `glyphplot` crate (`cargo install glyphplot --features cli`).
   Recommendation: `glyphplot-cli`, as `wasm-bindgen-cli` and `sqlx-cli` do,
   so the library pulls no terminal crates.
3. **`npx glyphplot`.** Options: a Node bin running wasm (Node's start is about
   31 ms, measured), whose terminal input must be parsed outside crossterm;
   native binaries per platform in npm `optionalDependencies` (unicel ADR-115
   rejected these for its build matrix); or none, only `cargo install` and
   release binaries. Recommendation: none until the packaging milestone, then
   the wasm bin with the byte parser written once in Rust.
4. **`src` on the element.** Options: the element fetches a URL (CSV, JSON or
   Arrow); or the page always fetches and sets `data`. Recommendation: `src`,
   at the streaming milestone, because it makes a chart work with no script
   beyond the import.
5. **Main thread or worker.** Options: charts run on the main thread as
   unicel's pieces do; or a worker with OffscreenCanvas and transferred
   buffers. Recommendation: main thread for v0.1.0. Revisit if budgets.md's
   million-point frames exceed the frame budget on the main thread.

## Sources

- unicel: `../animate-unicode/spec/SPEC.md` §6 (lines 393–452), §7
  (456–521), §8 (525–579), §9 (583–625), §10.8 (737–762);
  `spec/ROADMAP.md` §4.5 (592–603), §4.7 (611–619), §5.6 (713–720),
  §6.2 (775–799), REQ-BACK-001–002 (420–421); `spec/adr/005-typescript-shell-renders.md`,
  `008-one-wasm-module-per-category.md`, `025-re-create-the-player.md`,
  `100-fit-attribute.md`, `112-exports-in-rust.md`, `115-terminal-delivery.md`;
  `web/src/element.ts` (lines 75–107, 321–343), `web/src/wasm.ts` (24–58),
  `web/src/index.ts` (9–13), `web/src/mount.ts` (27–78), `web/package.json`;
  `crates/cli/src/main.rs`, `play.rs` (1–50), `paint.rs` (1–60),
  `crates/cli/Cargo.toml`, `crates/wasm/Cargo.toml` (1–8),
  `crates/unicel/examples/manifest.rs` (36–100),
  `spec/schemas/manifest.schema.json`.
- ratatui, https://github.com/ratatui/ratatui at `0297560`:
  `ratatui-widgets/src/chart.rs` (40–52, 158–200, 325–340, 530–548, 886–930,
  994–1100); `ratatui-core/src/symbols/marker.rs` (8–48);
  `ratatui-core/src/buffer/cell.rs` (38–60); `ratatui-core/src/widgets/widget.rs` (70);
  `ratatui/Cargo.toml` (22–62).
- crossterm, https://github.com/crossterm-rs/crossterm at `ab8681d`:
  `src/event/mouse.rs` (1–110), `src/event/commands.rs` (44–60),
  `src/terminal/sys/file_descriptor.rs` (48–56).
- Perspective, https://github.com/finos/perspective at `0857a7d`:
  `packages/viewer-charts/src/ts/index.ts` (30–50),
  `rust/perspective-viewer/src/ts/plugin.ts` (60–80),
  `rust/perspective-viewer/src/rust/lib.rs` (131–136),
  `rust/perspective-viewer/src/rust/custom_elements/viewer.rs` (488, 1078, 1367).
- Ignite UI: `https://registry.npmjs.org/igniteui-webcomponents-charts/latest`
  (7.1.0) tarball, `htmlTagName` strings and `lib/igc-category-chart-component.d.ts`.
- Vaadin Charts: https://vaadin.com/docs/latest/components/charts
- Lightweight Charts v5: https://tradingview.github.io/lightweight-charts/docs/migrations/from-v4-to-v5
- React custom elements: https://react.dev/reference/react-dom/components#custom-html-elements
- HTML lazy loading: https://html.spec.whatwg.org/multipage/urls-and-fetching.html#lazy-loading-attributes
- NO_COLOR: https://no-color.org
- Name checks: `https://crates.io/api/v1/crates/<name>`, `https://registry.npmjs.org/<name>`,
  2026-10-18.
- Measurements: `discovery/bench/surfaces/` (README, `src/`, `ssr.mjs`);
  the TSX check, `scratchpad/surfaces/tsx/t.tsx` (outside the repository).
