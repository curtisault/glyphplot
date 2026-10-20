# Scales, axes, layout and formatting

## Summary

glyphplot should write its scales, ticks, number formatting and calendar arithmetic by hand in the core, as a port of d3's algorithms, and take no formatting or date crate into a wasm module. A port of d3's linear ticks, `nice`, SI tick labels and UTC calendar ticks with labels adds 5.5 KB gzipped to a minimal wasm-bindgen module and matched d3 on all 803 captured cases. The cheapest crate that formats a date, `time`, adds 8.5 KB and does less; `chrono` adds 13.8 KB; `jiff` with `strftime` adds 63.6 KB; ICU4X decimal formatting adds 25.4 KB and date formatting 120.9 KB; an IANA time zone database adds 150–161 KB. Rust's own `{:.2}` float formatting adds 10.4 KB, so chart code should not use it.

Time zones and locales stay out of wasm. The shell turns an IANA zone name into a table of offset transitions over the visible span with `Intl.DateTimeFormat` (388 lookups and 0.5–1.7 ms for a year in Node), and a BCP 47 locale into a d3-shaped locale object of about 435 bytes (1.7–1.9 ms). Both cross the boundary once, as inputs, so a frame stays a pure function of its inputs; the CLI builds the same table from the system's tzdata. The core formats every label itself, in the browser and the terminal alike.

Ticks are fitted to cells: a label is a whole number of cells wide, the tick count starts at one tick per four cells and falls until labels sit at least one blank cell apart, and the layout is computed in cells from the outside in, with fixed sizes below which the title, the legend, the axes and finally all decoration drop.

## Scope and method

Covered: d3-scale (linear, log, symlog, time, tick formats), d3-array ticks, d3-format, d3-time and d3-time-format, Vega's tick count and label overlap code, Vega-Lite's default tick count, plotters' key points and label areas, ratatui's `Axis` and chart layout, unicel's clock and data pieces, and the wasm size of chrono, chrono-tz, time, jiff (with and without its tz database), num-format, ryu, ICU4X decimal and datetime, js-sys `Intl`, and a hand-written port.

Built: a throwaway crate at `discovery/bench/scales/` (its README says how to rerun everything). It holds one wasm-bindgen export whose body is chosen by a Cargo feature, one variant per dependency; the hand-written port (`src/hand.rs`); a Node script that captures d3's outputs as fixtures; a fixture test and property tests; a sample of fitted axes; Node timings of `Intl` and of the wasm port.

Not covered: ordinal, band and colour scale code was read in d3 but not ported or measured; pow and sqrt scales; the Observable Plot and ECharts axis code; label layout for non-Latin scripts beyond noting width; right-to-left axes; Safari and Chrome timings (Node only).

Machine for every measurement: Apple M5 Pro, 18 cores, 48 GB, macOS, 2026-10-18, shared with other agents benchmarking at the same time (`uptime` load averages 7.3 to 10.9 during the timings). Toolchain from `mise exec`: Rust 1.97 with wasm32, Node 26, wasm-pack 0.15.0.

## What the libraries do

### d3: ticks, nice, formats

- Linear ticks are a step of 1, 2 or 5 times a power of ten, chosen so the count of ticks is near the requested count; the thresholds between factors are √50, √10 and √2 (`d3-array/src/ticks.js:1-27`). Tick values are computed as `i × step` or `i / (1/step)` from integer indices, so `0.1 + 0.2` errors never reach a label (`ticks.js:35-42`).
- `nice` widens the domain to multiples of the step, repeating up to ten times until the step stops changing; if it never settles it leaves the domain unchanged (`d3-scale/src/linear.js:19-55`). The port found the case: a count of 1 on a domain that straddles zero never settles (12 of the 803 fixtures).
- A tick format is derived from the step: `tickFormat(start, stop, count, "s")` picks one SI prefix for every tick from the larger end of the domain and the digits the step needs (`d3-scale/src/tickFormat.js:4-29`, `d3-format/src/precisionPrefix.js`). Every label on an axis then shares a unit and a width, which suits a grid: `0.0G 0.5G 1.0G`.
- d3-format's prefixes run from y (10⁻²⁴) to Y (10²⁴) and include `µ` (U+00B5) (`d3-format/src/locale.js:11`); its minus sign is U+2212 by default. A locale is five to eight fields: decimal, thousands, grouping, currency, numerals, percent, minus, nan (`d3-format/locale/de-DE.json` is 4 fields, 80 bytes).
- Time ticks choose from 18 intervals (1, 5, 15, 30 s; 1, 5, 15, 30 min; 1, 3, 6, 12 h; 1, 2 days; a week; 1, 3 months; a year) the one whose duration is nearest the span divided by the count, in ratio; beyond a year they use year multiples of 1, 2, 5 × 10ⁿ; below a second, milliseconds (`d3-time/src/ticks.js:14-50`). Intervals align to the calendar: hours to multiples of the step of the hour of the day, weeks to Sundays, months to multiples of the step from January.
- A time label is the coarsest field the tick is not on a boundary of: `.%L`, `:%S`, `%I:%M`, `%I %p`, `%a %d`, `%b %d`, `%B`, `%Y` (`d3-scale/src/time.js:20-37`). Labels on one axis therefore differ in width (`November`, `Nov 08`).
- d3-time knows two zones only: local (through `Date.getHours`) and UTC (`d3-time/src/hour.js:11,23`). Any other zone is the page's problem.
- Log ticks are every integer multiple of each power when the domain spans fewer decades than the count, else powers chosen by linear ticks on the exponents (`d3-scale/src/log.js:70-107`). Symlog (`sign(x)·log1p(|x|/c)`) reuses linear ticks over the raw domain (`d3-scale/src/symlog.js:5-24`), so its ticks bunch at one end; glyphplot needs its own (0 and ±10ⁿ).
- Weight for comparison: the minified d3 packages a time and linear axis needs (d3-array, d3-scale, d3-format, d3-time, d3-time-format) are 18.4 KB gzipped together, measured with `gzip -9` on each package's `dist/*.min.js` from npm.

