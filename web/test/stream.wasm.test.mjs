// Streaming across the wasm boundary, through the shell's compiled code and
// the build under test (REQ-TEST-020): every append of one animation frame is
// one write to wasm (REQ-STREAM-010); each commit's `plot-data` detail
// (REQ-STREAM-070); appending ends on the frame a replace of the same rows
// draws; `x.follow` across the boundary and `End` (REQ-VIEW-060); `paused`
// keeps appends unwritten (REQ-A11Y-080); `src`'s formats served over HTTP
// (REQ-DATA-150); and the timing of REQ-STREAM-050.
//
// The build has no `staging(n)`, `append(n)` or `set_arrow` yet (v0.6.0's
// ring and Arrow reader are another agent's work). The shell is checked on
// both paths: the fallback, which commits the tail whole, and the staging
// path over a JS shim of `staging` and `append` that keeps the ring in JS and
// commits it. Tests needing the module's own members are `todo` until it has
// them; they run as soon as it does.
import { test } from "node:test";
import assert from "node:assert/strict";
import { createServer } from "node:http";
import { BUILD, manifest, types, moduleFor, instantiate, shell, series, median, schema, check } from "./support/wasm.mjs";
import { arrowStream, walk } from "./support/arrow.mjs";

const TYPE = types[0];

// ---------------------------------------------------------------- the module, instrumented

/** Calls the shell made on wasm charts, by member name. */
const calls = new Map();
const count = (k) => calls.set(k, (calls.get(k) ?? 0) + 1);
const called = (k) => calls.get(k) ?? 0;
/** Whether charts made from now on get the JS shim of `staging` and `append`. */
let shim = false;

/**
 * `staging(n)` and `append(n)` as the boundary states them (web/src/api.ts),
 * over the real chart's `reserve_column` and `commit`: the staging buffer is a
 * reserved column the shell fills; `append` reads it, applies the ring's rules
 * to a JS copy of the table (x the first column), commits that whole, and
 * returns `[rows, dropped, late, version]`. It keeps its copy by watching the
 * shell's own reserves and commits.
 */
function stagingShim(c, memory) {
  let table = null; // { name, desc, cols: number[][] }
  const reserved = new Map();
  let staged = null;
  const retain = { rows: 1_000_000 };
  const commitWhole = () => {
    const n = table.cols[0].length;
    table.cols.forEach((v, i) => {
      const ptr = c.reserve_column(i, 0, n);
      new Float64Array(memory.buffer, ptr, n).set(v);
    });
    const desc = structuredClone(table.desc);
    desc.tables[table.name].length = n;
    return c.commit(JSON.stringify(desc));
  };
  return {
    setRetain(rows) {
      retain.rows = rows;
    },
    reserve_column(i, kind, n) {
      const ptr = c.reserve_column(i, kind, n);
      reserved.set(i, { ptr, n, kind });
      return ptr;
    },
    commit(json) {
      const desc = JSON.parse(json);
      const [name, t] = Object.entries(desc.tables)[0];
      const cols = t.columns.map((_, i) => {
        const r = reserved.get(i);
        return r ? [...(r.kind === 1 ? new Uint32Array(memory.buffer, r.ptr, r.n) : new Float64Array(memory.buffer, r.ptr, r.n))] : [];
      });
      reserved.clear();
      table = { name, desc, cols };
      return c.commit(json);
    },
    staging(n) {
      const k = table.cols.length;
      staged = { ptr: c.reserve_column(0, 0, n * k), n, k };
      return staged.ptr;
    },
    append(n) {
      assert.equal(n, staged.n, "append(n) takes the n staged");
      const view = new Float64Array(memory.buffer, staged.ptr, n * staged.k).slice();
      let late = 0;
      const x = table.cols[0];
      for (let r = 0; r < n; r++) {
        const row = table.cols.map((_, j) => view[j * n + r]);
        const last = x.length ? x[x.length - 1] : -Infinity;
        if (row[0] === last) table.cols.forEach((v, j) => (v[v.length - 1] = row[j]));
        else if (row[0] < last) late++;
        else table.cols.forEach((v, j) => v.push(row[j]));
      }
      const dropped = Math.max(0, x.length - retain.rows);
      if (dropped) table.cols.forEach((v) => v.splice(0, dropped));
      const version = commitWhole();
      return Float64Array.of(table.cols[0].length, dropped, late, version);
    },
  };
}

