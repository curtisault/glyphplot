// Times label formatting and time-zone lookups in Node, per call, best and
// median of runs. Needs target/pkg-hand and target/pkg-js-intl (sizes.sh).
//   node timing.mjs
import { readFileSync } from "node:fs";

const N = 20000;
function time(name, f, reps = 5) {
  const per = [];
  for (let r = 0; r < reps; r++) {
    const t0 = performance.now();
    for (let i = 0; i < N; i++) f(i);
    per.push(((performance.now() - t0) * 1000) / N);
  }
  per.sort((a, b) => a - b);
  console.log(`${name.padEnd(58)} min ${per[0].toFixed(3)} µs  median ${per[Math.floor(reps / 2)].toFixed(3)} µs`);
}
async function load(v) {
  const m = await import(new URL(`./target/pkg-${v}/scales_bench.js`, import.meta.url));
  m.initSync({ module: readFileSync(new URL(`./target/pkg-${v}/scales_bench_bg.wasm`, import.meta.url)) });
  return m;
}
const hand = await load("hand");
const intl = await load("js-intl");
const t0 = Date.UTC(2026, 9, 18, 13, 7, 21);

// Warm up.
for (let i = 0; i < 2000; i++) { hand.run(100 + i, t0, "x"); intl.run(1, t0 + i, "America/New_York"); }

time("wasm hand: nice+ticks+SI labels+UTC ticks+labels (1 call)", (i) => hand.run(100 + (i % 50), t0 + i * 1000, "x"));
time("wasm js-intl: new DateTimeFormat + format, via js-sys", (i) => intl.run(1, t0 + i * 60000, "America/New_York"));
const cached = new Intl.DateTimeFormat("en-US", { timeZone: "America/New_York", dateStyle: "medium", timeStyle: "short" });
time("JS: cached Intl.DateTimeFormat.format", (i) => cached.format(t0 + i * 60000));
time("JS: new Intl.DateTimeFormat + format", (i) => new Intl.DateTimeFormat("en-US", { timeZone: "America/New_York", dateStyle: "medium", timeStyle: "short" }).format(t0 + i * 60000));
const off = new Intl.DateTimeFormat("en-US", { timeZone: "America/New_York", timeZoneName: "longOffset" });
const offsetAt = (t) => {
  const s = off.formatToParts(t).find((p) => p.type === "timeZoneName").value; // "GMT-04:00"
  if (s === "GMT") return 0;
  const sign = s[3] === "-" ? -1 : 1;
  return sign * (Number(s.slice(4, 6)) * 60 + Number(s.slice(7, 9)));
};
time("JS: offset at an instant (formatToParts longOffset)", (i) => offsetAt(t0 + i * 3600000));
const nf = new Intl.NumberFormat("de-DE", { maximumFractionDigits: 2 });
time("JS: cached Intl.NumberFormat.format", (i) => nf.format(1234.5 + i));

// The transitions of a zone over a span: sample daily, bisect each change to the minute.
function transitions(zone, a, b) {
  const f = new Intl.DateTimeFormat("en-US", { timeZone: zone, timeZoneName: "longOffset" });
  const at = (t) => f.formatToParts(t).find((p) => p.type === "timeZoneName").value;
  const out = [[a, at(a)]];
  let calls = 1;
  for (let t = a + 864e5; t <= b; t += 864e5) {
    calls++;
    if (at(t) !== out[out.length - 1][1]) {
      let lo = t - 864e5, hi = t;
      while (hi - lo > 6e4) { const mid = Math.floor((lo + hi) / 2 / 6e4) * 6e4; calls++; if (at(mid) === out[out.length - 1][1]) lo = mid; else hi = mid; }
      out.push([hi, at(hi)]);
    }
  }
  return { out, calls };
}
const year = 365 * 864e5;
for (const span of [1, 10]) {
  const ms = [];
  let r;
  for (let k = 0; k < 5; k++) { const s = performance.now(); r = transitions("America/New_York", Date.UTC(2020, 0, 1), Date.UTC(2020, 0, 1) + span * year); ms.push(performance.now() - s); }
  ms.sort((a, b) => a - b);
  console.log(`JS: transitions of America/New_York over ${span} year(s): ${r.out.length - 1} changes, ${r.calls} lookups, min ${ms[0].toFixed(2)} ms median ${ms[2].toFixed(2)} ms`);
}
