// The browser frame and pointer bench (REQ-PERF-050, REQ-PERF-060,
// REQ-BENCH-020): the successor of discovery/bench/budgets/js/bench-core.mjs,
// run on what ships. Each chart is made with the JS API and mounted on a
// canvas by the shell's own `mount`, so a timed frame is the shell's display
// frame: the inputs applied (`chart.input` in wasm), the chart's frame
// (`chart.frame` in wasm) and the canvas draw, each timed by mount with
// performance.now() (`Mount.timing`). bench/web.mjs drives it in headless
// Chromium through `window.bench`; the page does nothing on its own.
//
// The play is REQ-PERF-030's: 300 frames of a view panning across a fixture
// of 1,000,000 points, the x domain the data's span shifted by
// 0.1 × span × sin(2πk / 300) (discovery's pan), at 80 × 24 and 320 × 120;
// then 300 pointer moves left to right across the plot at mid height
// (REQ-PERF-050). Each step is one display frame: the page sets the view or
// sends the move, and reads the timing after mount's animation frame.
//
// The streaming row (REQ-PERF-070): a chart holding a 1,000,000-point
// fixture (a full ring at the default `retain.rows`) takes one sample a
// simulated millisecond, each its own `append()`, and draws at 60 Hz, for 10
// simulated seconds: 600 frames run back to back, each the frame's appends,
// the write to wasm (`flush`), `chart.frame` and the canvas draw, each timed
// with performance.now(). Their sum over 10 is the CPU it costs a second.

import { chart, chooseTheme, makeRenderer, mount, manifest, type Chart, type FrameTiming, type Mount } from "glyphplot";
import { InputKind, type Meta } from "glyphplot/src/api.ts";

/** A fixture as `target/fixtures/index.json` lists it. */
interface FixtureColumn {
  file?: string;
  name: string;
  type: string;
  role?: string;
  time_unit?: string;
  unit?: string;
}
interface Fixture {
  name: string;
  rows: number;
  shape: string;
  tables: { name: string; length: number; columns: FixtureColumn[] }[];
  matrices: unknown[];
}

/** One case's samples, ms. */
export interface Samples {
  input: number[];
  frame: number[];
  draw: number[];
  /** Display frames that drew nothing (a move that changed no cell). */
  idle: number;
}

/** What a case reports. */
export interface CaseResult {
  type: string;
  fixture: string;
  points: number;
  cols: number;
  rows: number;
  frames: Samples;
  moves: Samples;
  /** Whether performance.now() was precise (cross-origin isolated: 5 µs, else 100 µs). */
  isolated: boolean;
}

const STEPS = 300;
const nextFrame = (): Promise<number> => new Promise((r) => requestAnimationFrame(r));

async function index(): Promise<Fixture[]> {
  const r = await fetch("/fixtures/index.json");
  if (!r.ok) throw new Error(`fixtures/index.json: HTTP ${r.status}`);
  return ((await r.json()) as { fixtures: Fixture[] }).fixtures;
}

async function f64(file: string): Promise<Float64Array> {
  const r = await fetch(`/fixtures/${file}`);
  if (!r.ok) throw new Error(`fixtures/${file}: HTTP ${r.status}`);
  return new Float64Array(await r.arrayBuffer());
}

/** The chart types the manifest lists, with the shapes each takes. */
async function charts(): Promise<{ slug: string; shapes: string[] }[]> {
  const m = await manifest();
  return (m.charts as Meta[]).map((c) => ({ slug: c.slug, shapes: [...c.shapes] }));
}

/** How a chart of `values` with no fixture of its own is fed: as tests/timing does natively. */
const AS_VALUES = " as values";

/**
 * The 1,000,000-point fixture of each shape a chart takes, by name. A chart
 * of `values` (`bar`) with none of its own takes the one-series 1M series
 * fixture's `y` as one `value` column, each row its own category, as
 * crates/glyphplot/tests/timing does for the native budgets.
 */
async function fixturesFor(shapes: string[]): Promise<string[]> {
  const all = await index();
  const own = all.filter((f) => f.rows === 1_000_000 && shapes.includes(f.shape)).map((f) => f.name);
  if (own.length > 0 || !shapes.includes("values")) return own;
  const series = all.find((f) => f.rows === 1_000_000 && f.shape === "series" && f.tables.length === 1);
  return series ? [series.name + AS_VALUES] : [];
}

