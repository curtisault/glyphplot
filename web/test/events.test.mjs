// events.ts: plot-hover only on a new datum, plot-view at most once a frame
// and exactly one final (REQ-EVENT-020, REQ-EVENT-030).
import { test } from "node:test";
import assert from "node:assert/strict";
import { EventGate, WHEEL_IDLE_MS } from "../src/events.ts";

const hover = (series, index) => ({ type: "plot-hover", detail: index === null ? null : { source: "pointer", series, index, x: index, y: 1 } });
const view = (final, source = "wheel") => ({ type: "plot-view", detail: { source, cause: "zoom", final, view: { version: 1 } } });

test("plot-hover: the frame's last, only when the datum changes, null when it leaves", () => {
  const g = new EventGate();
  assert.deepEqual(g.frame([hover("y", 1), hover("y", 2)]).map((e) => e.detail.index), [2]);
  assert.deepEqual(g.frame([hover("y", 2)]), []);
  assert.deepEqual(g.frame([hover("y", 3)]).length, 1);
  assert.deepEqual(g.frame([hover("y", null)]), [hover("y", null)]);
  assert.deepEqual(g.frame([hover("y", null)]), []);
  g.resetHover();
  assert.equal(g.frame([hover("y", 3)]).length, 1);
});

test("plot-view: one step a frame, a final supersedes the frame's steps, the shell closes an open gesture once", () => {
  const g = new EventGate();
  const a = g.frame([view(false), view(false)]);
  assert.equal(a.length, 1);
  assert.deepEqual(g.open, { source: "wheel", cause: "zoom" });
  assert.equal(g.close("pointer", {}), null, "a pointer up does not end a wheel gesture");
  const f = g.close("wheel", { version: 1 });
  assert.deepEqual(f, { type: "plot-view", detail: { source: "wheel", cause: "zoom", final: true, view: { version: 1 } } });
  assert.equal(g.close("wheel", {}), null, "exactly once");
  const b = g.frame([view(false, "pointer"), view(true, "pointer")]);
  assert.deepEqual(b.map((e) => e.detail.final), [true]);
  assert.equal(g.open, null);
  g.frame([view(false, "keyboard")]);
  assert.equal(g.close("key", {}).detail.final, true);
  g.frame([view(false, "touch")]);
  assert.equal(g.close("motion", {}).detail.source, "touch");
  assert.equal(WHEEL_IDLE_MS, 150);
});

test("other events pass in order", () => {
  const g = new EventGate();
  const sel = { type: "plot-select", detail: {} };
  const cur = { type: "plot-cursor", detail: {} };
  assert.deepEqual(g.frame([sel, cur]), [sel, cur]);
});
