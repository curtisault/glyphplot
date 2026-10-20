# Interaction

## Summary

- A chart's interactive state is one plain, serialisable **view state**: the x
  and y domains, the cursor, the selection, the hidden series, the page's time
  cursor and an in-flight view animation. A frame is a pure function of data,
  options, view state and `t`, as the brief requires; input is not part of
  that function. Input changes the view state, and the view state is what is
  drawn, read, set, linked and put in a URL.
- Input is folded into view state **in wasm**, by a reducer
  `input(view, event) → (view, events out)` that lives in the core crate. The
  shell only turns pixels into cells and recognises gestures; the CLI uses the
  same reducer natively. The reducer has to live where the data, the layout and
  the scales are, because a pointer move needs all three (hit test, inverse
  scale, snap, clamp). There is no input log in the core: unlike unicel's
  ADR-114, the view state already makes the frame reproducible.
- One pointer move costs **0.6–0.8 µs at 80 × 24 and 4–5 µs at 320 × 120** with
  1,000,000 points, in wasm under Node and natively: hit test by binary search
  (0.04–0.15 µs), readout, and a crosshair composed over the cached base frame.
  That is at most **0.03 % of a 16.7 ms frame**. Crossing into wasm with two
  numbers costs 0.03–0.05 µs. Drawing the whole frame again from the data
  instead costs **1.5–2.3 ms** (10–14 % of a frame) when all 1M points are
  visible. So hover MUST NOT redraw from data; pan and zoom do, and their cost
  is the frame budget, not the input budget.
- Hit testing is per chart kind: a binary search on sorted x for x-ordered
  charts (line, area, candles, bars), arithmetic for binned charts (heatmap,
  histogram), and a spatial index for scatter. A screen-space grid over 1M
  scatter points answers in 0.6–21 µs but costs 4–10 ms to build per view
  change, so it is built lazily on the first hover after a view settles; until
  then a linear scan (1.2–1.4 ms) answers.
- The event vocabulary follows surfaces.md's naming rule (`plot-` prefix, never
  fired for a property the page set): `plot-hover`, `plot-select`,
  `plot-brush`, `plot-view`, `plot-legend`, plus `plot-cursor` for the time
  cursor. `detail` shapes are drafted below as JSON.
- Every interaction is reachable by keyboard (arrows step datum to datum,
  Shift+arrows pan, + and − zoom, Enter selects and brushes) and by touch (one
  finger scrubs, two fingers pan, pinch zooms, double-tap resets). A visually
  hidden table of the current view and a polite live region carry the data to
  screen readers.

## Scope and method

Covered: the interactions the brief lists, their mapping onto a grid of cells
with sub-cell precision, the split between the TypeScript shell and wasm, the
cost of a pointer move at 60 Hz (measured), the view-state model, the input
event shape and the events out (drafted as JSON), determinism, keyboard,
touch, screen readers and the terminal.

Read, with code: uPlot (cursor, drag-select, sync, `posToVal`, `closestIdx`),
TradingView Lightweight Charts (crosshair modes, wheel, pinch, long-tap,
tracking mode, kinetic scroll), Vega (event-stream selectors, picking,
selections, view state, ARIA), crossterm (mouse modes and event types) and
ratatui (where input and view state live in a TUI). Revisions cloned on
2026-10-18 into the session scratchpad: uPlot `7187ad7`, lightweight-charts
`9db7ce7`, vega `fdd9ab9`, ratatui `0297560`, crossterm `ab8681d`. unicel:
`spec/ROADMAP.md` §5.1, `spec/adr/114-input-log.md`,
`spec/schemas/input-log.schema.json`, `spec/SPEC.md` §6–§8,
`crates/wasm/src/lib.rs`, `crates/unicel/src/{piece,grid,subcell}.rs`,
`web/src/{element,mount,wasm}.ts`.

Built: `discovery/bench/interaction/` (throwaway; its README says how to
rerun). A line chart of 1,000,000 sorted points in braille with axes and a
readout row, in unicel's grid shape, and a 1,000,000-point scatter. Timed
natively and in wasm under Node 26's V8 (standing in for Chrome; no browser
run), at 80 × 24 and 320 × 120 cells.

Not covered: DOM event dispatch and canvas paint time in a real browser (no
Playwright run), Safari's behaviour (trackpad pinch, Pointer Events details),
screen-reader behaviour with real assistive technology, and a data-space
spatial index for scatter (proposed, not measured). Each is marked where it
matters.

## What the libraries do

### uPlot

- The cursor is a position in CSS pixels; `posToVal` maps a position to a
  scale's value by the scale's min, max and distribution (linear, log, asinh,
  custom) (`src/uPlot.js:3039-3070`). The nearest datum is a binary search on
  the x column, `closestIdx` (`src/utils.js:2-21`), called from
  `closestIdxFromXpos` (`src/uPlot.js:3072-3075`). Hover is "nearest by x", not
  by distance; a `hover.prox` option limits it.
- Mouse only: it binds `mousedown`, `mouseup`, `click`, `dblclick`,
  `mousemove`, `mouseleave`, `mouseenter` (`src/opts.js:484-493`). There is no
  keyboard handling, no ARIA and no touch handling in the library: a grep of
  `src/` for `keydown`, `aria` and `touch` finds only a comment. Wheel zoom and
  touch zoom are demos, not features (`demos/zoom-wheel.html`,
  `demos/zoom-touch.html`).
- Drag selects an x range by default and a selection can set the scale (zoom)
  (`src/opts.js:495-508`: `drag: {setScale, x: true, y: false}`); double-click
  resets.
- Linked charts share a sync key (`src/sync.js`): a plain pub/sub list of
  plots. The cursor crosses in data units, not pixels: the source writes
  `posToVal` of its cursor into `sync.values` (`src/uPlot.js:3611-3618`) and the
  receiver maps them back with its own scale (`src/uPlot.js:3690-3713`). So
  charts of different sizes and scales link correctly.
- Legend and DOM updates are batched to one `requestAnimationFrame`
  (`src/uPlot.js:2536-2542`).

### Lightweight Charts

