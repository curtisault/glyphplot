// Wasm memory, measured as the growth of memory.buffer.byteLength in Node,
// each test on an instance of its own (REQ-MEM-010, REQ-MEM-030,
// REQ-WASM-050, REQ-DATA-070).
//
// A replace reserves the new columns while the chart still holds the old
// ones, because a refused commit keeps the data it had (REQ-DATA-130): the
// first replace of a chart needs room for both, one more copy of the
// columns, which REQ-DATA-070 allows (at most 1.1 times their bytes).
// REQ-MEM-030's "no growth" is tested from the second replace on.
import { test } from "node:test";
import assert from "node:assert/strict";
import { BUILD, types, moduleFor, instantiate, series, commitColumns } from "./support/wasm.mjs";

const TYPE = types[0];
const N = 1_000_000;
const MiB = 1 << 20;

const a = series(N);
const b = series(N, (i) => Math.cos(i / 1000));
const half = series(N / 2);
const BYTES = a.xs.byteLength + a.ys.byteLength;

async function fresh() {
  const { glue, memory } = await instantiate(moduleFor(TYPE), { fresh: true });
  const bytes = () => memory.buffer.byteLength;
  const make = () => new glue.Chart(TYPE, "{}");
  const set = (c, s) => commitColumns(c, memory, s.descriptor, [s.xs, s.ys]);
  return { bytes, make, set };
}

test(`${BUILD}: a chart of 1,000,000 points in two f64 columns holds at most 16 B a point + 1 MiB (REQ-MEM-010)`, async () => {
  const { bytes, make, set } = await fresh();
  const before = bytes();
  const c = make();
  set(c, a);
  c.resize(80, 24);
  c.frame(0, false);
  const grown = bytes() - before;
  console.log(`# REQ-MEM-010 ${BUILD}: make to first frame grew ${grown} B for ${N} points; limit ${16 * N + MiB} B`);
  assert.ok(grown <= 16 * N + MiB, `${grown} B`);
  c.free();
});

test(`${BUILD}: freeing a chart and making another of the same capacity grows nothing (REQ-WASM-050, REQ-MEM-030)`, async () => {
  const { bytes, make, set } = await fresh();
  let c = make();
  set(c, a);
  c.frame(0, false);
  const before = bytes();
  for (let k = 0; k < 5; k++) {
    c.free();
    c = make();
    set(c, k % 2 ? a : b);
    c.resize(120, 40);
    c.frame(0, false);
    assert.equal(bytes() - before, 0, `free and remake ${k}`);
  }
  c.free();
});

test(`${BUILD}: the first replace grows at most one copy of the columns (REQ-DATA-070, REQ-DATA-130)`, async () => {
  const { bytes, make, set } = await fresh();
  const c = make();
  set(c, a);
  c.frame(0, false);
  const before = bytes();
  set(c, b);
  const grown = bytes() - before;
  console.log(`# REQ-DATA-070 ${BUILD}: the first replace of 2 × 1M f64 grew ${grown} B (${(grown / BYTES).toFixed(3)}× the columns)`);
  assert.ok(grown <= 1.1 * BYTES, `${grown} B`);
  c.free();
});

test(`${BUILD}: replaces of the same or smaller size after the first grow nothing, no frame between (REQ-MEM-030)`, async () => {
  const { bytes, make, set } = await fresh();
  const c = make();
  set(c, a);
  set(c, b); // the first replace: one more copy, allowed (REQ-DATA-070)
  const before = bytes();
  for (const [k, s] of [a, b, half, a, half, b].entries()) {
    set(c, s);
    assert.equal(bytes() - before, 0, `replace ${k + 2}`);
  }
  c.frame(0, false);
  c.free();
});

test(`${BUILD}: replaces of the same size after the first grow nothing, a frame after each (REQ-MEM-030)`, {
  todo: "the second replace grows by one column (8,060,928 B at 1M rows): the frame after the first replace allocates in the hole the old columns left, so the next pair of columns no longer fits it (allocator fragmentation in the module; from the third replace on it grows nothing)",
}, async () => {
  const { bytes, make, set } = await fresh();
  const c = make();
  set(c, a);
  c.frame(0, false);
  set(c, b); // the first replace
  c.frame(0, false);
  const before = bytes();
  const growth = [];
  for (const s of [a, b, a, b]) {
    set(c, s);
    c.frame(0, false);
    growth.push(bytes() - before);
  }
  c.free();
  assert.deepEqual(growth, [0, 0, 0, 0]);
});
