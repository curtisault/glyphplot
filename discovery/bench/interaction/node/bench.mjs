// Node harness: the same steps as src/main.rs, in wasm, called from JS once per event
// (the crossing included) and looped inside wasm (no crossing). See ../README.md.
import { readFileSync } from "node:fs";
import { initSync, Bench } from "../pkg/interaction_bench.js";

const exports = initSync({ module: readFileSync(new URL("../pkg/interaction_bench_bg.wasm", import.meta.url)) });
const RUNS = 5;
const N = Number(process.argv[2] ?? 1_000_000);

function path(k, n, cols, rows) {
  const f = k / n;
  return [9 + 0.01 + f * (cols - 9 - 0.02), 1 + (rows - 4) * (0.5 + 0.4 * Math.sin(f * 37))];
}

function stats(v) {
  v.sort((a, b) => a - b);
  return [v[0], v[Math.floor(v.length / 2)]];
}

function row(size, what, [min, med]) {
  console.log(`| ${size} | ${what} | ${min.toFixed(3)} | ${med.toFixed(3)} | ${((med / 16667) * 100).toFixed(3)} % |`);
}

// Per event from JS: the call, and for the drawing paths a fresh Uint32Array/Uint8Array view over the frame,
// as unicel's shell makes after every frame (REQ-WASM-020), reading one cell so the view is used.
function fromJs(b, call, n, cols, rows, view) {
  const v = [];
  let sink = 0;
  for (let r = 0; r < RUNS; r++) {
    const t = performance.now();
    for (let k = 0; k < n; k++) {
      const [x, y] = path(k, n, cols, rows);
      sink += call(x, y);
      if (view) {
        const cells = new Uint32Array(exports.memory.buffer, b.cells_ptr(), cols * rows);
        const colors = new Uint8Array(exports.memory.buffer, b.colors_ptr(), cols * rows);
        sink += cells[k % (cols * rows)] + colors[0];
      }
    }
    v.push(((performance.now() - t) * 1000) / n);
  }
  globalThis.sink = sink;
  return stats(v);
}

function inside(b, kind, n) {
  const v = [];
  for (let r = 0; r < RUNS; r++) {
    const t = performance.now();
    globalThis.sink = b.run_inside(kind, n);
    v.push(((performance.now() - t) * 1000) / n);
  }
  return stats(v);
}

console.log(`node ${process.version}, points: ${N}, runs: ${RUNS}; per-event µs, min and median; share of 16.667 ms (median)\n`);
console.log("| size | step | min µs | median µs | of 16.7 ms |");
console.log("|---|---|---|---|---|");
for (const [cols, rows] of [[80, 24], [320, 120]]) {
  const size = `${cols}×${rows}`;
  const t0 = performance.now();
  const b = new Bench(N, cols, rows);
  const setup = performance.now() - t0;
  row(size, "js→wasm noop(x, y) → u32 (the crossing alone)", fromJs(b, (x, y) => b.noop(x, y), 200_000, cols, rows, false));
  row(size, "move_only, from JS", fromJs(b, (x, y) => b.move_only(x, y), 200_000, cols, rows, false));
  row(size, "hit_only (binary search, 1M), from JS", fromJs(b, (x, y) => b.hit_only(x, y), 200_000, cols, rows, false));
  row(size, "hit_only, inside wasm", inside(b, 1, 200_000));
  row(size, "pointer (hit + readout + crosshair over cached base) + views, from JS", fromJs(b, (x, y) => b.pointer(x, y), 20_000, cols, rows, true));
  row(size, "pointer, inside wasm", inside(b, 2, 20_000));
  row(
    size,
    "pointer_json (JSON.stringify + serde_json), from JS",
    fromJs(b, (x, y) => b.pointer_json(JSON.stringify({ kind: "move", x, y })), 20_000, cols, rows, true),
  );
  row(size, "pointer_full (full frame from 1M points), from JS", fromJs(b, (x, y) => b.pointer_full(x, y), 100, cols, rows, true));
  b.zoom_to(0.45, 0.55);
  row(size, "pointer_full, zoomed to 10 % (100k visible), from JS", fromJs(b, (x, y) => b.pointer_full(x, y), 500, cols, rows, true));
  row(size, "pan 0.5 cell, 10 % visible, inside wasm", inside(b, 5, 500));
  b.zoom_to(0, 1);
  row(size, "pan 0.5 cell, all visible, inside wasm", inside(b, 5, 100));
  const vb = [];
  for (let r = 0; r < RUNS; r++) {
    const t = performance.now();
    b.scatter_build();
    vb.push((performance.now() - t) * 1000);
  }
  row(size, "scatter_build (per view change)", stats(vb));
  row(size, "scatter_query (grid), from JS", fromJs(b, (x, y) => b.scatter_query(x, y), 50_000, cols, rows, false));
  row(size, "scatter_linear (scan 1M), from JS", fromJs(b, (x, y) => b.scatter_linear(x, y), 20, cols, rows, false));
  console.log(`\n${size}: setup ${setup.toFixed(1)} ms; wasm memory ${(exports.memory.buffer.byteLength / 1048576).toFixed(1)} MiB\n`);
  b.free();
}
