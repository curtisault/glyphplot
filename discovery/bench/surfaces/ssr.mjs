// Throwaway: a first frame as text from Node, as a server would render it into
// the element before the element loads. Prints the times in ms.
import { readFileSync } from "node:fs";
const t0 = performance.now();
const { initSync, render_text } = await import("./pkg/bench_surfaces.js");
const bytes = readFileSync(new URL("./pkg/bench_surfaces_bg.wasm", import.meta.url));
const t1 = performance.now();
initSync({ module: bytes });
const t2 = performance.now();
const n = +(process.argv[2] ?? 1000);
const values = new Float64Array(n).map((_, i) => Math.sin(i / 9) * 10 + i / 20);
const t3 = performance.now();
const text = render_text(values, 80, 24);
const t4 = performance.now();
for (let i = 0; i < 100; i++) render_text(values, 80, 24);
const t5 = performance.now();
console.log(JSON.stringify({ n, importAndRead: +(t1 - t0).toFixed(2), instantiate: +(t2 - t1).toFixed(2), firstFrame: +(t4 - t3).toFixed(2), warmFrame: +((t5 - t4) / 100).toFixed(3), lines: text.split("\n").length - 1 }));
