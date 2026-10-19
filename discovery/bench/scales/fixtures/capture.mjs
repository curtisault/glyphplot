// Captures d3's ticks, nice domains and tick labels as JSON lines, for
// tests/d3.rs to compare the hand-written port against. Throwaway.
//   node fixtures/capture.mjs <dir holding node_modules/d3-scale@4.0.2 ...> > fixtures/d3.jsonl
import { pathToFileURL } from "node:url";
import { join } from "node:path";

const root = process.argv[2];
const load = (p) => import(pathToFileURL(join(root, "node_modules", p, "src/index.js")).href);
const { scaleLinear, scaleUtc } = await load("d3-scale");

// A fixed LCG so the cases are the same on every run.
let seed = 42;
const rand = () => ((seed = (seed * 1103515245 + 12345) % 2147483648) / 2147483648);

const lines = [];
const linear = (a, b, n) => {
  const s = scaleLinear().domain([a, b]);
  const ticks = s.ticks(n);
  const f = s.tickFormat(n, "s");
  const nice = scaleLinear().domain([a, b]).nice(n).domain();
  lines.push(JSON.stringify({ kind: "linear", a, b, n, ticks, labels: ticks.map(f), nice }));
};
const utc = (a, b, n) => {
  const s = scaleUtc().domain([a, b]);
  const ticks = s.ticks(n).map(Number);
  const f = s.tickFormat();
  lines.push(JSON.stringify({ kind: "utc", a, b, n, ticks, labels: ticks.map((t) => f(new Date(t))) }));
};

for (const [a, b] of [[0, 1], [0, 10], [-1, 1], [0, 0.001], [1e-9, 3e-9], [0, 1e9], [-37.5, 112.25], [99.5, 100.5], [1, 0], [123456, 123789], [0.1, 0.3]]) {
  for (const n of [2, 3, 5, 10, 20]) linear(a, b, n);
}
for (let i = 0; i < 400; i++) {
  const mag = Math.pow(10, Math.floor(rand() * 24) - 12);
  const a = (rand() * 2 - 1) * mag, b = a + rand() * mag * 3;
  linear(a, b, 1 + Math.floor(rand() * 20));
}
const t0 = Date.UTC(2026, 9, 18, 13, 7, 21, 250);
for (const span of [500, 9e3, 6e4, 9e5, 36e5, 864e5, 3 * 864e5, 30 * 864e5, 120 * 864e5, 400 * 864e5, 10 * 365 * 864e5, 80 * 365 * 864e5]) {
  for (const n of [3, 5, 10, 20]) utc(t0, t0 + span, n);
}
for (let i = 0; i < 300; i++) {
  const a = Math.floor(Date.UTC(1990, 0, 1) + rand() * 40 * 365 * 864e5);
  const span = Math.floor(Math.pow(10, 2 + rand() * 9));
  utc(a, a + span, 2 + Math.floor(rand() * 18));
}
console.log(lines.join("\n"));
