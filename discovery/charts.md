# Chart taxonomy and prior art

## Summary

glyphplot should ship charts as families, each family one Cargo feature and
one wasm module, with the variants inside a family chosen by a `mode` option:
`line` (line, step, area, band, baseline), `candlestick` (candle, hollow,
OHLC bar, Heikin-Ashi, later Renko), `bar` (vertical, horizontal, grouped,
stacked, histogram, waterfall), `sparkline`, `scatter` and `heatmap` make the
first six modules, and about a dozen more cover the long tail. The prior art
splits the same way: ratatui, textplots, uPlot and Lightweight Charts each
draw line, step, area and bars from one series model with a type switch.
Every line-like chart must reduce its visible points to first, last, minimum
and maximum per dot column before it rasterises: on this machine that drew
1,000,000 points of uniform noise into a 320 × 120 braille plot in 3.5–4.5 ms,
against 325–420 ms for drawing every segment (ratatui's way, and plotext's),
and the frames were identical cell for cell. Because a cell has one colour,
two candles can never share a cell, so the candlestick chart picks its glyphs
from the candle spacing and merges candles in powers of two below one cell
each, as Lightweight Charts conflates bars below one pixel. Of unicel's three
data pieces, the glyph vocabulary, the one-ink rules, the eighth-block bar
ends, the dashed gridlines and the ring buffer carry over; the seeded
generators, the fixed sizes, the fixed four-column candles and the
row-quantised prices do not.

## Scope and method

- Read the owner's brief (`README.md`, "The briefs", topic 1), unicel's three data pieces
  (`../animate-unicode/crates/unicel/src/pieces/data/{candlesticks,bar_chart,sparkline}.rs`),
  its sub-cell canvases (`crates/unicel/src/subcell.rs`), grid
  (`crates/unicel/src/grid.rs`), piece contract (`crates/unicel/src/piece.rs`)
  and the module-per-category decision in `docs/IMPLEMENTATION_PLAN.md` line 65.
