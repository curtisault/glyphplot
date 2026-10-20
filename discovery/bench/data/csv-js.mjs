// CSV parsed in JS (split, Number, Date.parse) into Float64Arrays, for comparison
// with parsing it in wasm. Run after the native binary has written the fixtures:
//   mise exec -- node csv-js.mjs
import { readFileSync } from "node:fs";
import { performance } from "node:perf_hooks";
const dir = new URL("./target/fixtures/", import.meta.url);
function parse(s) {
  const lines = s.split("\n");
  const n = lines.length - 2; // header and the trailing empty line
  const t = new Float64Array(n), v = new Float64Array(n);
  for (let i = 0; i < n; i++) {
    const line = lines[i + 1];
    const c = line.indexOf(",");
    const a = line.slice(0, c), b = line.slice(c + 1);
    const x = Number(a);
    t[i] = Number.isNaN(x) ? Date.parse(a) : x;
    v[i] = b === "" ? NaN : Number(b);
  }
  return [t, v];
}
console.log("| CSV parsed in JS | rows | min ms | median ms |\n|---|---|---|---|");
for (const n of [1000, 100000, 1000000]) {
  for (const kind of ["", "-iso"]) {
    const s = readFileSync(new URL(`walk-${n}${kind}.csv`, dir)).toString();
    const ms = [];
    for (let r = 0; r < 5; r++) {
      const s0 = performance.now();
      parse(s);
      ms.push(performance.now() - s0);
    }
    ms.sort((a, b) => a - b);
    console.log(`| ${kind ? "RFC 3339 (Date.parse)" : "epoch ms"} | ${n} | ${ms[0].toFixed(3)} | ${ms[2].toFixed(3)} |`);
  }
}
