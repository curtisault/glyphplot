// Inputs in the plan's encoding give events whose JSON events.schema.json
// accepts (REQ-INPUT-020, REQ-EVENT-010, REQ-HIT-020, contract rule 18): a
// scripted session of every input kind, written in input.schema.json's form,
// encoded by the shell's own `encode`, played on each chart over several
// fixtures; `input`'s bit 4 says exactly when events are pending.
import { test } from "node:test";
import assert from "node:assert/strict";
import { BUILD, manifest, moduleOfType, shell, fixtureIndex, fixtureData, commitColumns, schema, check } from "./support/wasm.mjs";

const COLS = 80;
const ROWS = 24;

/** A session of every input kind, in input.schema.json's form. */
function script() {
  const mouse = (kind, x, y, buttons = 0, mods = []) => ({ kind, x, y, pointer: "mouse", buttons, mods });
  const touch = (kind, x, y, buttons = 0) => ({ kind, x, y, pointer: "touch", buttons, mods: [] });
  const out = [];
  for (let x = 10.5; x < 75; x += 7) out.push(mouse("move", x, 12.5)); // hover
  out.push(mouse("down", 30.5, 10.5, 1), mouse("move", 40.5, 10.5, 1), mouse("up", 40.5, 10.5)); // a drag pans
  out.push(mouse("down", 20.5, 12.5, 1, ["shift"]), mouse("move", 50.5, 12.5, 1, ["shift"]), mouse("up", 50.5, 12.5, 0, ["shift"])); // a shift brush
  out.push({ kind: "tap", x: 40.5, y: 12.5, count: 1 }, { kind: "tap", x: 40.5, y: 12.5, count: 2 }); // a select, then the reset
  out.push({ kind: "wheel", x: 40.5, y: 12.5, dx: 0, dy: -3, mods: ["ctrl"] }, { kind: "wheel", x: 40.5, y: 12.5, dx: 0, dy: 0, mods: ["ctrl"] }); // zoom and its end
  out.push({ kind: "wheel", x: 40.5, y: 12.5, dx: 0, dy: 4, mods: [] }); // a plain wheel scrolls the page
  out.push({ kind: "pinch", x: 40.5, y: 12.5, scale: 1.5, end: false }, { kind: "pinch", x: 40.5, y: 12.5, scale: 1, end: true });
  out.push({ kind: "pan", dx: 5, dy: 0, velocity: 0, end: false }, { kind: "pan", dx: 0, dy: 0, velocity: 12, end: true });
  out.push(touch("down", 30.5, 12.5, 1), { kind: "press", x: 30.5, y: 12.5 }, touch("move", 45.5, 12.5, 1), touch("up", 45.5, 12.5)); // a long-press brush
  out.push(touch("down", 35.5, 12.5, 1), touch("move", 38.5, 12.5, 1), touch("up", 38.5, 12.5)); // a scrub
  const shellKeys = ["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "Home", "End", "PageUp", "PageDown", "+", "=", "-", "0", "Enter", "Escape", " ", "?", "1", "2", "3", "4", "5", "6", "7", "8", "9"];
  for (const key of shellKeys) out.push({ kind: "key", key, mods: [] });
  for (const key of ["ArrowLeft", "ArrowRight", "Enter", "ArrowRight", "ArrowRight", "Enter", "Escape", "Escape"]) out.push({ kind: "key", key, mods: ["shift"] });
  for (const key of ["Enter", "Escape", "1", "1"]) out.push({ kind: "key", key, mods: [] });
  out.push({ kind: "leave" });
  return out.map((e, i) => ({ t: i * 0.05, ...e }));
}

const SESSION = script();
const index = fixtureIndex();

test(`${BUILD}: the session is in input.schema.json's form`, async () => {
  const validate = await schema("input");
  check(validate, SESSION, "the session");
  for (const e of SESSION) check(validate, e, JSON.stringify(e));
});

for (const meta of manifest.charts) {
  // The series fixtures for a series chart; for a chart of another shape
  // (candles, bar), every fixture of its shapes up to 10,000 rows.
  const named = index.fixtures.filter((f) => meta.shapes.includes(f.shape) && ["sine-1k", "multi-4", "labels-wide", "random-walk-100k", "nan-gaps", "one", "empty"].includes(f.name));
  const fixtures = named.length > 0 ? named : index.fixtures.filter((f) => meta.shapes.includes(f.shape) && f.rows <= 10_000);
  test(`${BUILD}: ${meta.slug}: every event the session gives validates against events.schema.json (REQ-EVENT-010, REQ-INPUT-020)`, async () => {
    const validate = await schema("events");
    const { glue, memory } = await moduleOfType(meta.slug);
    const seen = new Map();
    for (const f of fixtures) {
      const c = new glue.Chart(meta.slug, "{}");
      const { descriptor, columns } = fixtureData(f);
      commitColumns(c, memory, descriptor, columns);
      c.resize(COLS, ROWS);
      c.frame(0, false);
      for (const e of SESSION) {
        const n = shell.input.encode(e);
        const bits = c.input(n.kind, n.x, n.y, n.a, n.b, n.buttons, n.mods, e.t);
        assert.equal(bits & ~7, 0, `${JSON.stringify(e)}: only bits 1, 2 and 4`);
        const events = JSON.parse(c.events_json());
        assert.ok(Array.isArray(events));
        assert.equal(events.length > 0, (bits & 4) !== 0, `${f.name} ${JSON.stringify(e)}: bit 4 is set exactly when events are pending (${c.events_json()})`);
        for (const ev of events) {
          check(validate, ev, `${f.name} after ${JSON.stringify(e)}: ${JSON.stringify(ev)}`);
          seen.set(ev.type, (seen.get(ev.type) ?? 0) + 1);
          if (ev.type === "plot-view") check(await schema("view"), ev.detail.view, "plot-view's view");
        }
        assert.equal(c.events_json(), "[]", "reading the events clears them");
        if (bits & 3) c.frame(e.t, false);
      }
      c.free();
    }
    console.log(`# ${BUILD} ${meta.slug}: events seen ${JSON.stringify(Object.fromEntries(seen))} over ${fixtures.map((f) => f.name).join(", ")}`);
    // An OHLCV chart draws one instrument: no series to toggle, so no plot-legend.
    const types = ["plot-hover", "plot-view", "plot-select", "plot-brush", "plot-cursor", ...(meta.shapes.includes("ohlcv") ? [] : ["plot-legend"])];
    for (const type of types) assert.ok(seen.has(type), `the session gives ${type}`);
  });
}

test(`${BUILD}: an input past the encoding's table changes nothing and gives no event`, async () => {
  const meta = manifest.charts[0];
  const { glue, memory } = await moduleOfType(meta.slug);
  const c = new glue.Chart(meta.slug, "{}");
  const f = index.fixtures.find((x) => x.name === "sine-1k");
  const { descriptor, columns } = fixtureData(f);
  commitColumns(c, memory, descriptor, columns);
  c.frame(0, false);
  const v = c.view_json();
  assert.equal(c.input(10, 40, 12, 0, 0, 0, 0, 0), 0, "kind 10");
  assert.equal(c.input(9, 0, 0, 99, 0, 0, 0, 0), 0, "key code 99");
  assert.equal(c.events_json(), "[]");
  assert.equal(c.view_json(), v);
  c.free();
});
