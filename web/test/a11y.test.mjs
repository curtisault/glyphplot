// a11y.ts: the slider's value text (REQ-A11Y-030), the table caption
// (REQ-A11Y-020), the figure's name (REQ-A11Y-010), the once-a-second throttle,
// and the live region's interval and words (REQ-A11Y-100).
import { test } from "node:test";
import assert from "node:assert/strict";
import { valueText, captionOf, figureName, Formats, throttle, KEY_SHORTCUTS, CONTROLS, announceSeconds, latestText, ANNOUNCE_MIN_S } from "../src/a11y.ts";

const fmt = new Formats("en-US");
const desc = { tables: { data: { length: 3, columns: [{ name: "t", type: "timestamp" }, { name: "a", type: "f64" }, { name: "b", type: "f64" }, { name: "c", type: "f64" }, { name: "d", type: "f64" }, { name: "e", type: "f64" }] } } };

test("value text: x, then up to four series, then and N more", () => {
  const t = Date.UTC(2026, 9, 18, 10, 32);
  const text = valueText({ series: "a", x: t, y: 1, datum: { t, a: 104.2, b: 1200, c: null, d: 4, e: 5 } }, desc, fmt);
  assert.equal(text.replace(/\s/g, " "), "Oct 18, 2026, 10:32 AM, a 104.2, b 1,200, c no value, d 4, and 1 more");
  assert.equal(valueText({ series: "y", x: 3, y: 0.5 }, null, fmt), "3, y 0.5");
  assert.equal(valueText(null, null, fmt), "No point selected");
});

test("caption states the count when the view holds more", () => {
  assert.equal(captionOf({ caption: "Price by time", rows: [[1]], total: 1 }, fmt), "Price by time");
  assert.equal(captionOf({ caption: "Price by time", rows: new Array(500), total: 1000000 }, fmt), "Price by time: 500 of 1,000,000 points; zoom in for more");
});

test("figure name, key shortcuts, controls", () => {
  assert.equal(figureName("line", ["close", "open"]), "line: close, open");
  assert.equal(figureName("line", []), "line");
  for (const k of ["ArrowLeft", "Home", "PageDown", "Shift+ArrowLeft", "+", "-", "0", "Enter", "Escape", "Space", "?"]) assert.ok(KEY_SHORTCUTS.split(" ").includes(k), k);
  assert.deepEqual(CONTROLS.map((c) => c.key), ["-", "+", "0", "ArrowLeft", "ArrowRight"]);
});

test("throttle runs at once, then at most once per interval", async () => {
  let n = 0;
  const th = throttle(() => n++, 40);
  th.run();
  th.run();
  th.run();
  assert.equal(n, 1);
  await new Promise((r) => setTimeout(r, 60));
  assert.equal(n, 2);
  th.cancel();
});

test("announce: the interval is the attribute's seconds, at least 5 (REQ-A11Y-100)", () => {
  assert.equal(ANNOUNCE_MIN_S, 5);
  assert.equal(announceSeconds("12"), 12);
  assert.equal(announceSeconds("1"), 5);
  assert.equal(announceSeconds(""), 5);
  assert.equal(announceSeconds("soon"), 5);
  assert.equal(announceSeconds(null), 5);
});

test("announce: the latest row read as the x, then up to four values (REQ-A11Y-100)", () => {
  const t = Date.UTC(2026, 9, 18, 10, 32);
  assert.equal(latestText({ t, a: 104.2, b: 1200, c: null, d: 4, e: 5 }, desc, fmt), "Latest: Oct 18, 2026, 10:32 AM, a 104.2, b 1,200, c no value, d 4, and 1 more");
  assert.equal(latestText(null, desc, fmt), "");
  const plain = { tables: { data: { columns: [{ name: "x", type: "f64" }, { name: "y", type: "f64" }] } } };
  assert.equal(latestText({ x: 3, y: 0.5 }, plain, fmt), "Latest: 3, y 0.5");
});
