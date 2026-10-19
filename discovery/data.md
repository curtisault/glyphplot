# Data model and injection

## Summary

A glyphplot chart should hold its data as columns of `f64` in wasm memory,
and every column should cross the boundary as a typed array with at most one
copy. Text formats should be parsed outside the chart modules. JSON is parsed
by `JSON.parse` in the TypeScript shell, CSV by the shell in the browser and by
the core in the CLI, and only typed columns and a small descriptor reach
wasm. A hand-written Arrow IPC reader goes in the core. The measurements
decide this. Getting two 1M-row `Float64Array` columns into wasm takes 4.3 ms
median and grows memory by the columns' own 15.4 MiB. The same data as JSON,
parsed by serde_json inside wasm, takes 163 ms, grows the module's memory by
80 MiB that is never returned, and adds 26.5–34.4 KB gzip to every module that
carries it. `JSON.parse` in the shell followed by one copy takes 73 ms and
adds 0 bytes to the module. A hand-written Arrow reader reads 1M rows in
7.4 ms and adds 3.8 KB gzip. arrow-rs adds 262 KB gzip and arrow2 adds 92 KB.
Live feeds go into a ring buffer that is allocated once, with a retention cap
in rows and, optionally, in age. Appending one sample costs 0.06 µs and a
batch of 1000 costs 3–4 µs in wasm, so 1 kHz and 10 Hz × 1000 are negligible.
Lines are drawn from M4 aggregates per dot column, cached by data version and
view. M4 over 1M points takes 2.5–2.8 ms in wasm at 160 and 640 dot columns
and keeps every column's true minimum and maximum. LTTB loses them in 95% of
columns. Candles are resampled to calendar intervals, and scatter is
rasterised point by point. The library should take celestial positions as
data and convert between frames. Planet positions belong in an optional
astronomy pack that uses JPL's Keplerian elements: 6.9 KB gzip, 1.6 µs for
eight planets, within 297 arcseconds of VSOP87A. VSOP87A itself costs 688 KB
gzip.

## Scope and method

This report covers the topic's bullets in PROMPT.md §2: how a page and the
CLI give a chart its data, the data contract and its types, the domain shapes
including celestial, downsampling, and the recommended API on each surface.

The measurements come from a throwaway crate,
`discovery/bench/data/` (README.md there says how to rerun). It holds one
wasm-bindgen boundary with each ingestion path, the ring and each downsampler
behind its own Cargo feature. `sizes.sh` builds one module per feature set
with `wasm-pack build --target web --release` and unicel's release profile
(`opt-level = "s"`, `lto`, `codegen-units = 1`, `panic = "abort"`, `strip`,
`wasm-opt -Os --enable-bulk-memory --enable-nontrapping-float-to-int`, as
`../animate-unicode/Cargo.toml` and `crates/wasm/Cargo.toml`). It prints each
`.wasm` raw and through `gzip -9`. `src/bin/native.rs` writes the fixtures and
times the native side. `bench.mjs` times the wasm side in Node. Each case runs
on a fresh wasm instance, so the memory growth it reports
(`memory.buffer.byteLength` after minus before) is that case's alone.

The fixtures are a random walk (`core::walk`, mulberry32 seed 7) of 1k, 100k
and 1M rows. Each row is `t` (epoch ms, one second apart from 2026-10-18) and
`v` (a float). Each fixture is written as CSV with epoch-ms times (33.1 MB at
1M), CSV with RFC 3339 times (44.1 MB), JSON columns (33.1 MB), JSON row
objects (43.1 MB) and an Arrow IPC stream (16.3 MB, batches of 65,536 rows,
`t` as `Timestamp(ms, "UTC")`, `v` as nullable `Float64`). Every wasm case
checks the sum of `v` against the source.

Machine and conditions: Apple M5 Pro, 18 cores, 48 GB, macOS 26.5.2, on
2026-10-18. The toolchain was rustc 1.97.1, wasm-pack 0.15.0, wasm-bindgen
0.2.129 and Node v26.5.1. The machine was shared with other discovery agents
benchmarking at the same time. The 1-minute load average was between 3.67
and 11.60 during the runs (`uptime` before and after each). Each wasm case ran
5 times per run and each native case 7 times per run, and each bench ran 3
times. The tables give the minimum of the minimums and the median of the
medians. Node's V8 stands in for Chrome. Chrome and Safari were not
measured, so browser numbers are **(unverified)** until agent 6's harness
runs them.

Not covered: Safari and Chrome timings, Web Worker parsing and transferables,
`SharedArrayBuffer`, Parquet, Arrow's file format, dictionary and Utf8 columns
in the Arrow reader (it reads Float64, Int64 and Timestamp), apache-arrow JS
decode times, `f32` storage, min/max pyramids, and the Moon's position.

## Findings: getting data in

### What wasm-bindgen does with each kind of argument

- A `&[f64]` or `Vec<f64>` parameter is passed by `passArrayF64ToWasm0` in the
  generated glue (`discovery/bench/data/target/pkg/all/bench.js:407-412`). The
  glue calls the module's `malloc` for `8n` bytes, runs
  `Float64Array.set` into wasm memory, and passes the pointer and length. It
  makes one copy. With `&[f64]` the glue frees the buffer after the call, so
  storing the values costs a second copy in Rust (`extend_from_slice`). With
  `Vec<f64>`, Rust takes the buffer and makes no second copy.
- A `&str` is encoded to UTF-8 into wasm memory (`passStringToWasm0`,
  `bench.js:414`), so any text format pays an encode and a copy before parsing.
- A buffer the Player owns (`reserve(n) -> *mut f64`) can be written by JS
  through `new Float64Array(memory.buffer, ptr, n)`. This is unicel's pattern
  for the grid (`../animate-unicode/web/src/mount.ts:88-91`), used in the
  other direction. Only the page's own copy into the view remains.
- A pitfall, measured: `new Float64Array(memory.buffer, player.reserve(n), n)`
  evaluates `memory.buffer` before `reserve`. When `reserve` grows memory, the
  old `ArrayBuffer` is detached and the constructor throws
  `TypeError: Cannot perform Construct on a detached ArrayBuffer` (first run of
  `bench.mjs`, 100k rows). The shell must take the address first and read
  `memory.buffer` after it. unicel's mount.ts already re-takes its views each
  frame for this reason (`mount.ts:88`).
- Wasm memory only grows. The core instruction set has `memory.grow` and no
  shrink (https://webassembly.github.io/spec/core/syntax/instructions.html,
  "Memory Instructions"). unicel shares one instance of a module among every
  tag of its category (`../animate-unicode/web/src/wasm.ts:1-8,20-34`).
  glyphplot shares one instance per chart type in the same way, so a spike
  in one chart's load stays in that module's memory for the life of the page.

### Each path measured (wasm in Node; 1M rows, two columns)