- Crosshair modes: `Normal`, `Magnet` (snap the horizontal line to the
  series' value, the default), `Hidden`, `MagnetOHLC`
  (`src/model/crosshair.ts:42-58`, default at line 117).
- Wheel: `deltaY` zooms the time scale about the pointer, `deltaX` scrolls,
  `preventDefault` when cancelable (`src/gui/chart-widget.ts:597-620`), with
  a delta-mode correction (`chart-widget.ts:574-595`). By default the plain
  wheel zooms (`src/api/options/chart-options-defaults.ts:38-55`): the page
  cannot scroll past a chart with the wheel.
- Touch is its own handler on Touch Events, not Pointer Events
  (`src/gui/mouse-event-handler.ts`): long tap at 240 ms, double-tap within 30
  px, click cancelled past 5 px (`mouse-event-handler.ts:71-81`), and pinch
  from two touches. `touchmove` is non-passive so it can `preventDefault`
  (line 536). Options choose whether horizontal and vertical touch drags are
  the chart's or the page's (`horzTouchDrag`, `vertTouchDrag`,
  `chart-options-defaults.ts:41-42`).
- On touch, a drag scrolls the chart and a long press enters "tracking mode",
  in which a drag moves the crosshair; it ends on the next tap by default
  (`src/model/chart-model.ts:228-258`, `src/gui/pane-widget.ts:304-394`).
- Kinetic scroll is on for touch and off for mouse by default
  (`chart-options-defaults.ts:56-59`), with a decay coefficient 0.997 a
  millisecond, speeds 0.2–7 px/ms and a 15 px minimum move
  (`src/gui/pane-widget.ts:38-43`, `src/model/kinetic-animation.ts`).
- No keyboard, no ARIA: a grep of `src/` for `keydown`, `aria-` and `role=`
  finds nothing.

### Vega

- Interaction is declared as event streams parsed from a selector grammar:
  `[pointerdown, window:pointerup] > window:pointermove!` is "moves between a
  down and an up", with filters and `{throttle, debounce}`
  (`packages/vega-event-selector/src/event-selector.js:86-195`;
  `packages/vega/test/specs-valid/map.vg.json:45`). Streams update signals;
  signals drive scales and marks.
- Hit testing is scenegraph picking on every move
  (`packages/vega-scenegraph/src/CanvasHandler.js:25-29,156-168`): fine for
  hundreds of marks, not for a million points.
- Selections are stored as tuples of fields and values, a field typed as an
  enumeration (`E`) or a range (`R`, `R-E`, …)
  (`packages/vega-selections/src/selectionTest.js:5-16`), combined across
  views by `union` or `intersect`
  (`packages/vega-selections/src/selectionResolve.js`). A selection is data,
  so it crosses views and serialises.
- The whole interactive state can be exported and restored:
  `view.getState()` / `view.setState()` (signals and modified data)
  (`packages/vega-view/src/state.js:20-48`). This is the model glyphplot's
  view state follows, with a fixed shape instead of arbitrary signals.
- ARIA: the container gets `role="graphics-document"` and
  `aria-roledescription="visualization"` and the description as its label
  (`packages/vega-view/src/aria.js:1-16`).

### The terminal: crossterm and ratatui

- `EnableMouseCapture` turns on X10/normal (1000), button-motion (1002),
  any-motion (1003), RXVT (1015) and SGR (1006) reporting
  (`crossterm/src/event/commands.rs:44-60`). Mode 1003 gives hover.
- `MouseEvent` carries `column` and `row` as `u16` and the modifiers
  (`src/event/mouse.rs:24-33`); kinds are `Down`, `Up`, `Drag`, `Moved` and
  four scroll directions (`mouse.rs:73-90`). There is no pixel-precision mode
  (SGR-pixels, 1016): positions are whole cells. In the terminal a crosshair
  moves by a cell, two braille dot columns, and there is no pinch.
- ratatui widgets do not handle input; the application does, and passes the
  view into the widget. The `Chart` widget takes `bounds: [f64; 2]` per axis
  (`ratatui-widgets/src/chart.rs:44,75`); examples map mouse cells to state by
  hand (`examples/apps/custom-widget/src/main.rs:226-257`). The view belongs
  to the caller, as data.

### unicel

- unicel's planned interaction (`spec/ROADMAP.md` §5.1) forwards
  `pointermove`, `pointerdown`, `pointerup` and `pointerleave` in cells with
  fractions (REQ-EL-503, REQ-WASM-501) into an input log of at most 512 events
  stamped with play time; a frame is a function of `t` and the events at or
  before it (ADR-114, `input-log.schema.json`). That suits art that reacts
  (a ripple's radius from `t − t_press`). It does not suit a chart: a zoom done
  600 events ago must still hold, and a page must be able to set a view that
  no event produced.
- unicel's canvas painter redraws only cells whose glyph or colours changed
  (REQ-MOUNT-060), so the number of cells an event changes is what it costs to
  paint.

## The interactions on a grid of cells

| Interaction | Pointer | Keyboard | Touch | Terminal | Changes |
|---|---|---|---|---|---|
| Crosshair and readout | move | ← → step datum; ↑ ↓ change series | one-finger drag (scrub) | mouse move (1003) or keys | `cursor` |
| Pan | drag | Shift+← →, PageUp/PageDown | two-finger drag; kinetic on release | drag, Shift+← → | `x.domain` |
| Zoom | Ctrl/⌘+wheel, trackpad pinch | + − about the cursor | pinch about its centre | scroll, + − | `x.domain` (and `y` when `fit` is fixed) |
| Brush a range | Shift+drag | Enter sets the anchor, arrows extend, Enter commits, Esc cancels | long press then drag | Shift+drag, or the keys | `selection` |
| Select a datum or series | click | Enter on the cursor's datum | tap | click, Enter | `selection.points` |
| Toggle a series | click its legend entry | 1–9 toggle the nth series | tap the legend entry | click, 1–9 | `hidden` |
| Reset the view | double-click | 0 or Home | double-tap | 0 or Home | `x`, `y` to fit |
| Linked views | none (the page links) | none | none | none | `x.domain`, `cursor.x` |
| Time cursor | none (the page drives it) | none | none | none | `time` |

### Pointer to cell to datum

- The shell measures the drawn element once per resize (a ResizeObserver, as
  unicel's mount does) and converts each event:
  `x = (clientX − left) / cellWidth`, `y = (clientY − top) / cellHeight`, in
  cells with fractions. That is all the shell computes per event.
- In wasm, the fraction gives the dot inside the cell for the chart's canvas:
  `dot = floor(x × W), floor(y × H)` with W × H = 2 × 4 for braille and
  octants, 2 × 3 for sextants, 2 × 2 for quadrants, 1 × 2 for half blocks
  (unicel `crates/unicel/src/subcell.rs:1-10`). In the terminal the fraction is
  always 0.5 (the cell's centre).
- The dot's centre maps to data through the inverse of the axis scale
  (uPlot's `posToVal`). For x-ordered data the nearest datum is a binary search
  on the x column (uPlot's `closestIdx`): 20 comparisons at 10⁶ points.
- When a dot column holds many data (1M points over 142 dot columns is about
  7,000 a column), "nearest by x" picks an arbitrary one of them. The readout
  then reports the column's aggregate (first, last, min, max, count), which the
  downsampler already holds for drawing; the crosshair snaps to whichever of
  the column's min and max is nearer the pointer's y. The bench did not
  measure this variant; it reads two cached numbers a column.
- The crosshair is drawn at dot precision where the plot is blank: a braille
  column of dots (`⡇` or `⢸`) for the vertical rule and a dot row for the
  horizontal, so the rule sits on the datum, not on the cell's edge. Where the
  series occupies the cell the glyph stays and takes the crosshair's colour.
  The bench drew `┊` and `┄` in blank cells; a frame from it is in the bench
  output.
- Binned charts (heatmap, histogram, bars) hit-test by arithmetic: the cell
  and the band scale give the bin in O(1). Candles hit-test like a line (by x,
  then report OHLCV).

### What the shell does and what crosses into wasm

| Where | What | How often |
|---|---|---|
| shell (TS) | pixels → cells; gesture recognition (tap, double-tap, long press, pinch, two-finger pan); coalescing moves to one per frame; dispatching `CustomEvent`s | every DOM event; one wasm call per frame for moves |
| wasm | the reducer: hit test, inverse scales, snapping, clamping, view update, which events to emit; drawing the overlay over the cached base, or the base again when the view or the data changed | once per animation frame with input, at most |
| page | sets `view`, sets `time`, links charts by setting each other's `view` | on its own events |

- Chrome dispatches `pointermove`, `mousemove`, `touchmove` and `wheel` aligned
  to animation frames since Chrome 60, and `getCoalescedEvents()` returns the
  merged points (https://developer.chrome.com/blog/aligning-input-events).
  `getCoalescedEvents` is not Baseline and needs a secure context
  (https://developer.mozilla.org/en-US/docs/Web/API/PointerEvent/getCoalescedEvents).
  The shell keeps the latest move and sends it on the next frame; it does not
  need coalesced points (a crosshair and a pan need only the latest). Wheel
  deltas are summed per frame.
- Down, up, key, wheel-end and leave are sent at once, in order, so a click
  between two moves is never lost.
- The call returns a small bitmask: overlay changed, base changed, events
  pending. Only when "events pending" is set does the shell read the events
  out (one string of JSON). A plain hover that stays on the same datum crosses
  once and dispatches nothing.

## Determinism: where input is folded

The brief fixes that a frame is a pure function of data, options, view state
and `t`. Three ways to honour it with input:

1. **The shell folds input into view state** (TS) and calls `set_view` each
   frame. The shell would need the scales, the layout and the data to hit-test
   and snap, which live in wasm; it would duplicate them, and the CLI would
   need a second implementation in Rust.
2. **Wasm folds input into view state** with a pure reducer
   `reduce(view, event, ctx) → (view', events)`, ctx being the data, options
   and layout. The view state is explicit, serialisable and settable; the
   frame is a function of it. Animated view changes (kinetic pan, an eased
   zoom, a reset) are a `motion` record inside the view state stamped with
   `t`, so the frame at any `t` is still determined. The reducer is unit-tested
   with scripted event lists; the contract replays a script and checks the
   resulting view and frame.
3. **Log the input** as unicel's ADR-114 does, and derive the view by
   replaying the log each frame. Determinism holds, but a log capped at 512
   events forgets old zooms (the view would jump), replay cost grows with the
   session, and a view set by the page or a URL is not an event.

Recommendation: 2. The input log is not needed for determinism. Exports of
"what the reader did" (unicel's REQ-INPUT-002) are served by recording
`(t, view)` whenever the view changes, in the shell, which is smaller and
exact.

## The view state (draft for `spec/schemas/view.schema.json`)

Rules: every domain is in the data's units (the x column's unit, a timestamp
in its declared unit for time); `null` means "fit" (the chart's default);
`version` lets the shape grow. The page sets it as a whole or in part (a
partial object merges; a key set to `null` returns to the default). Zoom is
not a separate number: it is the domain's span against the data's extent.

```json
{
  "version": 1,
  "x": { "domain": [1760000000000, 1760086400000], "follow": false },
  "y": { "domain": null, "fit": "visible" },
  "axes": { "volume": { "domain": null, "fit": "visible" } },
  "cursor": { "x": 1760043200000, "y": 212.4, "series": "close", "index": 51234, "locked": false },
  "selection": {
    "x": [1760010000000, 1760030000000],
    "y": null,
    "points": [{ "series": "close", "index": 51234 }]
  },
  "hidden": ["volume"],
  "time": null,
  "motion": null
}
```

| Field | Type | Meaning |
|---|---|---|
| `x.domain` | `[number, number]` or `null` | The visible x range; `null` fits the data. |
| `x.follow` | boolean | Live mode: the domain keeps its span and ends at the newest datum, as appends arrive (data.md's streaming). A pan away from the end sets it false; `End` sets it true. |
| `y.domain` | `[number, number]` or `null` | The visible y range; `null` with `fit`. |
| `y.fit` | `"visible"` \| `"all"` \| `"fixed"` | Fit y to the data in the x domain (default), to all data, or keep `domain`. |
| `axes` | object of scale id → `{domain, fit}` | Further y scales (a volume pane, a right axis), keyed by the scale ids the options name. |
| `cursor` | object or `null` | The crosshair: data-space `x` and `y`, and the datum it snapped to (`series`, `index`). `locked` keeps it when the pointer leaves (a click or Enter locks; Esc unlocks). |
| `selection.x`, `.y` | `[number, number]` or `null` | A brushed range per axis. |
| `selection.points` | array of `{series, index}` | Selected data. |
| `hidden` | array of series ids | Series toggled off. |
| `time` | number or `null` | The page's time cursor, in x units: a vertical marker and a readout, independent of the pointer. |
| `motion` | object or `null` | A view animation in flight: `{ "kind": "ease" \| "kinetic", "t0": 12.5, "from": [a, b], "to": [c, d], "duration": 0.25 }` for the x domain, `t0` in play time. The domain at `t` is computed from it; at `t ≥ t0 + duration` it folds into `x.domain`. With reduced motion there is none. |

Indices in `cursor` and `selection.points` are positions in the data as held
now; after a replace or a trimming append they are re-derived from `x`, so a
page linking or saving a view SHOULD rely on `x` values, not indices.

## The input event (draft for `spec/schemas/input.schema.json`)

The JSON form is canonical: it is what tests, the contract, the CLI's scripted
input and a recorded session use. The browser crosses with a numeric encoding
of the same fields (`input(kind, x, y, a, b, buttons, mods, t)` → `u32`),
because the hot path should not build strings; the bench measured both
(JSON adds 0.4–0.5 µs an event at 80 × 24, which is affordable but pointless
on the hot path).

```json
[
  { "t": 3.20, "kind": "move",  "x": 41.37, "y": 10.62, "pointer": "mouse", "buttons": 0, "mods": [] },
  { "t": 3.25, "kind": "down",  "x": 41.40, "y": 10.62, "pointer": "touch", "buttons": 1, "mods": [] },
  { "t": 3.40, "kind": "up",    "x": 47.10, "y": 10.80, "pointer": "touch", "buttons": 0, "mods": [] },
  { "t": 3.60, "kind": "wheel", "x": 41.37, "y": 10.62, "dx": 0, "dy": -1.5, "mods": ["ctrl"] },
  { "t": 3.80, "kind": "pinch", "x": 40.00, "y": 12.00, "scale": 1.25, "end": false },
  { "t": 3.90, "kind": "pan",   "dx": -2.5, "dy": 0, "end": true, "velocity": -18.0 },
  { "t": 4.10, "kind": "tap",   "x": 41.40, "y": 10.62, "count": 2 },
  { "t": 4.30, "kind": "press", "x": 41.40, "y": 10.62 },
  { "t": 4.50, "kind": "key",   "key": "ArrowRight", "mods": ["shift"] },
  { "t": 4.70, "kind": "leave" }
]
```

| Kind | Fields | From |
|---|---|---|
| `move` | `x`, `y` in cells (fractions inside a cell), `pointer`, `buttons`, `mods` | `pointermove`, crossterm `Moved`/`Drag` |
| `down`, `up` | the same | `pointerdown`/`pointerup`, crossterm `Down`/`Up` |
| `leave` | — | `pointerleave`, focus loss |
| `wheel` | `x`, `y`, `dx`, `dy` in lines, `mods` | `wheel` (pixels ÷ line height), crossterm scrolls (±1) |
| `pinch` | centre `x`, `y`; `scale` since the last pinch event; `end` | the shell's two-pointer recogniser |
| `pan` | `dx`, `dy` in cells; `end`; `velocity` in cells a second at the end | the shell's two-finger recogniser |
| `tap` | `x`, `y`, `count` (1 or 2) | the shell's recogniser (5 px slop, 500 ms double-tap, as Lightweight Charts) |
| `press` | `x`, `y` | long press (Lightweight Charts uses 240 ms) |
| `key` | `key` (the DOM `KeyboardEvent.key`), `mods` | `keydown` on the focused element, crossterm `KeyEvent` |

`t` is play time in seconds, used only to stamp `motion`; the reducer does
not read a clock. `mods` is a subset of `shift`, `ctrl`, `alt`, `meta`.

## Events out (draft for `spec/schemas/events.schema.json`)

Rules, as surfaces.md sets them: `CustomEvent`s on the element, bubbling and
composed, prefixed `plot-`; fired for the reader's actions only, never for a
property the page set, so linked charts cannot echo. Each `detail` carries
`source`: `"pointer"`, `"touch"`, `"keyboard"` or `"wheel"`. `plot-hover` fires
when the datum under the cursor changes, at most once an animation frame.
`plot-view` fires at most once a frame during a gesture with `final: false`,
and once with `final: true` when it ends (pointer up, wheel idle for 150 ms,
key up, kinetic motion done), so a page can update a URL only on `final`.

```json
{
  "plot-hover":  { "source": "pointer", "series": "close", "index": 51234, "x": 1760043200000, "y": 212.4,
                   "datum": { "t": 1760043200000, "open": 211.9, "high": 212.8, "low": 211.5, "close": 212.4, "volume": 1820 },
                   "aggregate": null, "cell": { "col": 41, "row": 10 } },
  "plot-select": { "source": "keyboard", "series": "close", "index": 51234, "x": 1760043200000, "y": 212.4,
                   "datum": { "close": 212.4 }, "selection": { "points": [{ "series": "close", "index": 51234 }] } },
  "plot-brush":  { "source": "pointer", "x": [1760010000000, 1760030000000], "y": null, "count": 3301, "final": true },
  "plot-view":   { "source": "wheel", "cause": "zoom", "final": false,
                   "view": { "version": 1, "x": { "domain": [1760020000000, 1760060000000], "follow": false } } },
  "plot-legend": { "source": "pointer", "series": "volume", "visible": false, "hidden": ["volume"] },
  "plot-cursor": { "source": "keyboard", "x": 1760043200000, "locked": true }
}
```

- `plot-hover` `detail` is `null` when the cursor leaves the plot. When a dot
  column holds more than one datum, `datum` is the snapped one and
  `aggregate` is `{ "count", "first", "last", "min", "max" }` of the column.
- `plot-view.cause` is `pan`, `zoom`, `reset`, `brush-zoom` or `follow`.
- `plot-brush` with `x: null` and `y: null` is a cleared brush.
- `plot-cursor` fires when the reader locks, unlocks or steps a locked cursor;
  it is the event a page listens to for a "scrub" that drives other content.

## Linked views and a URL that holds a view

- Linking is the page's job, by the rules above: on `plot-view` from chart A,
  set `B.view = { x: e.detail.view.x }`; on `plot-hover`, set
  `B.view = { cursor: { x: e.detail.x } }`. Setting a property fires nothing,
  so there is no loop. Values are in data units, so charts of different sizes
  link correctly (as uPlot's sync, `src/uPlot.js:3611-3618`).
- A small helper SHOULD do the common case declaratively: a `link` attribute
  naming a group; charts of one group share `x` and `cursor.x`. uPlot's
  `sync.js` is 25 lines; this is the same.
- A view round-trips through JSON: `JSON.stringify(el.view)` into the URL's
  query and back through the `view` attribute or property. A compact URL form
  (`?x=a,b`) is the page's choice, not the library's.

## Touch

- Pointer Events carry touch as `pointerType: "touch"` with one `pointerId` a
  finger; the shell tracks active pointers and recognises gestures itself.
  Lightweight Charts uses Touch Events (`mouse-event-handler.ts`) for older
  Safari; whether glyphplot needs that fallback depends on the Safari versions
  the owner supports **(unverified: Pointer Events in Safari 13 and later)**.
- The owner's test is "the reader scrubs it with a finger", so one-finger
  horizontal drag scrubs the cursor (Lightweight Charts makes it pan and puts
  the crosshair behind a long press; the brief's test reverses that). Two
  fingers pan, pinch zooms about its centre, a tap selects, a double-tap
  resets, a long press starts a brush.
- The drawn element gets `touch-action: pan-y`: vertical swipes still scroll
  the page, horizontal drags and pinches reach the chart
  (https://developer.mozilla.org/en-US/docs/Web/CSS/touch-action). unicel's
  interactive pieces take `touch-action: none` (`ROADMAP.md` §5.1 note under
  REQ-EL-503); a chart inside a scrolling article must not trap the reader's
  scroll.
- Kinetic pan on release, as Lightweight Charts does on touch only
  (`chart-options-defaults.ts:56-59`), as a `motion` in the view state; none
  with `prefers-reduced-motion` unless `motion` is set.

## Keyboard

- The drawn element is focusable (`tabindex="0"`) with a visible focus ring.
  Keys act only while it has focus, so a page's own shortcuts are untouched.
- Proposed bindings: ← → move the cursor to the previous or next datum (and
  pan when it reaches the edge); Shift+← → pan by a tenth of the view;
  PageUp/PageDown pan by a view; ↑ ↓ move the cursor to the next series at that
  x; + and − (and `=`) zoom about the cursor; 0 resets; Home/End go to the first
  and last datum, End also setting `follow`; Enter locks the cursor and
  selects, or with Shift starts and commits a brush; Esc clears the brush,
  unlocks the cursor, then blurs; 1–9 toggle series; `?` shows the keys. The
  terminal adds h j k l and q.
- This differs from surfaces.md's terminal keys (← → pan, ↑ ↓ zoom): stepping
  datum by datum has to be the plain arrows, because it is the only way a
  keyboard or screen-reader user reads the data one value at a time.

## Screen readers

- The drawn element: `role="graphics-document"`,
  `aria-roledescription="chart"`, an `aria-label` from the `label` attribute or
  the chart's title, and `aria-describedby` pointing at a summary (type, series,
  x range, y range, count). Vega does the same with "visualization"
  (`vega-view/src/aria.js:1-16`). Whether arrow keys reach the element in a
  screen reader's browse mode without `role="application"` is
  **(unverified)**; the owner's device checks settle it.
- A data table: a visually hidden `<table>` sibling of the drawn element (not
  inside the canvas), with a caption, column headers from the series and the
  rows of the current view, downsampled to at most 100 rows (the aggregate
  per bucket, as the readout gives) so a 1M-point chart does not put a million
  rows in the DOM. It is rebuilt on `final` view changes only. A "show data"
  toggle MAY make it visible.
- A live region (`aria-live="polite"`, `aria-atomic="true"`) speaks the
  readout when the cursor moves by keyboard, debounced to 250 ms; it does not
  speak pointer hover. For a streaming chart it speaks the newest value only
  when the page opts in, at most every 5 s (a11y-theming.md decides the
  numbers).

## Measurements

Machine: Apple M5 Pro, 18 cores, 48 GB, macOS; 2026-10-18; shared with other
discovery agents' benchmarks. `uptime` load averages: 9.2–11.3 during the
native runs, 10.4–12.1 during the Node runs. Commands, from
`discovery/bench/interaction/`: `mise exec -- cargo run --release` (native)
and `mise exec -- wasm-pack build --target web --release --out-dir pkg &&
mise exec -- node node/bench.mjs` (wasm under Node v26.5.1's V8, standing in
for Chrome). Each binary ran 3 times; each step inside ran 5 times over a
sweep of events (100,000 cheap events, down to 100 full frames); the table
gives the minimum of all 15 and the median of the three runs' medians, per
event. "From JS" includes the call across the boundary and, for drawing paths,
making fresh `Uint32Array`/`Uint8Array` views over the frame as unicel's shell
does after every frame. Data: 1,000,000 points (x ascending with irregular
gaps, y a random walk); scatter: 1,000,000 points in five clusters. Chart:
8-cell y labels, an axis row, a label row and a readout row.

| What | Size | Native min / median µs | Wasm (Node) min / median µs | Median of 16.7 ms |
|---|---|---|---|---|
| Crossing alone: `noop(x, y) → u32` from JS | any | — | 0.027 / 0.049 | 0.0003 % |
| Pointer → dot → `view.cursor` | any | 0.004 / 0.005 | 0.031 / 0.035 (from JS) | 0.0002 % |
| + hit test, binary search over 1M sorted x | any | 0.033 / 0.040 | 0.101 / 0.153 (from JS); 0.046 / 0.054 (inside) | 0.001 % |
| Hover event: hit + readout + crosshair over cached base | 80 × 24 | 0.55 / 0.61 | 0.73 / 0.79 (from JS) | 0.005 % |
| Hover event | 320 × 120 | 3.8 / 4.9 | 4.1 / 5.0 (from JS) | 0.030 % |
| Hover event as JSON (`JSON.stringify` + serde_json) | 80 × 24 | 0.69 / 0.80 | 1.16 / 1.22 | 0.007 % |
| Hover event as JSON | 320 × 120 | 4.4 / 5.0 | 4.8 / 5.3 | 0.032 % |
| Hover event that redraws the full frame, 1M visible | 80 × 24 | 1,689 / 1,745 | 1,480 / 1,660 | 10.0–10.5 % |
| Hover event that redraws the full frame, 1M visible | 320 × 120 | 1,989 / 2,310 | 2,124 / 2,267 | 13.6–13.9 % |
| The same, zoomed to 10 % (100k visible) | 80 × 24 | 204 / 220 | 191 / 231 | 1.3–1.4 % |
| The same, zoomed to 10 % | 320 × 120 | 342 / 384 | 401 / 440 | 2.3–2.6 % |
| Pan event (view moves: full frame + overlay), 1M visible | 80 × 24 | 1,592 / 1,776 | 1,547 / 1,745 (inside) | 10.5–10.7 % |
| Pan event, 1M visible | 320 × 120 | 2,045 / 2,280 | 2,053 / 2,185 (inside) | 13.1–13.7 % |
| Pan event, 100k visible | 80 × 24 / 320 × 120 | 189 / 210; 361 / 385 | 203 / 218; 399 / 417 | 1.3–2.5 % |
| Scatter: build a screen-space grid over 1M points (per view change) | 80 × 24 | 3,847 / 4,295 | 4,903 / 6,193 | 26–37 % |
| Scatter: build | 320 × 120 | 5,698 / 8,801 | 5,734 / 9,998 | 53–60 % |
| Scatter: nearest by the grid (rings in dot space) | 80 × 24 | 0.53 / 0.57 | 0.75 / 0.83 | 0.005 % |
| Scatter: nearest by the grid | 320 × 120 | 14.4 / 15.9 | 21.0 / 21.3 | 0.13 % |
| Scatter: nearest by scanning all 1M points | 80 × 24 / 320 × 120 | 1,386 / 1,650; 1,182 / 1,373 | 1,165 / 1,357; 1,138 / 1,202 | 7–10 % |
| Cells changed per hover event (what a diffing painter redraws) | 80 × 24 / 320 × 120 | 8.2 of 1,920; 202 of 38,400 | — | — |
| Cells changed per pan event (2–3 cells a step) | 80 × 24 / 320 × 120 | 297 of 1,920; 3,209 of 38,400 | — | — |
| Wasm memory, 2M points (line and scatter, x and y as f64) | — | — | 39.6–43.4 MiB | — |
| Bench module size (includes serde_json) | — | — | 103,076 B; 47,460 B gzip -9 | — |

Notes on the numbers:

- An earlier native run at load 7.4–9.3 (before the scatter query was fixed;
  the other paths unchanged) gave the full-frame hover at 924 / 951 µs
  (80 × 24) and 1,160 / 1,169 µs (320 × 120): about half the figures above.
  The machine's load roughly doubled the data-pass times; the per-event hover
  path stayed under 5 µs in every run.
- The hover path at 320 × 120 is dominated by copying the cached base
  (38,400 × 5 bytes) into the shown frame. Restoring only the cells the last
  overlay touched would make it O(rows + cols); not measured.
- Cells changed per hover depend on how far the pointer moves: the 80 × 24
  sweep moved 0.036 cells an event, the 320 × 120 sweep 0.155. When the
  crosshair moves by a column and a row it changes about rows + cols + the
  readout's width.
- The full-frame cost is the 1M-point min/max pass per frame. data.md's
  downsampling (a cached or pyramided min/max per view) is what lowers pan and
  zoom; nothing here lowers it.

At 60 Hz a hover costs at most 0.03 % of the frame and a pan at 1M visible
points 10–14 %. In a browser the DOM event and the canvas repaint of the
changed cells come on top **(unverified: not measured here)**.

## Recommendation

1. **View state is the model.** One serialisable object (above) per chart,
   gettable and settable as the `view` property and attribute, carried in
   `plot-view`, round-tripping through JSON. A frame is a pure function of
   data, options, view and `t`.
2. **The reducer is in the core, in Rust.** `Chart::input(&mut View, &Input,
   &Ctx) -> Changes` (with `Changes` the overlay/base/events bitmask and the
   events out), used by wasm and by the CLI. The shell converts pixels to
   cells, recognises gestures, coalesces moves to one call per animation frame
   and dispatches events. No input log in the core; the shell records
   `(t, view)` changes for exports.
3. **Two layers in the frame.** A base (data, axes, legend) cached while data,
   options, size and view domains hold; an overlay (crosshair, readout,
   brush, time cursor, focus) composed over it per input. Hover never redraws
   the base. The overlay is drawn in wasm into the same grid, so a `<pre>`, the
   canvas, the terminal and the exports show the same thing.
4. **Hit testing by chart kind**: binary search on sorted x; O(1) for bins;
   for scatter, a spatial index built lazily on the first hover after the view
   settles, with a linear scan answering until it exists. A data-space index
   built once per data change would avoid rebuilding on pan; measure it when
   scatter lands.
5. **Numeric calls on the hot path**, JSON for the view and the events out.
6. **Events**: `plot-hover`, `plot-select`, `plot-brush`, `plot-view`,
   `plot-legend`, `plot-cursor`; reader actions only; `final` on gestures.
7. **Input defaults**: drag pans; Shift+drag brushes; Ctrl/⌘+wheel and pinch
   zoom; plain wheel scrolls the page; one finger scrubs; two fingers pan;
   arrows step datum by datum. Options change drag and wheel (open questions).
8. **Accessibility from the first chart**: focusable, keyboard-complete, a
   hidden table of the view, a live region for keyboard moves.

## Requirements to propose

Milestones follow the brief's phase order: v0.1.0 the first chart (line) end
to end with pan, zoom and crosshair; v0.2.0 the contract and budgets; v0.3.0
candlestick, bar and sparkline; v0.4.0 streaming; v0.5.0 the site; v0.6.0 the
CLI. The synthesis may renumber.

- REQ-VIEW-010: Every chart MUST hold its interactive state in one view state of the shape in `spec/schemas/view.schema.json`, and its frame MUST be a function of data, options, view state and `t` only: two charts given the same four MUST produce identical grids. (v0.1.0)
- REQ-VIEW-020: Every domain in the view state MUST be in data units; `null` MUST mean the chart's fitted default. (v0.1.0)
- REQ-VIEW-030: The element MUST expose `view` as a property (get: the current view; set: a full or partial view, merged, `null` keys reset) and as an attribute of JSON text; `JSON.parse(JSON.stringify(el.view))` set back MUST reproduce the same frame. (v0.1.0)
- REQ-VIEW-040: Setting `view` or `time` from the page MUST NOT fire any `plot-*` event. (v0.1.0)
- REQ-VIEW-050: A view animation (eased zoom, kinetic pan, reset) MUST be a `motion` record in the view state stamped with play time, so the frame at any `t` is determined without replaying input; with reduced motion and no `motion` attribute there MUST be none. (v0.1.0)
- REQ-VIEW-060: With `x.follow` true, an append MUST keep the x span and move the domain to end at the newest datum; a pan away from the end MUST set it false. (v0.4.0)
- REQ-INPUT-010: The core MUST fold input into view state with one reducer, `Chart::input(view, event, ctx)`, used by the wasm module and the CLI alike; it MUST read no clock and keep no state outside the view and the chart's caches. (v0.1.0)
- REQ-INPUT-020: Input events MUST have the shape of `spec/schemas/input.schema.json`, with positions in cells and fractions inside a cell; the wasm boundary MUST accept them as a numeric call returning a bitmask of overlay changed, base changed and events pending. (v0.1.0)
- REQ-INPUT-030: The shell MUST send at most one `move` per animation frame (the latest) and MUST send `down`, `up`, `leave`, `tap`, `key` and summed `wheel` deltas in order before it. (v0.1.0)
- REQ-INPUT-040: The contract MUST replay a scripted input list against every chart (a hover sweep, a drag, a wheel zoom, a key sequence) and check that two fresh charts reach the same view and the same frame, and that the resulting view, set on a third, gives that frame. (v0.2.0)
- REQ-HIT-010: For x-ordered charts the cursor MUST snap to the datum nearest the pointer's dot column by binary search on x; for binned charts to the bin under the pointer; tests MUST check both against a brute-force answer over fixture data. (v0.1.0 line; v0.3.0 bars, candles)
- REQ-HIT-020: When a dot column holds more than one datum, `plot-hover` MUST carry the column's `aggregate` (count, first, last, min, max). (v0.1.0)
- REQ-HIT-030: The scatter chart MUST answer a hover over 1M points in under 0.1 ms median natively once its index is built, and MUST answer (by scan) before it is. (scatter's milestone)
- REQ-OVERLAY-010: A hover that does not change the view MUST NOT redraw the base layer; the bench's check is that a hover event at 320 × 120 with 1M points costs under 0.1 ms median natively (measured 4.9 µs). (v0.2.0)
- REQ-OVERLAY-020: The crosshair, readout, brush and time cursor MUST be drawn into the grid in wasm, so the `<pre>`, canvas, terminal and exports show them the same. (v0.1.0)
- REQ-EVENT-010: The element MUST dispatch `plot-hover`, `plot-select`, `plot-brush`, `plot-view`, `plot-legend` and `plot-cursor` as bubbling, composed `CustomEvent`s with `detail` of the shapes in `spec/schemas/events.schema.json`, each with `source`. (v0.1.0; `plot-brush` and `plot-legend` by v0.3.0)
- REQ-EVENT-020: `plot-hover` MUST fire only when the hovered datum changes, at most once an animation frame, with `null` when the cursor leaves. (v0.1.0)
- REQ-EVENT-030: `plot-view` MUST fire at most once an animation frame during a gesture with `final: false` and exactly once with `final: true` when the gesture ends. (v0.1.0)
- REQ-LINK-010: Charts sharing a `link` attribute value MUST share `x.domain` and `cursor.x` in data units, without event loops; a Playwright test MUST show two linked charts of different widths agreeing after a pan. (v0.3.0)
- REQ-KEY-010: The drawn element MUST be focusable and every interaction (cursor, pan, zoom, reset, brush, select, toggle series) MUST be reachable by the keys listed in the spec's keyboard table, acting only while it has focus. (v0.1.0)
- REQ-TOUCH-010: On touch, one-finger horizontal drag MUST move the cursor, two fingers MUST pan, pinch MUST zoom about its centre, double-tap MUST reset, and vertical one-finger drags MUST scroll the page (`touch-action: pan-y`). (v0.1.0)
- REQ-TOUCH-020: Plain wheel MUST scroll the page; Ctrl or ⌘ with the wheel, and trackpad pinch, MUST zoom about the pointer, unless the `wheel` option says otherwise. (v0.1.0)
- REQ-A11Y-010: The drawn element MUST have `role="graphics-document"`, `aria-roledescription="chart"`, an accessible name and a description of type, series, ranges and count. (v0.1.0)
- REQ-A11Y-020: The element MUST keep a visually hidden table of the current view's data, at most 100 rows (aggregated per bucket), rebuilt on final view changes; axe MUST find no serious issue. (v0.1.0)
- REQ-A11Y-030: A polite live region MUST announce the readout when the cursor moves by keyboard, at most every 250 ms, and MUST NOT announce pointer hover. (v0.1.0)
- REQ-TERM-010: The CLI MUST map crossterm mouse events (whole cells, at the cell's centre) and keys to the same input events and reducer as the browser. (v0.6.0)

## Decisions with a real alternative

### View state is the model; no input log in the core

- Context: the brief makes view state an input of the frame; unicel's ADR-114
  makes an input log the input of the frame.
- Options: (a) view state, reduced from input; (b) an input log replayed each
  frame; (c) both: a log, with the view as a cache.
- Recommendation: (a). A chart's interaction is durable (a zoom must outlive
  512 events) and settable by the page; a log is neither. Exports record
  `(t, view)` changes in the shell.

### Where input is reduced

- Context: hit testing and snapping need data, scales and layout.
- Options: (a) in wasm, one Rust reducer shared with the CLI; (b) in the TS
  shell, with `set_view` into wasm; (c) split: pan and zoom in TS, hit testing
  in wasm.
- Recommendation: (a). One implementation, testable natively, used by the
  terminal; the shell stays small. Measured crossing cost: 0.03–0.05 µs.

### Overlay in wasm versus a DOM overlay

- Context: the crosshair and the readout change at pointer rate.
- Options: (a) composed into the grid in wasm over a cached base; (b) drawn by
  the shell as DOM or a second canvas over the chart.
- Recommendation: (a). Measured 0.6–5 µs an event, and the diffing painter
  redraws only the changed cells. The `<pre>`, the terminal, SSR text and
  exports show the crosshair without a second implementation. (b) would allow
  pixel-smooth lines but breaks the one-glyph-a-cell look.

### Numeric input calls versus JSON events

- Options: (a) numeric `input(kind, x, y, a, b, buttons, mods, t)`; (b) a JSON
  string an event; (c) a shared ring of events in wasm memory the shell writes.
- Recommendation: (a) for the hot path; JSON stays the canonical documented
  form and the form for tests and the CLI. (b) measured 0.4–0.5 µs more an
  event and pulls serde into every module (size is budgets.md's). (c) saves
  nothing measurable at one call a frame.

### Scatter hit index

- Options: (a) screen-space grid rebuilt per view (measured 4–10 ms build,
  0.6–21 µs query); (b) linear scan per hover (1.2–1.4 ms); (c) a data-space
  grid or k-d tree built once per data change (not measured).
- Recommendation: (c) when scatter is built, with (b) as the fallback; measure
  it then. (a) only if (c) loses.

### One-finger touch: scrub or pan

- Options: (a) one finger scrubs, two pan (the owner's test); (b) one finger
  pans and a long press enters scrubbing (Lightweight Charts' default).
- Recommendation: (a), with an option for (b).

## Open questions for the owner

1. **Mouse drag: pan or brush?** Options: drag pans and Shift+drag brushes
   (Lightweight Charts' convention, market charts); drag brushes and zooms to
   the brush (uPlot's default, analytics). Recommendation: pan by default, an
   option `drag: "pan" | "brush" | "zoom"`.
2. **Plain wheel: page scroll or chart zoom?** Options: page scroll, zoom with
   Ctrl/⌘ (no scroll trap); plain wheel zooms (Lightweight Charts' default).
   Recommendation: page scroll by default; option `wheel: "zoom"` for
   full-screen dashboards.
3. **Arrow keys: step the cursor or pan?** surfaces.md proposes ← → pan in the
   terminal. Recommendation: plain arrows step datum by datum everywhere
   (screen-reader users need it); Shift+arrows pan.
4. **Does the element ship a `link` attribute** or leave linking to pages and
   the `view` property? Recommendation: ship `link`; it is small.
5. **Touch Events fallback for old Safari?** Depends on the oldest Safari
   supported. Recommendation: Pointer Events only, unless the owner names a
   Safari older than Pointer Events support.
6. **Is the readout in the grid, or a DOM tooltip, or both?** Recommendation:
   in the grid (one row, or beside the cursor) by default; `plot-hover` lets a
   page draw its own; option to hide the in-grid one.

## Sources

- uPlot, https://github.com/leeoniya/uPlot at `7187ad7`: `src/uPlot.js`
  (lines 2536-2542, 3039-3075, 3611-3618, 3654-3740, 3975-3998),
  `src/utils.js:2-21`, `src/sync.js`, `src/opts.js:470-530`,
  `demos/zoom-wheel.html`, `demos/zoom-touch.html`.
- Lightweight Charts, https://github.com/tradingview/lightweight-charts at
  `9db7ce7`: `src/gui/mouse-event-handler.ts:71-81,536-655`,
  `src/gui/chart-widget.ts:569-620`, `src/gui/pane-widget.ts:38-43,304-394,814-847`,
  `src/model/crosshair.ts:42-136`, `src/model/kinetic-animation.ts`,
  `src/model/chart-model.ts:228-258`,
  `src/api/options/chart-options-defaults.ts:38-62`.
- Vega, https://github.com/vega/vega at `fdd9ab9`:
  `packages/vega-event-selector/src/event-selector.js`, `packages/vega/test/specs-valid/map.vg.json:45`,
  `packages/vega-scenegraph/src/CanvasHandler.js:15-40,156-168`,
  `packages/vega-selections/src/selectionTest.js:1-40`,
  `packages/vega-selections/src/selectionResolve.js`,
  `packages/vega-view/src/state.js:20-48`, `packages/vega-view/src/aria.js:1-16`.
- crossterm, https://github.com/crossterm-rs/crossterm at `ab8681d`:
  `src/event/commands.rs:40-62`, `src/event/mouse.rs:24-33,73-90`.
- ratatui, https://github.com/ratatui/ratatui at `0297560`:
  `ratatui-widgets/src/chart.rs:44,75`,
  `examples/apps/custom-widget/src/main.rs:226-257`.
- unicel (`/Users/curtisault/projects/animate-unicode`): `spec/ROADMAP.md`
  §5.1 (REQ-PIECE-503/504, REQ-WASM-501, REQ-EL-503, REQ-INPUT-001–003,
  REQ-CONTRACT-502), `spec/adr/114-input-log.md`,
  `spec/schemas/input-log.schema.json`, `spec/SPEC.md` §6.1, §7, §8.2
  (REQ-MOUNT-060), `crates/wasm/src/lib.rs`, `crates/unicel/src/piece.rs`,
  `crates/unicel/src/subcell.rs:1-60`.
- Chrome, "Aligned input events",
  https://developer.chrome.com/blog/aligning-input-events (fetched
  2026-10-18).
- MDN, `PointerEvent.getCoalescedEvents()`,
  https://developer.mozilla.org/en-US/docs/Web/API/PointerEvent/getCoalescedEvents
  (fetched 2026-10-18); `touch-action`,
  https://developer.mozilla.org/en-US/docs/Web/CSS/touch-action (not fetched;
  the `pan-y` behaviour is from it).
- glyphplot discovery: `discovery/surfaces.md` (event naming rule, element
  properties, terminal keys), `discovery/budgets.md` (pointer-move figure).
- Measurements: `discovery/bench/interaction/` (`src/lib.rs`, `src/main.rs`,
  `node/bench.mjs`, `README.md`), run 2026-10-18 as described under
  Measurements.