async function instrumented(name) {
  const { glue, exports } = await instantiate(name, { fresh: true });
  function Chart(type, options) {
    const c = new glue.Chart(type, options);
    const extra = shim ? stagingShim(c, exports.memory) : {};
    if (shim) {
      const r = JSON.parse(options).retain?.rows;
      if (r) extra.setRetain(r);
    }
    return new Proxy(c, {
      get(target, key) {
        if (Object.hasOwn(extra, key)) {
          return (...args) => {
            count(String(key));
            return extra[key](...args);
          };
        }
        const v = target[key];
        if (typeof v !== "function") return v;
        return (...args) => {
          count(String(key));
          return v.apply(target, args);
        };
      },
    });
  }
  return { ...glue, Chart, default: async () => ({ memory: exports.memory, __wbindgen_add_to_stack_pointer: exports.__wbindgen_add_to_stack_pointer }) };
}

const imports = {};
for (const name of Object.keys(manifest.imports)) imports[name] = () => instrumented(name);
shell.wasm.setManifest({ charts: manifest.charts, modules: manifest.modules, fallback: manifest.fallback, budgets: manifest.budgets, imports });

/** Whether the build's own module has the v0.6.0 streaming members. */
const probe = await instantiate(moduleFor(TYPE), { fresh: true });
const probeChart = new probe.glue.Chart(TYPE, "{}");
const REAL = {
  staging: typeof probeChart.staging === "function" && typeof probeChart.append === "function",
  arrow: typeof probeChart.set_arrow === "function",
};
probeChart.free();
const NO_STAGING = REAL.staging ? false : "the module has no staging(n) and append(n) yet (the ring, v0.6.0): on at integration";
const NO_ARROW = REAL.arrow ? false : "the module has no set_arrow yet (the Arrow reader, v0.6.0): on at integration";

// ---------------------------------------------------------------- frames, by hand

/** The animation frames requested and not yet run. */
let frames = [];
globalThis.requestAnimationFrame = (f) => frames.push(f);
/** Runs the frames requested so far: one display frame. */
function frame() {
  const run = frames;
  frames = [];
  for (const f of run) f(0);
}

/** A chart of TYPE holding `n` rows x = 0 … n − 1, y = sin, loaded, at 80 × 24; `staged` gives it the shim. */
async function made(n, { staged = false, options = {} } = {}) {
  shim = staged;
  const s = series(n);
  const ch = shell.chart.chart(TYPE, { data: { x: s.xs, y: s.ys }, options, cols: 80, rows: 24 });
  await ch.ready;
  shim = false;
  frame();
  calls.clear();
  return ch;
}

const PATHS = [
  ["the fallback (commit)", false],
  ["staging(n) + append(n) (a JS shim)", true],
];

// ---------------------------------------------------------------- REQ-STREAM-010

for (const [path, staged] of PATHS) {
  test(`${BUILD}: ${path}: 1,000 appends in one frame are one write to wasm (REQ-STREAM-010)`, async () => {
    const ch = await made(100, { staged });
    assert.equal(ch.streaming, staged ? "staging" : "commit");
    for (let i = 0; i < 1000; i++) ch.append({ x: 100 + i, y: Math.cos(i) });
    assert.equal(frames.length, 1, "one frame requested for the 1,000");
    assert.equal(called("commit") + called("append"), 0, "nothing written before the frame");
    frame();
    if (staged) assert.deepEqual([called("staging"), called("append"), called("commit")], [1, 1, 0], [...calls].join(" "));
    else assert.equal(called("commit"), 1, [...calls].join(" "));
    assert.equal(ch.lastCommit.rows, 1100);
    assert.match(ch.summary(), /1,100 points/);
    // Each form in one frame: a row, rows, typed columns.
    calls.clear();
    ch.append({ x: 1100, y: 0 });
    ch.append([{ x: 1101, y: 1 }, { x: 1102, y: 2 }]);
    ch.append({ x: Float64Array.of(1103, 1104), y: Float64Array.of(3, 4) });
    frame();
    assert.equal(called("commit") + called("append"), 1);
    assert.equal(ch.lastCommit.rows, 1105);
    assert.equal(ch.data.tables.data.length, 1105);
    ch.free();
  });
}

