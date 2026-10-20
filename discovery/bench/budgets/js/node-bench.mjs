// Throwaway. Node runner for bench-core.mjs against pkg/6-downsample.
//   node node-bench.mjs bench [runs]     frame, hover and streaming timings
//   node node-bench.mjs memory           wasm pages with two 1M-point charts (zero-copy load)
//   node node-bench.mjs memory-copy      wasm pages after a 1M-point load through set_data (copies)
import { readFileSync } from "node:fs";
import { cpus, loadavg } from "node:os";
import * as mod from "../pkg/6-downsample/ladder.js";
import { run, format, memoryReport } from "./bench-core.mjs";

const bytes = readFileSync(new URL("../pkg/6-downsample/ladder_bg.wasm", import.meta.url));
const exports = mod.initSync({ module: bytes });
const memory = exports.memory;
const [mode = "bench", runs = "5"] = process.argv.slice(2);
console.log(`node ${process.version}, ${cpus()[0].model}, load ${loadavg().map((l) => l.toFixed(2)).join(" ")}, ${new Date().toISOString()}`);
if (mode === "bench") {
  console.log(format(run(mod, memory, { runs: Number(runs) })));
} else if (mode === "memory") {
  console.log(JSON.stringify(memoryReport(mod, memory)));
} else if (mode === "memory-copy") {
  const pages = () => memory.buffer.byteLength / 65536;
  const n = 1_000_000;
  const xs = new Float64Array(n), ys = new Float64Array(n);
  for (let i = 0; i < n; i++) { xs[i] = i; ys[i] = Math.sin(i); }
  const out = { atInit: pages() };
  const c = new mod.LineChart(320, 120, n);
  c.set_data(xs, ys);
  c.fit();
  c.frame();
  out.afterSetData1M = pages();
  console.log(JSON.stringify(out));
}
console.log(`load after ${loadavg().map((l) => l.toFixed(2)).join(" ")}`);
