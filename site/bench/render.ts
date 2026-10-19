// The renderer bench (REQ-RENDER-010, REQ-RENDER-050): the shell's own two
// canvas renderers, web/src/render.ts, on synthetic grids. It succeeds
// discovery/bench/rendering/canvas/, whose page drew with its own renderers;
// this one measures what ships. The cases are discovery's: a braille scatter
// coloured by value at 80 × 24 and 320 × 120 and a full-redraw density frame
// at 320 × 120, plus the scatter in one colour (the REQ-RENDER-010 ratio) and
// a hover over a still frame (few cells change).

import { fontFace } from "glyphplot";
import { makeRenderer, type RendererName } from "glyphplot/src/render.ts";

interface Case {
  name: string;
  kind: "scatter" | "density" | "hover";
  colour: "value" | "one";
  cols: number;
  rows: number;
  /** Cell width in device pixels (discovery: 10 at 80 × 24, 8 at 320 × 120). */
  cw: number;
}

const CASES: Case[] = [
  { name: "braille scatter, coloured by value", kind: "scatter", colour: "value", cols: 80, rows: 24, cw: 10 },
  { name: "braille scatter, coloured by value", kind: "scatter", colour: "value", cols: 320, rows: 120, cw: 8 },
  { name: "braille scatter, one colour", kind: "scatter", colour: "one", cols: 320, rows: 120, cw: 8 },
  { name: "braille density, full redraw", kind: "density", colour: "value", cols: 320, rows: 120, cw: 8 },
  { name: "hover over a still line frame", kind: "hover", colour: "value", cols: 320, rows: 120, cw: 8 },
];
const RENDERERS: RendererName[] = ["text", "atlas"];
const WARM = 10;
const FRAMES = 60;

/** A palette of 255 RGBA entries: tokens, eight series, and 128 scale steps (indices 32–159) of a viridis-like ramp. */
function palette(): Uint8Array {
  const p = new Uint8Array(255 * 4);
  const set = (i: number, r: number, g: number, b: number): void => p.set([r, g, b, 255], i * 4);
  set(0, 0xe8, 0xe8, 0xe8);
  set(1, 0x9a, 0x9a, 0x9a);
  set(2, 0x3a, 0x3a, 0x3a);
  set(3, 0xff, 0xb0, 0x00);
  set(4, 0x11, 0x11, 0x11);
  const series = [0x4e9af1, 0xf5a623, 0x7ed321, 0xd0021b, 0x9013fe, 0x50e3c2, 0xb8e986, 0xbd10e0];
  series.forEach((c, i) => set(8 + i, c >> 16, (c >> 8) & 255, c & 255));
  const stops = [
    [68, 1, 84],
    [59, 82, 139],
    [33, 145, 140],
    [94, 201, 98],
    [253, 231, 37],
  ];
  for (let k = 0; k < 128; k++) {
    const v = (k / 127) * (stops.length - 1);
    const i = Math.min(stops.length - 2, Math.floor(v));
    const f = v - i;
    const a = stops[i] ?? [0, 0, 0];
    const b = stops[i + 1] ?? [0, 0, 0];
    set(32 + k, ...([0, 1, 2].map((j) => Math.round((a[j] ?? 0) + ((b[j] ?? 0) - (a[j] ?? 0)) * f)) as [number, number, number]));
  }
  return p;
}

const LEVEL = [0, 0x01, 0x09, 0x0b, 0x1b, 0x1f, 0x3f, 0x7f, 0xff];

/** Frame `f` of a case: code points, inks and backs (255: none). */
function grid(c: Case, f: number, cells: Uint32Array, colors: Uint8Array, backs: Uint8Array): void {
  const t = f * 0.05;
  backs.fill(255);
  for (let y = 0; y < c.rows; y++) {
    for (let x = 0; x < c.cols; x++) {
      const i = y * c.cols + x;
      let v = 0.5 + 0.25 * Math.sin(x * 0.11 + t) + 0.25 * Math.cos(y * 0.23 - t * 0.7 + x * 0.03);
      v = Math.min(1, Math.max(0, v));
      const ink = c.colour === "one" ? 8 : 32 + Math.round(v * 127);
      if (c.kind === "density") {
        cells[i] = 0x2800 + (LEVEL[Math.round(v * 8)] ?? 0);
        colors[i] = ink;
      } else if (c.kind === "scatter") {
        // Arbitrary dot masks, a third of cells empty, new every frame.
        let h = Math.imul(x * 374761393 + y * 668265263 + f * 2246822519, 2654435761) >>> 0;
        h ^= h >>> 15;
        cells[i] = h % 3 === 0 ? 0x20 : 0x2800 + (((h >>> 8) & 255) || 1);
        colors[i] = ink;
      } else {
        // A still braille line, and a crosshair column and readout row that move a cell a frame.
        const yLine = Math.round((c.rows - 1) * (0.5 + 0.4 * Math.sin(x / 9)));
        const col = (f * 3) % c.cols;
        cells[i] = y === yLine ? 0x28ff : 0x20;
        colors[i] = 8;
        if (x === col) {
          cells[i] = y === yLine ? 0x253c : 0x2502;
          colors[i] = 3;
        }
        if (y === 0 && x >= col && x < col + 18) {
          cells[i] = 0x30 + ((x + f) % 10);
          colors[i] = 0;
        }
      }
    }
  }
}

