// Throwaway (see ../README.md). Which raster backend headless Chrome used, and
// whether atlas-versus-fillText holds with the GPU off. Usage:
//   FONT=<path to a monospace .woff2> node backend.mjs [runs=3]
import { chromium } from "playwright-core";
import { readFileSync } from "node:fs";

const RUNS = Number(process.argv[2] ?? 3);
const CHROME = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
// Any monospace woff2 works (for example a subset written by ../font/subset.py);
// page.js draws with it under the family name "bench mono".
if (!process.env.FONT) {
  console.error("Set FONT to the path of a monospace .woff2 font, e.g. FONT=../font/target/base.woff2");
  process.exit(1);
}
const font = readFileSync(process.env.FONT).toString("base64");
const page_js = readFileSync(new URL("./page.js", import.meta.url), "utf8");
const CASES = [
  { kind: "density", model: "pal256", renderer: "atlas", cols: 320, rows: 120, cw: 8 },
  { kind: "density", model: "pal256", renderer: "direct", cols: 320, rows: 120, cw: 8 },
  { kind: "density", model: "rgb", renderer: "direct", cols: 320, rows: 120, cw: 8 },
  { kind: "scatter", model: "rgb", renderer: "tint", cols: 320, rows: 120, cw: 8 },
  { kind: "density", model: "pal256", renderer: "atlas", cols: 80, rows: 24, cw: 10 },
  { kind: "density", model: "pal256", renderer: "direct", cols: 80, rows: 24, cw: 10 },
];
for (const [label, args] of [["default", []], ["gpu off", ["--disable-gpu"]]]) {
  const browser = await chromium.launch({ executablePath: CHROME, headless: true, args });
  const page = await browser.newPage();
  await page.setContent("<!doctype html><body></body>");
  const gl = await page.evaluate(() => {
    const c = document.createElement("canvas").getContext("webgl");
    const d = c && c.getExtension("WEBGL_debug_renderer_info");
    return d ? c.getParameter(d.UNMASKED_RENDERER_WEBGL) : "no webgl";
  });
  console.log(JSON.stringify({ backend: label, webgl: gl }));
  await page.close();
  for (let run = 1; run <= RUNS; run++) for (const c of CASES) {
    const p = await browser.newPage();
    await p.setContent("<!doctype html><body style='margin:0'></body>");
    await p.evaluate(async (b64) => document.fonts.add(await new FontFace("bench mono", Uint8Array.from(atob(b64), (ch) => ch.charCodeAt(0)).buffer).load()), font);
    await p.addScriptTag({ content: page_js });
    const r = await p.evaluate((o) => window.runCase(o), c);
    console.log(JSON.stringify({ backend: label, run, ...c, median: r.median, p95: r.p95 }));
    await p.close();
  }
  await browser.close();
}
