// Data across the wasm boundary, through the shell's compiled code and the
// build under test (REQ-TEST-020): typed columns cross with one copy
// (REQ-MEM-020, REQ-DATA-050); memory.buffer is read only after the call that
// returns an address, and a 1,000,000-row column filled across a memory
// growth sums right (REQ-DATA-060); a replace of two 1,000,000-row columns
// takes at most 10 ms and grows memory by at most 1.1 times their bytes
// (REQ-DATA-070); the shorthands draw as the full form (REQ-DATA-120); `us`
// and `ns` columns keep their origin (REQ-DATA-020).
import { test } from "node:test";
import assert from "node:assert/strict";
import { BUILD, manifest, types, moduleFor, instantiate, shell, series, median, retimed, fixtureIndex, fixtureData, commitColumns } from "./support/wasm.mjs";

const TYPE = types[0];
const N = 1_000_000;

// ---------------------------------------------------------------- an instrumented module for the shell

/** Every call the shell makes on a wasm chart, and every read of memory.buffer, in order. */
const log = [];
let real = null;

/** The module of TYPE, fresh, its Chart and memory wrapped to log what the shell calls. */
async function instrumented(name) {
  const { glue, exports } = await instantiate(name, { fresh: true });
  real = exports;
  function Chart(type, options) {
    const c = new glue.Chart(type, options);
    return new Proxy(c, {
      get(target, key) {
        const v = target[key];
        if (typeof v !== "function") return v;
        return (...args) => {
          log.push(String(key));
          return v.apply(target, args);
        };
      },
    });
  }
  const memory = {
    get buffer() {
      log.push("buffer");
      return exports.memory.buffer;
    },
  };
  return { ...glue, Chart, default: async () => ({ memory, __wbindgen_add_to_stack_pointer: exports.__wbindgen_add_to_stack_pointer }) };
}

const imports = {};
for (const name of Object.keys(manifest.imports)) imports[name] = () => instrumented(name);
shell.wasm.setManifest({ charts: manifest.charts, modules: manifest.modules, fallback: manifest.fallback, budgets: manifest.budgets, imports });

const bytes = () => real.memory.buffer.byteLength;
const ADDRESS_CALLS = new Set(["reserve_column", "cells_ptr", "colors_ptr", "backs_ptr"]);

/** The x and y columns of n rows, as Float64Arrays. */
function columns(n, y = (i) => Math.sin(i / 1000)) {
  const s = series(n, y);
  return { x: s.xs, y: s.ys };
}

test(`${BUILD}: memory.buffer is read only right after a call that returns an address (REQ-DATA-060, REQ-WASM-030)`, async () => {
  log.length = 0;
  const ch = shell.chart.chart(TYPE, { data: columns(N), cols: 80, rows: 24 });
  await ch.ready;
  ch.frame(0);
  ch.data = columns(N, (i) => Math.cos(i / 1000));
  ch.frame(0);
  ch.data = [{ x: 1, y: 2, s: "a" }, { x: 2, y: 3, s: "b" }];
  ch.frame(0);
  const reads = log.map((k, i) => [k, log[i - 1]]).filter(([k]) => k === "buffer");
  assert.ok(reads.length >= 6, "the shell made views");
  for (const [, before] of reads) assert.ok(ADDRESS_CALLS.has(before), `memory.buffer read after ${before}: ${log.join(" ")}`);
  assert.ok(!log.includes("set_column"), "typed arrays are written into reserved buffers, not passed by value");
  ch.free();
});

test(`${BUILD}: a typed column reaches wasm memory with one copy (REQ-MEM-020, REQ-DATA-050)`, async () => {
  const warm = shell.chart.chart(TYPE, {});
  await warm.ready; // the module is loaded and its memory is in place
  warm.free();
  const { x, y } = columns(N);
  const ch = shell.chart.chart(TYPE, {});
  await ch.ready;
  log.length = 0;
  const w0 = bytes();
  const a0 = process.memoryUsage().arrayBuffers;
  ch.data = { x, y };
  const a1 = process.memoryUsage().arrayBuffers;
  const grown = bytes() - w0;
  assert.ok(grown <= x.byteLength + y.byteLength + (1 << 20), `wasm memory grew ${grown} B for ${x.byteLength + y.byteLength} B of columns: one copy`);
  assert.ok(a1 - a0 < x.byteLength / 2, `the shell allocated ${a1 - a0} B of array buffers outside wasm: no intermediate copy`);
  assert.deepEqual(log.filter((k) => k !== "buffer"), ["reserve_column", "reserve_column", "commit", "events_json"]);
  assert.match(ch.summary(), /1,000,000 points/);
  ch.free();
});

