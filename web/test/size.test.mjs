// The cell and grid from the font size and the box (REQ-EL-080, REQ-EL-085,
// REQ-EL-100), and `fit` with fixed cols and rows (REQ-EL-110).
import { test } from "node:test";
import assert from "node:assert/strict";
import { cellWidth, fontFloor, gridFor, fitBox, fitOf, OBSERVED, DEFAULT_ROWS } from "../src/element.ts";

test("cell width round(0.6 × font-size × dpr), the font raised to its floor", () => {
  assert.equal(cellWidth(13, 1), 8);
  assert.equal(cellWidth(16, 2), 19);
  assert.equal(cellWidth(8, 1), 7, "8 px at 1× is drawn at 11 px");
  assert.equal(cellWidth(8, 2), 11, "8 px at 2× is drawn at 9 px");
  assert.deepEqual([fontFloor(1), fontFloor(1.5), fontFloor(2), fontFloor(3)], [11, 11, 9, 9]);
});

test("whole cells that fit the box, within a pixel of rounding", () => {
  assert.deepEqual(gridFor(800, 192, 8), { cols: 100, rows: 12 });
  assert.deepEqual(gridFor(799, 191, 8), { cols: 100, rows: 12 });
  assert.deepEqual(gridFor(797, 190, 8), { cols: 99, rows: 11 });
  assert.deepEqual(gridFor(3, 3, 8), { cols: 1, rows: 1 });
  assert.equal(DEFAULT_ROWS, 12);
});

test("the observed attributes are the plan's v0.6.0 attributes and timeZone", () => {
  assert.deepEqual([...OBSERVED].sort(), ["announce", "cols", "data", "fit", "label", "link", "loading", "locale", "mono", "motion", "options", "paused", "rows", "src", "theme", "time", "timezone", "type", "view"]);
});

test("fit: width makes the cell W / cols; contain the largest 1:2 cell inside W × H, centred, as width when H is 0", () => {
  assert.deepEqual([fitOf("width"), fitOf("contain"), fitOf("cover"), fitOf(""), fitOf(null)], ["width", "contain", null, null, null]);
  // No fit: the font's cell, centred.
  assert.deepEqual(fitBox(null, 600, 200, 40, 10, 10), { cell: 10, x: 100, y: 0, width: 400, height: 200 });
  // width: 600 / 40 = 15, whatever the height.
  assert.deepEqual(fitBox("width", 600, 0, 40, 10, 10), { cell: 15, x: 0, y: 0, width: 600, height: 300 });
  assert.deepEqual(fitBox("width", 600, 400, 40, 10, 10), { cell: 15, x: 0, y: 50, width: 600, height: 300 });
  // contain: min(600 / 40, 150 / 20) = 7.5, centred across.
  assert.deepEqual(fitBox("contain", 600, 150, 40, 10, 10), { cell: 7.5, x: 150, y: 0, width: 300, height: 150 });
  // contain, a tall box: min(15, 800 / 20 = 40) = 15, centred down.
  assert.deepEqual(fitBox("contain", 600, 800, 40, 10, 10), { cell: 15, x: 0, y: 250, width: 600, height: 300 });
  // contain with H = 0 acts as width.
  assert.deepEqual(fitBox("contain", 600, 0, 40, 10, 10), fitBox("width", 600, 0, 40, 10, 10));
  // The aspect is kept: a cell twice as tall as wide.
  for (const [w, h] of [[640, 333], [1203, 97], [50, 999]]) {
    const b = fitBox("contain", w, h, 37, 9, 8);
    assert.ok(b.width <= w + 1e-9 && b.height <= h + 1e-9, `${w} × ${h}`);
    assert.ok(Math.abs(b.width - w) < 1e-9 || Math.abs(b.height - h) < 1e-9, `${w} × ${h}: touches a side`);
    assert.equal(b.height / 9, 2 * (b.width / 37));
  }
});
