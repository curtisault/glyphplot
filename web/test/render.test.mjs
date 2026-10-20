// render.ts: only changed cells are redrawn (REQ-RENDER-030); a double-width
// glyph and its tail redraw together; a frame with back colours is drawn as
// one cols × 2·rows image, scaled without smoothing (REQ-RENDER-040).
import { test } from "node:test";
import assert from "node:assert/strict";
import { diffCells, backPixels, hasBacks, TextRenderer, AtlasRenderer } from "../src/render.ts";

const grid = (cells, colors, backs, cols) => ({ cells: Uint32Array.from(cells), colors: Uint8Array.from(colors), backs: Uint8Array.from(backs), cols });

test("a cell is redrawn when its glyph, ink or back changes", () => {
  const a = grid([65, 66, 67, 68], [0, 0, 0, 0], [255, 255, 255, 255], 4);
  const b = grid([65, 90, 67, 68], [0, 0, 3, 0], [255, 255, 255, 7], 4);
  assert.deepEqual([...diffCells(b, a, false)], [0, 1, 1, 1]);
  assert.deepEqual([...diffCells(a, a, false)], [0, 0, 0, 0]);
  assert.deepEqual([...diffCells(a, null, false)], [1, 1, 1, 1]);
  assert.deepEqual([...diffCells(a, a, true)], [1, 1, 1, 1]);
});

test("a double-width glyph and its tail redraw together, not across rows", () => {
  const a = grid([0x4e2d, 0, 65, 66], [0, 0, 0, 0], [255, 255, 255, 255], 2);
  const b = grid([0x4e2d, 0, 65, 66], [0, 0, 0, 0], [255, 7, 255, 255], 2);
  assert.deepEqual([...diffCells(b, a, false)], [2, 1, 0, 0]);
  const c = grid([0x4e2d, 0, 65, 66], [5, 0, 0, 0], [255, 255, 255, 255], 2);
  assert.deepEqual([...diffCells(c, a, false)], [1, 2, 0, 0]);
  const d = grid([66, 65, 0, 67], [0, 0, 0, 0], [255, 255, 255, 255], 2);
  const e = grid([66, 65, 0, 67], [0, 9, 0, 0], [255, 255, 255, 255], 2);
  assert.deepEqual([...diffCells(e, d, false)], [0, 1, 0, 0], "a tail at a row's start has no partner before it");
});

test("backPixels: each cell's back in two pixels of a cols × 2·rows image, none transparent", () => {
  const rgba = new Uint8Array(255 * 4);
  rgba.set([10, 20, 30, 255], 32 * 4);
  rgba.set([40, 50, 60, 255], 33 * 4);
  const px = new Uint8ClampedArray(2 * 4 * 4).fill(9);
  backPixels(Uint8Array.from([32, 255, 255, 33]), 2, rgba, px);
  const pixel = (x, y) => [...px.slice((y * 2 + x) * 4, (y * 2 + x) * 4 + 4)];
  assert.deepEqual([pixel(0, 0), pixel(0, 1)], [[10, 20, 30, 255], [10, 20, 30, 255]]);
  assert.deepEqual([pixel(1, 0), pixel(1, 1), pixel(0, 2), pixel(0, 3)], [[0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0]]);
  assert.deepEqual([pixel(1, 2), pixel(1, 3)], [[40, 50, 60, 255], [40, 50, 60, 255]]);
  assert.equal(hasBacks(Uint8Array.from([255, 255])), false);
  assert.equal(hasBacks(Uint8Array.from([255, 7])), true);
});

/** A 2D context that records the calls that paint. */
function fakeContext(width = 0, height = 0) {
  const calls = [];
  const ctx = {
    calls,
    canvas: { width, height },
    imageSmoothingEnabled: true,
    globalCompositeOperation: "source-over",
    font: "",
    fillStyle: "#000",
    textAlign: "left",
    textBaseline: "alphabetic",
    measureText: () => ({ fontBoundingBoxAscent: 10, fontBoundingBoxDescent: 3 }),
    createImageData: (w, h) => ({ width: w, height: h, data: new Uint8ClampedArray(w * h * 4) }),
  };
  for (const m of ["clearRect", "fillRect", "fillText", "drawImage", "putImageData", "save", "restore", "beginPath", "rect", "clip", "moveTo", "lineTo", "closePath", "fill", "arc"]) {
    ctx[m] = (...a) => calls.push([m, a.map((x) => (typeof x === "object" && x !== null && "width" in x ? `${x.width}x${x.height}` : x)), ctx.imageSmoothingEnabled]);
  }
  return ctx;
}

class FakeCanvas {
  constructor(width = 0, height = 0) {
    this.width = width;
    this.height = height;
    this.ctx = fakeContext(width, height);
  }
  getContext() {
    return this.ctx;
  }
}

test("a frame of back colours is drawn as one cols × 2·rows image without smoothing, glyphs over it; a few changed cells fill one by one", () => {
  const saved = globalThis.OffscreenCanvas;
  globalThis.OffscreenCanvas = FakeCanvas;
  try {
    const canvas = new FakeCanvas();
    const r = new TextRenderer(canvas);
    const palette = new Uint8Array(255 * 4).fill(255);
    r.setPalette(palette, true);
    const cols = 6;
    const rows = 3;
    r.resize(cols, rows, 8);
    const ctx = canvas.ctx;
    const n = cols * rows;
    const g = { cols, rows, cells: new Uint32Array(n).fill(32), colors: new Uint8Array(n), backs: new Uint8Array(n) };
    for (let i = 0; i < n; i++) g.backs[i] = 32 + i;
    g.cells[3] = 65; // a glyph over the backs
    ctx.calls.length = 0;
    assert.deepEqual(r.draw(g), { cells: n, full: true, backImage: true });
    const names = ctx.calls.map((c) => c[0]);
    const images = ctx.calls.filter((c) => c[0] === "drawImage");
    assert.deepEqual(images, [["drawImage", ["6x6", 0, 0, 6, 6, 0, 0, 48, 48], false]], "one image, cols × 2·rows, scaled to the canvas, unsmoothed");
    assert.equal(ctx.calls.filter((c) => c[0] === "fillRect").length, 1, "the ground once, no back a cell");
    assert.ok(names.indexOf("fillText") > names.indexOf("drawImage"), "glyphs over the backs");
    // One back changes: that cell's fill, no image.
    ctx.calls.length = 0;
    g.backs[4] = 100;
    assert.deepEqual(r.draw(g), { cells: 1, full: false, backImage: false });
    assert.equal(ctx.calls.filter((c) => c[0] === "drawImage").length, 0);
    // More than half the cells change (a pan): one image again.
    ctx.calls.length = 0;
    for (let i = 0; i < n; i++) g.backs[i] = 40 + i;
    assert.deepEqual(r.draw(g), { cells: n, full: true, backImage: true });
    assert.equal(ctx.calls.filter((c) => c[0] === "drawImage").length, 1);
    assert.equal(r.backImages, 2);
    // A frame without backs draws no image.
    g.backs.fill(255);
    r.invalidate();
    assert.deepEqual(r.draw(g), { cells: n, full: true, backImage: false });
    assert.equal(r.backImages, 2);
    assert.equal(typeof AtlasRenderer, "function");
  } finally {
    globalThis.OffscreenCanvas = saved;
  }
});