| path | median ms | memory growth | adds to module, gzip |
|---|---|---|---|
| (a) `Float64Array` as `&[f64]`, copied into a `Vec` | 5.58 | 23.1 MiB | 0 (base) |
| (a') `Float64Array` as `Vec<f64>` | 4.27 | 15.4 MiB | 0 (base) |
| (b) JS `set` into a Player-owned `Vec` | 4.99 | 15.4 MiB | 0 (base) |
| (b') JS writes the Player-owned `Vec` value by value | 7.30 | 15.4 MiB | 0 (base) |
| (c) JSON columns, serde_json in wasm | 163 | 80.0 MiB | +34.4 KB (+26.5 KB if serde_json is already in for options) |
| (c') JSON row objects, serde_json in wasm | 265 | 89.5 MiB | (same) |
| (c'') JSON columns, `JSON.parse` in JS, then (a') | 72.9 | 15.4 MiB | 0 |
| (c''') JSON rows, `JSON.parse`, columns built in JS, then (a') | 202 | 15.4 MiB | 0 |
| (d) CSV, epoch ms, parsed in wasm | 156 | 48.4 MiB | +17.3 KB (+16.1 KB with serde_json in) |
| (d') CSV, RFC 3339 times, parsed in wasm | 193 | 58.8 MiB | (same) |
| CSV parsed in JS (`split`, `Number`), epoch ms | 155 | JS heap | 0 |
| CSV parsed in JS, RFC 3339 (`Date.parse`) | 256 | JS heap | 0 |
| (e) Arrow IPC as `&[u8]`, hand-written reader | 7.42 | 33.0 MiB | +3.8 KB |
| (e') Arrow IPC written into a Player-owned buffer, same reader | 7.33 | 33.0 MiB | +3.8 KB |
| (e'') Arrow IPC as `&[u8]`, arrow-rs 60.0.0 | 10.3 | 38.1 MiB | +262 KB |
| Arrow IPC, arrow2 0.18.0 (size only) | — | — | +92.5 KB |

The full tables, including 1k and 100k rows and minimums, are under
Measurements.

What the numbers say:

- Typed arrays cost about 2 ns a value with one copy. Paths (a'), (b) and
  (b') are equivalent when the page already holds a typed array. Path (b)
  pays off only when the page fills the wasm view directly, for example
  decoding a binary WebSocket frame into it, because it saves the page's own
  allocation. Path (a) costs one extra copy and peaks at 1.5× the columns'
  bytes.
- JSON is the wrong thing to parse in wasm. serde_json in wasm is 2.2 times
  slower than `JSON.parse` plus a copy (163 ms against 73 ms for columns). It
  grows the shared module memory by 5.2 times the data (80 MiB for 15.4 MiB of
  columns, because the string is copied in as UTF-8 and parsed into
  `Vec<Option<f64>>`). It adds 26.5–34.4 KB gzip to every chart module. Row
  objects are slow either way (202–265 ms), because building 1M JS objects
  is the cost. Columns are the fast JSON shape.
- CSV parsed in wasm and CSV parsed in JS cost about the same for numbers (156
  against 155 ms). Wasm is faster for RFC 3339 times (193 against 256 ms,
  because the hand-written RFC 3339 parser beats `Date.parse`). In wasm, CSV
  costs +17.3 KB gzip, most of it Rust's `str::parse::<f64>`, which
  serde_json does not share (+16.1 KB even with serde_json in), and it grows
  memory 3.1 times the data. The CLI needs a CSV parser in Rust anyway. The
  browser does not need one in each chart module.
- Arrow IPC is the fast path for big data. The hand-written reader is the same
  speed as typed arrays plus the IPC copy (7.4 ms), costs 3.8 KB gzip, and is
  138 lines (`src/core.rs`, `mod ipc`). The Arrow spec pads and aligns IPC
  buffers to 8 bytes ("Buffer Alignment and Padding",
  https://arrow.apache.org/docs/format/Columnar.html). So a reader could
  also view Float64 columns in place inside the IPC bytes instead of copying
  them out. That would cut growth from 33 to about 16.5 MiB **(unverified)**.
  arrow-rs is correct and 1.4 times slower, but at 262 KB gzip it is out of
  scale for a chart module. arrow2 is archived upstream (GitHub API
  `repos/jorgecarleitao/arrow2`: `"archived": true`, last push 2024-02-27).
  Decoding in the shell with apache-arrow 21.2.0 would cost 48.5 KB gzip of
  JS (`Arrow.esnext.min.js`, 182,479 bytes raw, `gzip -9`), shared across
  modules but loaded by any page that uses Arrow.
- At 1k rows every path takes under 1 ms (`JSON.parse` + copy 0.12 ms, serde
  in wasm 0.85 ms), so the `data` attribute's JSON is cheap at the sizes it is
  meant for.

### Streaming and the ring

| append into a ring of 1M (wasm, Node) | median a call | memory growth |
|---|---|---|
| `push(t, v)`, one sample a call (10,000 calls) | 0.062 µs | 0 |
| `push_batch(&[f64], &[f64])`, 1000 a call (100 calls) | 3.78 µs | 0 |
| staging buffer (`staging(n)` then JS `set`, `commit(n)`), 1000 a call | 2.97 µs | 0 |
| `new Ring(1_000_000)` (two `Vec<f64>`) | 1.39 ms once | 15.4 MiB once |

At 1 kHz single appends cost 62 µs a second, and at 10 Hz batches of 1000
cost 38 µs a second. The boundary is not the bottleneck for live feeds. The
page's own event handling is. Even so, the shell should coalesce appends made
within one animation frame into one staging write. Each wasm call is then
made once a frame, not once a message. A ring allocated once never grows
memory after its first fill. A growable `Vec` trimmed from the front would
`memmove` the whole retained history on every trim. LightweightCharts' streaming
model is `update(bar)`: a time equal to the last replaces it, and an earlier
time is refused unless `historicalUpdate` is set
(`src/api/iseries-api.ts:165-190`,
https://github.com/tradingview/lightweight-charts). That is the rule a
forming candle needs.

## Findings: the data contract

### Types and their storage

| contract type | in JSON | binary (typed array / Arrow) | held in the chart as |
|---|---|---|---|
| `f64` | number, null | `Float64Array` / Float64 | `f64`, NaN missing |
| `f32` | number, null | `Float32Array` / Float32 | `f64` (widened once) |
| `i64` | number, null | `BigInt64Array` / Int64 | `f64` (exact to 2^53) |
| `u32` | number, null | `Uint32Array` / UInt32 | `f64`, or `u32` for indexes |
| `bool` | true, false, null | `Uint8Array` / Bool bitmap | `u8` (0, 1, 255 missing) |
| `timestamp` | number in `time_unit`, RFC 3339 string, null | `Float64Array` / Timestamp | `f64` epoch ms; `us`/`ns` as `f64` offsets from an `i64` origin |
| `duration` | number in `time_unit`, null | `Float64Array` / Duration | `f64` ms |
| `category` | index into `categories`, null | `Uint32Array` + labels / Dictionary | `u32` codes, `u32::MAX` missing, labels once |
| `string` | string, null | labels list / Utf8 | offsets + UTF-8 bytes, for labels and readouts only |

Reasons:

- Everything numeric becomes `f64` so that scales, downsamplers and hit tests
  are written once rather than once per type. That avoids generic code
  monomorphised per type, which grows each module. `f32` input is widened and
  costs memory. An `f32` storage option is an open question below.
- Epoch milliseconds in `f64` are exact for integers up to 2^53 ms, about
  285,000 years. Microseconds are exact until 2^53 µs after 1970, about
  the year 2255. Nanoseconds since 1970 are not exact: the `f64` step at
  1.8e18 ns is 256 ns. A packet capture's nanosecond times therefore keep
  their resolution only as offsets from an origin. 2^53 ns is 104 days of
  span at 1 ns resolution, which covers any one capture. These are
  arithmetic facts about IEEE 754 binary64.
- Timestamps are instants in UTC. `zone` names the IANA zone that ticks and
  labels are drawn in, because calendar ticks on days and months depend on it.
  The frame stays a pure function of data, options, view and `t` only if the
  zone comes from the data or the options and not from the host. unicel
  passes the wall clock in for the same reason (`set_clock`,
  `../animate-unicode/crates/wasm/src/lib.rs:105-110`).

### Missing values, gaps and order

- One representation inside the chart: NaN for numbers and times,
  `u32::MAX` for categories. JSON `null`, an empty CSV field and a cleared
  Arrow validity bit all map to it. The hand-written reader does this in one
  pass over the bitmap (`src/core.rs`, `ipc::read`).
- A missing y breaks a line or area into separate runs. A missing x drops the
  row. uPlot draws the same way: `null` y values are gaps, and
  `spanGaps: true` joins them (https://github.com/leeoniya/uPlot,
  `docs/README.md:41-44,98`). A chart option `gap` (a duration) may also break a
  line where consecutive x values are further apart than that, as market
  sessions and telemetry dropouts need.
- x must be non-decreasing for time series, and binary search and the
  downsamplers depend on it. uPlot requires ascending, unique x
  (`docs/README.md:43`). LightweightCharts requires ordered data in `setData`
  (`iseries-api.ts:147`). Rejecting unsorted data surprises users,
  and the check is one O(n) pass. The recommendation is to check on load and
  stably sort a copy with one warning when needed.
- Streaming rows with an earlier x than the last are late. The default is to
  drop them and count them. Inserting them is an option for later (open
  question below). An equal x replaces the last row, as LightweightCharts
  does.

### Units and scale factors

`unit` is display text only (`USD`, `°C`, `B/s`). `scale` and `offset` turn
stored integers into displayed values: prices in integer cents use
`scale: 0.01`, and a sensor in raw ADC counts uses its calibration. They apply
when formatting ticks, readouts and the data table, never to the stored
values. Downsampling and hit testing therefore stay exact on the stored values.

### Channels: which column is what

A chart finds each channel (x, y, open, ..., source, target, ra, dec) in
this order: the option `encode` (`{"x": "time", "y": ["p50", "p99"]}`), then a
column's `role`, then a column whose name is the channel's name. Small data
then needs no configuration (`{"t": [...], "close": [...]}` draws), and
large or foreign data can be mapped without being renamed.

### Domain shapes

Each shape is a table, or a pair of tables, with columns in given roles. The
shapes are named so that a chart's META can say which one it takes, as
unicel's `Meta.data` names a shape (`../animate-unicode/spec/ROADMAP.md:777`,
REQ-PIECE-506).

| shape | roles | used by |
|---|---|---|
| `series` | `x` (time or number), one or more `y`, optional `series` category for long format | line, area, step, sparkline, band (`lo`, `hi`) |
| `ohlcv` | `x`, `open`, `high`, `low`, `close`, optional `volume` | candlestick, OHLC, volume pane |
| `values` | one `value` column, unbinned | histogram (bins in the chart), box, violin |
| `bins` | `lo`, `hi`, `count` | histogram already binned |
| `counts` | `category`, one or more `value` | bar, stacked bar, pie-like |
| `points` | `x`, `y`, optional `size`, `color`, `label`, `series` | scatter, bubble |
| `matrix` | a matrix: rows × cols, row-major, labels | heatmap, adjacency matrix, calendar heatmap |
| `graph` | tables `nodes` (`id`, optional `label`, `group`, `weight`, `x`, `y`) and `edges` (`source`, `target`, optional `weight`) | network, Sankey, chord |
| `geo` | `lon`, `lat` in degrees (WGS 84), frame `geographic` | maps, tracks |
| `sky` | `ra`, `dec` (frame `icrs` or `ecliptic`) or `az`, `alt` (frame `altaz`), optional `mag`, `label` | sky chart |
| `orbits` | `a` (au), `e`, `i`, `node`, `peri`, `anomaly` (degrees), `epoch` | orbit plot |
| `ephemeris` | `x` time plus `ra`/`dec` or `x`/`y`/`z` (frame `heliocentric-ecliptic`) | orbit plot, sky chart tracks |

Edges name nodes either by `u32` index into `nodes` or by strings equal to
`nodes.id`. The shell maps strings to indexes, so the core holds only
indexes. Angles are always degrees. RA is degrees, not hours, and the
formatter shows hours.

### Draft schema (for `spec/schemas/data.schema.json`)

This schema and the example after it validate with jsonschema 4.25.1
(`Draft202012Validator`, `check_schema`, format checker on), run with
`uv run --with 'jsonschema[format]==4.25.1'`. The two shorthands
(`{"t":["2026-10-18T14:52:00Z"],"close":[1.5]}` and `[{"t":1,"v":null}]`)
also validate. Two wrong documents are rejected: a category column without
`categories`, and `time_unit` on an `f64` column.

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://github.com/curtisault/glyphplot/spec/schemas/data.schema.json",
  "title": "Chart data",
  "description": "What a chart draws: named tables of typed columns and named matrices. In JSON every column carries its values; through the binary API (typed arrays, Arrow IPC) the same descriptor travels without `values` and the values arrive as buffers. Two shorthands, an object of arrays and an array of row objects, are normalised by the shell to `data` before anything crosses into wasm.",
  "oneOf": [
    { "$ref": "#/$defs/data" },
    { "$ref": "#/$defs/columnsShorthand" },
    { "$ref": "#/$defs/rowsShorthand" }
  ],
  "$defs": {
    "name": { "type": "string", "pattern": "^[A-Za-z_][A-Za-z0-9_.-]{0,63}$" },
    "data": {
      "type": "object",
      "required": ["tables"],
      "additionalProperties": false,
      "properties": {
        "version": { "const": 1, "description": "The data contract's version." },
        "tables": {
          "type": "object",
          "minProperties": 1,
          "propertyNames": { "$ref": "#/$defs/name" },
          "additionalProperties": { "$ref": "#/$defs/table" },
          "description": "Tables by name. A chart that draws one table takes the first; a graph takes `nodes` and `edges`."
        },
        "matrices": {
          "type": "object",
          "propertyNames": { "$ref": "#/$defs/name" },
          "additionalProperties": { "$ref": "#/$defs/matrix" }
        }
      }
    },
    "table": {
      "type": "object",
      "required": ["columns"],
      "additionalProperties": false,
      "properties": {
        "length": { "type": "integer", "minimum": 0, "description": "Rows. Required when no column carries values; otherwise every column's values have this length." },
        "sorted": { "$ref": "#/$defs/name", "description": "A column the rows are in non-decreasing order of. The chart verifies it on load and sorts a copy if it is wrong." },
        "frame": {
          "enum": ["icrs", "ecliptic", "galactic", "altaz", "geographic", "heliocentric-ecliptic"],
          "description": "The coordinate frame of the table's angle or position columns (roles ra/dec, lon/lat, az/alt, x/y/z). Angles are degrees; distances are au unless a column's `unit` says otherwise."
        },
        "epoch": { "type": "string", "description": "The equinox or epoch of a celestial frame, as `J2000` or an RFC 3339 date-time." },
        "columns": { "type": "array", "minItems": 1, "maxItems": 256, "items": { "$ref": "#/$defs/column" } }
      }
    },
    "column": {
      "type": "object",
      "required": ["name", "type"],
      "additionalProperties": false,
      "properties": {
        "name": { "$ref": "#/$defs/name" },
        "type": {
          "enum": ["f64", "f32", "i64", "u32", "bool", "timestamp", "duration", "category", "string"],
          "description": "f64, f32, i64 and u32 are numbers; timestamp is an instant since 1970-01-01T00:00:00Z in `time_unit`; duration is a span in `time_unit`; category is an index into `categories`; string is a free label per row."
        },
        "role": {
          "enum": ["x", "y", "y2", "series", "open", "high", "low", "close", "volume", "lo", "hi", "count", "value", "category", "size", "color", "label", "id", "source", "target", "weight", "group", "ra", "dec", "lon", "lat", "az", "alt", "mag", "z", "a", "e", "i", "node", "peri", "anomaly", "epoch"],
          "description": "What the column is to a chart. A chart finds each channel by the option `encode`, then by `role`, then by a column of that name."
        },
        "unit": { "type": "string", "maxLength": 32, "description": "The unit of the values for display and formatting (`USD`, `°C`, `B/s`, `%`). Not a conversion." },
        "scale": { "type": "number", "description": "Displayed value = stored value × scale + offset (prices in integer cents: scale 0.01)." },
        "offset": { "type": "number" },
        "time_unit": { "enum": ["s", "ms", "us", "ns"], "description": "For timestamp and duration columns; default ms." },
        "zone": { "type": "string", "description": "For timestamp columns: the IANA zone (`America/New_York`) or `UTC` in which ticks and labels are drawn. Values are UTC instants whatever the zone; default UTC." },
        "categories": { "type": "array", "items": { "type": "string" }, "maxItems": 65535, "description": "For category columns: the labels the values index, in legend order." },
        "values": {
          "type": "array",
          "description": "The column's values in JSON. null is missing: NaN for a number or time, no category, an empty label. A timestamp may be a number in `time_unit` or an RFC 3339 string.",
          "items": { "type": ["number", "string", "boolean", "null"] }
        }
      },
      "allOf": [
        { "if": { "properties": { "type": { "const": "category" } } },
          "then": { "required": ["categories"], "properties": { "values": { "items": { "type": ["integer", "null"], "minimum": 0 } } } } },
        { "if": { "properties": { "type": { "enum": ["f64", "f32", "i64", "u32", "duration"] } } },
          "then": { "properties": { "values": { "items": { "type": ["number", "null"] } } } } },
        { "if": { "properties": { "type": { "const": "timestamp" } } },
          "then": { "properties": { "values": { "items": { "anyOf": [{ "type": ["number", "null"] }, { "type": "string", "format": "date-time" }] } } } } },
        { "if": { "properties": { "type": { "const": "bool" } } },
          "then": { "properties": { "values": { "items": { "type": ["boolean", "null"] } } } } },
        { "if": { "properties": { "type": { "const": "string" } } },
          "then": { "properties": { "values": { "items": { "type": ["string", "null"], "maxLength": 256 } } } } },
        { "if": { "required": ["time_unit"] },
          "then": { "properties": { "type": { "enum": ["timestamp", "duration"] } } } }
      ]
    },
    "matrix": {
      "type": "object",
      "required": ["rows", "cols"],
      "additionalProperties": false,
      "properties": {
        "rows": { "type": "integer", "minimum": 0 },
        "cols": { "type": "integer", "minimum": 0 },
        "values": { "type": "array", "items": { "type": ["number", "null"] }, "description": "rows × cols values, row by row; null is missing." },
        "row_labels": { "type": "array", "items": { "type": "string" } },
        "col_labels": { "type": "array", "items": { "type": "string" } },
        "unit": { "type": "string", "maxLength": 32 },
        "symmetric": { "type": "boolean", "description": "An adjacency matrix of an undirected graph: only the upper triangle is read." }
      }
    },
    "columnsShorthand": {
      "type": "object",
      "minProperties": 1,
      "propertyNames": { "$ref": "#/$defs/name", "not": { "enum": ["tables", "matrices", "version"] } },
      "additionalProperties": { "type": "array", "items": { "type": ["number", "string", "boolean", "null"] } },
      "description": "`{\"t\": [...], \"close\": [...]}`: one table of columns by name. Types are inferred: numbers f64, RFC 3339 strings timestamps (ms, UTC), other strings categories, booleans bool."
    },
    "rowsShorthand": {
      "type": "array",
      "items": { "type": "object", "additionalProperties": { "type": ["number", "string", "boolean", "null"] } },
      "description": "`[{\"t\": ..., \"close\": ...}, ...]`: one table of rows, turned into columns by the shell with the same inference."
    }
  }
}
```

The example. It validates against the schema above. It has an OHLCV table
with a category, a bool and a label column, a graph, a sky table, a
binary-only descriptor (no `values`; the buffers come as typed arrays) and a
matrix.

```json
{
  "version": 1,
  "tables": {
    "prices": {
      "length": 3,
      "sorted": "t",
      "columns": [
        { "name": "t", "type": "timestamp", "role": "x", "time_unit": "ms", "zone": "America/New_York", "values": [1792330200000, 1792330260000, "2026-10-18T14:52:00Z"] },
        { "name": "open", "type": "f64", "role": "open", "unit": "USD", "values": [182.40, 182.55, 182.31] },
        { "name": "high", "type": "f64", "role": "high", "unit": "USD", "values": [182.71, 182.60, 182.50] },
        { "name": "low", "type": "f64", "role": "low", "unit": "USD", "values": [182.30, 182.20, 182.05] },
        { "name": "close", "type": "f64", "role": "close", "unit": "USD", "values": [182.55, 182.31, null] },
        { "name": "volume", "type": "i64", "role": "volume", "values": [12000, 9800, 15100] },
        { "name": "sym", "type": "category", "role": "series", "categories": ["ACME"], "values": [0, 0, 0] },
        { "name": "halted", "type": "bool", "values": [false, false, true] },
        { "name": "note", "type": "string", "role": "label", "values": [null, null, "halt"] }
      ]
    },
    "nodes": {
      "columns": [
        { "name": "id", "type": "string", "role": "id", "values": ["gw", "db", "web"] },
        { "name": "group", "type": "category", "role": "group", "categories": ["edge", "core"], "values": [0, 1, 1] }
      ]
    },
    "edges": {
      "columns": [
        { "name": "source", "type": "u32", "role": "source", "values": [0, 0] },
        { "name": "target", "type": "u32", "role": "target", "values": [1, 2] },
        { "name": "bps", "type": "f64", "role": "weight", "unit": "B/s", "values": [1.2e6, 3.4e5] }
      ]
    },
    "stars": {
      "frame": "icrs",
      "epoch": "J2000",
      "columns": [
        { "name": "ra", "type": "f64", "role": "ra", "unit": "deg", "values": [101.2872, 88.7929] },
        { "name": "dec", "type": "f64", "role": "dec", "unit": "deg", "values": [-16.7161, 7.4071] },
        { "name": "mag", "type": "f32", "role": "mag", "values": [-1.46, 0.42] },
        { "name": "name", "type": "string", "role": "label", "values": ["Sirius", "Betelgeuse"] }
      ]
    },
    "binary": {
      "length": 1000000,
      "sorted": "t",
      "columns": [
        { "name": "t", "type": "timestamp", "role": "x", "time_unit": "ns" },
        { "name": "rtt", "type": "f32", "role": "y", "unit": "ms" }
      ]
    }
  },
  "matrices": {
    "latency": {
      "rows": 2, "cols": 3,
      "values": [1.0, 2.5, null, 0.5, 4.0, 3.0],
      "row_labels": ["eu", "us"], "col_labels": ["a", "b", "c"],
      "unit": "ms"
    }
  }
}
```

The schema does not check that every column of a table has the same length,
that `sorted` names a column, or that edge indexes are within `nodes`. The
shell checks these and warns, as unicel ignores data of the wrong shape with
one warning (REQ-DATA-003, `../animate-unicode/spec/ROADMAP.md:782`).

## Findings: celestial coordinates and orbits

What a sky chart and an orbit plot need:

- A sky chart draws points given in a frame: ICRS RA and Dec (J2000),
  ecliptic longitude and latitude, galactic coordinates, or horizontal alt and
  az. Converting ICRS to ecliptic is one rotation by the obliquity
  (23.43928° at J2000; `core::ra_dec` does the reverse rotation). Converting
  RA and Dec to alt and az needs the observer's latitude and longitude and
  the sidereal time, which is a short polynomial in the Julian date (Meeus,
  *Astronomical Algorithms*, 2nd ed., ch. 12 and 13). This is projection
  maths and belongs in the sky chart module, with an orthographic or
  stereographic projection onto the dot grid.
- Stars come as data: catalogue positions with magnitudes. That is a table
  in frame `icrs`, never something to compute.
- Planets can be computed or given. Measured (native, `src/bin/native.rs`,
  2026-10-18 0h, observer at the Earth–Moon barycentre in both models,
  geometric positions with no light time, aberration or precession to date):

| model | adds to module, gzip | 8 planets, native | max difference |
|---|---|---|---|
| JPL approximate Keplerian elements, Table 1 (1800–2050) | +6.9 KB (mostly `sin`, `cos`, `atan2`, which a polar or sky chart carries anyway) | 1.5 µs min, 1.6–2.0 µs median | — |
| VSOP87A (vsop87 3.0.0, https://github.com/Razican/vsop87-rs) | +680 KB (688 KB module) | 0.20–0.33 ms min, 0.25–0.48 ms median | — |
| Kepler against VSOP87A, separation in RA/Dec | | | Uranus 4.6″, Mercury 7.8″, Mars 10.8″, Venus 31.1″, Neptune 34.2″, Jupiter 86.2″, Saturn 297.1″ |

JPL states nominal errors for the Table 1 elements of 15–600″ in heliocentric
longitude, the largest for Saturn (https://ssd.jpl.nasa.gov/planets/approx_pos.html,
"Accuracy"). The measured differences fall inside those errors. One braille
dot of an all-sky chart 160 dots wide spans 2.25°, which is 8100″. Even a
zoomed 10° field 160 dots wide spans 225″ a dot. So the Keplerian model is
dot-accurate for any terminal-resolution sky chart, and VSOP87's precision
cannot be seen at this resolution. The Kepler tables do not cover the Moon.
It needs its own series (Meeus ch. 47, some 60 terms in longitude), whose
size was not measured.

Recommendation: chart modules compute no ephemerides. The sky chart and the
orbit plot take positions or elements as data, in a declared frame, and do
frame conversions and projection. An optional astronomy domain pack (its own
module or TypeScript, as agent 7's brief requires of packs) computes planet
positions from the JPL Keplerian elements, plus the Moon from a short series,
for pages that want a live sky without a data source. Precise work brings
ephemerides as data, for example from JPL Horizons
(https://ssd.jpl.nasa.gov/horizons/), in the `ephemeris` shape.

## Findings: downsampling

Implemented in `src/core.rs` (`columns` for min-max and M4, `lttb`). Each
takes segments, so a wrapped ring is read in place. Each binary-searches the
view `[x0, x1)` and scans only the rows inside it.

- **min-max per column**: the minimum and maximum of each dot column, in
  index order, at most 2 points a column.
- **M4** (Jugel et al., "M4: A Visualization-Oriented Time Series Data
  Aggregation", PVLDB 7(10), 2014, https://www.vldb.org/pvldb/vol7/p797-jugel.pdf):
  the first, last, minimum and maximum of each column, at most 4 points a
  column. The paper shows that a line drawn through M4's points rasterises
  identically to one drawn through all the data at that width, because the
  first and last points carry the segments between columns, which min-max
  loses.
- **LTTB** (Steinarsson, "Downsampling Time Series for Visual
  Representation", MSc thesis, University of Iceland, 2013,
  https://skemman.is/handle/1946/15343; the page returned 403 to curl, so the
  citation is from the thesis's record): one point a bucket, chosen by the
  largest triangle area.

Measured on 1M points with the whole series in view:

| downsampler | dot columns | points out | wasm min / median ms | native min / median ms | columns where the output misses the true min or max |
|---|---|---|---|---|---|
| min-max | 160 | 320 | 2.33 / 2.71 | 1.19 / 2.01 | 0 of 160 |
| M4 | 160 | 635 | 2.35 / 2.52 | 1.19 / 2.04 | 0 of 160 |
| LTTB | 160 | 160 | 1.90 / 2.29 | 1.63 / 2.12 | 153 of 160 |
| min-max | 640 | 1280 | 2.59 / 2.87 | 1.26 / 2.18 | 0 of 640 |
| M4 | 640 | 2516 | 2.44 / 2.78 | 1.23 / 2.22 | 0 of 640 |
| LTTB | 640 | 640 | 2.09 / 2.31 | 1.60 / 2.50 | 607 of 640 |
| M4, wrapped 1M ring | 160 | 633 | 2.03 / 2.56 | 1.13 / 2.09 | — |
| M4, wrapped 1M ring | 640 | 2525 | 2.59 / 2.75 | 1.21 / 2.08 | — |
| M4, view of 10k rows of 1M | 640 | 2178 | 0.035 / 0.077 | 0.015 / 0.015 | — |

The 160 and 640 dot columns are 80 and 320 cells of braille. All three take
about the same time, because one pass over 1M points dominates. LTTB outputs
one point a column by construction, so it cannot reach both of a column's
extremes. It missed one or both in 95% of columns, which for telemetry means
a latency spike can vanish from the chart.

Which to use:

- **Line, area, step, sparkline**: M4 at the plot's dot-column width
  (2 a cell for braille, 2 for sextants and octants), then the braille line
  rasteriser. It costs 2.5–2.8 ms in wasm for 1M points in view. It is
  recomputed only when the data version, the view's x range or the width
  changes, so a hover frame costs nothing. A zoomed view of 10k rows costs
  0.08 ms. A pan at 60 Hz over the full 1M stays near 2.8 ms a frame. If that
  is too much for agent 6's frame budget, a min/max pyramid (blocks of 2^k
  rows) makes any view O(columns × log n) **(unverified)**.
- **Candlestick, OHLC**: resample to a calendar interval (1 s, 5 s, 15 s,
  1 min, 5 min, 15 min, 1 h, 4 h, 1 d, 1 w, 1 mo), with open the first, high
  the maximum, low the minimum, close the last and volume the sum. Pick the
  smallest interval for which the candles fit the plot at one candle per
  cell, or per three dot columns. These are M4's aggregates over
  time-aligned buckets rather than pixel-aligned ones. Pixel-aligned buckets
  would make candles that change meaning as the view pans. Never LTTB.
- **Scatter, bubble**: no downsampling. Every visible point is set in the dot
  grid, an O(n) pass like min-max. A dot is on or off, so drawing all points
  is exact. Colour by density counts points per cell **(unverified cost;
  expected near the min-max pass)**.
- **Bar, histogram**: aggregate by bin (count or sum), never sample.
- LTTB is an option for a sparkline whose shape matters more than its
  extremes, never a default.

## Recommended API

### Rust core crate

- `Table` holds named columns (`Column { name, kind, unit, scale, offset, zone, values: Vec<f64> | Codes(Vec<u32>, labels) | Labels(...) }`),
  a `sorted` column and a data version (`u64`, bumped on every change).
- `Table::from_columns(descriptor, buffers)`, `Table::from_arrow_ipc(&[u8])`
  (the hand-written reader, extended to Float32, Int32, UInt32, Bool, Utf8 and
  Dictionary), `Table::from_csv(impl BufRead)` and `Table::from_json(&str)`.
  The last two sit behind a `text` feature that the CLI and native users
  enable and the wasm chart modules do not.
- `Live`: a ring of columns with `retain_rows` and `retain_age`, `append_row`,
  `append_batch`, the equal-x replace rule and a late-row counter. It exposes
  `segments()` for the chart.
- `sample::m4`, `sample::minmax`, `sample::ohlc(interval)` and `sample::lttb`,
  all on segments, all writing into a caller's reused buffer. A cache keyed
  by `(data version, x0, x1, width)`.

### wasm boundary (the Player)

- `reserve_column(i, kind, n) -> ptr` and `commit(descriptor_json)` replace
  all the data. The descriptor is the schema's `data` without `values`, a few
  hundred bytes, so serde_json, which unicel already uses for options
  (`../animate-unicode/crates/unicel/src/options.rs:9`), reads it.
  `set_column(i, Vec<f64>)` is the same through the glue for pages that
  already hold a typed array.
- `set_arrow(&[u8])` or `reserve_bytes(n)` then `commit_arrow()`.
- `staging(n) -> ptr` and `append(n)` for streams, with one call a frame.
- No text parsing in a chart module.

### The element `<glyph-plot>`

- `data` property: the data object, the columns shorthand, the rows
  shorthand, `{ descriptor, columns: { name: TypedArray } }`, or an Arrow IPC
  `ArrayBuffer`/`Uint8Array`. Setting it replaces everything. Reading it
  returns the descriptor and lengths, not a copy of the data.
- `data` attribute: JSON (either shorthand or the data object), for small
  inline data and the server-rendered first frame.
- `src` attribute: a URL the shell fetches. Its type is taken from the
  `Content-Type` or the extension (`.csv`, `.json`, `.ndjson`, `.arrow`,
  `.arrows`). CSV and JSON are parsed in TS.
- `append(rows)`: one row object (`{t, v}`), an array of row objects, or
  columns `{t: Float64Array, v: Float64Array}`. Appends are coalesced per
  animation frame.
- `retain` attribute and option: `{"rows": 1000000, "age": "1h"}`. `clear()`.
- A `glyph-data` event after each commit with `{rows, dropped, late, version}`.

### JS object without the element

`const chart = await glyphplot.chart("line", { options })`, then
`chart.setData(...)` and `chart.append(...)` with the same forms as the
property, and `chart.frame(t)` / `chart.text()`. The element is a thin
wrapper over it (agent 8 decides the names).

### CLI

- `glyphplot line --from data.csv`, `--from -` for stdin, `--format
  csv|tsv|json|ndjson|arrow` when the extension does not say.
- `--x t --y p50,p99` map channels (the `encode` option), and
  `--zone America/New_York` sets the zone.
- Streaming: `--follow` reads NDJSON or CSV lines from a pipe or a growing
  file as they arrive, appends them to the ring each frame, and honours
  `--retain-rows` and `--retain-age`. Example:
  `tail -f metrics.csv | glyphplot line --follow --from -`.
- The CLI links the core's `text` feature, since size does not matter natively.

## Measurements

All wasm timings are Node v26.5.1 (V8 standing in for Chrome), each case on a
fresh instance, 5 repetitions × 3 runs. All native timings are the release
profile (`opt-level = "s"`), 7 repetitions × 3 runs. The machine was an Apple
M5 Pro (18 cores, 48 GB, macOS 26.5.2) on 2026-10-18, shared with other
benchmarks, with 1-minute load averages of 3.67–11.60. Each cell is the
minimum of minimums, then the median of medians. Commands are those in
`discovery/bench/data/README.md`.

| what | number | how |
|---|---|---|
| Base module (Table, typed-array paths, wasm-bindgen, dlmalloc) | 18,161 B raw, 8,269 B gzip | `sizes.sh`, `gzip -9`, no features |
| Base + serde_json `Value` options, as unicel | 40,114 / 19,195 B | `--features options` |
| + JSON data (serde derive, columns and rows) over base | 92,711 / 42,623 B (+34,354 gz) | `--features json` |
| + JSON data over options | 100,871 / 45,722 B (+26,527 gz) | `--features options,json` |
| + CSV parser over base | 40,319 / 25,616 B (+17,347 gz) | `--features csv` |
| + CSV over options | 61,459 / 35,304 B (+16,109 gz) | `--features options,csv` |
| + hand-written Arrow IPC reader | 26,357 / 12,035 B (+3,766 gz) | `--features arrowmin` |
| + arrow-rs 60.0.0 IPC reader | 1,028,515 / 269,953 B (+261,684 gz) | `--features arrowrs` |
| + arrow2 0.18.0 IPC reader | 416,399 / 100,780 B (+92,511 gz) | `--features arrow2`, `RUSTFLAGS='--cfg getrandom_backend="wasm_js"'` |
| + ring | 21,289 / 9,471 B (+1,202 gz) | `--features ring` |
| + min-max, M4, LTTB | 23,109 / 10,545 B (+2,276 gz) | `--features downsample` |
| + Kepler planets and RA/Dec | 31,784 / 15,200 B (+6,931 gz) | `--features kepler` |
| + VSOP87A planets | 974,033 / 687,930 B (+679,661 gz) | `--features vsop` |
| json + csv + arrowmin + ring + downsample + kepler | 142,599 / 70,581 B | `--features all` |
| apache-arrow 21.2.0 JS, whole library | 182,479 B / 48,477 B gzip | `Arrow.esnext.min.js` from the npm tarball, `gzip -9` |
| wasm-bindgen glue JS, `all` | 18,473 B / 3,885 B gzip | `target/pkg/all/bench.js` |
| Typed arrays in, 1k / 100k / 1M rows, (a') `Vec<f64>` | 0.028/0.058; 0.317/0.516; 3.73/4.27 ms; growth 0 / 1.6 / 15.4 MiB | `bench.mjs` |
| (a) `&[f64]` + copy | 0.033/0.077; 0.487/0.688; 3.17/5.58 ms; 0 / 2.4 / 23.1 MiB | `bench.mjs` |
| (b) reserve + `set` | 0.018/0.033; 0.414/0.550; 4.48/4.99 ms; 0 / 1.6 / 15.4 MiB | `bench.mjs` |
| (b') reserve + per-value writes | 0.043/0.102; 0.390/0.667; 5.64/7.30 ms | `bench.mjs` |
| (c) JSON columns, serde in wasm | 0.666/0.847; 17.2/17.8; 160/163 ms; 0.1 / 9.4 / 80.0 MiB | `bench.mjs` |
| (c') JSON rows, serde in wasm | 0.718/0.940; 25.3/28.9; 256/265 ms; 0.1 / 10.3 / 89.5 MiB | `bench.mjs` |
| (c'') JSON columns, `JSON.parse` + (a') | 0.089/0.119; 6.78/7.41; 65.5/72.9 ms; 0 / 1.6 / 15.4 MiB | `bench.mjs` |
| (c''') JSON rows, `JSON.parse` + JS columns + (a') | 0.158/0.271; 8.75/16.4; 178/202 ms | `bench.mjs` |
| (d) CSV epoch ms in wasm | 0.676/0.842; 16.2/17.4; 138/156 ms; 0 / 5.8 / 48.4 MiB | `bench.mjs` |
| (d') CSV RFC 3339 in wasm | 0.501/0.848; 20.2/21.5; 180/193 ms; 0.1 / 6.8 / 58.8 MiB | `bench.mjs` |
| CSV in JS, epoch ms | 0.176/0.316; 13.5/14.9; 133/155 ms | `csv-js.mjs` |
| CSV in JS, RFC 3339 | 0.206/0.415; 22.4/24.5; 247/256 ms | `csv-js.mjs` |
| (e) Arrow, hand-written, `&[u8]` | 0.045/0.067; 0.789/1.39; 6.52/7.42 ms; 0 / 4.8 / 33.0 MiB | `bench.mjs` |
| (e') Arrow, hand-written, Player-owned bytes | 0.030/0.073; 0.831/0.948; 6.74/7.33 ms; 33.0 MiB at 1M | `bench.mjs` |
| (e'') Arrow, arrow-rs | 0.056/0.151; 1.28/1.69; 10.1/10.3 ms; 0 / 5.9 / 38.1 MiB | `bench.mjs` |
| Native parse, 1M rows: CSV ms / CSV RFC 3339 / JSON cols / JSON rows | 39.7/69.6; 46.1/54.6; 37.8/40.6; 55.9/86.3 ms | `native` |
| Native Arrow 1M: hand-written / arrow-rs | 1.71/3.35; 3.23/4.86 ms | `native` |
| Native copy of two 1M columns | 0.247/0.364 ms | `native` |
| Ring push one, wasm | 0.062 µs a call (10,000 calls 0.491/0.615 ms) | `bench.mjs` |
| Ring push batch of 1000, wasm | 3.78 µs a call (100 calls 0.288/0.378 ms) | `bench.mjs` |
| Ring staging + commit, 1000, wasm | 2.97 µs a call (100 calls 0.133/0.297 ms) | `bench.mjs` |
| Ring of 1M, allocation | 1.18/1.39 ms, 15.4 MiB once | `bench.mjs` |
| Ring push one / batch of 1000, native | 0.0048 µs / 0.739 µs a call | `native` |
| Downsamplers over 1M | see the table under Findings: downsampling | `bench.mjs`, `native` |
| Columns missing a true extreme, LTTB | 153 of 160, 607 of 640; M4 and min-max 0 | `native` |
| Kepler vs VSOP87A, 7 planets, 2026-10-18 0h | 4.6–297.1″ | `native` |
| Kepler / VSOP87A, 8 planets, native | 1.5/1.6–2.0 µs; 196–325/246–481 µs | `native` |
| Wasm memory after `reserve` without re-reading `memory.buffer` | `TypeError: Cannot perform Construct on a detached ArrayBuffer` at 100k rows | first `bench.mjs` run |

## Recommendation

1. Put one binary boundary into wasm: typed columns and a small JSON
   descriptor. A Player takes values as `Vec<f64>` parameters or through a
   buffer it reserves for the shell to fill, and it never parses data text.
2. Parse JSON with `JSON.parse` in the shell, and CSV in the shell in the
   browser and in the core for the CLI. That keeps 26.5–34.4 KB (JSON) and
   16–17 KB (CSV) gzip out of every chart module. It keeps 3–5× data-sized
   spikes out of the shared wasm memory and runs JSON 2.2 times faster.
3. Put a hand-written Arrow IPC stream reader in the core, in every chart
   module, at a few KB gzip (3.8 KB measured for three types). arrow-rs
   (262 KB) and arrow2 (92 KB, archived) are rejected for the browser.
   Arrow can come in v0.2.0. Typed arrays come first.
4. Hold every numeric and time column as `f64`. Timestamps are epoch ms, and
   `us`/`ns` columns are offsets from an `i64` origin. Missing is NaN or
   `u32::MAX`. A missing y breaks a line. x is checked non-decreasing and
   sorted if not.
5. Hold live data in a ring allocated once, with `retain.rows` (default 1M)
   and optional `retain.age`. Coalesce appends per animation frame. Drop and
   count late rows. An equal x replaces the last row.
6. Draw lines from M4 per dot column, cached by data version and view.
   Resample candles to calendar intervals. Rasterise every scatter point.
   LTTB is opt-in only.
7. Take celestial positions as data in declared frames, and convert and
   project in the sky chart. Planet positions come from an optional astronomy
   pack on JPL's Keplerian elements, never from VSOP87 in the browser.
8. Data changes take effect at once. A frame is a pure function of the data
   as it is, the options, the view and `t`, with no easing log for data
   (unicel's REQ-WASM-503 eases data over time; glyphplot should not by
   default).

## Requirements to propose

Milestones assume the phase order in PROMPT.md: v0.1.0 is the core and the
line chart end to end through the element, v0.2.0 adds candlestick, bar,
sparkline and streaming, v0.3.0 the site and the CLI, and v0.4.0 the next tier
and the domain packs. The synthesis renumbers if the phases differ.
"The reference machine" is the one under Scope and method. "Node" is Node 26
running the release module.

- REQ-DATA-010: Every chart MUST accept its data in the shape of
  `spec/schemas/data.schema.json` (named tables of columns typed `f64`, `f32`,
  `i64`, `u32`, `bool`, `timestamp`, `duration`, `category`, `string`, and named
  matrices), and the schema MUST validate the example in the spec. v0.1.0
- REQ-DATA-020: A chart MUST hold numeric, timestamp and duration columns as
  `f64`. Timestamps MUST be held as epoch milliseconds. A column with
  `time_unit` `us` or `ns` MUST be held as offsets from an `i64` origin, so that
  `ns` values spanning under 2^53 ns round-trip exactly (tested with
  1792281600000000001 to …004). v0.1.0
- REQ-DATA-030: JSON `null`, an empty CSV field, NaN and a cleared Arrow
  validity bit MUST all be read as missing. A missing y MUST break a line or
  area into separate runs, and a row with a missing x MUST NOT be drawn
  (snapshot fixtures `gap` and `nan-x`). v0.1.0
- REQ-DATA-040: A chart MUST check on load that its x column is
  non-decreasing. When it is not, the chart MUST draw a stably sorted copy
  and warn once on the console, and the frame MUST equal the frame of the
  sorted data. v0.1.0
- REQ-DATA-050: The Player MUST take column values only as typed arrays
  (passed as `Vec<f64>`, or written by the shell into a buffer the Player
  reserved) plus a descriptor without `values`. The chart modules MUST NOT
  contain a CSV or JSON data parser (checked by `mise run size --names`
  finding no `parse_csv`/`from_json` symbol). v0.1.0
- REQ-DATA-060: The shell MUST read `memory.buffer` only after the call that
  returns a buffer's address. A Node test MUST fill a 1M-row reserved column
  across a memory growth and check its sum. v0.1.0
- REQ-DATA-070: Replacing a chart's data with two 1M-row `Float64Array`
  columns MUST take at most 10 ms (median of 5) in Node on the reference
  machine. It MUST grow wasm memory by at most 1.1 times the columns' bytes
  (measured 4.3 ms and 15.4 MiB for 15.3 MiB). v0.1.0
- REQ-DATA-080: A chart MUST find each channel by the option `encode`, then
  by a column's `role`, then by a column of the channel's name. When none
  matches, it MUST draw the empty-data frame and warn once naming the
  channel. v0.1.0
- REQ-DATA-090: Timestamp values MUST be UTC instants. The column's `zone`
  (default `UTC`) MUST decide only tick placement and labels, and a frame MUST
  NOT depend on the host's time zone (the same frame under `TZ=UTC` and
  `TZ=Asia/Tokyo`). v0.1.0
- REQ-DATA-100: `unit`, `scale` and `offset` MUST apply to ticks, readouts
  and the data table and MUST NOT change stored values. A readout of a column
  with `scale: 0.01` and stored 18240 MUST show 182.40. v0.2.0
- REQ-DATA-110: The element's `data` property and the CLI MUST accept an
  Arrow IPC stream (uncompressed; Float64, Float32, Int32, Int64, UInt32,
  Timestamp, Bool, Utf8 and Dictionary<Int32, Utf8> columns, with validity
  bitmaps). The core's own reader MUST read it, and the reader MUST add at
  most 8 KB gzip to a chart module. v0.2.0
- REQ-DATA-120: The element MUST accept the data object, the columns
  shorthand, the rows shorthand (property or `data` attribute) and a `src`
  URL of CSV, JSON, NDJSON or Arrow. The shell MUST parse the text formats
  and infer shorthand types as the schema says. v0.1.0 (property and
  attribute), v0.2.0 (`src`)
- REQ-DATA-130: The data core every chart module carries (typed column
  store, ring, M4 and min-max, Arrow reader) MUST add at most 16 KB gzip over
  an empty module of the same boundary (measured 7.3 KB without the Arrow
  reader's extra types). v0.2.0
- REQ-DATA-140: Graph data MUST be `nodes` and `edges` tables whose
  `source`/`target` are `u32` indexes into `nodes` or strings equal to
  `nodes.id`. Matrices MUST be row-major `rows × cols`. An edge naming no node
  MUST be dropped with one warning. v0.4.0
- REQ-STREAM-010: `append()` MUST accept one row object, an array of row
  objects, or columns of typed arrays. The shell MUST coalesce all appends
  made within one animation frame into one write to wasm (a Node test counts
  one `commit` for 1000 appends in a frame). v0.2.0
- REQ-STREAM-020: A streaming chart MUST keep its rows in a ring allocated
  once with capacity `retain.rows` (default 1,000,000). Appending 3M rows to a
  1M ring MUST NOT grow wasm memory after the first 1M. v0.2.0
- REQ-STREAM-030: `retain.age` SHOULD drop rows whose x is older than the
  newest x minus that duration, and the oldest kept row MUST be the first
  with x ≥ newest − age. v0.2.0
- REQ-STREAM-040: An appended row whose x equals the last row's x MUST
  replace that row. A row whose x is less MUST be dropped and counted in the
  `late` field of the next `glyph-data` event. v0.2.0
- REQ-STREAM-050: In Node on the reference machine, appending one row MUST
  take at most 1 µs and appending a batch of 1000 at most 50 µs (median)
  (measured 0.062 µs and 2.97–3.78 µs). v0.2.0
- REQ-STREAM-060: After `data` is replaced, the frame MUST equal the frame of
  a new chart given the same data, options, view and `t`. v0.1.0
- REQ-SAMPLE-010: Line, area, step and sparkline charts MUST draw from M4
  aggregates of the rows in view at the plot's dot-column width whenever the
  rows in view exceed four times the dot columns. The frame MUST equal the
  frame drawn from every row (snapshot comparison on 1M-row fixtures). v0.1.0
- REQ-SAMPLE-020: Aggregates MUST be cached by data version, view x range and
  width. A frame with all three unchanged MUST NOT read the data (a scan
  counter in the test build stays 0). v0.1.0
- REQ-SAMPLE-030: M4 over 1M rows in view at 640 dot columns MUST take at
  most 8 ms (median of 7) in wasm in Node on the reference machine (measured
  2.8 ms). v0.1.0
- REQ-SAMPLE-040: Candlestick and OHLC charts MUST resample to the smallest
  interval in the ladder 1 s, 5 s, 15 s, 1 min, 5 min, 15 min, 1 h, 4 h, 1 d,
  1 w, 1 mo at which the candles in view fit the plot. Open MUST be the first
  row, high the maximum, low the minimum, close the last and volume the sum.
  Buckets MUST align to the interval in the column's zone. v0.2.0
- REQ-SAMPLE-050: Scatter and bubble charts MUST set a dot for every visible
  point. No point may be dropped by sampling. v0.4.0
- REQ-SAMPLE-060: LTTB MAY be offered as `sample: "lttb"` and MUST NOT be
  any chart's default. v0.2.0
- REQ-SKY-010: The sky chart MUST take positions as data in frame `icrs`,
  `ecliptic` or `altaz` (degrees). Given observer latitude, longitude and
  time, it MUST convert `icrs` to `altaz` within 0.1° of astropy's result for
  ten fixture stars. v0.4.0
- REQ-SKY-020: Chart modules MUST NOT compute planetary or lunar positions.
  An astronomy pack MAY compute planets from JPL's approximate Keplerian
  elements, and its positions MUST be within 0.1° of VSOP87A for dates from
  1800 to 2050 (tested at 20 dates). v0.4.0
- REQ-SKY-030: The orbit plot MUST draw each orbit's ellipse from the
  `orbits` shape (a, e, i, node, peri, anomaly, epoch) and each body's
  position at the view's time by Kepler's equation. v0.4.0

## Decisions with a real alternative

1. **Where text formats are parsed.**
   Context: JSON and CSV must be accepted. Chart modules are one per type and
   share a module's memory, and memory never shrinks.
   Options: (a) parse in wasm, with serde_json and a Rust CSV parser in every
   module; (b) parse in the TS shell and send typed columns; (c) put a
   separate shared "data" wasm module beside the chart modules.
   Recommendation: (b). It is 2.2 times faster for JSON and the same speed
   for CSV, adds 0 bytes to chart modules (against 26.5–34.4 and 16–17 KB),
   and raises no memory spikes (80 MiB for 1M rows in (a)). (c) adds a second
   instance and a copy between the two.
2. **Arrow IPC reader.**
   Options: hand-written in the core (+3.8 KB measured for a subset), arrow-rs
   (+262 KB), arrow2 (+92 KB, archived), or apache-arrow in the shell (48.5 KB
   gzip JS).
   Recommendation: hand-written in the core, v0.2.0, its fixtures written by
   arrow-rs in a dev-only test.
3. **Column storage.**
   Options: (a) everything `f64`; (b) typed storage per column (`f32`, `i64`,
   `u8`) with generic code paths; (c) `f64` plus an opt-in `f32` for values.
   Recommendation: (a) for v0.1.0. One code path keeps modules small. 1M rows
   of OHLCV is 48 MB (six `f64` columns), which fits. (c) is an open question.
4. **Default downsampler for lines.**
   Options: M4, min-max, LTTB.
   Recommendation: M4. It has the same cost as the others (2.5–2.8 ms for 1M
   rows) and draws the same frame as all the data, which min-max does not
   promise across columns. LTTB misses extremes in 95% of columns.
5. **Live storage.**
   Options: a ring allocated once, or a growable `Vec` trimmed from the front.
   Recommendation: the ring. It never grows memory after its first fill, it
   costs 0.06 µs an append, and the downsamplers read its two segments in
   place.
6. **Data changes and time.**
   Options: (a) unicel's input log, with data eased over up to 1 s as a
   function of `t` (REQ-WASM-503); (b) data applies immediately and the frame
   is a pure function of the data's current version.
   Recommendation: (b). Charts are read for values. A tween shows values
   that never existed, and replaying a 1 kHz log to seek is costly.
   Transitions can come later as a view option.
7. **Celestial positions.**
   Options: (a) the library computes them (VSOP87 at 688 KB gzip, or
   Keplerian elements at 6.9 KB); (b) the library expects them as data and
   only converts frames and projects; (c) (b) plus an optional astronomy pack
   with Keplerian elements.
   Recommendation: (c).
8. **Late rows in a stream.**
   Options: drop and count them, or insert them by binary search (a shift of
   up to the ring's length).
   Recommendation: drop and count by default. Inserting is an open question.

## Open questions for the owner

1. **Does the library compute celestial positions?** (PROMPT open question 5.)
   Options: compute in the sky chart; expect them as data; expect them and
   ship an astronomy pack that computes planets from JPL's Keplerian
   elements. Recommendation: the third. It is dot-accurate at any terminal
   resolution (measured within 297″ of VSOP87A, against a dot of at least
   225″), costs 6.9 KB, and stays out of chart modules.
2. **The default retention cap and a per-page memory budget.** Options: 1M
   rows a chart (16 MB for two columns), 100k, or no default. Recommendation:
   1M, matching the owner's million-point budget, with agent 6 setting the
   per-page memory budget.
3. **`f32` storage for values.** Options: `f64` only, or an opt-in `f32`
   that halves memory for values (timestamps stay `f64`).
   Recommendation: `f64` only until a measured page needs more.
4. **Default time zone for display.** Options: UTC, or the page's local zone,
   resolved by the shell to an IANA name and passed in. Recommendation: the
   element defaults to the page's zone and passes it in explicitly, the CLI
   and tests default to UTC, and the frame stays pure either way.
5. **Late rows.** Options: drop and count, or insert. Recommendation: drop in
   v0.2.0, and reconsider when a telemetry example needs out-of-order
   arrival.
6. **Arrow in the first milestone or the second.** Recommendation: the
   second. Typed arrays and JSON cover the first chart's examples.

## Sources

- Measurements: `discovery/bench/data/` (`src/core.rs`, `src/lib.rs`,
  `src/bin/native.rs`, `sizes.sh`, `bench.mjs`, `csv-js.mjs`, `combine.py`,
  README.md), run on 2026-10-18 as under Scope and method.
- wasm-bindgen 0.2.129 generated glue: `discovery/bench/data/target/pkg/all/bench.js:407-417`
  (`passArrayF64ToWasm0`, `passStringToWasm0`).
- unicel: `../animate-unicode/web/src/wasm.ts:1-34` (one module instance
  shared per category), `web/src/mount.ts:88-91` (views re-taken because memory
  moves), `crates/wasm/src/lib.rs:105-110` (wall clock passed in),
  `crates/unicel/src/options.rs:9-30` (serde_json for options),
  `spec/ROADMAP.md:775-797` (§6.2 data-driven pieces, REQ-DATA-001..004,
  REQ-WASM-503), `spec/schemas/piece-data.schema.json`,
  `crates/unicel/src/pieces/data/{candlesticks,bar_chart,sparkline}.rs`
  (data from seeded generators today), `docs/IMPLEMENTATION_PLAN.md:58,65`
  (module sizes and gates).
- uPlot data format: https://github.com/leeoniya/uPlot, `docs/README.md:41-55,98`
  (columnar arrays, ascending unique x, `null` gaps, `spanGaps`).
- TradingView Lightweight Charts: https://github.com/tradingview/lightweight-charts,
  `src/api/iseries-api.ts:145-203` (`setData` ordered, `update` replaces equal
  time, `historicalUpdate`, `pop`).
- Apache Arrow columnar format and IPC framing:
  https://arrow.apache.org/docs/format/Columnar.html ("Buffer Alignment and
  Padding", "Encapsulated message format").
- arrow-rs 60.0.0 (`arrow-ipc`, `arrow-array`, `arrow-schema`), crates.io.
  arrow2 0.18.0, crates.io, archived: https://api.github.com/repos/jorgecarleitao/arrow2
  (`"archived": true`, `"pushed_at": "2024-02-27T19:55:35Z"`).
- apache-arrow 21.2.0 npm tarball (`npm view apache-arrow dist.tarball`),
  `Arrow.esnext.min.js`.
- M4: Jugel, Jerzak, Hackenbroich, Markl, PVLDB 7(10), 2014,
  https://www.vldb.org/pvldb/vol7/p797-jugel.pdf.
- LTTB: Steinarsson, MSc thesis, University of Iceland, 2013,
  https://skemman.is/handle/1946/15343 (returned HTTP 403 to curl on 2026-10-18).
- JPL approximate planetary positions, Table 1 and accuracy table:
  https://ssd.jpl.nasa.gov/planets/approx_pos.html (fetched 2026-10-18).
  JPL Horizons: https://ssd.jpl.nasa.gov/horizons/.
- vsop87 3.0.0: https://github.com/Razican/vsop87-rs (crates.io API
  `crates/vsop87`).
- Meeus, *Astronomical Algorithms*, 2nd ed., Willmann-Bell, 1998, ch. 12–13
  (sidereal time, coordinate transformation) and ch. 47 (Moon). Cited from the
  book's chapter structure, not read in this session **(unverified)**.
- WebAssembly core specification, memory instructions (no shrink):
  https://webassembly.github.io/spec/core/syntax/instructions.html.
- jsonschema 4.25.1 (PyPI), used to validate the draft schema.
