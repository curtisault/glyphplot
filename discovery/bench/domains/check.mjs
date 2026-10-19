// Throwaway: calls the size probe's exports once to show they compute.
// Usage: node check.mjs <domain_sizes.wasm built with astro,indicators>
import { readFileSync } from "node:fs";
const { instance } = await WebAssembly.instantiate(readFileSync(process.argv[2]), {});
const e = instance.exports, mem = () => new Float64Array(e.memory.buffer);
const out = 4 << 20; // scratch above the 1 MiB stack and the data segment
if (e.memory.buffer.byteLength < out + 64) e.memory.grow(Math.ceil((out + 64 - e.memory.buffer.byteLength) / 65536));
e.planet_radec_altaz(3, 2461041.5, 51.48, 0.0, out);
console.log("Mars 2026-01-01 RA, Dec, alt, az (Greenwich):", [...mem().slice(out / 8, out / 8 + 4)].map((v) => v.toFixed(3)).join(", "));
const n = 1_000_000, x = out + 64, y = x + n * 8;
if (e.memory.buffer.byteLength < y + n * 8 * 3) e.memory.grow(Math.ceil((y + n * 24 - e.memory.buffer.byteLength) / 65536));
const xs = mem().subarray(x / 8, x / 8 + n);
let p = 100; for (let i = 0; i < n; i++) { p *= 1 + Math.sin(i * 0.37) * 0.001; xs[i] = p; }
for (const [name, f] of [["sma 20", () => e.sma(x, n, 20, y)], ["ema 20", () => e.ema(x, n, 20, y)],
  ["bollinger 20,2", () => e.bollinger(x, n, 20, 2, y, y + n * 8, y + n * 16)], ["rsi 14", () => e.rsi(x, n, 14, y)],
  ["macd 12,26,9", () => e.macd(x, n, 12, 26, 9, y, y + n * 8, y + n * 16)]]) {
  f(); const t0 = performance.now(); for (let k = 0; k < 10; k++) f(); console.log(`${name} over 1M: ${((performance.now() - t0) / 10).toFixed(2)} ms`);
}