/** A fixture in the binary data form, and its x column for the pan. */
async function load(name: string): Promise<{ data: unknown; x: Float64Array; rows: number }> {
  if (name.endsWith(AS_VALUES)) return loadValues(name.slice(0, -AS_VALUES.length));
  const fx = (await index()).find((f) => f.name === name);
  if (!fx) throw new Error(`no fixture ${name}`);
  if (fx.tables.length !== 1 || fx.matrices.length > 0) throw new Error(`fixture ${name}: one table expected`);
  const t = fx.tables[0]!;
  const columns: Record<string, Float64Array | Uint32Array> = {};
  let x: Float64Array | null = null;
  for (const c of t.columns) {
    if (!c.file) continue;
    const v = await f64(c.file);
    columns[c.name] = c.type === "category" ? Uint32Array.from(v) : v;
    if (c.role === "x") x = v;
  }
  if (!x) throw new Error(`fixture ${name}: no x column`);
  const descriptor = {
    tables: {
      [t.name]: {
        length: t.length,
        columns: t.columns.map((c) => {
          const d: Record<string, unknown> = { name: c.name, type: c.type };
          for (const k of ["role", "time_unit", "unit"] as const) if (c[k] !== undefined) d[k] = c[k];
          return d;
        }),
      },
    },
  };
  return { data: { descriptor, columns }, x, rows: t.length };
}

/** A series fixture's `y` as one `value` column; the pan runs over its category indices. */
async function loadValues(name: string): Promise<{ data: unknown; x: Float64Array; rows: number }> {
  const fx = (await index()).find((f) => f.name === name);
  if (!fx || fx.tables.length !== 1) throw new Error(`no one-table fixture ${name}`);
  const t = fx.tables[0]!;
  const y = t.columns.find((c) => c.role === "y");
  if (!y?.file) throw new Error(`fixture ${name}: no y column`);
  const value = await f64(y.file);
  const descriptor = { tables: { [t.name]: { length: t.length, columns: [{ name: "value", type: "f64", role: "value" }] } } };
  return { data: { descriptor, columns: { value } }, x: Float64Array.of(0, t.length - 1), rows: t.length };
}

const empty = (): Samples => ({ input: [], frame: [], draw: [], idle: 0 });

function record(s: Samples, t: Readonly<FrameTiming>): void {
  s.input.push(t.input);
  if (!t.drew) s.idle++;
  s.frame.push(t.frame);
  s.draw.push(t.draw);
}

/** Waits for mount's next display frame and returns its timing; the page's rAF runs after mount's, which was requested first. */
async function step(m: Mount): Promise<Readonly<FrameTiming>> {
  const before = m.timing.tick;
  for (let i = 0; i < 10; i++) {
    await nextFrame();
    if (m.timing.tick !== before) return m.timing;
  }
  throw new Error("mount drew no frame within 10 animation frames");
}

let stage: HTMLElement | null = null;

/** One chart at one size: the 300-frame pan, then the 300 pointer moves. */
async function run(type: string, fixture: string, cols: number, rows: number): Promise<CaseResult> {
  const { data, x, rows: points } = await load(fixture);
  stage ??= document.getElementById("stage");
  if (!stage) throw new Error("no #stage");
  const canvas = document.createElement("canvas");
  canvas.style.cssText = "display:block;width:640px;height:auto";
  stage.replaceChildren(canvas);
  let c: Chart | null = null;
  let m: Mount | null = null;
  try {
    c = chart(type, { data: data as never });
    await c.ready;
    const choice = chooseTheme({}, null, "dark", null);
    m = mount(canvas, c, { palette: c.palette(choice.theme), paintGround: choice.paintGround, waitForSize: true });
    m.resize(cols, rows, 8);
    // The first frames: wait until mount has drawn at this size.
    for (let i = 0; i < 30 && m.frames === 0; i++) await nextFrame();
    if (m.frames === 0) throw new Error("no first frame");
    const lo = x[0]!;
    const hi = x[x.length - 1]!;
    const span = hi - lo;
    // One warm-up frame, as discovery's harness, then the pan.
    c.view = { x: { domain: [lo, hi] } };
    await step(m);
    const frames = empty();
    for (let k = 0; k < STEPS; k++) {
      const off = 0.1 * span * Math.sin((2 * Math.PI * k) / STEPS);
      c.view = { x: { domain: [lo + off, hi + off] } };
      record(frames, await step(m));
    }
    // The pointer: the fitted view, then moves across the plot at mid height, in cells.
    c.view = { x: null };
    await step(m);
    const moves = empty();
    for (let k = 0; k < STEPS; k++) {
      const px = 8 + ((cols - 9) * k) / STEPS;
      m.send({ kind: InputKind.move, x: px, y: rows / 2, a: 0, b: 0, buttons: 0, mods: 0 });
      record(moves, await step(m));
    }
    return { type, fixture, points, cols, rows, frames, moves, isolated: self.crossOriginIsolated };
  } finally {
    m?.stop();
    c?.free();
    canvas.remove();
  }
}

