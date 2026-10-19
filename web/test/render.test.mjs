// render.ts: only changed cells are redrawn (REQ-RENDER-030); a double-width
// glyph and its tail redraw together.
import { test } from "node:test";
import assert from "node:assert/strict";
import { diffCells } from "../src/render.ts";

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
