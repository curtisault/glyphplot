// The wasm side of the bench, in Node (V8 standing in for Chrome). Needs the
// fixtures from the native binary and the modules from sizes.sh; see README.md.
//   mise exec -- node bench.mjs
// Every case runs REPS times, each on a fresh wasm instance, so the memory
// growth it reports is that case's alone.
import { readFileSync } from "node:fs";
import { performance } from "node:perf_hooks";

const REPS = 5;
const dir = new URL("./target/", import.meta.url);
const bytes = (v) => readFileSync(new URL(`pkg/${v}/bench_bg.wasm`, dir));
const modules = { all: new WebAssembly.Module(bytes("all")), arrowrs: new WebAssembly.Module(bytes("arrowrs")) };
let fresh = 0;
/** A fresh instance of a variant's module: the glue imported anew (a query string defeats the module cache). */
async function load(v) {
  const glue = await import(new URL(`pkg/${v}/bench.js?i=${fresh++}`, dir));
  const exports = glue.initSync({ module: modules[v] });
  return { glue, memory: exports.memory };
}
const stats = (xs) => {
  const s = [...xs].sort((a, b) => a - b);
  return [s[0], s[Math.floor(s.length / 2)]];
};
const fmt = (x) => (x < 0.1 ? x.toFixed(4) : x < 10 ? x.toFixed(3) : x.toFixed(1));
const MB = (b) => (b / 1048576).toFixed(1);

/** Runs `setup` (untimed) then `f` (timed) REPS times on fresh instances. */
async function bench(v, setup, f) {
  const ms = [], grow = [];
  let check;
  for (let r = 0; r < REPS; r++) {
    const { glue, memory } = await load(v);
    const ctx = setup(glue, memory);
    const before = memory.buffer.byteLength;
    const s = performance.now();
    check = f(ctx, glue, memory);
    ms.push(performance.now() - s);
    grow.push(memory.buffer.byteLength - before);
  }
  return { ms: stats(ms), grow: stats(grow)[1], check };
}

const fixture = (name) => readFileSync(new URL(`fixtures/${name}`, dir));
function walk(n) {
  // The same walk as core::walk (mulberry32 seeded 7).
  let a = 7;
  const rnd = () => {
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), a | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
  const t = new Float64Array(n), v = new Float64Array(n);
  let y = 100;
  for (let i = 0; i < n; i++) {
    y += rnd() - 0.5;
    t[i] = 1792281600000 + i * 1000;
    v[i] = y;
  }
  return { t, v };
}

console.log("| path | rows | min ms | median ms | wasm memory growth MB (median) | check |\n|---|---|---|---|---|---|");
for (const n of [1000, 100000, 1000000]) {
  const { t, v } = walk(n);
  const sum = v.reduce((a, b) => a + b, 0);
  const csv = fixture(`walk-${n}.csv`).toString();
  const csvIso = fixture(`walk-${n}-iso.csv`).toString();
  const cols = fixture(`walk-${n}-cols.json`).toString();
  const rows = fixture(`walk-${n}-rows.json`).toString();
  const ipc = new Uint8Array(fixture(`walk-${n}.arrows`));
  const row = (name, r) => console.log(`| ${name} | ${n} | ${fmt(r.ms[0])} | ${fmt(r.ms[1])} | ${MB(r.grow)} | ${Math.abs(r.check - sum) < 1e-6 * Math.abs(sum) ? "ok" : "BAD " + r.check} |`);
  const table = (g) => new g.Table();

  row("(a) Float64Array as &[f64], copied into a Vec", await bench("all", table, (tb) => (tb.set_slice(0, t), tb.set_slice(1, v), tb.sum(1))));
  row("(a') Float64Array as Vec<f64>, one copy", await bench("all", table, (tb) => (tb.set_vec(0, t), tb.set_vec(1, v), tb.sum(1))));
  row("(b) JS writes into a wasm-owned Vec (reserve + set)", await bench("all", table, (tb, g, mem) => {
    // The address first: reserve may grow memory, which detaches an ArrayBuffer read before it.
    const pt = tb.reserve(0, n);
    new Float64Array(mem.buffer, pt, n).set(t);
    const pv = tb.reserve(1, n);
    new Float64Array(mem.buffer, pv, n).set(v);
    return tb.sum(1);
  }));
  row("(b') JS fills a wasm-owned Vec value by value", await bench("all", table, (tb, g, mem) => {
    const pt = tb.reserve(0, n), pv = tb.reserve(1, n); // pt stays valid: Vec addresses do not move when memory grows.
    const T = new Float64Array(mem.buffer, pt, n), V = new Float64Array(mem.buffer, pv, n);
    for (let i = 0; i < n; i++) { T[i] = t[i]; V[i] = v[i]; }
    return tb.sum(1);
  }));
  row("(c) JSON columns, serde_json in wasm", await bench("all", table, (tb) => (tb.set_json_columns(cols), tb.sum(1))));
  row("(c') JSON rows of objects, serde_json in wasm", await bench("all", table, (tb) => (tb.set_json_rows(rows), tb.sum(1))));
  row("(c'') JSON columns, JSON.parse in JS then (a')", await bench("all", table, (tb) => {
    const o = JSON.parse(cols);
    tb.set_vec(0, Float64Array.from(o.t));
    tb.set_vec(1, Float64Array.from(o.v));
    return tb.sum(1);
  }));
  row("(c''') JSON rows, JSON.parse in JS, columns built in JS, then (a')", await bench("all", table, (tb) => {
    const o = JSON.parse(rows);
    const T = new Float64Array(o.length), V = new Float64Array(o.length);
    for (let i = 0; i < o.length; i++) { T[i] = o[i].t; V[i] = o[i].v ?? NaN; }
    tb.set_vec(0, T);
    tb.set_vec(1, V);
    return tb.sum(1);
  }));
  row("(d) CSV, epoch ms, parsed in wasm", await bench("all", table, (tb) => (tb.set_csv(csv), tb.sum(1))));
  row("(d') CSV, RFC 3339 times, parsed in wasm", await bench("all", table, (tb) => (tb.set_csv(csvIso), tb.sum(1))));
  row("(e) Arrow IPC as &[u8], hand-written reader", await bench("all", table, (tb) => (tb.set_arrow(ipc), tb.sum(1))));
  row("(e') Arrow IPC written into a wasm-owned buffer, hand-written reader", await bench("all", table, (tb, g, mem) => {
    const p = tb.reserve_bytes(ipc.length);
    new Uint8Array(mem.buffer, p, ipc.length).set(ipc);
    tb.commit_arrow();
    return tb.sum(1);
  }));
  row("(e'') Arrow IPC as &[u8], arrow-rs", await bench("arrowrs", table, (tb) => (tb.set_arrow_rs(ipc), tb.sum(1))));
}

