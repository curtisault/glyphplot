// mise run bench:web (REQ-PERF-060, REQ-PERF-050's wasm half, REQ-BENCH-020):
// bench/web.html in headless Chromium over the release build (site/dist and
// web/pkg, served gzip -9 by bench/serve.mjs). For every chart in the
// manifest and every 1,000,000-point fixture of a shape it takes, at 80 × 24
// and 320 × 120: 300 display frames of REQ-PERF-030's pan, then 300 pointer
// moves across the plot. Prints the load average, a row per case and
// measure, and exits 1 when any row is over its budget.
//
// A frame is the shell's display frame: the chart's frame in wasm plus the
// canvas draw, both timed in the page with performance.now() by mount
// (`Mount.timing`); the table gives the two apart. A pointer move is the
// input (hit test and readout) plus the overlay frame in wasm (the crosshair
// over the cached base), REQ-PERF-050's "in wasm"; its canvas draw is given
// beside it and not gated. A case over budget is timed twice more and its
// best run counts, as contract rule 30 does natively (the plan, "Contract
// rules"): a slow chart is slow every time, a loaded machine only now and then.
//
//   node bench/web.mjs [type …]
import { chromium } from "@playwright/test";
import { ensureFixtures, load, machine, needBuild, stats, table } from "./common.mjs";
import { serve } from "./serve.mjs";

/** REQ-PERF-060: a frame in headless Chromium, 1M points. */
const FRAME = { avg: 8, worst: 16 };
/** REQ-PERF-050: a pointer move in wasm, 1M points. */
const MOVE = { avg: 0.5, worst: 2 };
const SIZES = [
  [80, 24],
  [320, 120],
];
const RETRIES = 2;

const only = process.argv.slice(2).filter((a) => !a.startsWith("-"));
needBuild("bench/web.html");
ensureFixtures();

const server = await serve();
const browser = await chromium.launch({ headless: true });
const rows = [];
const notes = [];
let over = false;
let isolated = null;
const started = Date.now();
try {
  console.log(`bench:web: headless Chromium ${browser.version()} (Playwright), ${machine()}, ${new Date().toISOString()}`);
  const page = await browser.newPage({ viewport: { width: 1000, height: 800 } });
  page.on("pageerror", (e) => console.error(`page error: ${e.message}`));
  page.on("console", (m) => {
    if (m.type() === "error" || m.type() === "warning") console.error(`page ${m.type()}: ${m.text()}`);
  });
  await page.goto(`${server.url}/bench/web.html`);
  await page.waitForFunction(() => window.benchReady === true);
  const charts = (await page.evaluate(() => window.bench.charts())).filter((c) => only.length === 0 || only.includes(c.slug));
  if (charts.length === 0) throw new Error(only.length ? `no chart named ${only.join(", ")}` : "the manifest lists no chart");
  for (const c of charts) {
    const fixtures = await page.evaluate((shapes) => window.bench.fixturesFor(shapes), c.shapes);
    if (fixtures.length === 0) {
      notes.push(`${c.slug}: no 1,000,000-point fixture of shape ${c.shapes.join(" or ")}; not timed`);
      over = true;
      continue;
    }
    for (const fx of fixtures) {
      for (const [cols, rws] of SIZES) {
        let best = null;
        let runs = 0;
        for (let attempt = 0; attempt <= RETRIES; attempt++) {
          const r = await page.evaluate(([t, f, w, h]) => window.bench.run(t, f, w, h), [c.slug, fx, cols, rws]);
          runs++;
          isolated = r.isolated;
          const m = measure(r);
          if (!best || score(m) < score(best)) best = m;
          if (!best.overFrame && !best.overMove) break;
        }
        const tag = runs > 1 ? ` (best of ${runs})` : "";
        const f = best.frame;
        const p = best.move;
        rows.push([c.slug, fx, `${cols} × ${rws}`, `frame${tag}`, ms(f.avg), ms(f.worst), `${FRAME.avg} / ${FRAME.worst}`, `${ms(best.wasm.avg)} / ${ms(best.wasm.worst)}`, `${ms(best.draw.avg)} / ${ms(best.draw.worst)}`, best.overFrame ? "OVER" : ""]);
        rows.push([c.slug, fx, `${cols} × ${rws}`, `pointer${tag}`, ms(p.avg), ms(p.worst), `${MOVE.avg} / ${MOVE.worst}`, `${ms(best.moveWasm.avg)} / ${ms(best.moveWasm.worst)}`, `${ms(best.moveDraw.avg)} / ${ms(best.moveDraw.worst)}`, best.overMove ? "OVER" : ""]);
        if (best.idle > 0) notes.push(`${c.slug} ${fx} ${cols} × ${rws}: ${best.idle} of 300 moves changed no cell and drew no frame (input timed alone)`);
        over ||= best.overFrame || best.overMove;
      }
    }
  }
} finally {
  await browser.close();
  await server.close();
}

console.log(
  table(["chart", "fixture", "cells", "measure", "avg ms", "worst ms", "budget avg / worst", "wasm avg / worst", "draw avg / worst", ""], rows),
);
console.log(
  `frame = chart.frame in wasm + the canvas draw (text renderer), 300 display frames panning the fixture; pointer = chart.input + the overlay frame in wasm, 300 moves, its draw not gated. performance.now() in the page, cross-origin isolated: ${isolated}.`,
);
for (const n of notes) console.log(`note: ${n}`);
console.log(`bench:web: ${((Date.now() - started) / 1000).toFixed(1)} s, load averages after ${load()}`);
if (over) {
  console.log("bench:web: OVER budget");
  process.exit(1);
}

function measure(r) {
  const sum = (a, b) => a.map((v, i) => v + b[i]);
  const frame = stats(sum(r.frames.frame, r.frames.draw));
  const move = stats(sum(r.moves.input, r.moves.frame));
  const m = {
    frame,
    wasm: stats(r.frames.frame),
    draw: stats(r.frames.draw),
    move,
    moveWasm: stats(r.moves.frame),
    moveDraw: stats(r.moves.draw),
    idle: r.moves.idle,
  };
  m.overFrame = frame.avg > FRAME.avg || frame.worst > FRAME.worst;
  m.overMove = move.avg > MOVE.avg || move.worst > MOVE.worst;
  return m;
}

function score(m) {
  return Math.max(m.frame.avg / FRAME.avg, m.frame.worst / FRAME.worst, m.move.avg / MOVE.avg, m.move.worst / MOVE.worst);
}

function ms(v) {
  return v < 0.1 ? v.toFixed(3) : v < 10 ? v.toFixed(2) : v.toFixed(1);
}
