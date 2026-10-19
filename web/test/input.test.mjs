// input.ts: the numeric encoding (REQ-INPUT-020), per-frame coalescing
// (REQ-INPUT-030) and the touch recogniser (REQ-TOUCH-010).
import { test } from "node:test";
import assert from "node:assert/strict";
import { encode, InputQueue, Gestures, keyCode, modsOf, buttonsOf, GESTURE } from "../src/input.ts";
import { InputKind, KEYS } from "../src/api.ts";

test("the plan's input encoding table", () => {
  assert.deepEqual(encode({ kind: "move", x: 3.5, y: 2.25, pointer: "touch", buttons: 1, mods: ["shift"] }), { kind: 0, x: 3.5, y: 2.25, a: 0, b: 0, buttons: 1 + 512, mods: 1 });
  assert.deepEqual(encode({ kind: "down", x: 1, y: 1, pointer: "pen", buttons: 1 }).buttons, 257);
  assert.deepEqual(encode({ kind: "leave" }), { kind: 3, x: 0, y: 0, a: 0, b: 0, buttons: 0, mods: 0 });
  assert.deepEqual(encode({ kind: "wheel", x: 1, y: 2, dx: 0, dy: -3, mods: ["ctrl", "meta"] }), { kind: 4, x: 1, y: 2, a: 0, b: -3, buttons: 0, mods: 10 });
  assert.deepEqual(encode({ kind: "pinch", x: 40, y: 12, scale: 1.25, end: true }), { kind: 5, x: 40, y: 12, a: 1.25, b: 1, buttons: 0, mods: 0 });
  assert.deepEqual(encode({ kind: "pan", dx: -2.5, dy: 0, end: true, velocity: -18 }), { kind: 6, x: -18, y: 1, a: -2.5, b: 0, buttons: 0, mods: 0 });
  assert.deepEqual(encode({ kind: "tap", x: 1, y: 1, count: 2 }).a, 2);
  assert.equal(encode({ kind: "press", x: 1, y: 1 }).kind, 8);
  assert.equal(encode({ kind: "key", key: "Escape" }).a, 13);
  assert.throws(() => encode({ kind: "key", key: "x" }), /not one the chart takes/);
  assert.equal(modsOf(["shift", "ctrl", "alt", "meta"]), 15);
  assert.equal(buttonsOf(33, "mouse"), 1);
});

test("key codes: the list, the shell sends 0–24 only", () => {
  assert.equal(KEYS.length, 31);
  assert.deepEqual([keyCode("ArrowLeft"), keyCode("PageDown"), keyCode("+"), keyCode("0"), keyCode(" "), keyCode("?"), keyCode("1"), keyCode("9")], [0, 7, 8, 11, 14, 15, 16, 24]);
  assert.equal(keyCode("h"), -1);
  assert.equal(keyCode("Tab"), -1);
});

const ev = (kind, o = {}) => ({ kind: InputKind[kind], x: 0, y: 0, a: 0, b: 0, buttons: 0, mods: 0, ...o });

test("one move a frame, the latest, after the discrete events in order", () => {
  const q = new InputQueue();
  q.push(ev("move", { x: 1 }));
  q.push(ev("key", { a: 1 }));
  q.push(ev("move", { x: 2 }));
  q.push(ev("tap", { a: 1 }));
  q.push(ev("move", { x: 3 }));
  const out = q.take();
  assert.deepEqual(out.map((e) => e.kind), [InputKind.key, InputKind.tap, InputKind.move]);
  assert.equal(out[2].x, 3);
  assert.equal(q.pending, false);
});

test("a down, up or leave supersedes the move queued before it", () => {
  const q = new InputQueue();
  q.push(ev("move", { x: 1 }));
  q.push(ev("up", { x: 2 }));
  assert.deepEqual(q.take().map((e) => e.kind), [InputKind.up]);
});

test("wheel deltas summed, pinches multiplied, pans summed until one ends", () => {
  const q = new InputQueue();
  q.push(ev("wheel", { a: 1, b: 2, x: 1, mods: 2 }));
  q.push(ev("wheel", { a: 0.5, b: -1, x: 5, mods: 2 }));
  q.push(ev("pinch", { a: 1.1 }));
  q.push(ev("pinch", { a: 1.2 }));
  q.push(ev("pan", { a: 1, b: 0 }));
  q.push(ev("pan", { a: 2, b: 1, y: 1, x: -4 }));
  q.push(ev("pan", { a: 3 }));
  const out = q.take();
  assert.equal(out.length, 4);
  assert.deepEqual([out[0].a, out[0].b, out[0].x], [1.5, 1, 5]);
  assert.ok(Math.abs(out[1].a - 1.32) < 1e-12);
  assert.deepEqual([out[2].a, out[2].b, out[2].y, out[2].x], [3, 1, 1, -4]);
  assert.equal(out[3].a, 3);
  const q2 = new InputQueue();
  q2.push(ev("wheel", { b: 1, mods: 0 }));
  q2.push(ev("wheel", { b: 1, mods: 2 }));
  assert.equal(q2.take().length, 2, "a change of mods is a new wheel");
});

