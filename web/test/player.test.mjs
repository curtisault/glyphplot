// The wasm player in Node, against the build under test (REQ-TEST-020): the
// exports (REQ-WASM-010), the manifest (REQ-MANIFEST-010, 020), make and
// frame with the memory views equal to text() (REQ-WASM-030), the error
// strings (REQ-WASM-020), traps in the debug build (REQ-WASM-040, 060) and
// free (REQ-WASM-050).
import { test } from "node:test";
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { BUILD, DEBUG, PKG, debugOnly, releaseOnly, manifest, types, moduleFor, instantiate, moduleOfType, shell, views, textOf, series, commitColumns } from "./support/wasm.mjs";

/** The plan's boundary table, v0.3.0 members, plus set_zone (v0.5.0, exported early). */
const CHART_MEMBERS = [
  "set_options", "reserve_column", "set_column", "commit", "set_locale", "set_zone", "view_json", "set_view", "input", "events_json",
  "resize", "set_mono", "frame", "cols", "rows", "cells_ptr", "colors_ptr", "backs_ptr", "text", "summary", "table_json", "meta_json", "free",
];
/** wasm-bindgen's own members of every exported class. */
const GLUE_MEMBERS = ["constructor", "__destroy_into_raw"];
const MODULE_FUNCTIONS = ["Chart", "charts_json", "default", "initSync", "resolve_palette", "wide_tail"];

const moduleNames = Object.keys(manifest.imports);

test(`${BUILD}: each module exports exactly the boundary's members (REQ-WASM-010, REQ-WASM-060)`, async () => {
  for (const name of moduleNames) {
    const { glue } = await instantiate(name);
    const want = DEBUG ? [...MODULE_FUNCTIONS, "start", "debug_reference", "debug_fixture_digest"] : MODULE_FUNCTIONS;
    assert.deepEqual(Object.keys(glue).sort(), [...want].sort(), `${name}: module functions`);
    const own = Object.getOwnPropertyNames(glue.Chart.prototype).filter((k) => !GLUE_MEMBERS.includes(k));
    assert.deepEqual(own.sort(), [...CHART_MEMBERS].sort(), `${name}: Chart members`);
    assert.equal(glue.wide_tail(), 0);
  }
});

test(`${BUILD}: the manifest maps every type to the module that plays exactly its types (REQ-MANIFEST-010, REQ-MANIFEST-020)`, async () => {
  assert.deepEqual(Object.keys(manifest.modules), types, "modules lists the charts, in registry order");
  assert.deepEqual(Object.keys(manifest.imports).sort(), [...new Set(Object.values(manifest.modules))].sort());
  assert.deepEqual(Object.keys(manifest.budgets).sort(), Object.keys(manifest.imports).sort());
  assert.deepEqual(JSON.parse(readFileSync(join(PKG, "budgets.json"), "utf8")), manifest.budgets, "budgets.json equals the manifest's budgets");
  assert.deepEqual(Object.keys(JSON.parse(readFileSync(join(PKG, "sizes.json"), "utf8"))).sort(), Object.keys(manifest.imports).sort());
  assert.deepEqual(Object.keys(manifest.wasm).sort(), Object.keys(manifest.imports).sort(), "wasm gives a URL for every module");
  for (const [name, url] of Object.entries(manifest.wasm)) {
    assert.equal(url().href, pathToFileURL(join(PKG, name, "glyphplot_bg.wasm")).href, `${name}: wasm names the module's own file`);
    assert.ok(existsSync(url()), `${name}: its wasm exists`);
  }
  if (DEBUG) {
    assert.equal(manifest.fallback, "all");
    for (const t of types) assert.equal(manifest.modules[t], "all", t);
    assert.equal(moduleFor("debug-panic"), "all", "a debug chart falls back to all");
    assert.equal(moduleFor("no-such-chart"), "all");
  } else {
    assert.equal(manifest.fallback, null);
    for (const m of manifest.charts) assert.equal(manifest.modules[m.slug], m.family, `${m.slug} is played by its family's module`);
    assert.equal(moduleFor("no-such-chart"), null);
    assert.equal(moduleFor("debug-panic"), null);
  }
  const played = [];
  for (const name of moduleNames) {
    const { glue } = await instantiate(name);
    const slugs = JSON.parse(glue.charts_json()).map((m) => m.slug).filter((s) => s !== "debug-panic");
    assert.deepEqual(slugs, types.filter((t) => manifest.modules[t] === name), `${name} plays exactly the types the manifest gives it`);
    played.push(...JSON.parse(glue.charts_json()).filter((m) => m.slug !== "debug-panic"));
  }
  assert.deepEqual(played.map((m) => m.slug).sort(), [...types].sort());
  for (const m of played) assert.deepEqual(m, manifest.charts.find((c) => c.slug === m.slug), `${m.slug}: the module's meta equals the manifest's`);
});