### Vega and Vega-Lite: how many ticks, and overlap

- Vega-Lite's default tick count for a continuous axis is `ceil(size / 40)` pixels, and `ceil(size / 10)` for binned data (`vega-lite/src/compile/axis/properties.ts:293-319`). At an 8 px cell that is one tick per five cells.
- Vega lowers the count until the d3 step is at least a minimum step (`vega-scale/src/ticks.js:21-55`), and thins ticks by keeping every other one until no more than the count remain, falling back to the two ends (`ticks.js:65-99`).
- Label overlap is removed after layout by `parity` (hide every other label, repeat) or `greedy` (hide a label that overlaps the last kept one), stopping at three labels and then keeping the last (`vega-view-transforms/src/Overlap.js:34-128`). Separation is in pixels.

### plotters and ratatui

- plotters asks each range for at most N key points and steps the scale through 2, 5 and 10 divisions of a power of ten (`plotters/src/coord/ranged1d/types/numeric.rs:134-177`); light and bold points give minor and major ticks (`coord/ranged1d/mod.rs:141-169`). Label areas are sized by the caller in pixels (`chart/builder.rs:201`), not from the labels. Dates go through chrono (`plotters/Cargo.toml:22,116`).
- ratatui's `Axis` holds bounds and labels the caller supplies; there is no tick generation (`ratatui-widgets/src/chart.rs:40-51`). Labels are spread evenly across the width whatever their values, the first flush left, the last flush right, each truncated to its slot (`chart.rs:886-930`). Layout takes the bottom row for x labels, one more for the axis line, and the widest y label for a left gutter (`chart.rs:752-790`). The legend hides when it would take more than a quarter of the width or height (`chart.rs:583`).
- Neither places ticks at data values in cells with labels fitted to the space; that is what glyphplot needs to add.

### unicel

- `Wall { unix, offset }` is a moment and a fixed offset in minutes; `local()` turns it into civil fields with Hinnant's algorithm in about 40 lines and no dependency (`crates/unicel/src/clock.rs:7-67`). The shell passes `Date.now()` and `-getTimezoneOffset()` (`spec/SPEC.md`, REQ-MOUNT-030). This is the model to extend: a table of offsets over a span instead of one offset.
- candlesticks sets a round price per row and labels at the first round interval at least three rows apart (`pieces/data/candlesticks.rs:119-137`), with the last price drawn as a label on the axis (`:140-146`). bar-chart takes a nice step of a quarter of the maximum (`pieces/data/bar_chart.rs:46-60`). Both format with `format!("{:.2}")`, which pulls Rust's float formatting into the module (10.4 KB gzipped, measured below).
- unicel's SPEC notes that rounding a range instead of smoothing it keeps a picture from pumping without state (`spec/SPEC.md:228`). The same holds for an axis domain that follows streaming data.

## Size of each dependency in wasm

Each variant is one call: `base` formats two integers and a `&str` with `format!` (any chart module has this); every other row is `base` plus the call shown. Built with `wasm-pack build --target web --release` with unicel's release profile and `wasm-opt -Os`; sizes are bytes of the `.wasm`, gzipped with `gzip -9`. Two clean builds gave identical bytes. Crate versions from `Cargo.lock`: chrono 0.4.45, chrono-tz 0.10.4, time 0.3.55, jiff 0.2.38, num-format 0.4.4, ryu 1.0.23, icu_decimal and icu_datetime 2.3.0, js-sys 0.3.106, wasm-bindgen 0.2.129.