test(`${BUILD}: a reserved 1,000,000-row column filled across a memory growth sums right (REQ-DATA-060)`, async () => {
  const { glue, memory } = await instantiate(moduleFor(TYPE), { fresh: true });
  const c = new glue.Chart(TYPE, "{}");
  const p0 = c.reserve_column(0, 0, N);
  let x = new Float64Array(memory.buffer, p0, N);
  for (let i = 0; i < N / 2; i++) x[i] = i;
  const before = memory.buffer.byteLength;
  const p1 = c.reserve_column(1, 0, N); // grows memory
  assert.ok(memory.buffer.byteLength > before, "the second reservation grew memory");
  assert.equal(x.length, 0, "the view made before the growth is detached");
  x = new Float64Array(memory.buffer, p0, N); // made again after the call
  for (let i = N / 2; i < N; i++) x[i] = i;
  new Float64Array(memory.buffer, p1, N).fill(1);
  c.commit(JSON.stringify({ version: 1, tables: { data: { length: N, columns: [{ name: "x", type: "f64" }, { name: "y", type: "f64" }] } } }));
  // The buffers moved into the chart's columns with no copy: the column is still at its address.
  const held = new Float64Array(memory.buffer, p0, N);
  let sum = 0;
  for (let i = 0; i < N; i++) sum += held[i];
  assert.equal(sum, (N * (N - 1)) / 2);
  assert.match(c.summary(), /x from 0 to 999,?999; values from 1 to 1; 1,000,000 points/);
  c.free();
});

test(`${BUILD}: replacing two 1,000,000-row columns takes at most 10 ms (median of 5) and grows memory at most 1.1× their bytes (REQ-DATA-070)`, async () => {
  const a = columns(N);
  const b = columns(N, (i) => Math.cos(i / 1000));
  const ch = shell.chart.chart(TYPE, { cols: 80, rows: 24 });
  await ch.ready;
  ch.data = a;
  ch.frame(0);
  const limit = 1.1 * (a.x.byteLength + a.y.byteLength);
  const play = () => {
    const times = [];
    for (let k = 0; k < 5; k++) {
      const w = bytes();
      const t0 = performance.now();
      ch.data = k % 2 ? a : b;
      times.push(performance.now() - t0);
      const grown = bytes() - w;
      assert.ok(grown <= limit, `replace ${k} grew ${grown} B, over ${limit}`);
      ch.frame(0);
    }
    return median(times);
  };
  const { best: m, runs } = await retimed(play, 10);
  console.log(`# REQ-DATA-070 ${BUILD}: replace of 2 × 1M f64 through chart.data: median ${m.toFixed(2)} ms, best of [${runs.map((t) => t.toFixed(2)).join(", ")}] (Node ${process.versions.node}, load ${(await import("node:os")).loadavg()[0].toFixed(2)})`);
  assert.ok(m <= 10, `median ${m.toFixed(2)} ms`);
  ch.free();
});

/** The text of a 60 × 16 frame of `data`. */
async function frameOf(data, options = {}) {
  const ch = shell.chart.chart(TYPE, { data, options, cols: 60, rows: 16 });
  await ch.ready;
  ch.frame(0);
  const text = ch.text();
  const summary = ch.summary();
  ch.free();
  return { text, summary };
}