interface Row {
  run: number;
  case: string;
  size: string;
  renderer: RendererName;
  median: number;
  p95: number;
  cells: number;
}

async function runCase(c: Case, renderer: RendererName, run: number, pal: Uint8Array, stage: HTMLElement): Promise<Row> {
  const canvas = document.createElement("canvas");
  const dpr = devicePixelRatio || 1;
  canvas.style.width = `${(c.cols * c.cw) / dpr}px`;
  canvas.style.height = `${(c.rows * c.cw * 2) / dpr}px`;
  stage.replaceChildren(canvas);
  const r = makeRenderer(renderer, canvas);
  r.resize(c.cols, c.rows, c.cw);
  r.setPalette(pal, true);
  const ctx = canvas.getContext("2d") as CanvasRenderingContext2D;
  const n = c.cols * c.rows;
  const cells = new Uint32Array(n);
  const colors = new Uint8Array(n);
  const backs = new Uint8Array(n);
  const times: number[] = [];
  let redrawn = 0;
  for (let f = 0; f < WARM + FRAMES; f++) {
    grid(c, f, cells, colors, backs);
    const t0 = performance.now();
    const s = r.draw({ cols: c.cols, rows: c.rows, cells, colors, backs });
    ctx.getImageData(0, 0, 1, 1); // wait for the raster
    const dt = performance.now() - t0;
    if (f >= WARM) {
      times.push(dt);
      redrawn += s.cells;
    }
    if (f % 10 === 9) await new Promise((res) => requestAnimationFrame(res));
  }
  times.sort((a, b) => a - b);
  const q = (p: number): number => times[Math.min(times.length - 1, Math.floor(p * times.length))] ?? NaN;
  return { run, case: c.name, size: `${c.cols} × ${c.rows}`, renderer, median: q(0.5), p95: q(0.95), cells: Math.round(redrawn / FRAMES) };
}

const med = (a: number[]): number => {
  const s = [...a].sort((x, y) => x - y);
  return s[Math.floor(s.length / 2)] ?? NaN;
};

/** The table discovery's summarise.mjs printed: min and median of the runs' medians, the median p95. */
function table(rows: Row[], ua: string): string {
  const by = new Map<string, Row[]>();
  for (const r of rows) {
    const k = `${r.size} | ${r.case} | ${r.renderer}`;
    by.set(k, [...(by.get(k) ?? []), r]);
  }
  const out = [
    `browser: ${ua}`,
    `pixel ratio ${devicePixelRatio}; ${FRAMES} frames after ${WARM} warm-up a run; ms a frame (draw + 1 px getImageData)`,
    "",
    "| size | case | renderer | runs | frame ms: min of medians | median of medians | median p95 | cells redrawn a frame |",
    "|---|---|---|---|---|---|---|---|",
  ];
  for (const [k, rs] of by) {
    const m = rs.map((r) => r.median);
    out.push(`| ${k} | ${rs.length} | ${Math.min(...m).toFixed(1)} | ${med(m).toFixed(1)} | ${med(rs.map((r) => r.p95)).toFixed(1)} | ${rs[0]?.cells ?? 0} |`);
  }
  out.push("");
  for (const renderer of RENDERERS) {
    const of = (name: string): number => med(rows.filter((r) => r.renderer === renderer && r.size === "320 × 120" && r.case === name).map((r) => r.median));
    const coloured = of("braille scatter, coloured by value");
    const one = of("braille scatter, one colour");
    const ratio = coloured / one;
    out.push(`REQ-RENDER-010, ${renderer}: 320 × 120 scatter, every cell its colour ${coloured.toFixed(1)} ms / one colour ${one.toFixed(1)} ms = ${ratio.toFixed(2)} (at most 2: ${ratio <= 2 ? "pass" : "FAIL"})`);
  }
  return out.join("\n");
}

declare global {
  interface Window {
    benchDone?: { table: string; rows: Row[] };
  }
}

async function run(): Promise<void> {
  const button = document.getElementById("run") as HTMLButtonElement;
  const status = document.getElementById("status") as HTMLElement;
  const out = document.getElementById("out") as HTMLElement;
  const stage = document.getElementById("stage") as HTMLElement;
  button.disabled = true;
  const runs = Math.max(1, Number(new URLSearchParams(location.search).get("runs") ?? 3) || 3);
  const pal = palette();
  const rows: Row[] = [];
  const total = runs * CASES.length * RENDERERS.length;
  for (let run = 1; run <= runs; run++) {
    for (const c of CASES) {
      for (const r of RENDERERS) {
        status.textContent = `run ${run} of ${runs}: ${c.cols} × ${c.rows} ${c.name}, ${r} (${rows.length + 1} of ${total})`;
        rows.push(await runCase(c, r, run, pal, stage));
      }
    }
  }
  stage.replaceChildren();
  const text = table(rows, navigator.userAgent);
  out.textContent = text;
  status.textContent = "Done.";
  button.disabled = false;
  window.benchDone = { table: text, rows };
}

async function start(): Promise<void> {
  const style = document.createElement("style");
  style.textContent = fontFace();
  document.head.append(style);
  await document.fonts.load('16px "glyphplot mono"', "⠿0");
  document.getElementById("run")?.addEventListener("click", () => void run());
  if (new URLSearchParams(location.search).has("auto")) await run();
}

void start();
