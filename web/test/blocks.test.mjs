// blocks.ts: every shape against the Unicode Standard's definitions
// (REQ-RENDER-020). The sample names below are from Unicode 16.0.0's
// UnicodeData.txt, read with Python 3.14's unicodedata (unidata_version
// 16.0.0) on 2026-10-19.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import {
  blockShape,
  blockCodePoints,
  dotRects,
  fillBlock,
  OCTANT_MASKS,
  SEXTANT_MASKS,
  QUADRANTS,
  WEDGE_NAMES,
  shapeArea,
  wedgeFromName,
} from "../src/blocks.ts";

const maskOf = (digits) => [...digits].reduce((m, d) => m | (1 << (Number(d) - 1)), 0);
const pop = (m) => m.toString(2).split("").filter((b) => b === "1").length;

/** The dots (1-based) whose centres a shape covers, on a cols × rows dot grid. */
function litDots(shape, cols, rows) {
  const out = [];
  for (let r = 0; r < rows; r++) {
    for (let c = 0; c < cols; c++) {
      const x = (c + 0.5) / cols;
      const y = (r + 0.5) / rows;
      if (shape.rects.some(([x0, y0, x1, y1]) => x >= x0 && x < x1 && y >= y0 && y < y1)) out.push(r * cols + c + 1);
    }
  }
  return out.join("");
}

test("octants: 230 code points, masks ascending, the names' dot numbers", () => {
  assert.equal(OCTANT_MASKS.length, 230);
  const samples = {
    0x1cd00: "3", 0x1cd10: "45", 0x1cd21: "246", 0x1cd3c: "1237", 0x1cd5e: "13467", 0x1cd79: "148",
    0x1cd8b: "12458", 0x1cd94: "2368", 0x1cd97: "1468", 0x1cd9a: "13468", 0x1cda0: "3568", 0x1cde5: "2345678",
  };
  for (const [cp, digits] of Object.entries(samples)) {
    const s = blockShape(Number(cp));
    assert.ok(s, `U+${Number(cp).toString(16)} has a shape`);
    assert.equal(litDots(s, 2, 4), digits, `BLOCK OCTANT-${digits}`);
    assert.equal(OCTANT_MASKS[Number(cp) - 0x1cd00], maskOf(digits));
  }
  for (let i = 0; i < 230; i++) {
    const s = blockShape(0x1cd00 + i);
    assert.ok(Math.abs(shapeArea(s) - pop(OCTANT_MASKS[i]) / 8) < 1e-12);
  }
  assert.equal(blockShape(0x1cde6), undefined);
});

test("sextants: 60 code points, the names' dot numbers", () => {
  assert.equal(SEXTANT_MASKS.length, 60);
  const samples = { 0x1fb00: "1", 0x1fb13: "35", 0x1fb14: "235", 0x1fb27: "146", 0x1fb28: "1246", 0x1fb3b: "23456" };
  for (const [cp, digits] of Object.entries(samples)) {
    assert.equal(litDots(blockShape(Number(cp)), 2, 3), digits, `BLOCK SEXTANT-${digits}`);
  }
  for (let i = 0; i < 60; i++) assert.ok(Math.abs(shapeArea(blockShape(0x1fb00 + i)) - pop(SEXTANT_MASKS[i]) / 6) < 1e-12);
});

test("block elements and quadrants by name", () => {
  const rect = (cp) => blockShape(cp).rects;
  assert.deepEqual(rect(0x2580), [[0, 0, 1, 0.5]]); // UPPER HALF BLOCK
  assert.deepEqual(rect(0x2581), [[0, 7 / 8, 1, 1]]); // LOWER ONE EIGHTH BLOCK
  assert.deepEqual(rect(0x2587), [[0, 1 / 8, 1, 1]]); // LOWER SEVEN EIGHTHS BLOCK
  assert.deepEqual(rect(0x2588), [[0, 0, 1, 1]]); // FULL BLOCK
  assert.deepEqual(rect(0x2589), [[0, 0, 7 / 8, 1]]); // LEFT SEVEN EIGHTHS BLOCK
  assert.deepEqual(rect(0x258f), [[0, 0, 1 / 8, 1]]); // LEFT ONE EIGHTH BLOCK
  assert.deepEqual(rect(0x2590), [[0.5, 0, 1, 1]]); // RIGHT HALF BLOCK
  assert.deepEqual(rect(0x2594), [[0, 0, 1, 1 / 8]]); // UPPER ONE EIGHTH BLOCK
  assert.deepEqual(rect(0x2595), [[7 / 8, 0, 1, 1]]); // RIGHT ONE EIGHTH BLOCK
  assert.deepEqual([0x2591, 0x2592, 0x2593].map((cp) => blockShape(cp).alpha), [0.25, 0.5, 0.75]);
  const words = { "UPPER LEFT": 1, "UPPER RIGHT": 2, "LOWER LEFT": 4, "LOWER RIGHT": 8 };
  for (const [cp, name, mask] of QUADRANTS) {
    const fromName = name.replace("QUADRANT ", "").split(" AND ").reduce((m, w) => m | words[w], 0);
    assert.equal(mask, fromName, name);
    assert.equal(litDots(blockShape(cp), 2, 2), [1, 2, 3, 4].filter((d) => mask & (1 << (d - 1))).join(""), name);
  }
});