test(`${BUILD}: the columns and rows shorthands, JSON text and the binary form draw as the full form (REQ-DATA-120, REQ-DATA-010)`, async () => {
  const xs = [0, 1, 2, 3, 4, 5, 6, 7];
  const ys = [3, 1, 4, 1, 5, 9, 2, 6];
  const full = await frameOf({ version: 1, tables: { data: { columns: [{ name: "x", type: "f64", values: xs }, { name: "y", type: "f64", values: ys }] } } });
  assert.match(full.summary, /8 points/);
  assert.deepEqual(await frameOf({ x: xs, y: ys }), full, "columns shorthand");
  assert.deepEqual(await frameOf(xs.map((x, i) => ({ x, y: ys[i] }))), full, "rows shorthand");
  assert.deepEqual(await frameOf(JSON.stringify({ x: xs, y: ys })), full, "JSON text");
  assert.deepEqual(await frameOf({ x: Float64Array.from(xs), y: Float64Array.from(ys) }), full, "typed columns");
  assert.deepEqual(
    await frameOf({ descriptor: { tables: { data: { columns: [{ name: "x", type: "f64" }, { name: "y", type: "f64" }] } } }, columns: { x: Float64Array.from(xs), y: Int32Array.from(ys) } }),
    full,
    "binary form",
  );
  // Timestamps and categories from the shorthand.
  const t = ["2026-10-18T00:00:00Z", "2026-10-18T00:01:00Z", "2026-10-18T00:02:00Z", "2026-10-18T00:03:00Z"];
  const stamped = await frameOf({ t, close: [1, 2, 3, 2] });
  const ms = { version: 1, tables: { data: { columns: [{ name: "t", type: "timestamp", values: t.map((s) => Date.parse(s)) }, { name: "close", type: "f64", values: [1, 2, 3, 2] }] } } };
  assert.deepEqual(stamped, await frameOf(ms), "RFC 3339 strings draw as epoch ms");
  assert.deepEqual(await frameOf({ t: t.map((s) => new Date(s)), close: [1, 2, 3, 2] }), stamped, "Dates draw as epoch ms");
  assert.match(stamped.summary, /2026-10-18/);
  const long = await frameOf([{ x: 1, y: 1, series: "a" }, { x: 1, y: 2, series: "b" }, { x: 2, y: 3, series: "a" }, { x: 2, y: 1, series: "b" }]);
  assert.match(long.summary, /series/);
});

const ORIGIN = 1792281600000000000n;

test(`${BUILD}: the module holds an ns column given as offsets from an origin (REQ-DATA-020)`, async () => {
  const { glue, memory } = await instantiate(moduleFor(TYPE));
  const c = new glue.Chart(TYPE, "{}");
  const p0 = c.reserve_column(0, 0, 4);
  new Float64Array(memory.buffer, p0, 4).set([1, 2, 3, 4]);
  const p1 = c.reserve_column(1, 0, 4);
  new Float64Array(memory.buffer, p1, 4).set([1, 3, 2, 4]);
  const d = `{"version":1,"tables":{"d":{"length":4,"columns":[{"name":"t","type":"timestamp","time_unit":"ns","origin":${ORIGIN}},{"name":"y","type":"f64"}]}}}`;
  assert.equal(c.commit(d), 1);
  c.frame(0, false);
  assert.match(c.summary(), /2026-10-1[78].*4 points in view/);
  c.free();
});

test(`${BUILD}: the shell keeps an ns column's origin (REQ-DATA-020)`, () => {
  const p = shell.data.prepare({
    descriptor: { tables: { d: { columns: [{ name: "t", type: "timestamp", time_unit: "ns", origin: Number(ORIGIN) }, { name: "y", type: "f64" }] } } },
    columns: { t: Float64Array.of(1, 2, 3, 4), y: Float64Array.of(1, 3, 2, 4) },
  });
  assert.match(shell.data.descriptorJson(p), /"origin":1792281600000000000[,}]/);
  assert.deepEqual([...shell.data.materialise(p.sources[0])], [1, 2, 3, 4], "the offsets as given");
  const q = shell.data.prepare({
    descriptor: { tables: { d: { columns: [{ name: "t", type: "timestamp", time_unit: "ns", origin: ORIGIN + 1n }, { name: "y", type: "f64" }] } } },
    columns: { t: Float64Array.of(0, 1, 2, 3), y: Float64Array.of(1, 3, 2, 4) },
  });
  assert.match(shell.data.descriptorJson(q), /"origin":1792281600000000001[,}]/, "a bigint origin is written exactly");
  assert.throws(() => shell.data.prepare({ descriptor: { tables: { d: { columns: [{ name: "t", type: "timestamp", time_unit: "ns", origin: 1.5 }] } } }, columns: { t: Float64Array.of(1) } }), /`origin` must be an integer/);
});

