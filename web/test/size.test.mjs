// The cell and grid from the font size and the box (REQ-EL-080, REQ-EL-085, REQ-EL-100).
import { test } from "node:test";
import assert from "node:assert/strict";
import { cellWidth, fontFloor, gridFor, OBSERVED, DEFAULT_ROWS } from "../src/element.ts";

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

test("the observed attributes are the plan's v0.3.0 attributes", () => {
  assert.deepEqual([...OBSERVED].sort(), ["cols", "data", "label", "loading", "locale", "mono", "motion", "options", "rows", "theme", "time", "type", "view"]);
});