test(`${BUILD}: make, frame, and the memory views equal text() after every frame (REQ-WASM-030, REQ-ARCH-020)`, async () => {
  for (const type of types) {
    const { glue, memory } = await moduleOfType(type);
    const c = new glue.Chart(type, "{}");
    assert.deepEqual(JSON.parse(c.meta_json()), manifest.charts.find((m) => m.slug === type));
    const s = series(1000);
    commitColumns(c, memory, s.descriptor, [s.xs, s.ys]);
    const sizes = [[80, 24], [manifest.charts.find((m) => m.slug === type).min.cols, manifest.charts.find((m) => m.slug === type).min.rows], [200, 50], [1, 1], [17, 9]];
    for (const [cols, rows] of sizes) {
      c.resize(cols, rows);
      for (const [t, paper, mono] of [[0, false, false], [2.5, true, false], [0, false, true]]) {
        c.set_mono(mono);
        c.frame(t, paper);
        const text = c.text(); // may allocate: the views are made after it (REQ-WASM-030)
        const g = views(c, memory);
        assert.equal(g.cols, cols);
        assert.equal(g.rows, rows);
        assert.equal(textOf(g, glue.wide_tail()), text, `${type} ${cols}×${rows} t=${t}: views equal text()`);
        assert.equal(g.colors.length, cols * rows);
        assert.equal(g.backs.length, cols * rows);
      }
    }
    // A call that grows memory detaches the old views; the shell's views, made after the call, still equal text().
    const { Guarded } = shell.wasm;
    const loaded = (await moduleOfType(type)).loaded;
    const gd = new Guarded(c, loaded);
    c.resize(80, 24);
    c.frame(0, false);
    const before = gd.grid();
    const big = series(400_000);
    const pages = memory.buffer.byteLength;
    commitColumns(c, memory, big.descriptor, [big.xs, big.ys]);
    assert.ok(memory.buffer.byteLength > pages, "the commit grew memory");
    assert.equal(before.cells.length, 0, "a view made before growth is detached");
    gd.call((r) => r.frame(0, false));
    assert.equal(textOf(gd.grid(), glue.wide_tail()), c.text());
    c.free();
  }
});

test(`${BUILD}: the JS Chart plays every type through the shell's loader (REQ-JS-010, REQ-LOAD-010)`, async () => {
  for (const type of types) {
    const ch = shell.chart.chart(type, { data: { x: [1, 2, 3, 4], y: [3, 1, 4, 1] }, cols: 40, rows: 12 });
    await ch.ready;
    const g = ch.frame(0);
    assert.equal(textOf(g), ch.text());
    assert.equal(g.cols * g.rows, 480);
    assert.match(ch.summary(), new RegExp(type));
    ch.free();
  }
});

test(`${BUILD}: making a chart throws the boundary's messages, and the module keeps working (REQ-WASM-020)`, async () => {
  const { glue, memory } = await moduleOfType(types[0]);
  const thrown = (f) => {
    try {
      f();
    } catch (e) {
      return e instanceof Error ? e.message : String(e);
    }
    return null;
  };
  assert.equal(thrown(() => new glue.Chart("no-such-chart", "{}")), 'there is no chart named "no-such-chart"');
  assert.match(thrown(() => new glue.Chart(types[0], "{")), /^options are not a JSON object: ./);
  assert.match(thrown(() => new glue.Chart(types[0], '{"mode": 1')), /^options are not a JSON object: ./);
  const c = new glue.Chart(types[0], "{}");
  assert.match(thrown(() => c.set_options("{")), /^options are not a JSON object: ./);
  const s = series(100);
  commitColumns(c, memory, s.descriptor, [s.xs, s.ys]);
  c.frame(0, false);
  assert.ok(c.text().trim().length > 0, "the module draws after the throws");
  c.free();
  // Through the shell: a type no module plays rejects `ready` with the same message.
  const ch = shell.chart.chart("no-such-chart");
  await assert.rejects(ch.ready, { message: 'there is no chart named "no-such-chart"' });
});

test(`${BUILD}: options that are JSON but not an object throw, and blank options are none (REQ-WASM-020)`, async () => {
  const { glue } = await moduleOfType(types[0]);
  for (const bad of ["[1]", "1", "null", '"s"']) {
    assert.throws(() => new glue.Chart(types[0], bad), { message: /^options are not a JSON object: ./ }, bad);
    const c = new glue.Chart(types[0], "{}");
    assert.throws(() => c.set_options(bad), { message: /^options are not a JSON object: ./ }, bad);
    c.free();
  }
  for (const none of ["", "  ", "\n"]) new glue.Chart(types[0], none).free();
});