| Variant | The call | wasm gz (B) | over base |
|---|---|---:|---:|
| empty | `x + t`, an f64 back | 5,858 | — |
| base | `format!("{}\|{}\|{}", i, i, s)` | 7,450 | 0 |
| fmt-float | `format!("{x:.2} {x}")` | 17,836 | +10.4 KB |
| ryu | `ryu::Buffer::format(x)` | 20,703 | +13.3 KB |
| num-format-en | `to_formatted_string(&Locale::en)` | 16,121 | +8.7 KB |
| num-format-any | `Locale::from_name(s)` then format | 24,114 | +16.7 KB |
| time | `OffsetDateTime::format(format_description!(…))` | 15,941 | +8.5 KB |
| chrono | `DateTime::from_timestamp_millis(t).format("%Y-%m-%d %H:%M")` | 21,270 | +13.8 KB |
| jiff-civil | `Offset::UTC.to_datetime(ts)`, `Display` of the date | 9,674 | +2.2 KB |
| jiff-utc | `ts.to_zoned(TimeZone::UTC).strftime(…)` | 71,066 | +63.6 KB |
| jiff-tz | `TimeZone::get(s)` with `tzdb-bundle-always`, `strftime` | 157,451 | +150.0 KB |
| chrono-tz | `s.parse::<Tz>()`, `with_timezone`, `format` | 168,324 | +160.9 KB |
| icu-decimal | `DecimalFormatter::try_new(locale)` at run time | 32,893 | +25.4 KB |
| icu-datetime | `FixedCalendarDateTimeFormatter<Gregorian, YMD+hm>` at run time | 128,346 | +120.9 KB |
| js-intl | `Intl.DateTimeFormat` through js-sys | 9,499 | +2.0 KB (+0.75 KB JS glue) |
| hand-linear | port: `nice`, `ticks`, SI labels | 11,047 | +3.6 KB |
| hand-time | port: UTC calendar ticks and labels | 11,767 | +4.3 KB |
| hand | both | 12,998 | +5.5 KB |

Findings from the table:

- An IANA zone database in wasm costs 150–161 KB gzipped, more than unicel's whole 150 KB module budget for a category. It is ruled out.
- A locale chosen at run time links every locale's data: num-format grows from 8.7 to 16.7 KB, and ICU4X's datetime formatter adds 121 KB for one field set. Locale data belongs to the page, which already has it in `Intl`.
- jiff is cheap for civil arithmetic (+2.2 KB) and expensive for `strftime` and `Zoned` (+63.6 KB).
- `{:.2}` costs 10.4 KB. The port formats fixed digits by scaling to an integer and placing the point; a first draft that used `f64::powf` and string slicing weighed 11.1 KB over base, and replacing `powf` with a table of powers of ten and the slices with iteration brought it to 5.5 KB (function sizes read with `wasm-tools print` on an unstripped build).

## Port fidelity, properties and speed

- Fixture test: 803 cases captured from d3 4.x with `fixtures/capture.mjs` (55 hand-picked linear domains at five counts, 400 random linear domains from 10⁻¹² to 10¹², 48 UTC spans from 500 ms to 80 years, 300 random UTC spans): 0 tick lists, 0 label lists and, after the non-settling `nice` was copied, 0 nice domains differ (`cargo test --release --test d3`). Two details mattered: JavaScript's `Math.round` rounds halves up, not away from zero, and `nice` returns the domain unchanged when it does not settle.
- Property tests (`tests/props.rs`, 200,000 cases each from a fixed generator, `cargo test --release --test props`): for linear axes from 4 to 304 cells over domains from 10⁻¹⁵ to 10¹⁵, and UTC axes over spans from 10 ms to 32 years, every placed tick is inside the domain, values and cells are monotone, labels are at least one blank cell apart and inside the axis, and the step is 1, 2 or 5 × 10ⁿ. `nice` always contains the domain. 14 linear and 1,021 time axes had no label at all; all were too narrow for one label (a 4-cell axis cannot hold `November`).
- `fit_x` (lower the count from one tick per four cells until labels fit) took 2.6 µs (median 2.7) for an 80-cell linear axis, 4.0 µs (median 4.4) for an 80-cell time axis, and 14.1 µs (median 14.7) at 320 cells, native, 5 runs of 20,000 calls (`cargo run --release --example fit`). That is under 0.5 % of a 4 ms frame, and it can be cached by domain and width.
- In wasm under Node, one call that niced a domain, made and labelled about ten linear ticks and about ten UTC ticks took 0.84 µs (median 0.87–0.93) over 3 runs of 5 × 20,000 calls (`node timing.mjs`).

Sample axes from `examples/fit.rs` (gap of one cell):

```
price 98.37..131.2, 40 cells
──┬─────┬─────┬─────┬─────┬─────┬─────┬─
 100   105   110   115   120   125   130

bytes 0..3.4e9, 40 cells
┬─────┬────┬─────┬─────┬─────┬────┬─────
0.0G 0.5G 1.0G  1.5G  2.0G  2.5G 3.0G

6 hours, 80 cells
─────┬──────┬─────┬──────┬─────┬──────┬─────┬──────┬──────┬─────┬──────┬─────┬──
   01:30  02 PM 02:30  03 PM 03:30  04 PM 04:30  05 PM  05:30 06 PM  06:30 07 PM
```

They show three things the design must handle. Tick spacing alternates by a cell (7, 6, 7 on the six-hour axis) because a value rarely lands on a cell centre. A gap of one cell reads as crowded; two reads well. d3's 12-hour labels without a date are ambiguous on a chart.

## Ticks in a grid

