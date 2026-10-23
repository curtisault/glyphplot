// 33 · Typed arrays, no element: the JS API alone (REQ-JS-010, REQ-MOUNT-010).
// `chart("line", { data })` takes two Float64Arrays in the columns
// shorthand, copied once into wasm (REQ-MEM-020); `mount(canvas, chart, …)`
// draws it into a canvas and turns pointer, wheel, touch and key events into
// the chart's input. Live, the card's element is hidden and this canvas takes
// its place:
//
//   import { chart, mount } from "glyphplot";
//   const c = chart("line", { data: { x, y } });   // x, y: Float64Array
//   await c.ready;
//   const m = mount(canvas, c, { input: true, palette: c.palette({}), waitForSize: true });
//   m.resize(cols, rows, cellWidthInDevicePx);

import type { Example } from "./api.ts";
import { normal, rng } from "../src/data/rng.ts";
import { make, Scope } from "./support/ui.ts";

const SEED = 33;
const N = 20_000;

function arrays(): { x: Float64Array; y: Float64Array } {
  const r = rng(SEED);
  const x = new Float64Array(N);
  const y = new Float64Array(N);
  for (let i = 0; i < N; i++) {
    x[i] = i / 100;
    y[i] = Math.sin(x[i] as number) * Math.exp(-(x[i] as number) / 150) + normal(r) * 0.05;
  }
  return { x, y };
}

export const example: Example = {
  slug: "typed-arrays",
  seed: SEED,
  start() {
    return { type: "line", data: arrays() };
  },
  live(el, host) {
    const s = new Scope();
    el.style.display = "none";
    s.add(() => el.style.removeProperty("display"));
    void import("glyphplot").then(async ({ chart, mount }) => {
      if (s.stopped) return;
      const c = chart("line", { data: arrays() });
      s.add(() => c.free());
      await c.ready;
      if (s.stopped) return;
      const canvas = make("canvas", "gp-ex-canvas");
      canvas.tabIndex = 0;
      canvas.setAttribute("role", "img");
      canvas.setAttribute("aria-label", "A damped sine of 20,000 points, drawn by the JS API into a canvas");
      host.prepend(canvas);
      s.add(() => canvas.remove());
      const style = getComputedStyle(host);
      const [r = 0, g = 0, b = 0] = (style.color.match(/\d+/g) ?? []).map(Number);
      const lightPage = 0.2126 * r + 0.7152 * g + 0.0722 * b < 128;
      const dpr = globalThis.devicePixelRatio || 1;
      const cw = Math.max(1, Math.round(0.6 * Math.max(13, Number.parseFloat(style.fontSize) || 13) * dpr));
      const cols = Math.max(20, Math.floor((host.clientWidth * dpr) / cw));
      const rows = 14;
      Object.assign(canvas.style, { display: "block", width: `${(cols * cw) / dpr}px`, height: `${(rows * cw * 2) / dpr}px` });
      const m = mount(canvas, c, { input: true, palette: c.palette(lightPage ? { paper: true } : {}), paintGround: false, waitForSize: true });
      s.add(() => m.stop());
      m.resize(cols, rows, cw);
    });
    return s.stop;
  },
};
