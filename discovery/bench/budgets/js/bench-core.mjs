// Throwaway: the wasm side of the frame, hover, streaming and memory
// measurements for discovery/budgets.md. The same scenarios as
// ladder/examples/native.rs. Runs in Node (node-bench.mjs) and in a page
// (page.html, driven by browser-bench.mjs). `mod` is the wasm-bindgen module
// (pkg/6-downsample/ladder.js), already initialised; `memory` its memory.

const T0 = 1_790_000_000_000;
const FRAMES = 300;

function fill(xs, ys, n) {
  let s = 1, v = 0;
  for (let i = 0; i < n; i++) {
    s = (Math.imul(s, 1664525) + 1013904223) >>> 0;
    v += (s >>> 8) / 16777216 - 0.5;
    xs[i] = T0 + i * 1000;
    ys[i] = v + 20 * Math.sin(i / 5000);
  }
}

// Zero-copy load: the chart reserves, JS writes through Float64Array views.
export function chart(mod, memory, cols, rows, n, ds) {
  const c = new mod.LineChart(cols, rows, n);
  c.reserve(n);
  fill(new Float64Array(memory.buffer, c.x_ptr(), n), new Float64Array(memory.buffer, c.y_ptr(), n), n);
  c.commit(n);
  c.set_time_axis(true);
  c.set_downsample(ds);
  c.fit();
  return c;
}

const stats = (ms) => [ms.reduce((a, b) => a + b, 0) / ms.length, Math.max(...ms)];

function pan(c, n) {
  const span = (Math.max(n, 2) - 1) * 1000;
  c.frame();
  const ms = [];
  for (let k = 0; k < FRAMES; k++) {
    const off = 0.1 * span * Math.sin((2 * Math.PI * k) / FRAMES);
    const at = performance.now();
    c.set_view(T0 + off, T0 + off + span);
    c.frame();
    ms.push(performance.now() - at);
  }
  return stats(ms);
}

function hover(c) {
  c.frame();
  const cols = c.cols();
  const ms = [];
  let sink = 0;
  for (let k = 0; k < FRAMES; k++) {
    const at = performance.now();
    sink += c.hover(8 + ((cols - 9) * k) / FRAMES, 5);
    ms.push(performance.now() - at);
  }
  return [...stats(ms), sink];
}

function stream(mod, memory, batch, every, ds) {
  const n = 1_000_000;
  const c = chart(mod, memory, 320, 120, n, ds);
  const span = (n - 1) * 1000;
  let next = n, busy = 0, nextFrame = 0;
  const frames = [];
  const bx = new Float64Array(batch), by = new Float64Array(batch);
  for (let ms = 0; ms < 10_000; ms++) {
    if (ms % every === 0) {
      for (let k = 0; k < batch; k++) {
        bx[k] = T0 + (next + k) * 1000;
        by[k] = Math.sin((next + k) / 777) * 30;
      }
      const at = performance.now();
      if (batch === 1) c.push(bx[0], by[0]);
      else c.push_batch(bx, by);
      busy += performance.now() - at;
      next += batch;
    }
    if (ms >= nextFrame) {
      nextFrame += 1000 / 60;
      const end = T0 + (next - 1) * 1000;
      const at = performance.now();
      c.set_view(end - span, end);
      c.frame();
      const t = performance.now() - at;
      busy += t;
      frames.push(t);
    }
  }
  c.free();
  return [busy / 10, ...stats(frames)];
}

function summary(label, runs) {
  const avgs = runs.map((r) => r[0]).sort((a, b) => a - b);
  const worsts = runs.map((r) => r[1]).sort((a, b) => a - b);
  const mid = Math.floor(runs.length / 2);
  return { label, avgMin: avgs[0], avgMed: avgs[mid], worstMin: worsts[0], worstMed: worsts[mid] };
}

export function memoryReport(mod, memory) {
  const pages = () => memory.buffer.byteLength / 65536;
  const out = { atInit: pages() };
  const a = chart(mod, memory, 320, 120, 1_000_000, true);
  a.frame();
  out.afterOneChart1M = pages();
  const b = chart(mod, memory, 320, 120, 1_000_000, true);
  b.frame();
  out.afterTwoCharts1M = pages();
  a.free();
  b.free();
  out.afterFreeingBoth = pages();
  return out;
}

export function run(mod, memory, { runs = 5, only } = {}) {
  const rows = [];
  const want = (s) => !only || only.includes(s);
  if (want("frame")) {
    for (const [cols, rws] of [[80, 24], [320, 120]]) {
      for (const n of [1_000, 10_000, 100_000, 1_000_000]) {
        for (const ds of [false, true]) {
          const c = chart(mod, memory, cols, rws, n, ds);
          const r = [];
          for (let i = 0; i < runs; i++) r.push(pan(c, n));
          c.free();
          rows.push(summary(`frame ${cols}x${rws} n=${n} ds=${ds}`, r));
        }
      }
    }
  }
  if (want("hover")) {
    for (const [cols, rws] of [[80, 24], [320, 120]]) {
      const c = chart(mod, memory, cols, rws, 1_000_000, true);
      const r = [];
      for (let i = 0; i < runs; i++) r.push(hover(c));
      c.free();
      rows.push(summary(`hover ${cols}x${rws} n=1000000`, r));
    }
  }
  if (want("stream")) {
    for (const [batch, every] of [[1, 1], [1000, 100]]) {
      for (const ds of [true, false]) {
        const r = [];
        for (let i = 0; i < runs; i++) r.push(stream(mod, memory, batch, every, ds));
        const busy = r.map((x) => x[0]).sort((a, b) => a - b);
        rows.push({ label: `stream batch=${batch} every=${every}ms ds=${ds}`, busyMin: busy[0], busyMed: busy[Math.floor(runs / 2)] });
        rows.push(summary("  its frames", r.map((x) => [x[1], x[2]])));
      }
    }
  }
  return rows;
}

export function format(rows) {
  const f = (v) => v.toFixed(3).padStart(9);
  return rows
    .map((r) =>
      r.busyMin !== undefined
        ? `${r.label}: busy ms/s min ${r.busyMin.toFixed(1)} med ${r.busyMed.toFixed(1)}`
        : `${r.label.padEnd(34)} avg ms min ${f(r.avgMin)} med ${f(r.avgMed)} | worst ms min ${f(r.worstMin)} med ${f(r.worstMed)}`,
    )
    .join("\n");
}