/** What the streaming row reports: ms summed over the run, by part. */
export interface StreamResult {
  type: string;
  fixture: string;
  cols: number;
  rows: number;
  /** Simulated seconds and samples appended. */
  seconds: number;
  samples: number;
  frames: number;
  /** The appends' own time (the shell's tail), the writes to wasm, and the frames (wasm and canvas). */
  append: number;
  write: number;
  frame: number;
  /** Rows held at the end. */
  held: number;
  isolated: boolean;
}

/** REQ-PERF-070: one sample a simulated ms into a full ring, drawn at 60 Hz, for `seconds` simulated seconds. */
async function stream(type: string, fixture: string, cols: number, rows: number, seconds = 10): Promise<StreamResult> {
  const { data, x, rows: points } = await load(fixture);
  const fx = (await index()).find((f) => f.name === fixture);
  const names = fx?.tables[0]?.columns.map((c) => ({ name: c.name, role: c.role })) ?? [];
  const xName = names.find((c) => c.role === "x")?.name ?? "x";
  const others = names.filter((c) => c.name !== xName).map((c) => c.name);
  stage ??= document.getElementById("stage");
  if (!stage) throw new Error("no #stage");
  const canvas = document.createElement("canvas");
  canvas.style.cssText = "display:block;width:640px;height:auto";
  stage.replaceChildren(canvas);
  let c: Chart | null = null;
  try {
    c = chart(type, { data: data as never });
    await c.ready;
    const choice = chooseTheme({}, null, "dark", null);
    const r = makeRenderer("text", canvas);
    r.setPalette(c.palette(choice.theme), choice.paintGround);
    c.resize(cols, rows);
    r.resize(cols, rows, 8);
    r.draw(c.frame(0));
    const step = (x[x.length - 1]! - x[0]!) / Math.max(1, points - 1) || 1;
    let at = x[x.length - 1]!;
    let walk = 0;
    const FRAMES = seconds * 60;
    let sent = 0;
    let append = 0;
    let write = 0;
    let frame = 0;
    for (let f = 0; f < FRAMES; f++) {
      const due = Math.floor(((f + 1) * 1000) / 60);
      const t0 = performance.now();
      for (; sent < due; sent++) {
        at += step;
        walk += Math.sin(sent * 12.9898) * 0.5;
        const row: Record<string, number> = { [xName]: at };
        for (const k of others) row[k] = walk;
        c.append(row);
      }
      const t1 = performance.now();
      c.flush();
      const t2 = performance.now();
      r.draw(c.frame(0));
      const t3 = performance.now();
      append += t1 - t0;
      write += t2 - t1;
      frame += t3 - t2;
      if (f % 60 === 59) await nextFrame(); // let the page breathe between simulated seconds, untimed
    }
    return { type, fixture, cols, rows, seconds, samples: sent, frames: FRAMES, append, write, frame, held: c.lastCommit?.rows ?? 0, isolated: self.crossOriginIsolated };
  } finally {
    c?.free();
    canvas.remove();
  }
}

declare global {
  interface Window {
    bench: { charts: typeof charts; fixturesFor: typeof fixturesFor; run: typeof run; stream: typeof stream };
    benchReady: true;
  }
}

window.bench = { charts, fixturesFor, run, stream };
window.benchReady = true;