- A tick's position is continuous: `c = (v − a) / (b − a) × (len − 1)` with cell centres spanning the domain. Its mark goes in cell `round(c)`; the error is at most half a cell, 0.6 % of an 80-cell axis.
- Sub-cell marks: a gridline through the plot is drawn in braille at the dot column `round(2c)` (two dots across a cell) or dot row `round(4c)` (four down), so it agrees with braille data to a dot. The axis line itself is box drawing (`┬ ┴ ┤ ├`) at cell precision; a half-cell can be shown with `╷`/`╵` on the line where the font has them, but this is not needed.
- A y axis with few rows can be snapped: choose the step per row as a nice number and the domain as a whole number of rows, as unicel's candlesticks does. Then every tick lands on a row exactly and a label is never a row off. This is an option per axis (`snap: "rows"`), the default for bar charts and candlesticks under 20 rows.
- Label density: count starts at `floor(len / 4)` for x and `floor(rows / 2)` for y and falls until labels fit with the gap (x: two cells by default, y: one blank row). This is Vega's lowering of the count, decided on whole labels in cells rather than on pixels, and it never needs a separate overlap pass. A tick whose label does not fit keeps its mark; the property tests check labels never overlap.
- A label's width is its East Asian Width in cells, not its byte or char count: Japanese month names from `Intl` are `10月`, three cells (`node locale.mjs`). The core's wide-glyph rule (`WIDE_TAIL`) applies to labels too.
- End labels are pushed inside the axis (Vega's `labelFlush`); a label may not overhang the chart.
- Time axes default to 24-hour labels (`15:00`, not `03 PM`) unless the locale's hour cycle is 12-hour (`hourCycle` from `Intl`), and SHOULD draw a context row under the ticks with the date at each day boundary when the span is under two days, as trading charts do.
- Log axes label powers of the base only (`1`, `10`, `100`, `1k`) and draw other multiples as unlabelled marks; symlog labels 0 and ±10ⁿ.

## Number and date formatting

- A d3-format specifier subset covers charts: `[sign][$][,][.precision][~][type]`, types `f` (fixed), `s` (SI), `%`, `e`, `d`, and `auto` (precision from the tick step, as `tickFormat` does). Fill, align and width are not needed: the layout aligns. Every axis label uses the axis's one format so labels share a unit.
- Currency is the locale's currency prefix and suffix, as d3 does; the currency itself (USD) is the page's option, not a locale lookup.
- Date formats use a strftime subset with d3's directives (`%Y %m %d %e %H %I %M %S %L %p %a %A %b %B %j %Z`), names from the locale object. Hinnant's civil algorithm (in unicel already) does the calendar.
- Locale: the page names a BCP 47 tag; the shell derives a d3-shaped object from `Intl.NumberFormat.formatToParts` and `Intl.DateTimeFormat` (decimal, group, minus, currency placement, hour cycle, month and day names). It is 435 bytes for de-DE and took 1.7–1.9 ms median per locale in Node (3 runs of 5 × 200). It crosses once, as an input. The CLI has en-US built in and reads the same object from a JSON file (`--locale-file`). Grouping other than threes (en-IN's 3,2) needs one more probe; not built.
- The minus sign: d3 uses U+2212 by default; Intl in Node 26 gave hyphen-minus for de-DE. The font must hold U+2212 if it is the default (rendering topic).

## Time zones

| Option | wasm cost | Crossing | Deterministic | CLI |
|---|---|---|---|---|
| UTC only | 0 | none | yes | same |
| Fixed offset (unicel's `Wall`) | 0 | once | yes | same |
| Offset table from the shell | about 1 KB of code **(unverified)**; data 16 B a transition | once per zone and span | yes, the table is an input | builds the table from tzdata (jiff, system zoneinfo) |
| IANA tzdata in wasm (jiff or chrono-tz) | +150 to +161 KB gz | none | yes, but tzdata version is baked in | native jiff reads `/usr/share/zoneinfo` |
| Format every label in JS (`Intl`) | +2 KB gz | per label, per frame | no: output depends on the browser's ICU | impossible; CLI has no Intl |

- `Intl` costs in Node 26: an offset lookup through `formatToParts` with `timeZoneName: "longOffset"` 1.1 µs (median 1.1–1.3); a cached `DateTimeFormat.format` 0.44–0.48 µs; constructing one 17.5–20.8 µs; America/New_York's transitions over one year by daily sampling and bisection to the minute, 388 lookups, 0.48–0.72 ms min and 1.2–1.7 ms median; over ten years, 3,871 lookups, 4.4 ms. 3 runs, each of 5 repetitions (`node timing.mjs`).
- So the table costs about a millisecond once per zone and span, against a per-frame crossing for every label. Local-time ticks then work on wall-clock fields: convert by the table's offset, floor in civil fields, convert back. A tick in a skipped hour is dropped and a repeated hour takes the earlier instant, which matches d3's local `timeScale`; it can be fixture-tested by running `capture.mjs` with `TZ=America/New_York`.
- The table covers the data's time extent plus the view; a pan past it asks the shell for more. The core never reads a clock or a zone (unicel REQ-CLOCK-001).

## Scale and axis model

- A scale maps a domain value to a continuous cell coordinate and back (`map`, `invert`; interaction needs `invert`). Kinds: `linear`, `log` (base), `symlog` (constant), `time` (zone), `band` and `point` (categories, padding), and colour scales `sequential`, `diverging`, `categorical`, `threshold` that map to palette indices (the cell colour model is the rendering topic's).
- A domain is one of: `"data"` (extent of every series bound to the scale), `"view"` (the x range comes from the view state; a y scale fits the data visible in it, TradingView's autoscale), `[a, b]` fixed by the page, or `{follow: seconds}` (the last window of a stream). `nice`, `zero` (include 0, default for bars) and `padding` (a fraction) adjust it. A domain that follows data is niced, not smoothed, so it moves in steps without hidden state.
- Band scales give each category a whole number of cells; the remainder is split into the outer padding, so bars do not alternate widths. Categories that do not fit their label are thinned to every k-th label, then truncated with `…`; text is never rotated or stacked in a grid.
- An axis binds a scale to a side of a pane: `orient` (`left`, `right`, `bottom`, `top`), `format`, `ticks` (count, `"auto"`, or values), `grid`, `snap`, `title`, `labelGap`. A pane holds one or two y axes; panes share the x axis.
- Dual y axes: gridlines come from the first axis only. The second axis MAY align its ticks to the first's rows (`align: "primary"`: pick its domain so it has the same number of nice intervals).
- Annotations are bound to scales and drawn in data coordinates: `hline` and `vline` (with an optional label in the axis gutter, the candlestick's last price), `band` (a range shaded on the background colour, or `░` in one ink), `label` (text at a point), `marker` (a glyph at a point: `▲ ▼ ● ◆`). An annotation label on an axis hides the tick labels it collides with.

## Layout

Layout runs once per size, option or domain change, in cells, from the outside in. The order matters because the y gutter's width depends on y labels, which depend only on rows, while x labels depend on the plot's width.

1. Title: one row at the top, truncated with `…`, when set and `rows ≥ 10`.
2. Legend: entries as `■ name` two cells apart. `"auto"` puts it outside in one row under the title when it fits in one row and `rows ≥ 12`; else inside the first pane's top-left corner when it takes at most a third of the pane's width and height (ratatui uses a quarter); else it is hidden from the frame and stays in the data table and events.
3. X axis: two rows at the bottom (line and labels) when `rows ≥ 6`; a time context row adds one when `rows ≥ 14`; an x title adds one when `rows ≥ 16`.
4. Panes: the rows left are split by weights (price 3, volume 1), each pane at least 3 rows; one separator row between panes when every pane keeps its minimum, else none. A pane under 3 rows draws without y labels; under 1 row it is not drawn.
5. Y axes: each pane's y ticks are fitted to its rows, giving its labels; the left gutter is the widest left label of any pane plus one cell for the axis line, so plots line up across panes; the right gutter likewise. Y labels are kept only while `gutters ≤ cols / 3`; else the right axis drops, then the left.
6. X ticks are fitted to the plot width left over (`cols − gutters`).
7. Annotations, legend (if inside) and crosshair readouts are placed last, over the data, in that order of priority.

When `cols < 16` or `rows < 5` a chart draws no axes, title or legend: data only, a sparkline. At `cols < 4` or `rows < 1` it draws nothing and says so through its accessible label. On resize the layout reruns; tick choice and labels are a pure function of cols, rows, options, data extent and view, so the same size always gives the same frame.

Draft options shape (to become part of `spec/schemas/options.schema.json`), with one example:

```json
{
  "title": "BTC-USD, 5 min",
  "locale": "de-DE",
  "timeZone": "America/New_York",
  "legend": { "position": "auto" },
  "panes": [
    { "id": "price", "weight": 3 },
    { "id": "volume", "weight": 1 }
  ],
  "scales": {
    "x": { "type": "time", "domain": "view", "nice": false },
    "price": { "type": "linear", "domain": "view", "nice": true, "zero": false, "padding": 0.05 },
    "volume": { "type": "linear", "domain": "view", "zero": true },
    "change": { "type": "diverging", "domain": [-0.05, 0, 0.05], "scheme": "red-blue" }
  },
  "axes": [
    { "scale": "price", "pane": "price", "orient": "right", "format": ",.2f", "ticks": "auto", "grid": true },
    { "scale": "volume", "pane": "volume", "orient": "right", "format": "~s", "ticks": 3 },
    { "scale": "x", "pane": "volume", "orient": "bottom", "format": "auto", "labelGap": 2, "context": true }
  ],
  "annotations": [
    { "type": "hline", "scale": "price", "value": 67250.5, "label": "last", "axis": "right" },
    { "type": "band", "scale": "x", "from": 1792328400000, "to": 1792351800000, "label": "session" }
  ]
}
```

Schema notes: `scales.<name>.type` is one of `linear log symlog time band point sequential diverging categorical threshold`; `domain` is `"data" | "view" | [number, number] | {"follow": number}` (diverging takes three numbers); `timeZone` is `"UTC"`, an offset `"+05:30"`, `"local"` or an IANA name, and the shell, not the options, carries the resulting table into wasm; `locale` is a BCP 47 tag, with an optional `localeData` object in d3's shape that overrides it (the CLI's path); `format` is a d3-format subset string, `"auto"`, or a strftime subset string for time scales; `ticks` is `"auto"`, a count, or an array of values; `snap` is `"none" | "rows" | "cells"`.

## Measurements

| What | Number | How |
|---|---|---|
| Empty wasm-bindgen module | 5,858 B gz | `sizes.sh` variant `empty`, `gzip -9` of the `.wasm`, wasm-pack 0.15.0 `--target web --release`, unicel's profile, `wasm-opt -Os`; 2 clean builds, identical |
| Base module (integer `format!`, a String) | 7,450 B gz | as above, `base` |
| Rust `{:.2}` float formatting | +10.4 KB gz | as above, `fmt-float` minus `base` |
| ryu | +13.3 KB gz | as above |
| num-format, one locale / any locale | +8.7 / +16.7 KB gz | as above |
| time 0.3.55 with `format_description!` | +8.5 KB gz | as above |
| chrono 0.4.45, `format` | +13.8 KB gz | as above |
| jiff 0.2.38 civil only / `strftime` / with tzdb | +2.2 / +63.6 / +150.0 KB gz | as above |
| chrono-tz 0.10.4, zone by name | +160.9 KB gz | as above |
| ICU4X 2.3 decimal / datetime, locale at run time | +25.4 / +120.9 KB gz | as above |
| js-sys `Intl.DateTimeFormat` | +2.0 KB gz wasm, +0.75 KB gz JS glue | as above |
| Hand-written ticks, nice, SI, UTC ticks and labels | +5.5 KB gz (linear part 3.6, time part 4.3) | as above, `hand`, `hand-linear`, `hand-time` |
| d3 packages for the same (array, scale, format, time, time-format) | 18.4 KB gz JS | `gzip -9` of each npm package's `dist/*.min.js`, d3-scale 4.0.2 and peers |
| Port against d3 | 0 of 803 cases differ | `cargo test --release --test d3`, fixtures from `capture.mjs`, Node 26 |
| Label overlap, ticks in domain, monotone, nice | 0 failures in 3 × 200,000 cases | `cargo test --release --test props` |
| `fit_x`, 80 cells linear / time; 320 cells | 2.6 / 4.0 µs min (median 2.7 / 4.4); 14.1 µs (median 14.7) | `cargo run --release --example fit`, native, 5 runs of 20,000, load 9.2–9.3, shared machine |
| One wasm call: nice, ~10 linear and ~10 UTC labels | 0.84 µs min, 0.87–0.93 µs median | `node timing.mjs`, 3 runs × 5 repetitions × 20,000 calls, load 7.3–7.7, shared |
| `Intl` offset lookup | 1.11 µs min, 1.14–1.26 µs median | as above |
| `Intl.DateTimeFormat`, cached format / construct and format | 0.43–0.48 µs / 17.5–20.8 µs min | as above |
| Zone transitions, one year / ten years (America/New_York) | 0.48–0.72 ms min, 1.2–1.7 ms median / 4.4 ms min | as above, 388 / 3,871 lookups |
| Locale object from Intl | 1.66–1.74 ms min, 1.70–1.91 ms median; 435 B JSON for de-DE | `node locale.mjs`, 3 runs × 5 × 200, load 10.9, shared |

## Recommendation

- Write scales, ticks, number formatting, date formatting and civil-calendar arithmetic by hand in the core, as ports of d3-array `ticks`, d3-scale's scales and `tickFormat`, a d3-format subset, d3-time's intervals and a d3-time-format subset, with fixture tests against d3 captured by Node. Budget about 6–10 KB gzipped for all of it; the measured linear and UTC part is 5.5 KB. Add log, symlog, band and colour scales by the same method.
- Take no date, number-format, locale or tzdata crate into a wasm module. jiff MAY be used by the CLI only, behind a native-only feature, to read the system tzdata and parse timestamps.
- Ban Rust's float formatting from chart and core code that ships to wasm (`{}` and `{:.N}` on `f64`); format numbers through the core's formatter. This saves 10.4 KB a module and keeps labels identical to the fixtures.
- Time zones: UTC by default; a fixed offset or an IANA name resolved by the shell into an offset table that crosses once; the CLI builds the same table from tzdata. Locales: a BCP 47 tag resolved by the shell into a d3-shaped object; en-US built in.
- Fit ticks to cells by lowering the count until whole labels fit with a two-cell gap; place marks at the nearest cell and gridlines at the nearest braille dot; offer row snapping for small y axes.
- Lay out in cells from the outside in, with the fixed drop sizes above, y before x.

## Requirements to propose

Milestones assumed: v0.1.0 is the core and the line chart end to end; v0.2.0 adds candlestick, bar and sparkline (panes, band scale, log); v0.3.0 streaming; v0.5.0 the CLI; v0.6.0 the next tier of charts (heatmap and colour scales). The synthesis renumbers as the plan sets them.

- REQ-SCALE-010 (v0.1.0): The core MUST provide linear and time scales that map a domain value to a continuous cell coordinate and invert a cell coordinate to a domain value; `invert(map(v))` MUST equal `v` within 1e-9 relative for finite `v` in the domain.
- REQ-SCALE-020 (v0.1.0): A scale's domain MUST be one of `"data"`, `"view"`, a fixed `[a, b]`, or `{follow: seconds}`, and MUST be a pure function of the data, the options and the view state.
- REQ-SCALE-030 (v0.1.0): `nice` MUST give the same domain as d3-scale 4.0.2's `linear().nice(count)` for every case in the fixture file, including leaving a domain that does not settle in ten rounds unchanged.
- REQ-SCALE-040 (v0.2.0): The core MUST provide log (any base > 1) and band scales; a band scale MUST give every category the same whole number of cells.
- REQ-SCALE-050 (v0.2.0): The core SHOULD provide a symlog scale whose ticks are 0 and ±powers of ten within the domain.
- REQ-SCALE-060 (v0.6.0): The core MUST provide sequential, diverging, categorical and threshold colour scales that map a value to a palette index.
- REQ-TICK-010 (v0.1.0): Linear ticks MUST equal d3-array 3.2.4's `ticks(a, b, count)` exactly for every fixture case.
- REQ-TICK-020 (v0.1.0): UTC time ticks MUST equal d3-time 3.1.0's `utcTicks(a, b, count)` exactly for every fixture case.
- REQ-TICK-030 (v0.1.0): For any axis length from 4 to 400 cells and any finite domain, placed ticks MUST lie within the domain, be strictly increasing in value and non-decreasing in cell, and their labels MUST lie within the axis and be separated by at least the axis's label gap (default 2 cells on x, 1 row on y).
- REQ-TICK-040 (v0.1.0): A tick mark MUST be drawn in the cell nearest its continuous position; a gridline MUST be drawn at the braille dot nearest its position.
- REQ-TICK-050 (v0.2.0): A y axis with `snap: "rows"` MUST place every tick on a row exactly, with a nice step per row.
- REQ-TICK-060 (v0.2.0): Log axes MUST label only powers of the base and MAY mark other integer multiples without labels.
- REQ-TICK-070 (v0.2.0): A time axis SHOULD draw a context row with the date at each day boundary when its span is under two days and the layout has the row.
- REQ-FMT-010 (v0.1.0): Tick labels MUST default to d3-scale's `tickFormat` behaviour: one format for the axis, precision from the step, and for `s` one SI prefix from the larger end of the domain; labels MUST equal d3's for every fixture case.
- REQ-FMT-020 (v0.1.0): The formatter MUST accept the d3-format subset `[sign][$][,][.precision][~][type]` with types `f s % e d` and `auto`, and MUST reject other specifiers with an error naming the option.
- REQ-FMT-030 (v0.1.0): A label's width MUST be counted in cells by East Asian Width, a wide glyph two cells.
- REQ-FMT-040 (v0.1.0): Wasm modules MUST NOT contain Rust's float formatting; `mise run size --pieces` (or its glyphplot equivalent) MUST fail when a module's name section holds `core::fmt::float` functions.
- REQ-FMT-050 (v0.2.0): Date labels MUST accept a strftime subset (`%Y %m %d %e %H %I %M %S %L %p %a %A %b %B %j %Z`) and default to 24-hour multi-scale labels unless the locale's hour cycle is 12-hour.
- REQ-LOCALE-010 (v0.2.0): The element MUST accept a BCP 47 `locale` and pass the core a locale object in d3's shape derived from `Intl`; with none, the core MUST use en-US built in.
- REQ-LOCALE-020 (v0.5.0): The CLI MUST accept a locale object as a JSON file in the same shape.
- REQ-TZ-010 (v0.1.0): Time scales MUST default to UTC, and the core MUST NOT read a clock, a zone or tzdata.
- REQ-TZ-020 (v0.2.0): The element MUST accept `timeZone` as `UTC`, a fixed offset or an IANA name, and MUST give the core a table of offset transitions covering the data's time extent and the view, computed with `Intl.DateTimeFormat`.
- REQ-TZ-030 (v0.2.0): Local-time ticks over the table MUST equal d3-scale's `scaleTime().ticks()` run under `TZ=<zone>` for every fixture case, across DST changes.
- REQ-TZ-040 (v0.5.0): The CLI MUST build the same table from the system's tzdata, and a frame drawn by the CLI and by the element from the same data, options, view and zone MUST be identical.
- REQ-FMT-060 (v0.1.0): No wasm module MUST link a tz database, a locale database, chrono, time, jiff, num-format or ICU4X; `cargo tree -e normal --target wasm32-unknown-unknown` for each module MUST show none of them.
- REQ-LAYOUT-010 (v0.1.0): Layout MUST be computed in whole cells in the order title, legend, x axis, panes, y gutters, x ticks, annotations, and MUST be a pure function of cols, rows, options, data extent and view.
- REQ-LAYOUT-020 (v0.1.0): When cols < 16 or rows < 5 a chart MUST draw no axis, title or legend; the title MUST NOT be drawn below 10 rows; x axis labels MUST NOT be drawn below 6 rows.
- REQ-LAYOUT-030 (v0.1.0): Y labels MUST be dropped, right axis first, when the gutters would exceed a third of the columns.
- REQ-LAYOUT-040 (v0.2.0): Panes MUST split rows by weight with at least 3 rows each, share the x scale, and align their plot areas to the same left and right gutters.
- REQ-LAYOUT-050 (v0.2.0): A legend with position `auto` MUST go outside in one row when it fits and rows ≥ 12, else inside the first pane when it takes at most a third of its width and height, else be left out of the frame.
- REQ-AXIS-010 (v0.2.0): A pane MUST support a left and a right y axis bound to different scales; gridlines MUST come from one axis only.
- REQ-ANNOT-010 (v0.2.0): The core MUST draw `hline`, `vline`, `band`, `label` and `marker` annotations in data coordinates, clipped to their pane; an annotation label on an axis MUST hide the tick labels it overlaps.

## Decisions with a real alternative

1. **Hand-written scales and formats, or crates.** Context: one module per chart, each paying for what it links. Options: (a) port d3's algorithms by hand into the core (+5.5 KB measured for linear and UTC); (b) chrono or time for dates and num-format for numbers (+8.5 to +22 KB, less capable: no ticks, no SI tick formats); (c) ICU4X for locale-correct output (+25 to +121 KB). Recommendation: (a), with d3 fixtures as the oracle.
2. **Where time zones resolve.** Options: (a) UTC and fixed offsets only; (b) the shell resolves IANA names with `Intl` into an offset table passed in once; (c) tzdata in wasm (+150 KB); (d) JS formats every label. Recommendation: (b); it keeps frames pure and modules small, and the CLI builds the same table natively.
3. **Where locale data comes from.** Options: (a) en-US only; (b) the shell derives a d3-shaped object from `Intl`, the CLI reads a JSON file; (c) ICU4X compiled data. Recommendation: (b).
4. **Label overlap: prevent or remove.** Options: (a) lower the tick count until whole labels fit (Vega's minStep idea, in cells); (b) generate ticks then hide overlapping labels (Vega's parity or greedy). Recommendation: (a): it keeps every shown label on a nice step and was free of overlaps in 400,000 cases; (b) can leave uneven gaps.
5. **Tick placement: nearest cell or snapped domain.** Options: (a) mark at the nearest cell, gridline at the nearest dot; (b) widen the domain so ticks land on cells exactly. Recommendation: (a) by default, (b) as `snap: "rows"` for small y axes, the default for bars and candlesticks.
6. **Time label style.** Options: (a) d3's en-US multi-scale labels (`03 PM`); (b) 24-hour by default, 12-hour when the locale's hour cycle says so, with a date context row. Recommendation: (b); fixture-test with d3 `utcFormat` and the same specifiers.

## Open questions for the owner

1. Minus sign in labels: U+2212 (d3's default, typographically right, needs the font) or `-` (every terminal font has it). Recommendation: U+2212 in the browser, `-` in the CLI by default, switchable in the locale object.
2. Default x label gap: one cell (denser, crowded at 80 columns) or two (fewer labels). Recommendation: two.
3. Should a y axis that follows streaming data snap its domain to nice steps (moves in jumps, no state) or ease between domains (smooth, but the frame then depends on history)? Recommendation: snap; determinism is an owner decision already.
4. Is a time context row (date under hours) wanted in the first milestone, or is it v0.2.0? Recommendation: v0.2.0, with candlesticks.
5. Should the CLI accept IANA zones from day one (jiff with the system tzdata, native only) or UTC and offsets first? Recommendation: from the CLI's first milestone; it costs the wasm nothing.

## Sources

- d3-array, `src/ticks.js:1-55`, https://github.com/d3/d3-array (commit be0ae0d).
- d3-scale, `src/linear.js:19-55`, `src/tickFormat.js:4-29`, `src/time.js:20-71`, `src/log.js:70-107`, `src/symlog.js:5-24`, https://github.com/d3/d3-scale (commit d6904a4).
- d3-format, `src/locale.js:11,134-141`, `src/precisionPrefix.js`, `src/precisionFixed.js`, `src/formatPrefixAuto.js`, `locale/de-DE.json`, https://github.com/d3/d3-format (commit ebdc2d5).
- d3-time, `src/ticks.js:14-50`, `src/hour.js:11,23`, https://github.com/d3/d3-time (commit 03d890d).
- d3-time-format, `locale/de-DE.json`, https://github.com/d3/d3-time-format (commit 9c473e0).
- Vega, `packages/vega-scale/src/ticks.js:21-99`, `packages/vega-view-transforms/src/Overlap.js:34-128`, https://github.com/vega/vega (commit fdd9ab9).
- Vega-Lite, `src/compile/axis/properties.ts:293-319`, https://raw.githubusercontent.com/vega/vega-lite/main/src/compile/axis/properties.ts (fetched 2026-10-18).
- plotters, `plotters/src/coord/ranged1d/types/numeric.rs:134-177`, `coord/ranged1d/mod.rs:141-169`, `chart/builder.rs:201`, `Cargo.toml:22,116`, https://github.com/plotters-rs/plotters (commit c63248e).
- ratatui, `ratatui-widgets/src/chart.rs:40-51,583,752-790,886-930`, https://github.com/ratatui/ratatui (commit 0297560).
- jiff 0.2.38, `Cargo.toml` features and `src/lib.rs:600-690` (the `js`, `tzdb-bundle-always`, `tzdb-bundle-platform` features), from crates.io.
- icu_datetime 2.3.0, `src/lib.rs:78-107` (static field sets and binary size), from crates.io.
- unicel: `crates/unicel/src/clock.rs:7-67`, `crates/unicel/src/pieces/data/candlesticks.rs:119-146`, `crates/unicel/src/pieces/data/bar_chart.rs:46-91`, `spec/SPEC.md:228,236,404,541` in `../animate-unicode`.
- Measurements: `discovery/bench/scales/` (`sizes.sh`, `tests/d3.rs`, `tests/props.rs`, `examples/fit.rs`, `timing.mjs`, `locale.mjs`, `fixtures/capture.mjs`, `fixtures/d3.jsonl`).