test("wedges: the diagonal and side each name gives", () => {
  assert.equal(WEDGE_NAMES.length, 44);
  for (let i = 0; i < 44; i++) assert.ok(blockShape(0x1fb3c + i).polys.length === 1, WEDGE_NAMES[i]);
  // U+1FB3C: the triangle (0, 2/3), (1/2, 1), (0, 1).
  assert.ok(Math.abs(shapeArea(blockShape(0x1fb3c)) - 1 / 12) < 1e-12);
  // U+1FB46 LOWER RIGHT BLOCK DIAGONAL LOWER MIDDLE LEFT TO UPPER MIDDLE RIGHT: half the cell.
  assert.ok(Math.abs(shapeArea(blockShape(0x1fb46)) - 1 / 2) < 1e-12);
  // U+1FB3C–1FB51 and U+1FB52–1FB67 name the same diagonals with the opposite corner: each pair tiles the cell.
  for (let k = 0; k < 22; k++) {
    const a = WEDGE_NAMES[k].replace(/^.* BLOCK /, "");
    const b = WEDGE_NAMES[k + 22].replace(/^.* BLOCK /, "");
    assert.equal(a, b);
    const sum = shapeArea(blockShape(0x1fb3c + k)) + shapeArea(blockShape(0x1fb52 + k));
    assert.ok(Math.abs(sum - 1) < 1e-12, `${WEDGE_NAMES[k]}: ${sum}`);
  }
  assert.equal(wedgeFromName("NOT A WEDGE"), null);
});

test("triangular and eighth blocks", () => {
  for (const cp of [0x1fb6c, 0x1fb6d, 0x1fb6e, 0x1fb6f]) assert.ok(Math.abs(shapeArea(blockShape(cp)) - 1 / 4) < 1e-12);
  for (const cp of [0x1fb68, 0x1fb69, 0x1fb6a, 0x1fb6b]) assert.ok(Math.abs(shapeArea(blockShape(cp)) - 3 / 4) < 1e-12);
  for (const cp of [0x1fb9a, 0x1fb9b]) assert.ok(Math.abs(shapeArea(blockShape(cp)) - 1 / 2) < 1e-12);
  assert.deepEqual(blockShape(0x1fb70).rects, [[1 / 8, 0, 2 / 8, 1]]); // VERTICAL ONE EIGHTH BLOCK-2
  assert.deepEqual(blockShape(0x1fb7b).rects, [[0, 6 / 8, 1, 7 / 8]]); // HORIZONTAL ONE EIGHTH BLOCK-7
  assert.ok(Math.abs(shapeArea(blockShape(0x1fb81)) - 4 / 8) < 1e-12); // HORIZONTAL ONE EIGHTH BLOCK-1358
  assert.deepEqual(blockShape(0x1fb82).rects, [[0, 0, 1, 2 / 8]]); // UPPER ONE QUARTER BLOCK
  assert.deepEqual(blockShape(0x1fb8b).rects, [[1 / 8, 0, 1, 1]]); // RIGHT SEVEN EIGHTHS BLOCK
  assert.deepEqual(blockShape(0x1cea0).rects, [[0.5, 0.75, 1, 1]]); // RIGHT HALF LOWER ONE QUARTER BLOCK
  assert.deepEqual(blockShape(0x1ceaf).rects, [[0.75, 0.5, 1, 1]]); // LOWER HALF RIGHT ONE QUARTER BLOCK
});

test("dotRects merges runs and never overlaps", () => {
  assert.deepEqual(dotRects(0b11111111, 2, 4), [[0, 0, 1, 1]]);
  assert.deepEqual(dotRects(0b01010101, 2, 4), [[0, 0, 0.5, 1]]);
  for (let m = 0; m < 256; m++) {
    const rs = dotRects(m, 2, 4);
    assert.ok(Math.abs(rs.reduce((a, [x0, y0, x1, y1]) => a + (x1 - x0) * (y1 - y0), 0) - pop(m) / 8) < 1e-12);
  }
});

test("fillBlock covers whole pixels without overlap at odd cell sizes", () => {
  for (const w of [5, 7, 8, 11]) {
    const h = w * 2;
    for (const cp of blockCodePoints()) {
      const s = blockShape(cp);
      if (s.polys.length > 0 || s.alpha !== 1) continue;
      const px = new Uint8Array(w * h);
      const ctx = {
        globalAlpha: 1,
        fillRect(x, y, rw, rh) {
          for (let yy = y; yy < y + rh; yy++) for (let xx = x; xx < x + rw; xx++) px[yy * w + xx]++;
        },
        beginPath() {}, moveTo() {}, lineTo() {}, closePath() {}, fill() {},
      };
      assert.equal(fillBlock(ctx, cp, 0, 0, w, h), true);
      assert.ok(px.every((v) => v <= 1), `U+${cp.toString(16)} at ${w}×${h} overlaps itself`);
    }
    // The full block fills every pixel.
    let filled = 0;
    fillBlock({ globalAlpha: 1, fillRect: (x, y, rw, rh) => (filled += rw * rh) }, 0x2588, 0, 0, w, h);
    assert.equal(filled, w * h);
  }
  assert.equal(fillBlock({ globalAlpha: 1, fillRect() {} }, 0x41, 0, 0, 8, 16), false);
});

test("every shaped code point is in the bundled font's ranges, every coordinate in the cell", () => {
  const text = readFileSync(new URL("../fonts/unicode-range.txt", import.meta.url), "utf8");
  const ranges = text.split(",").map((r) => r.trim().replace(/^U\+/, "").split("-").map((h) => parseInt(h, 16)));
  const inFont = (cp) => ranges.some(([a, b = a]) => cp >= a && cp <= b);
  for (const cp of blockCodePoints()) {
    assert.ok(inFont(cp), `U+${cp.toString(16)} is not in the font`);
    const s = blockShape(cp);
    for (const r of s.rects) for (const v of r) assert.ok(v >= 0 && v <= 1);
    for (const p of s.polys) for (const [x, y] of p) assert.ok(x >= -1e-12 && x <= 1 + 1e-12 && y >= -1e-12 && y <= 1 + 1e-12);
  }
});