test(`${BUILD}: a bad append throws at the call and holds nothing; a read writes what waits (REQ-STREAM-010)`, async () => {
  const ch = await made(10);
  assert.throws(() => ch.append({ x: 10, nope: 1 }), /no column `nope`/);
  ch.append({ x: 10, y: 1 });
  assert.equal(ch.data.tables.data.length, 11, "the data getter writes the frame's appends first");
  assert.equal(called("commit"), 1);
  frame();
  assert.equal(called("commit"), 1, "and the frame finds nothing left");
  ch.free();
});

// ---------------------------------------------------------------- REQ-STREAM-070

for (const [path, staged] of PATHS) {
  test(`${BUILD}: ${path}: each commit's plot-data detail: rows, dropped, late, version (REQ-STREAM-070)`, async () => {
    const validate = await schema("events");
    const ch = await made(100, { staged, options: { retain: { rows: 150 } } });
    const v0 = ch.version;
    const seen = [];
    ch.subscribe((c) => c === "data" && seen.push(ch.lastCommit));
    for (let i = 0; i < 40; i++) ch.append({ x: 100 + i, y: i });
    ch.append({ x: 50, y: 0 }); // late
    frame();
    for (let i = 0; i < 20; i++) ch.append({ x: 140 + i, y: i });
    frame();
    ch.data = { x: [0, 1, 2], y: [3, 4, 5] };
    assert.deepEqual(seen, [
      { rows: 140, dropped: 0, late: 1, version: v0 + 1 },
      { rows: 150, dropped: 10, late: 0, version: v0 + 2 },
      { rows: 3, dropped: 0, late: 0, version: v0 + 3 },
    ]);
    for (const detail of seen) check(validate, { type: "plot-data", detail }, "plot-data");
    ch.free();
  });
}

// ---------------------------------------------------------------- appending and replacing end on one frame

for (const [path, staged] of PATHS) {
  test(`${BUILD}: ${path}: 1,000 rows appended over 60 frames draw the frame the same rows replaced draw`, async () => {
    const a = await made(0, { staged });
    const b = await made(0);
    const rows = [];
    for (let i = 0; i < 1000; i++) rows.push({ x: i, y: Math.sin(i / 37) * 10 + (i % 7) });
    for (let i = 0; i < 1000; i++) {
      a.append(rows[i]);
      if (i % 17 === 16) frame();
    }
    frame();
    b.data = rows;
    a.frame(0);
    b.frame(0);
    assert.equal(a.text(), b.text());
    a.free();
    b.free();
  });
}

// ---------------------------------------------------------------- REQ-VIEW-060

test(`${BUILD}: End sets x.follow and it crosses the boundary both ways; a pan clears it (REQ-VIEW-060)`, async () => {
  const ch = await made(1000);
  ch.view = { x: { domain: [100, 300] } };
  assert.equal(ch.view.x.follow, false);
  ch.input({ kind: "key", key: "End" });
  const events = ch.events();
  assert.ok(events.some((e) => e.type === "plot-view" && e.detail.cause === "follow"), JSON.stringify(events));
  assert.equal(ch.view.x.follow, true, "End sets follow");
  ch.view = { x: { follow: false } };
  assert.equal(ch.view.x.follow, false, "set back through the view");
  ch.view = { x: { follow: true } };
  assert.deepEqual(ch.view.x, { domain: [100, 300], follow: true }, "a patch of follow keeps the domain");
  ch.input({ kind: "pan", dx: 5, dy: 0 }); // a two-finger pan of 5 cells, then its end
  ch.input({ kind: "pan", dx: 0, dy: 0, end: true });
  ch.events();
  assert.equal(ch.view.x.follow, false, "a pan away from the end clears follow");
  ch.free();
});

test(`${BUILD}: the fallback follows: an append keeps the x span and ends the domain at the newest x (REQ-VIEW-060)`, async () => {
  const ch = await made(1000);
  ch.view = { x: { domain: [800, 950], follow: true } };
  for (let i = 0; i < 25; i++) ch.append({ x: 1000 + i, y: 0 });
  frame();
  assert.deepEqual(ch.view.x, { domain: [874, 1024], follow: true });
  ch.view = { x: { domain: [100, 200], follow: false } };
  ch.append({ x: 1025, y: 0 });
  frame();
  assert.deepEqual(ch.view.x, { domain: [100, 200], follow: false }, "without follow the domain stays");
  ch.free();
});

