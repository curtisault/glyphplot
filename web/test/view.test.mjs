// The view state across the boundary (REQ-VIEW-030, REQ-VIEW-010): view_json
// and set_view round-trip; a patch merges key by key and a key set to null
// returns to its default; the JS Chart's `view` does the same, `null`
// resetting every part; every view validates against view.schema.json.
import { test } from "node:test";
import assert from "node:assert/strict";
import { BUILD, types, moduleOfType, shell, series, commitColumns, schema, check } from "./support/wasm.mjs";

const TYPE = types[0];

async function made() {
  const { glue, memory } = await moduleOfType(TYPE);
  const c = new glue.Chart(TYPE, "{}");
  const s = series(1000);
  commitColumns(c, memory, s.descriptor, [s.xs, s.ys]);
  c.frame(0, false);
  return c;
}

const view = (c) => JSON.parse(c.view_json());

test(`${BUILD}: view_json is a whole view; set_view of it changes nothing (REQ-VIEW-030)`, async () => {
  const validate = await schema("view");
  const c = await made();
  const v0 = view(c);
  check(validate, v0, "the default view");
  assert.equal(v0.version, 1);
  c.set_view(JSON.stringify(v0));
  assert.deepEqual(view(c), v0);
  c.free();
});

test(`${BUILD}: a whole view set is read back as set (REQ-VIEW-030)`, async () => {
  const validate = await schema("view");
  const c = await made();
  const v0 = view(c);
  const set = {
    ...v0,
    x: { domain: [100, 400], follow: false },
    y: { domain: [-0.5, 0.5], fit: "visible" },
    cursor: { x: 250, y: Math.sin(5), series: "y", index: 250, locked: true },
    selection: { x: [200, 300], y: null, points: [{ series: "y", index: 250 }] },
    hidden: [],
  };
  check(validate, set, "the view set");
  c.set_view(JSON.stringify(set));
  const got = view(c);
  check(validate, got, "the view read");
  assert.deepEqual(got, set);
  c.frame(0, false);
  assert.deepEqual(view(c), set, "a frame keeps the view");
  c.free();
});

test(`${BUILD}: a patch merges key by key; null returns a key to its default (REQ-VIEW-030)`, async () => {
  const c = await made();
  const v0 = view(c);
  c.set_view(JSON.stringify({ x: { domain: [100, 400] }, cursor: { x: 250, y: 0, series: "y", index: 250, locked: false } }));
  let v = view(c);
  assert.deepEqual(v.x, { domain: [100, 400], follow: v0.x.follow }, "x.follow kept from before");
  assert.equal(v.cursor.index, 250);
  c.set_view(JSON.stringify({ x: { follow: true } }));
  v = view(c);
  assert.deepEqual(v.x, { domain: [100, 400], follow: true }, "a patch of x.follow keeps x.domain");
  c.set_view(JSON.stringify({ x: null }));
  v = view(c);
  assert.deepEqual(v.x, v0.x, "x: null is the default x");
  assert.equal(v.cursor.index, 250, "other keys kept");
  c.set_view(JSON.stringify({ cursor: null, hidden: null }));
  assert.deepEqual(view(c), v0, "everything back to the default");
  c.free();
});

test(`${BUILD}: a view that does not fit view.schema.json throws \`view: …\` and changes nothing`, async () => {
  const c = await made();
  const v0 = view(c);
  for (const bad of ['{"x":3}', '{"x":{"domain":[1,2,3]}}', "{"]) {
    assert.throws(() => c.set_view(bad), (e) => e.message.startsWith("view: "), bad);
    assert.deepEqual(view(c), v0, bad);
  }
  c.free();
});

test(`${BUILD}: the JS Chart's view: set before load, merged, null resets every part (REQ-VIEW-030, REQ-VIEW-040)`, async () => {
  const s = series(1000);
  const ch = shell.chart.chart(TYPE, { data: { x: s.xs, y: s.ys }, view: { x: { domain: [10, 20] } }, cols: 80, rows: 24 });
  assert.deepEqual(ch.view, { x: { domain: [10, 20] } }, "before load: the patches so far");
  await ch.ready;
  assert.deepEqual(ch.view.x.domain, [10, 20]);
  ch.view = { cursor: { x: 15, y: 0, series: "y", index: 15, locked: false } };
  assert.deepEqual(ch.view.x.domain, [10, 20], "a patch keeps x");
  assert.equal(ch.view.cursor.index, 15);
  assert.deepEqual(ch.events(), [], "a view set by script fires no event");
  ch.view = null;
  const fresh = shell.chart.chart(TYPE, { data: { x: s.xs, y: s.ys }, cols: 80, rows: 24 });
  await fresh.ready;
  assert.deepEqual(ch.view, fresh.view, "null is the default view");
  // A view moves between charts as JSON (REQ-VIEW-010).
  ch.view = { x: { domain: [300, 600] } };
  fresh.view = ch.view;
  ch.frame(0);
  fresh.frame(0);
  assert.equal(fresh.text(), ch.text());
  ch.free();
  fresh.free();
});