- Shallow-cloned and read the code of 19 repositories (commits in
  [Sources](#sources)); fetched two files each from matplotlib and bokeh.
  Depth went to uPlot, Lightweight Charts, ratatui and plotext, as briefed.
- Built a throwaway bench, `discovery/bench/charts/` (see its README), that
  times ratatui-widgets 0.3.2's `Chart`, a plain per-segment rasteriser and a
  per-dot-column min/max pass on the same data, and prints candle glyph
  samples. Timed plotext 6.1.0 on the same sizes from Python.
- Not covered: Highcharts, Plotly, Apache Superset and Grafana panels; wasm
  timings (agent 6 measures them); module sizes per family (agent 6); layout
  algorithms for Sankey, treemap and force graphs beyond naming them; the
  celestial charts' maths (agents 2 and 7).

## Chart taxonomy

Glyph families, as unicel's `subcell.rs` lines 3–8 names them, plus the
blocks: braille 2 × 4 dots (U+2800–28FF); octants 2 × 4 (U+1CD00–1CDE5,
Unicode 16); sextants 2 × 3 (U+1FB00–1FB3B); quadrants 2 × 2 (U+2596–259F);
lower eighths `▁`–`█` (U+2581–2588) and left eighths `▏`–`▉` (U+2589–258F)
for bar ends at an eighth of a cell; upper eighths `▔ 🮂 🮃 ▀ 🮄 🮅 🮆`
(U+2594, U+1FB82–1FB86) for ends that hang from the top; the half block `▀`
with ink and back colours for two values a cell (unicel's grid has a back
colour per cell, `grid.rs` lines 17 and 188); shades `░ ▒ ▓`; box drawing for
axes, frames and hollow shapes; geometric shapes (`● ◆ ▲ ■ ▪ ·`) for markers;
arrows for annotations.

Data shapes are named here only as far as a chart needs them; agent 2 owns the
data contract. "x-sorted" means the chart may binary-search its x column.

| Chart | Tier | What it draws | Best glyphs | Data shape | Interactions | Module, mode |
|---|---|---|---|---|---|---|
| Line | 1 | One or more series as polylines over a continuous x | braille | x column (f64 or timestamp, x-sorted) and one y column per series; NaN or null breaks the line | pan, zoom, x-crosshair snapped to the nearest datum, readout of every series at that x, legend toggle, brush an x range | `line`, mode `line` |
| Step (stairs) | 1 | A line that holds its value until the next x | braille, or box drawing `─│┌┐└┘` at one row a step | as line | as line | `line`, mode `step` (`before`, `after`, `middle`) |
| Area | 1 | A line filled to a baseline or stacked on the series below | braille line over octant or sextant fill (or braille fill as unicel's sparkline) | as line; stacking needs a shared x | as line; hover names the band under the pointer | `line`, mode `area` (`stack: none, stack, percent`) |
| Range and band | 2 | The area between two series (min/max, confidence, Bollinger) | octant fill, braille edges | x and two y columns | as line | `line`, mode `band` |
| Baseline | 2 | A line coloured above and below a value (Lightweight Charts' baseline series) | braille | as line, plus a base value | as line | `line`, mode `baseline` |
| Depth chart | 2 | Cumulative bid and ask volume against price, two stepped areas meeting at the spread | braille step lines over octant fill | two sorted price/size columns | hover price level | `line`, a preset of `step` + `area` (markets pack) |
| Candlestick | 1 | Open, high, low and close a period: a wick and a body, rising and falling in two inks | box drawing at wide spacing; braille (wick one dot column, body two) at narrow spacing; merged in powers of two below one cell | time, open, high, low, close (and volume); x-sorted; the last candle updates | pan, zoom by candle spacing, crosshair magnet to O/H/L/C, readout `o h l c`, scrub, live last candle | `candlestick`, mode `candle` |
| Hollow candle | 1 | Rising candles hollow, falling ones solid (unicel's default) | box drawing `┌┴┐ │ │ └┬┘`, `███` | as candlestick | as candlestick | `candlestick`, mode `hollow` |
| OHLC bar | 2 | A vertical high-low bar with ticks left (open) and right (close) | box drawing `├ ┤ │` | as candlestick | as candlestick | `candlestick`, mode `ohlc` |
| Heikin-Ashi | 2 | Candles of averaged OHLC | as candle | as candlestick; the chart transforms | as candlestick | `candlestick`, mode `heikin-ashi` |
| Renko, Kagi, point and figure | long tail | Fixed-size bricks or reversals, x is brick count, not time | box drawing, blocks | close price column, brick size | hover a brick shows its time span | `candlestick`, mode `renko` (a transform plus boxes) |
| Bar | 1 | One bar a category, vertical or horizontal | lower eighths (vertical), left eighths (horizontal), `█` | category column and one value column per series | hover a bar, click to select, legend toggle | `bar`, mode `bar` |
| Grouped and stacked bar | 1 | Bars side by side or on top of each other a category | eighths; stacked segments end on eighths only at the top | category and several value columns | hover a segment | `bar`, options `group`, `stack` |
| Histogram | 1 | Counts of a continuous value in bins | eighths | unbinned values (the chart bins) or bin edges and counts | hover a bin shows its range and count, brush a range | `bar`, mode `histogram` |
| Waterfall | 2 | Floating bars from running totals | eighths at both ends (lower and upper eighths) | category, signed delta | hover | `bar`, mode `waterfall` |
| Sparkline | 1 | A small line, area or column series with no axes, often many in a table | braille (two samples a cell, eight levels in two rows, as unicel's) | one value column per series, a ring buffer | hover a sample, live append | `sparkline`, modes `line`, `area`, `column`, `winloss` |
| Horizon | long tail | An area folded into bands and overlaid, for many series in little height | lower eighths with shaded inks | as sparkline, plus band count | hover | `sparkline`, mode `horizon` |
| Scatter | 1 | Points at (x, y), optionally sized and coloured by value | braille dots; geometric shapes per category in one ink | x, y, optional size, colour, category; unsorted | hover nearest point within a radius, brush a rectangle, click to select | `scatter`, mode `point` |
| Bubble | 2 | Scatter with size | `· • ● ⬤` by size | as scatter | as scatter | `scatter`, mode `bubble` |
| Density (2-D histogram) | 2 | Counts of points per cell | shades, colour scale | as scatter | hover a cell's count | `heatmap`, mode `bin2d` |
| Heatmap | 1 | A matrix of values as colour | `█` with colour, or `▀` with ink and back for two rows a cell; shades `░▒▓█` or unicel's `· ▪ ◼ ■ █` in one ink | rows × columns of f64, or x, y, value triples; categorical or binned axes | hover a cell, brush rows and columns | `heatmap`, mode `matrix` |
| Calendar heatmap | 2 | Days in weeks across, weekdays down (unicel's heatmap piece) | as heatmap | date, value | hover a day | `heatmap`, mode `calendar` |
| Spectrogram (streaming heatmap) | 2 | A heatmap whose columns arrive over time | as heatmap | time, bins | scrub, hover | `heatmap`, mode `matrix` with append |
| Box | 2 | Quartiles, whiskers and outliers by category | box drawing `┌─┐ ├ ┤`, `·` outliers | category, values (or precomputed quartiles) | hover shows the five numbers | `distribution`, mode `box` |
| Violin | long tail | A mirrored density per category | braille or octant fill | category, values | hover | `distribution`, mode `violin` |
| Strip, beeswarm | long tail | Every value as a dot by category | braille, `●` | category, values | hover a point | `distribution`, mode `strip` |
| Ridgeline | long tail | Overlapping density areas, one row a category | braille edges, octant fill | category, values | hover | `distribution`, mode `ridgeline` |
| Gauge | 2 | A dial or arc up to a value (unicel's gauge piece) | braille arc and needle | one value, range, zones | none beyond a readout; keyboard focus | `gauge`, mode `radial` |
| Bullet, meter | 2 | A horizontal bar against a target and bands | left eighths, `▕` for the target | value, target, bands | hover | `gauge`, mode `bullet` |
| Radar | long tail | A polygon per series over radial category axes | braille lines | categories × series values | hover a vertex | `polar`, mode `radar` |
| Polar line, rose | long tail | Values against an angle | braille | angle, radius | hover | `polar`, mode `line`, `rose` |
| Pie, donut | long tail | Shares as arcs | braille or octant arcs | category, value | hover a slice | `polar`, mode `pie` (the bar chart is usually the better choice) |
| Gantt | long tail | Tasks as bars from start to end on a time axis, with dependencies | left eighths at both ends, box-drawing arrows | task, start, end, dependency | pan, zoom on time, hover a task | `timeline`, mode `gantt` |
| Timeline | long tail | Events and spans on a time axis with labels | `●`, `├─┤`, text | time or start/end, label | pan, zoom, hover, scrub | `timeline`, mode `events` |
| Waffle, dot matrix | long tail | Unit squares coloured by category | `■`, `█` | category, count | hover a category | `waffle` |
| Treemap | long tail | Nested rectangles sized by value | box drawing borders, text, colour fill | hierarchy (id, parent, value) | hover, click to zoom into a node | `hierarchy`, mode `treemap` (`icicle` the other) |
| Sankey | long tail | Flows between node columns, width by value | braille or octant bands, `█` nodes | nodes, links with weights | hover a link or node, highlight its path | `flow`, mode `sankey` |
| Chord | long tail | Flows between categories around a circle | braille arcs and ribbons | square matrix | hover | `flow`, mode `chord` |
| Network, force graph | long tail | Nodes and edges, laid out by force, circle or tree | braille edges, `●` nodes, labels | nodes, edges, weights | drag a node, hover, click to select, pan, zoom | `graph`, modes `force`, `circle`, `tree` |
| Parallel coordinates | long tail | One polyline a row across k vertical axes | braille | rows × k numeric columns | brush on each axis, hover a line | `parallel` |
| Sky chart, horizon view | long tail | Stars and planets by RA and Dec (or alt and az) in a projection | braille grid lines, `· • ●` by magnitude, labels | RA, Dec (or alt, az), magnitude, name | pan, zoom, hover a star, time scrub | `sky`, modes `equatorial`, `horizon` |
| Orbit plot | long tail | Orbits as ellipses and bodies at positions in a plane, equal aspect | braille | positions x, y (computed elsewhere) and orbit paths | zoom, hover a body, time scrub | `scatter` with `aspect: equal` and line paths; the positions come from the page or a domain pack |

Notes on the table:

- Tier 1 is the owner's three plus the obvious next five. Tier 2 is what the
  first two domains need (markets: OHLC, band, depth; telemetry: box, gauge,
  calendar). The long tail is the brief's list.
- A mode is put in a family when it shares the family's scales, data shape
  and interaction and differs only in the marks it draws. A chart is its own
  family when it needs its own layout (treemap, Sankey, force) or its own
  coordinate system (polar, sky).
- Depth chart and orbit plot need no module of their own: each is a preset
  of an existing family, and the preset belongs in a domain pack (agent 7).

### Module boundaries

The core (always linked) holds what two or more families use: the grid and
sub-cell canvases, scales and ticks, number and date formatting, axes,
layout, legend, view state, input, the crosshair and readout, the
per-dot-column reduction, the ring buffer, and the marks every family draws
with (polyline, area fill, rect with eighth ends, point, rule, band, text).
A family module holds its own layout, transforms (binning, Heikin-Ashi,
Renko, quartiles, force simulation) and the glyph choices peculiar to it.
Never one family's code in another, as unicel's `AGENTS.md` rule for pieces.

| Cargo feature, wasm module | Modes | Milestone (proposed) |
|---|---|---|
| `line` | line, step, area | v0.1.0 |
| `line` | band, baseline | v0.3.0 |
| `candlestick` | candle, hollow, ohlc; volume pane drawn with the core's rect mark | v0.2.0 |
| `candlestick` | heikin-ashi | v0.3.0; renko later |
| `bar` | bar (vertical, horizontal, grouped, stacked) | v0.2.0 |
| `bar` | histogram | v0.3.0; waterfall v0.4.0 |
| `sparkline` | line, area, column | v0.2.0; winloss, horizon later |
| `scatter` | point | v0.3.0; bubble v0.4.0 |
| `heatmap` | matrix | v0.3.0; calendar, bin2d v0.4.0 |
| `distribution` | box | v0.4.0; violin, strip, ridgeline later |
| `gauge` | radial, bullet | v0.4.0 |
| `timeline`, `polar`, `hierarchy`, `flow`, `graph`, `parallel`, `sky`, `waffle` | as the table | after v0.4.0, one milestone each when a domain asks for it |

The milestone numbers assume v0.1.0 is the first chart end to end with the
contract, v0.2.0 the owner's three named charts with streaming, v0.3.0 the
next tier; the synthesis may renumber them.

The case for families over one module a chart name: unicel's split by
category repeats its ~33 KB gzipped base in every module, so "a page showing
every category loads 539 KB in all, not 249"
(`../animate-unicode/docs/IMPLEMENTATION_PLAN.md` line 65). glyphplot's core
is heavier than unicel's (scales, date formatting, input), so the repeated
base costs more a module (agent 6 has the number). The brief's list counts
about 45 chart names; as families it is 18 modules, six at the first tier. A
mode costs only its own marks and transform inside a module that already
holds the family's scales and layout.

## What carries over from unicel's data pieces

| Piece | Carries over | Does not |
|---|---|---|
| `candlesticks.rs` | The glyph vocabulary: hollow rising candles `┌┴┐ │ │ └┬┘`, solid falling ones `███`, doji `├┼┤` (lines 160–184). The last price as a dotted `┄` line to the axis with its label `├ 101.25` (lines 144–150). The one-ink rule that shape, not colour, says rising or falling (lines 6–9). The header readout `o h l c` and the signed change (lines 192–197). The forming candle in a brighter shade (lines 22–26, 158–163). | The seeded market (`Mulberry32` paths, lines 61–97): the data is the page's. The fixed 70 × 22 size and 15 slots of four columns (lines 21–22, 37–44): size comes from the element, spacing from the zoom. Prices rounded to one row (`row()`, line 129): a body end lands at a quarter row in braille or an eighth with blocks. The fixed `STEPS` list for the price step (line 45) and the hard-coded `09:30 + 5 min` clock (lines 106–109): scales and time formatting are the core's. |
| `bar_chart.rs` | Bar ends at an eighth of a row with `▁`–`▇` (lines 36, 149–160). Dashed `┈` gridlines that keep a cell clear of each bar and its value (lines 163–171). Values on top of bars (lines 172–175). The live dataset lit in the legend, in brackets in one ink (lines 5–9, 137–144). Easing between datasets (lines 124–133), as a transition on data replace keyed to `t`. | `nice_step` with four bands (lines 46–50): the core's tick generator replaces it. The layout fixed to `META.cols` and a bar width of four (lines 40, 83–96). Labels cut to the bar's width by `char` count (line 117), which is wrong for wide glyphs. Datasets parsed from an options JSON value (lines 67–79): fine for small data only. |
| `sparkline.rs` | A braille area two rows tall, two samples a cell, eight levels (lines 5–9, 185–191). A ring buffer a series (lines 103, 117–128), which becomes the streaming store. The label, latest value and min/max columns (lines 192–198) and the `Ns ago … now` axis (lines 175–177). In one ink each series keeps its own rows (lines 9–10). | The seeded signals (lines 62–93) and `fill()` replaying from zero on a backwards seek (lines 117–120): with injected data the ring is the data, not a cache of a generator. At most three series (line 41). A default scale of 0–100 (line 180). The `+1` that shows a zero as one dot (line 187) should become an option, since it makes zero and the floor look alike. |

Also from unicel: `ColorCanvas::cell_color` gives a cell the colour most of
its dots have (`subcell.rs` lines 628–651), and `draw` can draw only the dots
of that colour (`subcell.rs` lines 667–680). For charts, a z-order rule (the
series drawn last, or the hovered one, wins) is easier to predict than a
majority; see [Decisions](#decisions-with-a-real-alternative).

## Candle glyphs and spacing

One colour a cell means two candles can never share a cell: a rising and a
falling candle in one cell would have to share an ink. So below one cell a
candle, candles must be merged. Lightweight Charts does the same below one
device pixel: it merges bars in powers of two from 2 to 512
(`src/model/conflation/constants.ts` lines 5–17), choosing the level from
the bar spacing (`src/model/data-conflater.ts`, `calculateConflationLevelWithSmoothing`).
Its zoom is the bar spacing (default 6 px, minimum 0.5 px,
`src/api/options/time-scale-options-defaults.ts` lines 4–6) and the candle
width is a function of the spacing (`src/renderers/optimal-bar-width.ts`
lines 5–14).

The bench prints two narrow styles for the same 30 candles
(`cargo run --release -- candles`, see the bench README). One candle every
other cell, body ends at an eighth of a row:

```
  ▆ │
  █ ▅ ▅ │                       │
  🮂 🮂 █ │   │   │ │       │ │ │ │
  │ │ ▔ ▇ │ ▂ ▂ ▂ ▂   │   ▅ ▂ ▄ ▂ ▅       │   │ │ │
        🮅 ▂ █ │ │ █ │ │ ▁ 🮆 │ │   █ │ ▅ ▅ ▁ ▁ │ │ │
          │ │   │ ▀ ▀ █ █ │       🮂 ▁ 🮃 🮆 🮆 🮃 ▁ ▁ ▆
            │     │ │ 🮂 🮂 │       │ │ │ │ │ │ │   🮆 ▁ │     │
                      │ │               │           █ │ ▂ ▁ ▃
                        │                           ▀ ▀ │ │ █
                                                    │ │   │ │
```

One candle every other cell, braille, wick in the left dot column and body
in both:

```
  ⣷ ⡇
  ⣿ ⣷ ⣷ ⡆                       ⡆
  ⡟ ⡟ ⣿ ⡇   ⡆   ⡇ ⡀       ⡇ ⡆ ⡆ ⡇
  ⡇ ⠁ ⡏ ⣿ ⡄ ⣇ ⣇ ⣇ ⣇   ⡀   ⣷ ⡷ ⡿ ⠟ ⣷       ⡆   ⡄ ⡄ ⡀
  ⠁     ⣿ ⣇ ⣿ ⠁ ⡇ ⣿ ⡄ ⡇ ⣄ ⣿ ⡇ ⡇   ⣿ ⡇ ⣷ ⣶ ⣇ ⣀ ⡇ ⡇ ⡇
        ⠁ ⡏ ⡏   ⠇ ⡟ ⡟ ⣿ ⣿ ⡇ ⠁     ⡟ ⡗ ⡟ ⣿ ⣿ ⡟ ⡗ ⠟ ⣿
            ⡇     ⠁ ⠃ ⡏ ⡏ ⠃       ⠃ ⠁ ⠃ ⡇ ⡇ ⡇ ⡇   ⣿ ⣆ ⡄     ⡆
                      ⡇ ⡇               ⠁           ⣿ ⣇ ⣦ ⡧ ⣧
                        ⠁                           ⡟ ⡟ ⠁ ⡇ ⣿
                                                    ⠁ ⡇   ⠃ ⠇
```

What the samples show:

- The eighths style hides the wick in any cell the body ends in, and a body
  that starts and ends inside one cell has no glyph (there is no "middle
  eighths" block); the sample snaps it to an edge, which is wrong.
- The braille style keeps wick and body in every cell at a quarter row and
  has no such case. It reads as candles only with the gap cell; the same
  candles drawn in adjacent cells (the bench's first sample, not shown) run
  together.
- Neither narrow style can show hollow versus solid, so neither says rising
  or falling in one ink. That is an open question for the owner.

Proposed rule: at four or more cells a candle, unicel's box-drawing candles;
at two or three cells, braille with a gap; at one cell, braille without a
gap; below one cell, merge in powers of two until each merged candle has at
least one cell.

## Prior art

Each entry names the code read. Commits are in [Sources](#sources).

### uPlot (JavaScript, Canvas 2D)

- Data is columnar: `data[0]` is the x array and `data[i]` series i
  (`src/uPlot.js` lines 1571–1610). There is no append API: `setData`
  replaces the arrays and, unless told not to, rescales (lines 1602–1610).
  Its README says it can "live-stream data … at 60fps, though it may begin to
  struggle beyond 100k in-view points" (`README.md` line 10).
- The visible window is found by binary search, `i0 = closestIdx(min, data[0])`
  (`src/uPlot.js` lines 1776–1777, `src/utils.js` lines 2–21), so drawing cost
  follows the points in view, not the points held.
- The line path builder keeps, for each pixel column, the first, last, min
  and max y and draws one vertical stroke a column
  (`src/paths/linear.js` lines 5–14 and 59–119). This is the per-column
  reduction this report recommends, done inline while building the path.
- Gaps: a `null` y breaks the line (`src/paths/utils.js` lines 196–215);
  `undefined` is an alignment artefact and is skipped by the hover
  (`src/uPlot.js` lines 1111–1119).
- Pointer to datum: `posToVal` inverts the scale (lines 3039–3069),
  `posToIdx` binary-searches the x array (lines 3072–3075). The hover can
  then move to a nearby non-null value within a proximity
  (`cursor.dataIdx`, lines 1111–1135).
- Linked charts share a cursor through a keyed pub/sub registry
  (`src/sync.js` lines 1–26).
- Right: columnar arrays, window by binary search, per-pixel-column reduction,
  nulls as gaps, tiny size (its README claims ~50 KB minified, line 3, **(unverified)**).
  Wrong for glyphplot's purpose: no incremental append; a 1-million-point
  stream means replacing arrays every frame.

### TradingView Lightweight Charts (TypeScript, Canvas 2D)

- Series types are area, bar (OHLC), baseline, candlestick, histogram, line
  and custom (`src/model/series/`), and a line has a `LineType` of `Simple`,
  `WithSteps` or `Curved` (`src/renderers/draw-line.ts` lines 11–24): step
  is a mode of line.
- The horizontal axis is a logical index, not time: a bar's x is
  `width − (baseIndex + rightOffset − index + 0.5) × barSpacing − 1`
  (`src/model/time-scale.ts` lines 551–560) and `coordinateToIndex` inverts
  it (lines 575–585). Pan changes `rightOffset`, zoom changes `barSpacing`.
  Session gaps cost nothing because empty time is not on the axis.
- Streaming: `update(bar)` replaces the last bar when its time equals the
  last time and appends otherwise; an older time throws "Cannot update oldest
  data" (`src/model/data-layer.ts` lines 247–300, the throw at 262–264).
  `historicalUpdate` may change an existing older bar (lines 269–271).
- Large data: bars merge in powers of two when the spacing falls below one
  device pixel, cached per level (`src/model/data-conflater.ts`,
  `src/model/conflation/constants.ts` lines 5–17).
- Crosshair modes `Normal`, `Magnet` (snap to the close or value), `Hidden`
  and `MagnetOHLC` (snap to the nearest of O, H, L, C)
  (`src/model/crosshair.ts` lines 42–59).
- Input defaults: wheel and drag pan, wheel and pinch zoom, kinetic scroll
  (`src/api/options/chart-options-defaults.ts` lines 38–56).
- Right: index-based time axis, last-bar update, conflation by spacing,
  magnet crosshair. All four map onto cells directly (spacing in cells,
  conflation below one cell). Wrong for glyphplot: an append older than the
  last bar throws rather than inserting, so out-of-order feeds need the page
  to sort first.

### ratatui and tui-rs (Rust, terminal)

- `Chart` holds datasets of `&[(f64, f64)]` (`ratatui-widgets/src/chart.rs`
  lines 325–329, 371) with a `GraphType` of `Scatter`, `Line`, `Bar` or
  `Area` (lines 158–175): one widget, four modes. tui-rs, its unmaintained
  ancestor (`README.md` line 3), had only `Scatter` and `Line`
  (`src/widgets/chart.rs` lines 93–98).
- Rendering draws every point, then one canvas line for every pair of points
  (`chart.rs` lines 1049–1100). No reduction, no window: all points every
  frame. The bench measures what that costs.
- The canvas's pattern grids (braille, quadrant, sextant, octant) OR a dot
  into a cell's bit pattern and set the cell's colour to the last painter
  (`ratatui-widgets/src/canvas.rs` lines 140–219): last writer wins.
- Axis labels are given by the caller and spaced evenly; "giving more than 3
  labels is currently broken" (`chart.rs` lines 80–90). There is no tick
  generator and no time axis.
- `Sparkline` takes `Option<u64>` values, an absent value drawn with its own
  symbol (`sparkline.rs` lines 307–311): integers only. `BarChart` has groups
  and both directions (`barchart.rs` lines 83–108).
- Interaction: none; the widgets are immediate mode and stateless, and the
  application maps terminal mouse events itself.
- Right: one widget with modes; marker choice per dataset; absent values in
  sparklines. Wrong for glyphplot: no reduction, AoS input, caller-made
  labels, no interaction, integer sparklines.

### plotext 6.1.0 (Python with a C++ kernel, terminal)

- Version 6.1.0, committed 2026-09-07, moved drawing into a C++ kernel
  (`plotext/_kernel/cpp/`, 2,282 lines) behind a `figure.signal(...)` /
  `figure.draw(...)` / `figure.build()` API (`plotext/_plotter/plot.py`
  lines 210–236).
- `Signal::plot` densifies: every pair of connected points becomes all the
  points of the line between them (`_kernel/cpp/15_signal.cpp` lines
  128–152). No reduction.
- Markers include `hd` (quadrants), `fhd` (sextants) and `braille`, with
  `fhd` missing on Windows (`plotext/_constants/enums.py` lines 45–46).
- `hist` bins and then calls the bar drawer (`plotext/_plotter/draw.py`
  lines 253–256): histogram is a mode of bar. `candlestick` builds a segment
  and a rectangle a candle, one cell wide, with an `ohlc` style of `├─`
  and `─┤` ticks (lines 298–352).
- Ticks divide the range evenly instead of choosing round numbers: a
  1–1000 axis is labelled `1.0e0 1.7e2 3.3e2 5.0e2 6.7e2 8.3e2 1.0e3`
  (output of `figure.signal([sin(i/20) for i in range(1000)], marker="braille")`
  at 60 × 12, run on 2026-10-18).
- Interaction: none beyond reprinting (`interactive` mode reprints on every
  change, `_doc/sections/04_drawing.py`).
- Right: many chart kinds, candle and OHLC styles, box drawing frames.
  Wrong: no reduction (3.4 s for a million points, below), uneven ticks.

### textplots, asciichart, termgraph, YouPlot, gnuplot (terminal)

- textplots (Rust, on drawille): `Shape` is `Continuous`, `Points`,
  `Lines`, `Steps` or `Bars` (`src/lib.rs` lines 106–116), each drawn
  segment by segment (lines 463–545). Points below the y range are cast with
  `as u32`, which saturates to 0, so they are drawn on the bottom edge
  instead of clipped (lines 482–494). Data is `f32`.
- asciichart (JavaScript): one column a sample, width = the longest series
  plus the label offset (`asciichart.js` lines 63–67), joined with box
  drawing `╭╮╰╯│─` (lines 91–102). It reads well for short series and cannot
  scale x at all.
- termgraph (Python): horizontal bars of `▇` with `▏` for small values
  (`termgraph/constants.py` lines 15–16), stacked, vertical and calendar modes
  (`termgraph/args.py` lines 20–25).
- YouPlot (Ruby, UnicodePlot): bar, histogram, line, lines, scatter, density,
  box and count (`lib/youplot/backends/unicode_plot.rb` lines 15–191). Its
  `--progressive` mode re-parses one line, rebuilds and reprints the whole
  plot for every line read (`lib/youplot/command.rb` lines 66–88, 137–148),
  so a stream of n lines costs O(n²).
- gnuplot: the `dumb` terminal is one ASCII character a cell, 79 columns by
  default (`term/dumb.trm` line 113). The `block` terminal draws into a
  bitmap at sub-cell resolution and maps it to half blocks, quadrants,
  sextants, octants or braille (`term/block.trm` lines 68–72, 110–116), with
  mono, 16, 256 and truecolour output (lines 316–340). This is the same
  sub-cell approach as unicel's canvases.

### plotters (Rust)

- Each element and series kind is a Cargo feature: `candlestick`, `boxplot`,
  `errorbar`, `histogram`, `area_series`, `line_series`, `point_series`,
  `surface_series` (`plotters/Cargo.toml` lines 82–105). Prior art for one
  feature a chart kind, inside one crate.
- Interaction is limited to inverting the coordinate transform:
  `into_coord_trans` returns a closure from a backend pixel to a data value
  (`plotters/src/chart/context.rs` line 35, and a pair for dual axes in
  `dual_coord.rs` line 156).
- A text backend exists only as an example (`plotters/examples/console.rs`
  line 53).

### Observable Plot, Vega-Lite and Vega

- Observable Plot is a grammar of marks (`src/marks/`: area, bar, cell, dot,
  line, rect, rule, text, tick, box, bollinger, raster, contour, density,
  hexgrid, tree, waffle, …). Its pointer finds the nearest datum by a linear
  scan of every index with squashed distance (`pointerX` weights y by 0.01)
  inside a 40 px radius (`src/interactions/pointer.js` lines 9–10,
  138–157, 194–200). That is its only interaction (`src/interactions/` holds
  `pointer.js` alone): no pan, no zoom, no brush.
- Vega-Lite models interaction as selections: `point` (click, cleared by
  double-click) and `interval` (drag; translate by drag; zoom by wheel)
  (`src/selection.ts` lines 304–319). An interval bound to `scales` becomes
  pan and zoom (`src/compile/selection/scales.ts` lines 13–15). Nearest
  uses a Voronoi mark (`src/compile/selection/nearest.ts` lines 6, 48).
- Vega streams by changesets of insert, remove and modify
  (`packages/vega-dataflow/src/ChangeSet.js` lines 19–31) applied with
  `view.change` (`packages/vega-view/src/data.js` line 18).
- Right: interaction as data (a selection is a predicate the page can read
  and set); incremental changesets. Wrong for glyphplot: the runtime and
  grammar are far larger than one chart module should be.

### ECharts and Chart.js

- ECharts installs each chart type as its own export,
  `export {install as LineChart} from '../chart/line/install'`
  (`src/export/charts.ts` lines 27–28), so a bundler drops the rest: a
  module per chart type by tree-shaking.
- ECharts downsamples line series when there are more points than device
  pixels, with `lttb`, `minmax`, `average` and others, only on cartesian
  axes (`src/processor/dataSample.ts` lines 83–103). Candlesticks switch to
  a "large" path above 600 items (on by default) and render progressively in
  chunks of 3,000 above 10,000 (`src/chart/candlestick/CandlestickSeries.ts`
  lines 140–145).
  `appendData` does not update the axis extents, so the page must fix them
  (`src/core/echarts.ts` lines 1634–1660).
- Chart.js's decimation plugin offers LTTB and min-max
  (`src/plugins/plugin.decimation.js` lines 3, 82), only above
  `4 × availableWidth` points (line 242) and only for unparsed, x-indexed
  data (lines 216–238). Its interaction binary-searches only when the data is
  marked sorted, otherwise it scans everything
  (`src/core/core.interaction.js` lines 22–63), with modes `index`,
  `dataset`, `point`, `nearest`, `x`, `y` (lines 269–382).

### d3

- d3-zoom's view state is one affine transform `{k, x, y}` that rescales a
  scale's domain (`rescaleX`, `src/transform.js` lines 1, 27–34), with
  `scaleExtent` and `translateExtent` limits (`src/zoom.js` lines 42–60).
- d3-delaunay's `find(x, y, i)` walks the triangulation from the last found
  point, so a moving pointer costs a few steps a move
  (`src/delaunay.js` lines 122–128).
- d3-brush has x-only, y-only and 2-D brushes (`src/brush.js` lines 141–149).

### matplotlib and bokeh

- matplotlib gives every mouse event its data coordinates by inverting the
  axes' transform (`lib/matplotlib/backend_bases.py` lines 1388–1392); pan
  and zoom are modes of a toolbar (`NavigationToolbar2`, lines 3012, 3266,
  3304, 3330). Interaction is modal.
- bokeh attaches tools to a plot: `PanTool`, `WheelZoomTool`,
  `BoxZoomTool`, `BoxSelectTool`, `CrosshairTool`, `HoverTool` (with
  `mouse`, `hline` and `vline` modes), and `RangeTool`, a brush on one chart
  that drives another's range (`src/bokeh/models/tools.py` lines 450–1664).
  `ColumnDataSource.stream(new_data, rollover)` appends columns and drops
  from the start above `rollover` rows (`src/bokeh/models/sources.py`
  lines 470–486): a ring buffer by another name.

### What the prior art agrees on

1. Line, step, area and bars share one series model with a type switch
   (Lightweight Charts, ratatui, textplots, uPlot's path builders, plotext's
   `hist` calling `bar`).
2. Large data is reduced per output column before drawing (uPlot, ECharts,
   Chart.js) or per bar spacing (Lightweight Charts); the terminal libraries
   do not, and pay for it.
3. Pointer to datum is a scale inversion plus a binary search for x-sorted
   data (uPlot, Chart.js, Lightweight Charts, matplotlib); scattered 2-D data
   needs a nearest search (Observable Plot scans, Vega uses Voronoi,
   d3-delaunay walks).
4. Streaming is append to the right with a retention limit (bokeh
   `rollover`, Lightweight Charts `update`); the last bar of a candlestick
   updates in place.
5. No terminal chart library surveyed has pan, zoom or hover of its own.

## Measurements

All on an Apple M5 Pro (18 cores, 48 GB), macOS, 2026-10-18, a machine
shared with other benchmarking agents; `uptime` load averages were 6.8–10.1
(one-minute) across the runs. Rust 1.97 release builds, `opt-level = 3`,
native (not wasm). Each bench row is 5 repetitions in one process; the
100k and 1M rows were run in 3 processes, and the table gives the minimum
over all 15 timings and the median of the three per-process medians. The 1k
and 10k rows were run once (5 repetitions). "Lit cells" is the number of
non-blank cells in the frame. The plot area is the whole size given, with no
axes. Commands, from the repository root:
`mise exec -- cargo run --release --manifest-path discovery/bench/charts/Cargo.toml -- time`
(random walk) and `… -- time noise` (uniform noise).

| What | Number | How |
|---|---|---|
| ratatui `Chart`, braille line, 1M points, random walk, 320 × 120 | min 14.40 ms, median 14.71 ms | bench `time`, 3 × 5 runs |
| per-segment rasteriser, same | min 4.15 ms, median 4.35 ms | bench `time`, 3 × 5 runs |
| per-dot-column min/max, same | min 2.21 ms, median 2.26 ms | bench `time`, 3 × 5 runs |
| ratatui `Chart`, 1M points, random walk, 80 × 24 | min 11.40 ms, median 12.23 ms | as above |
| per-segment, 1M, random walk, 80 × 24 | min 2.30 ms, median 2.34 ms | as above |
| min/max, 1M, random walk, 80 × 24 | min 2.24 ms, median 2.31 ms | as above |
| ratatui `Chart`, 1M points, uniform noise, 320 × 120 | min 325.2 ms, median 347.1 ms | bench `time noise`, 3 × 5 runs |
| per-segment, 1M, noise, 320 × 120 | min 362.8 ms, median 387.9 ms | as above |
| min/max, 1M, noise, 320 × 120 | min 3.54 ms, median 4.18 ms | as above |
| ratatui `Chart`, 1M, noise, 80 × 24 | min 81.2 ms, median 90.8 ms | as above |
| per-segment, 1M, noise, 80 × 24 | min 80.4 ms, median 87.6 ms | as above |
| min/max, 1M, noise, 80 × 24 | min 2.62 ms, median 3.17 ms | as above |
| ratatui / per-segment / min/max, 100k, noise, 320 × 120 | min 32.3 / 33.8 / 0.87 ms; median 38.9 / 41.6 / 1.22 ms | as above |
| ratatui / per-segment / min/max, 100k, walk, 320 × 120 | min 1.91 / 0.55 / 0.22 ms; median 2.01 / 0.63 / 0.23 ms | bench `time`, 3 × 5 runs |
| ratatui / per-segment / min/max, 10k, walk, 320 × 120 | min 0.495 / 0.055 / 0.035 ms; median 0.522 / 0.061 / 0.035 ms | bench `time`, 1 × 5 runs |
| ratatui / per-segment / min/max, 1k, walk, 80 × 24 | min 0.029 / 0.005 / 0.003 ms; median 0.035 / 0.006 / 0.004 ms | bench `time`, 1 × 5 runs |
| Frames from per-segment and min/max are identical | equal lit-cell counts in all 16 configurations (8 walk, 8 noise); ratatui's differs only at 1k points (247 vs 245 at 80 × 24, 1,753 vs 1,762 at 320 × 120) | lit cells printed by the bench |
| plotext 6.1.0 `figure.signal(y, marker="braille")` + `build()`, 1M points, random walk, 80 × 24 | min 3,414 ms, median 4,171 ms | `uv run --with plotext==6.1.0 python -I pt.py` (script in the scratchpad, not the repo), 3 repetitions, one process; includes axes and frame |
| plotext, same, 1M, 320 × 120 | min 3,404 ms, median 3,423 ms | as above |
| plotext, 100k, 80 × 24 | min 585 ms, median 628 ms | as above |
| plotext, 1k, 80 × 24 | min 7.3 ms, median 8.8 ms | as above |

What the numbers say:

- Drawing every segment costs in proportion to the line's total length in
  dots. For a random walk the segments are short and the per-segment cost
  stays within 2× of min/max; for noise each segment crosses much of the
  height and it is 90–100× slower at 1M points.
- Min/max costs one pass over the visible points plus one span a dot column,
  so it barely depends on the data or the plot size: 2.2–4.5 ms native for
  1M points in every case measured. That one pass is the whole cost; a cached
  reduction (by view) or a precomputed min/max pyramid would remove it on
  frames where the view and data do not change. Agent 2 owns that choice.
- The reduction is exact for the drawn frame when x is sorted: the M4 result
  (Jugel et al., "M4: A Visualization-Oriented Time Series Data
  Aggregation", PVLDB 7(10), 2014, https://www.vldb.org/pvldb/vol7/p797-jugel.pdf)
  holds for braille dot columns, and the bench's equal lit-cell counts
  confirm it on these data.
- ratatui's `Chart` also draws a point for every datum before the lines
  (`chart.rs` lines 1056–1060), which is why it is 3–6× slower than the
  bare per-segment rasteriser on a random walk.
- Native numbers only; wasm will be slower by a factor agent 6 measures.

## Recommendation

1. Ship chart families, each one Cargo feature and one wasm module, with
   modes chosen by an option. First milestone families: `line` (line, step,
   area), then `candlestick` (candle, hollow, ohlc), `bar` (bar, grouped,
   stacked; histogram next), `sparkline`, then `scatter` and `heatmap`. The
   full table of 18 families is under [Module boundaries](#module-boundaries).
2. Build `line` first: it exercises scales, time axes, the reduction, gaps,
   pan, zoom and the crosshair, and step and area are a few marks on top.
3. Put the per-dot-column min/max reduction and the x-window binary search in
   the core and require them of every line-like chart (line, area, step,
   band, sparkline). Never draw a segment a datum.
4. Give the candlestick chart a spacing-driven glyph choice (box drawing at
   four or more cells, braille below) and power-of-two merging below one cell
   a candle; take Lightweight Charts' streaming rule (update the last candle,
   append a newer one) and its magnet crosshair.
5. Make `sparkline` its own module without axes, ticks or date formatting,
   if agent 6 measures that code at more than about 10 KB gzipped; otherwise
   make it a mode of `line`.
6. Resolve the pointer by scale inversion and binary search for x-ordered
   charts, by band index for bars, by direct inversion for heatmaps, and by a
   pick buffer (a datum id a cell, written while drawing) for scatter and the
   long-tail layouts.

## Requirements to propose

- REQ-CHART-010: Each chart family MUST be one Cargo feature of the core
  crate and one wasm module; a variant within a family MUST be selected by
  the `mode` option, never by a separate module. Checked by the build
  listing one module a family and the registry listing each mode. v0.1.0
- REQ-CHART-020: A chart's `META` MUST list its modes, and making a chart
  with a mode not in that list MUST fail with an error naming the valid
  modes. v0.1.0
- REQ-CHART-030: The `line` chart MUST draw the modes `line`, `step` (with
  `before`, `after` and `middle`) and `area` (to a baseline), each with a
  snapshot per fixture. v0.1.0
- REQ-CHART-040: A line-like chart (`line`, `sparkline`, and any mode drawing
  a polyline) MUST reduce the visible points to at most first, last, minimum
  and maximum per dot column before rasterising, and for x-sorted data its
  frame MUST equal, cell for cell, the frame drawn from every segment. v0.1.0
- REQ-CHART-050: A line-like chart MUST find its visible points by binary
  search on the x column when the data is x-sorted, so that a frame's cost
  does not grow with points outside the view. Checked by a test that a frame
  over 1,000 visible points of 1,000,000 held takes less than a tenth of the
  time of a frame over all 1,000,000. v0.1.0
- REQ-CHART-060: In `line`, `step` and `area`, a NaN or null y MUST break the
  line, with no segment drawn across it; the option `spanGaps: true` MAY
  join across it. v0.1.0
- REQ-CHART-070: The readout of a line-like chart MUST name the datum whose
  x is nearest the pointer's x value, not the minimum or maximum of the dot
  column under the pointer. v0.1.0
- REQ-CHART-080: When two series draw into one cell, the cell MUST take the
  colour of the series highest in z-order (the later in the series list,
  unless the view raises a hovered or selected series), so a frame is a
  function of data, options, view and `t` alone. v0.1.0
- REQ-CHART-090: The `candlestick` chart MUST draw the modes `candle`,
  `hollow` and `ohlc`. v0.2.0
- REQ-CHART-100: The `candlestick` chart MUST choose its glyphs from the
  candle spacing in cells: box-drawing candles at 4 cells or more; braille
  wick and body with a blank cell between candles at 2 or 3 cells; braille
  without the gap at 1 cell. Checked by snapshots at spacings 4, 2 and 1.
  v0.2.0
- REQ-CHART-110: Below one cell a candle, the `candlestick` chart MUST merge
  consecutive candles in groups of a power of two (2 to 512), each merged
  candle taking the first open, the last close, the highest high and the
  lowest low, so that no cell holds two candles. v0.2.0
- REQ-CHART-120: The `candlestick` chart MUST replace the last candle when
  given a candle with the same time, append a candle with a later time, and
  reject a candle earlier than the last with an error event, leaving the
  data unchanged. v0.2.0
- REQ-CHART-130: In one ink at 4 cells a candle or more, the `candlestick`
  chart MUST draw rising candles hollow and falling candles solid. v0.2.0
- REQ-CHART-140: The `bar` chart MUST draw vertical and horizontal bars,
  grouped and stacked, with the free end of each bar at an eighth of a cell
  (`▁`–`▇` vertical, `▏`–`▉` horizontal). v0.2.0
- REQ-CHART-150: The `sparkline` chart MUST draw without axes, MUST accept
  one appended sample a series into a ring buffer of a set capacity, and
  MAY show each series' label, latest value, minimum and maximum. v0.2.0
- REQ-CHART-160: The `bar` chart's `histogram` mode MUST bin unbinned values
  by a bin count or a bin width option, and MUST accept prebinned edges and
  counts; the counts in its readout MUST equal the counts of a reference
  binning of the fixture. v0.3.0
- REQ-CHART-170: The `scatter` chart MUST draw points as braille dots and
  MUST resolve the pointer to the nearest datum within a radius given in
  cells, or to none. v0.3.0
- REQ-CHART-180: Charts without an x order (scatter and every long-tail
  layout) MUST write the id of the datum drawn into each cell into a pick
  buffer during `frame`, and pointer hit tests MUST read that buffer. v0.3.0
- REQ-CHART-190: The `heatmap` chart MUST draw one value a cell with `█`, or
  two values a cell with `▀` in ink and back colours when the option
  `half: true` is set, and MUST draw a shade ramp in one ink. v0.3.0

## Decisions with a real alternative

### Chart families with modes, or one module a chart name

- Context: the brief asks for one module per chart type. The brief's list
  names about 45 chart types; many differ only in marks (step versus line,
  histogram versus bar, OHLC versus candle). Every module repeats the core
  (unicel: ~33 KB gz a module, `docs/IMPLEMENTATION_PLAN.md` line 65).
- Options: (a) one module per name, ~45 modules; (b) one module per family,
  modes inside, ~18 modules; (c) a few large groups as unicel's categories.
- Recommendation: (b). It keeps a page's download to the families it shows,
  as the brief wants, without paying the core again for a step chart beside a
  line chart. The prior art draws the same line: ratatui's `GraphType`,
  Lightweight Charts' `LineType`, textplots' `Shape`.

### Sparkline: its own module or a mode of line

- Context: the owner named sparkline separately; a sparkline is a line
  without axes, often fifty to a page.
- Options: (a) a `sparkline` module that leaves out axes, ticks and date
  formatting; (b) a `sparkline` mode of `line`.
- Recommendation: (a) if agent 6 measures the axis, tick and date code at
  more than about 10 KB gzipped; otherwise (b). Either way the drawing code
  (reduction, braille raster) is the core's.

### Histogram: a mode of bar or its own module

- Context: a histogram is bars over bins of a continuous axis.
- Options: (a) `bar` with `mode: histogram` and a binning transform;
  (b) a `histogram` module.
- Recommendation: (a). plotext's `hist` calls its bar drawer
  (`draw.py` lines 253–256); Vega-Lite and ECharts make a histogram a bar
  with a bin transform. Binning is a small transform.

### Line reduction: min/max per dot column, or LTTB

- Context: a million points must draw within budget. Chart.js and ECharts
  offer both LTTB and min/max (`plugin.decimation.js` lines 3, 82;
  `dataSample.ts` lines 99–103).
- Options: (a) first, last, min and max per dot column (M4); (b) LTTB to a
  point budget, then rasterise; (c) none.
- Recommendation: (a). It is exact for the drawn frame (equal lit cells in
  every bench configuration), costs one pass (2.2–4.5 ms native at 1M), and
  keeps peaks that LTTB may drop. (c) cost 325–420 ms on noise.

### Candle glyphs: fixed style or by spacing

- Context: one colour a cell; zoom changes candle spacing.
- Options: (a) one style at every zoom (unicel's four-cell candles, or
  plotext's one-cell candles); (b) choose the style from the spacing and
  merge below one cell, as Lightweight Charts sizes candles by bar spacing
  and conflates below one pixel.
- Recommendation: (b), with the thresholds in REQ-CHART-100 and 110.

### Pointer to datum: per-chart inversion or a pick buffer

- Context: x-ordered charts invert scales and binary-search; scatter and
  layouts need a nearest search. Prior art scans (Observable Plot), builds a
  Voronoi (Vega-Lite) or a Delaunay walk (d3).
- Options: (a) each chart builds a spatial index; (b) the core keeps a pick
  buffer of one `u32` datum id a cell, written while drawing; (c) a linear
  scan per move.
- Recommendation: analytical inversion for line, candlestick, bar and
  heatmap; (b) for the rest. In a cell grid the frame is already the spatial
  index: 320 × 120 cells × 4 bytes is 153,600 bytes a chart. Agent 3 decides
  the input boundary.

### Colour when series share a cell

- Context: unicel gives a cell the colour most of its dots have
  (`subcell.rs` lines 628–651); ratatui gives it the last painter's
  (`canvas.rs` lines 210–219).
- Options: (a) majority of dots; (b) z-order (last drawn or hovered wins);
  (c) a blend colour.
- Recommendation: (b). A hovered or selected series must win its cells to be
  seen, and z-order is a rule a reader can predict. Agent 4 owns the colour
  model and may decide otherwise.

## Open questions for the owner

1. The first milestones' charts. Options: (a) v0.1.0 `line` (line, step,
   area); v0.2.0 `candlestick`, `bar`, `sparkline` with streaming; v0.3.0
   `scatter`, histogram, `heatmap`; (b) candlestick first, since it is the
   owner's one-line test; (c) all eight at once. Recommendation: (a). Line
   exercises everything the candlestick needs and is easier to check, and
   the candlestick follows one milestone later.
2. One ink for narrow candles. Below four cells a candle, neither braille
   nor block candles can be hollow, so rising and falling look alike without
   colour. Options: (a) in one ink, never draw narrower than four cells a
   candle (fewer candles, always readable); (b) rising candles draw their body
   in the right dot column only, falling ones in both (`⢸` versus `⣿`), to be
   judged by eye; (c) accept colour-only direction below four cells.
   Recommendation: (a) as the default, with (b) tried on the site before it is
   chosen.
3. Layouts for the long tail (force graph, Sankey, treemap). Options:
   (a) write them in Rust in the family module, seeded so frames stay
   deterministic, porting d3-force, d3-sankey and d3-hierarchy (ISC) with
   attribution; (b) take positions from the page; (c) both, with (b) as the
   escape hatch. Recommendation: (c).
4. Pie and donut. Options: (a) leave them out; (b) a `polar` mode, late.
   Recommendation: (b), after v1.0, since a bar chart shows shares better in
   a grid of cells.

## Sources

Repositories, shallow-cloned on 2026-10-18 (URL, commit):

- uPlot, https://github.com/leeoniya/uPlot, 7187ad7d47cf0b2aae720043dbbc01dff79e8fd4
- Lightweight Charts, https://github.com/tradingview/lightweight-charts, 9db7ce733e95303ae1eeb79e664ee9be4ee5e696
- ratatui, https://github.com/ratatui/ratatui, 0297560d8d77871c0281a39182e4e650856e2ffd (ratatui-widgets 0.3.2, ratatui-core 0.1.2; the bench uses the same versions from crates.io)
- tui-rs, https://github.com/fdehau/tui-rs, 335f5a4563342f9a4ee19e2462059e1159dcbf25
- plotext, https://github.com/piccolomo/plotext, e51c92ac996a9d4b0a028e62580eeabf142ea293 (6.1.0); timed from PyPI `plotext==6.1.0`
- textplots-rs, https://github.com/loony-bean/textplots-rs, 49051a1a921ebe11348996cd95381929ece26e4d
- asciichart, https://github.com/kroitor/asciichart, e2978630375e3e300908a0903fd91296ba907c57
- termgraph, https://github.com/mkaz/termgraph, 8ed81e21ce07db18393c7af9f77238f642546d3a
- YouPlot, https://github.com/red-data-tools/YouPlot, e67219ca88ccdc5da9f14c4a346c24b8da8ce25c
- gnuplot, https://github.com/gnuplot/gnuplot, fbeb88eadedf927a4d778b41dd118e373f33eacb
- plotters, https://github.com/plotters-rs/plotters, c63248e0cf7c611185361ac492003e8a87d52ea8
- Observable Plot, https://github.com/observablehq/plot, 535723d5e433727720d9b673c31622821bb03210
- Vega-Lite, https://github.com/vega/vega-lite, 85fab8d12c873b3b224ef9a33c807642953325ba
- Vega, https://github.com/vega/vega, fdd9ab9d4faf48553c8dfdbcaf7284f485504aaf
- ECharts, https://github.com/apache/echarts, 34f4d927eafe251f26f63afd3aeefa30f2feba30
- Chart.js, https://github.com/chartjs/Chart.js, 7169e65147a47f3720957a6f156a33c838ab9f57
- d3-zoom, https://github.com/d3/d3-zoom, c8df708b78b46553bc4a0fbf1baf4ffc10cef8bd
- d3-delaunay, https://github.com/d3/d3-delaunay, afbeebfca170e81991e5fc26dbac52f34575a830
- d3-brush, https://github.com/d3/d3-brush, a8e15cc4c94e300bb2f4a1e11826b36da7def9b1

Files fetched on 2026-10-18:

- matplotlib `lib/matplotlib/backend_bases.py`, https://raw.githubusercontent.com/matplotlib/matplotlib/main/lib/matplotlib/backend_bases.py
- bokeh `src/bokeh/models/sources.py` and `tools.py`, https://raw.githubusercontent.com/bokeh/bokeh/branch-3.9/src/bokeh/models/

Papers:

- U. Jugel, Z. Jerzak, G. Hackenbroich, V. Markl, "M4: A Visualization-Oriented Time Series Data Aggregation", PVLDB 7(10), 2014, https://www.vldb.org/pvldb/vol7/p797-jugel.pdf

unicel (`../animate-unicode`): `crates/unicel/src/pieces/data/{candlesticks,bar_chart,sparkline,heatmap,gauge,radar}.rs`,
`crates/unicel/src/{subcell,grid,piece}.rs`, `docs/IMPLEMENTATION_PLAN.md` line 65.

Bench: `discovery/bench/charts/` (README, `src/main.rs`).