test(`${BUILD}: ns and us timestamps past 2^53 cross as an origin and offsets and round-trip exactly (REQ-DATA-020)`, async () => {
  const want = [1n, 2n, 3n, 4n].map((d) => ORIGIN + d);
  const given = {
    "a BigInt64Array": BigInt64Array.from(want),
    "integer strings": want.map(String),
    "RFC 3339 strings with nanoseconds": want.map((v) => `2026-10-18T00:00:00.00000000${v - ORIGIN}Z`),
    "a reversed BigInt64Array": BigInt64Array.from(want.slice().reverse()),
  };
  const { glue, memory } = await instantiate(moduleFor(TYPE));
  for (const [what, t] of Object.entries(given)) {
    const p = shell.data.prepare({ descriptor: { tables: { d: { columns: [{ name: "t", type: "timestamp", time_unit: "ns" }, { name: "y", type: "f64" }] } } }, columns: { t, y: Float64Array.of(1, 3, 2, 4) } });
    const json = shell.data.descriptorJson(p);
    const m = /"origin":(-?\d+)/.exec(json);
    assert.ok(m, `${what}: an origin in ${json}`);
    assert.equal(BigInt(m[1]), want[0], `${what}: the origin is the least value`);
    const offsets = shell.data.materialise(p.sources[0]);
    const back = [...offsets].map((o) => BigInt(m[1]) + BigInt(o));
    assert.deepEqual(what.startsWith("a reversed") ? back.reverse() : back, want, what);
    // The module takes the descriptor and the offsets.
    const c = new glue.Chart(TYPE, "{}");
    commitColumns(c, memory, json, [offsets, shell.data.materialise(p.sources[1])]);
    c.frame(0, false);
    assert.match(c.summary(), /2026-10-1[78].*4 points in view/, what);
    c.free();
  }
  // Microseconds, with a missing value, and sub-microsecond digits dropped.
  const us = shell.data.prepare({
    descriptor: { tables: { d: { columns: [{ name: "t", type: "timestamp", time_unit: "us" }] } } },
    columns: { t: ["2026-10-18T00:00:00.000002999Z", null, "1792281600000001"] },
  });
  assert.match(shell.data.descriptorJson(us), /"origin":1792281600000001[,}]/);
  const uo = shell.data.materialise(us.sources[0]);
  assert.deepEqual([uo[0], Number.isNaN(uo[1]), uo[2]], [1, true, 0]);
  // Appends to a column with an origin are offsets from it.
  const ns = shell.data.prepare({ descriptor: { tables: { d: { columns: [{ name: "t", type: "timestamp", time_unit: "ns" }, { name: "y", type: "f64" }] } } }, columns: { t: BigInt64Array.from(want), y: Float64Array.of(1, 3, 2, 4) } });
  const more = shell.data.appendTo(ns, [{ t: String(ORIGIN + 9n), y: 5 }]);
  assert.deepEqual([...shell.data.materialise(more.sources[0])], [0, 1, 2, 3, 8]);
  assert.equal(shell.data.describe(more).tables.d.columns[0].origin, undefined, "the data getter leaves out how values crossed");
});

test(`${BUILD}: M4 over 1,000,000 rows in view at 640 dot columns takes at most 8 ms, median of 7 (REQ-SAMPLE-030)`, async () => {
  // No export reaches M4 alone, so this times whole frames whose x range is new each time (the M4 cache keyed by
  // data version, x range and width misses): an upper bound on M4, which also redraws the base.
  const fx = fixtureIndex().fixtures.find((f) => f.name === "random-walk-1m");
  const { glue, memory } = await instantiate(moduleFor(TYPE), { fresh: true });
  const c = new glue.Chart(TYPE, "{}");
  const { descriptor, columns } = fixtureData(fx);
  commitColumns(c, memory, descriptor, columns);
  const plotCols = (cols) => {
    c.resize(cols, 12);
    c.frame(0, false);
    const axis = Math.max(...c.text().split("\n").map((r) => [...r].findIndex((ch) => ch === "┤")));
    return { cols, plot: cols - axis - 1 };
  };
  let at = plotCols(330);
  at = plotCols(at.cols + (320 - at.plot));
  assert.ok(at.plot >= 320, `${at.plot} cells of braille, 2 dot columns each`);
  const xs = columns[0];
  const [x0, x1] = [xs[0], xs[xs.length - 1]];
  const span = x1 - x0;
  let k = 0;
  const play = () => {
    const times = [];
    for (let i = 0; i < 8; i++) {
      k++;
      c.set_view(JSON.stringify({ x: { domain: [x0 - k * span * 1e-9, x1 + k * span * 1e-9] } }));
      const t0 = performance.now();
      c.frame(0, false);
      times.push(performance.now() - t0);
    }
    assert.match(c.summary(), /1,000,000 points in view/);
    times.shift(); // the first frame of a run is not counted: the first at this width also draws the axes' text
    return median(times);
  };
  const { best: m, runs } = await retimed(play, 8);
  console.log(`# REQ-SAMPLE-030 ${BUILD}: frame of 1M rows in view, new x range each, ${at.cols}×12 (${at.plot * 2} dot columns): median of 7 ${m.toFixed(2)} ms, best of [${runs.map((t) => t.toFixed(2)).join(", ")}] (Node ${process.versions.node}, load ${(await import("node:os")).loadavg()[0].toFixed(2)})`);
  assert.ok(m <= 8, `median ${m.toFixed(2)} ms`);
  c.free();
});