test(`${BUILD}: the module follows on append(n) (REQ-VIEW-060)`, { todo: NO_STAGING }, async () => {
  // The real module, not the shim: the core moves the domain itself.
  const { glue, memory } = await instantiate(moduleFor(TYPE), { fresh: true });
  const c = new glue.Chart(TYPE, "{}");
  const s = series(1000);
  const p0 = c.reserve_column(0, 0, 1000);
  new Float64Array(memory.buffer, p0, 1000).set(s.xs);
  const p1 = c.reserve_column(1, 0, 1000);
  new Float64Array(memory.buffer, p1, 1000).set(s.ys);
  c.commit(s.descriptor);
  c.set_view(JSON.stringify({ x: { domain: [800, 950], follow: true } }));
  const ptr = c.staging(25);
  const v = new Float64Array(memory.buffer, ptr, 50);
  for (let i = 0; i < 25; i++) v[i] = 1000 + i;
  const r = c.append(25);
  assert.equal(r[0], 1025);
  assert.deepEqual(JSON.parse(c.view_json()).x, { domain: [874, 1024], follow: true });
  c.free();
});

// ---------------------------------------------------------------- REQ-A11Y-080

test(`${BUILD}: paused keeps appends unwritten; resume writes them and shows the live end (REQ-A11Y-080)`, async () => {
  const ch = await made(100);
  ch.frame(0);
  const before = ch.text();
  ch.view = { x: { domain: [10, 60] } };
  ch.paused = true;
  for (let i = 0; i < 50; i++) ch.append({ x: 100 + i, y: 5 });
  frame();
  assert.equal(called("commit"), 0, "nothing written while paused");
  assert.equal(frames.length, 0, "no frame asked for");
  assert.equal(ch.latest().x, 149, "but the rows are kept");
  ch.view = { x: null };
  ch.frame(0);
  assert.equal(ch.text(), before, "the drawing is the data before the pause");
  ch.view = { x: { domain: [10, 60] } };
  ch.paused = false;
  assert.equal(called("commit"), 1);
  assert.deepEqual(ch.view.x, { domain: [99, 149], follow: true }, "resumed on the live end");
  ch.free();
});

// ---------------------------------------------------------------- REQ-STREAM-050

test(`${BUILD}: appending one row takes at most 1 µs and a batch of 1,000 at most 50 µs, median, in the shell plus wasm (REQ-STREAM-050)`, { todo: NO_STAGING }, async () => {
  const ch = await made(100_000);
  const one = [];
  for (let i = 0; i < 5000; i++) {
    const t0 = performance.now();
    ch.append({ x: 100_000 + i, y: i });
    ch.flush();
    one.push(performance.now() - t0);
  }
  const batch = [];
  const xs = new Float64Array(1000);
  const ys = new Float64Array(1000);
  for (let k = 0; k < 200; k++) {
    for (let i = 0; i < 1000; i++) {
      xs[i] = 200_000 + k * 1000 + i;
      ys[i] = i;
    }
    const t0 = performance.now();
    ch.append({ x: xs, y: ys });
    ch.flush();
    batch.push(performance.now() - t0);
  }
  const m1 = median(one.slice(500)) * 1000;
  const mb = median(batch.slice(20)) * 1000;
  console.log(`# REQ-STREAM-050 ${BUILD} (${ch.streaming}): append + write of 1 row median ${m1.toFixed(2)} µs, of 1,000 rows (typed columns) median ${mb.toFixed(1)} µs, into ${ch.lastCommit.rows} rows; performance.now() in Node ${process.versions.node}, load ${(await import("node:os")).loadavg()[0].toFixed(2)}`);
  assert.ok(m1 <= 1, `one row: ${m1.toFixed(2)} µs`);
  assert.ok(mb <= 50, `1,000 rows: ${mb.toFixed(1)} µs`);
  ch.free();
});

// ---------------------------------------------------------------- REQ-DATA-150