const sample = (o) => ({ id: 1, type: "mouse", px: 0, py: 0, x: 0, y: 0, buttons: 0, mods: 0, time: 0, ...o });

function recorder() {
  const out = [];
  const timers = [];
  const g = new Gestures((e) => out.push(e), (f, ms) => { timers.push({ f, ms }); return timers.length - 1; }, (h) => { if (timers[h]) timers[h].f = null; });
  return { g, out, fire: () => timers.forEach((t) => t.f && t.f()), kinds: () => out.map((e) => Object.keys(InputKind).find((k) => InputKind[k] === e.kind)) };
}

test("mouse: down, moves, up; a still click taps; two within 500 ms double-tap", () => {
  const r = recorder();
  r.g.move(sample({ px: 1 })); // hover
  r.g.down(sample({ buttons: 1 }));
  r.g.up(sample({ time: 100 }));
  r.g.down(sample({ buttons: 1, time: 300 }));
  r.g.up(sample({ time: 350, px: 3 }));
  assert.deepEqual(r.kinds(), ["move", "down", "up", "tap", "down", "up", "tap"]);
  assert.deepEqual([r.out[3].a, r.out[6].a], [1, 2]);
  const d = recorder();
  d.g.down(sample({ buttons: 1 }));
  d.g.move(sample({ px: 20, buttons: 1 }));
  d.g.up(sample({ px: 20 }));
  assert.deepEqual(d.kinds(), ["down", "move", "up"], "a drag does not tap");
});

test("touch: one finger scrubs with pointer touch; a long press presses; a quick still release taps", () => {
  const r = recorder();
  const t = (o) => sample({ type: "touch", ...o });
  r.g.down(t({}));
  r.g.move(t({ px: 30, x: 3 }));
  r.g.up(t({ px: 30, time: 200 }));
  assert.deepEqual(r.kinds(), ["down", "move", "up"]);
  assert.equal(r.out[1].buttons, 1 + 512);
  const p = recorder();
  p.g.down(t({}));
  p.fire();
  p.g.up(t({ time: 800 }));
  assert.deepEqual(p.kinds(), ["down", "press", "up"]);
  const q = recorder();
  q.g.down(t({}));
  q.g.up(t({ time: 100 }));
  assert.deepEqual(q.kinds(), ["down", "up", "tap"]);
  assert.equal(GESTURE.longPressMs, 500);
  const c = recorder();
  c.g.down(t({}));
  c.g.up(t({ time: 100 }), true);
  assert.deepEqual(c.kinds(), ["down", "up"], "a cancelled pointer (a page scroll) does not tap");
});

test("touch: two fingers pan and pinch about the centroid, then end with a velocity", () => {
  const r = recorder();
  const t = (o) => sample({ type: "touch", ...o });
  r.g.down(t({ id: 1, px: 0, py: 0, x: 0, y: 0 }));
  r.g.down(t({ id: 2, px: 100, py: 0, x: 10, y: 0, time: 10 }));
  r.g.move(t({ id: 1, px: -50, py: 0, x: -5, y: 0, time: 20 }));
  r.g.move(t({ id: 2, px: 150, py: 0, x: 15, y: 0, time: 30 }));
  r.g.up(t({ id: 1, time: 40 }));
  r.g.up(t({ id: 2, time: 50 }));
  const k = r.kinds();
  assert.deepEqual(k.slice(0, 2), ["down", "up"], "the first finger's scrub ends");
  assert.ok(k.includes("pinch"));
  const pinches = r.out.filter((e) => e.kind === InputKind.pinch);
  const scale = pinches.filter((e) => e.b === 0).reduce((s, e) => s * e.a, 1);
  assert.ok(Math.abs(scale - 2) < 1e-9, `spread 100 → 200 px: ${scale}`);
  const end = r.out.filter((e) => e.kind === InputKind.pan && e.y === 1);
  assert.equal(end.length, 1);
  assert.equal(pinches[pinches.length - 1].b, 1);
});