// Downsampling in wasm: 1M points in a table, the whole series in view.
{
  const n = 1000000;
  const { t, v } = walk(n);
  const { glue } = await load("all");
  const tb = new glue.Table();
  tb.set_vec(0, t);
  tb.set_vec(1, v);
  const x0 = t[0], x1 = t[n - 1] + 1;
  console.log("\n| downsampler (wasm) | width | points out | min ms | median ms |\n|---|---|---|---|---|");
  for (const w of [160, 640]) {
    for (const [name, how] of [["min-max", 0], ["M4", 1], ["LTTB", 2]]) {
      const ms = [];
      let out = 0;
      for (let r = 0; r < 7; r++) {
        const s = performance.now();
        out = tb.downsample(how, x0, x1, w);
        ms.push(performance.now() - s);
      }
      const [a, b] = stats(ms);
      console.log(`| ${name} | ${w} | ${out} | ${fmt(a)} | ${fmt(b)} |`);
    }
  }
  // A wrapped ring of 1M after 1.5M samples, M4.
  const big = walk(1500000);
  const ring = new glue.Ring(n);
  ring.push_batch(big.t, big.v);
  for (const w of [160, 640]) {
    const ms = [];
    let out = 0;
    for (let r = 0; r < 7; r++) {
      const s = performance.now();
      out = ring.downsample(1, big.t[500000], big.t[1499999] + 1, w);
      ms.push(performance.now() - s);
    }
    const [a, b] = stats(ms);
    console.log(`| M4 over a wrapped 1M ring | ${w} | ${out} | ${fmt(a)} | ${fmt(b)} |`);
  }
  const ms = [];
  let out = 0;
  for (let r = 0; r < 7; r++) {
    const s = performance.now();
    out = tb.downsample(1, t[500000], t[510000], 640);
    ms.push(performance.now() - s);
  }
  const [a, b] = stats(ms);
  console.log(`| M4, view of 10k of 1M | 640 | ${out} | ${fmt(a)} | ${fmt(b)} |`);
}

// Streaming into a ring of 1M: 10 s of 1 kHz singles, 10 s of 10 Hz batches of 1000.
{
  const { t, v } = walk(100000);
  console.log("\n| ring append in wasm (cap 1M) | calls | min ms total | median ms total | median us a call | growth MB |\n|---|---|---|---|---|---|");
  const ringCase = async (name, calls, f) => {
    const r = await bench("all", (g) => new g.Ring(1000000), f);
    console.log(`| ${name} | ${calls} | ${fmt(r.ms[0])} | ${fmt(r.ms[1])} | ${((r.ms[1] * 1000) / calls).toFixed(3)} | ${MB(r.grow)} |`);
  };
  await ringCase("push(t, v), one sample a call, 10 000 calls", 10000, (rg) => { for (let i = 0; i < 10000; i++) rg.push(t[i], v[i]); });
  await ringCase("push_batch(&[f64], &[f64]), 1000 a call, 100 calls", 100, (rg) => {
    for (let k = 0; k < 100; k++) rg.push_batch(t.subarray(k * 1000, k * 1000 + 1000), v.subarray(k * 1000, k * 1000 + 1000));
  });
  await ringCase("staging buffer + commit, 1000 a call, 100 calls", 100, (rg, g, mem) => {
    for (let k = 0; k < 100; k++) {
      const p = rg.staging(1000);
      const s = new Float64Array(mem.buffer, p, 2000);
      s.set(t.subarray(k * 1000, k * 1000 + 1000), 0);
      s.set(v.subarray(k * 1000, k * 1000 + 1000), 1000);
      rg.commit(1000);
    }
  });
  // Allocation of the ring itself.
  const r = await bench("all", () => null, (_, g) => new g.Ring(1000000).len());
  console.log(`| new Ring(1 000 000) (two Vec<f64>) | 1 | ${fmt(r.ms[0])} | ${fmt(r.ms[1])} | — | ${MB(r.grow)} |`);
}