test(`${BUILD}: a release build has no debug chart and no panic hook (REQ-WASM-060)`, { skip: releaseOnly }, async () => {
  for (const name of moduleNames) {
    const { glue } = await instantiate(name);
    assert.equal(glue.start, undefined);
    assert.throws(() => new glue.Chart("debug-panic", "{}"), (e) => e.message === 'there is no chart named "debug-panic"');
    assert.ok(!glue.charts_json().includes("debug-panic"));
  }
});

test(`${BUILD}: a trap stops only its chart; others draw and new ones are made (REQ-WASM-040, REQ-WASM-060)`, { skip: debugOnly }, async () => {
  const { glue, memory, loaded } = await moduleOfType("debug-panic");
  const errors = [];
  const error = console.error;
  console.error = (...a) => errors.push(a.join(" "));
  try {
    const other = new glue.Chart(types[0], "{}");
    const s = series(1000);
    commitColumns(other, memory, s.descriptor, [s.xs, s.ys]);
    other.frame(0, false);
    const want = other.text();
    const p = new glue.Chart("debug-panic", "{}");
    assert.throws(() => p.frame(0, false), (e) => e instanceof WebAssembly.RuntimeError);
    assert.equal(errors.length, 1, "one console.error before the trap");
    assert.match(errors[0], /^glyphplot: panicked at crates\/wasm\/src\/debug\.rs:\d+:\d+:\ndebug-panic: framed at t = 0/);
    // The panicked chart refuses every call.
    for (const call of [() => p.text(), () => p.frame(0, false), () => p.view_json(), () => p.cols(), () => p.resize(10, 10), () => p.input(0, 1, 1, 0, 0, 0, 0, 0)]) assert.throws(call);
    // Others keep drawing; new ones can be made.
    other.frame(0, false);
    assert.equal(other.text(), want);
    assert.equal(textOf(views(other, memory)), want);
    const fresh = new glue.Chart(types[0], "{}");
    commitColumns(fresh, memory, s.descriptor, [s.xs, s.ys]);
    fresh.frame(0, false);
    assert.equal(fresh.text(), want);
    // The shell's guard: the trap kills the chart, later calls refuse with its reason, others go on.
    const { Guarded } = shell.wasm;
    const g = new Guarded(new glue.Chart("debug-panic", "{}"), loaded);
    const sp = loaded.stack ? loaded.stack(0) : null;
    assert.throws(() => g.call((c) => c.frame(0, false)), (e) => e instanceof WebAssembly.RuntimeError);
    assert.match(g.dead, /^the chart stopped after a trap/);
    assert.throws(() => g.call((c) => c.text()), /^Error: glyphplot: the chart stopped after a trap/);
    if (sp !== null && loaded.stack) assert.equal(loaded.stack(0), sp, "the shell rewound the shadow stack");
    g.free();
    const after = new Guarded(fresh, loaded);
    after.call((c) => c.frame(0, false));
    assert.equal(textOf(after.grid()), want);
    other.free();
    fresh.free();
  } finally {
    console.error = error;
  }
});

test(`${BUILD}: a hundred traps leave the instance drawing (REQ-WASM-040)`, { skip: debugOnly }, async () => {
  const { glue, memory, loaded } = await moduleOfType("debug-panic");
  const { Guarded } = shell.wasm;
  const error = console.error;
  console.error = () => {};
  try {
    const other = new glue.Chart(types[0], "{}");
    const s = series(500);
    commitColumns(other, memory, s.descriptor, [s.xs, s.ys]);
    other.frame(0, false);
    const want = other.text();
    for (let i = 0; i < 100; i++) {
      const g = new Guarded(new glue.Chart("debug-panic", "{}"), loaded);
      assert.throws(() => g.call((c) => c.frame(i, false)));
      g.free();
    }
    other.frame(0, false);
    assert.equal(other.text(), want);
    other.free();
  } finally {
    console.error = error;
  }
});

test(`${BUILD}: free releases the chart; later calls throw (REQ-WASM-050)`, async () => {
  const { glue } = await moduleOfType(types[0]);
  const c = new glue.Chart(types[0], "{}");
  c.frame(0, false);
  c.free();
  assert.throws(() => c.text());
  assert.throws(() => c.frame(0, false));
  const ch = shell.chart.chart(types[0], { data: { x: [1, 2], y: [1, 2] } });
  await ch.ready;
  ch.free();
  assert.equal(ch.dead, "the chart was freed");
  assert.throws(() => ch.frame(0), /the chart was freed/);
});