const W = walk(50);
const BODIES = {
  "/walk.csv": ["text/csv", "t,y\n" + W.t.map((t, i) => `${new Date(t).toISOString()},${W.y[i]}`).join("\n") + "\n"],
  "/walk.json": ["application/json", JSON.stringify({ t: W.t.map((t) => new Date(t).toISOString()), y: W.y })],
  "/walk.ndjson": ["application/x-ndjson", W.t.map((t, i) => JSON.stringify({ t: new Date(t).toISOString(), y: W.y[i] })).join("\n")],
  "/feed": ["text/csv; charset=utf-8", "t,y\n" + W.t.map((t, i) => `${new Date(t).toISOString()},${W.y[i]}`).join("\n")],
  "/walk.ndjson.txt": ["text/plain", ""],
  "/fallback.csv": ["text/html; charset=utf-8", "<!doctype html><title>Not here</title>"],
  "/walk.arrows": ["application/octet-stream", Buffer.from(arrowStream({ t: { type: "timestamp", values: W.t }, y: { type: "f64", values: W.y } }))],
};

async function serving(f) {
  const server = createServer((req, res) => {
    const body = BODIES[req.url];
    if (!body) {
      res.writeHead(404).end();
      return;
    }
    res.writeHead(200, { "content-type": body[0] }).end(body[1]);
  });
  await new Promise((r) => server.listen(0, "127.0.0.1", r));
  try {
    return await f(`http://127.0.0.1:${server.address().port}`);
  } finally {
    server.close();
  }
}

test(`${BUILD}: src: CSV, JSON and NDJSON fetched and drawn as data; the format by Content-Type, else extension (REQ-DATA-150)`, async () => {
  await serving(async (base) => {
    for (const path of ["/walk.csv", "/walk.json", "/walk.ndjson", "/feed"]) {
      const got = await shell.source.loadSource(path, base);
      assert.equal(got.format, path === "/walk.json" ? "json" : path === "/walk.ndjson" ? "ndjson" : "csv", path);
      const ch = shell.chart.chart(TYPE, { data: got.data, cols: 80, rows: 24 });
      await ch.ready;
      ch.frame(0);
      assert.equal(ch.data.tables.data.columns[0].type, "timestamp", path);
      assert.match(ch.summary(), /50 points/, path);
      ch.free();
    }
    await assert.rejects(shell.source.loadSource("/walk.ndjson.txt", base), /src "\/walk.ndjson.txt": the format is not known/);
    await assert.rejects(shell.source.loadSource("/missing.csv", base), /src "\/missing.csv": HTTP 404/);
    await assert.rejects(shell.source.loadSource("/fallback.csv", base), /src "\/fallback.csv": the server sent an HTML page/);
  });
});

test(`${BUILD}: src: Arrow bytes without the module's set_arrow fail with a clear message (REQ-DATA-150)`, { skip: REAL.arrow ? "the module has set_arrow" : false }, async () => {
  await serving(async (base) => {
    const got = await shell.source.loadSource("/walk.arrows", base);
    assert.equal(got.format, "arrow");
    assert.equal(shell.arrow.arrowRows(got.bytes), 50);
    const ch = shell.chart.chart(TYPE, {});
    await ch.ready;
    assert.throws(() => (ch.data = got.bytes), /Arrow data needs a module with set_arrow \(v0\.6\.0\); this build's `\w+` module has none/);
    ch.free();
  });
});

test(`${BUILD}: src: Arrow bytes go to set_arrow and draw as the same rows in CSV do (REQ-DATA-150)`, { todo: NO_ARROW }, async () => {
  await serving(async (base) => {
    const arrow = await shell.source.loadSource("/walk.arrows", base);
    const csv = await shell.source.loadSource("/walk.csv", base);
    const a = shell.chart.chart(TYPE, { cols: 80, rows: 24 });
    const b = shell.chart.chart(TYPE, { data: csv.data, cols: 80, rows: 24 });
    await Promise.all([a.ready, b.ready]);
    const seen = [];
    a.subscribe((c) => c === "data" && seen.push(a.lastCommit));
    a.data = arrow.bytes;
    assert.equal(seen[0]?.rows, 50);
    a.frame(0);
    b.frame(0);
    assert.equal(a.text(), b.text());
    a.free();
    b.free();
  });
});
