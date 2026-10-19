// Throwaway (see ../README.md). Runs page.js's cases in headless Chrome and
// prints one JSON line per case per run. Usage:
//   FONT=<path to a monospace .woff2> node run.mjs [runs=3] > results.jsonl
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

const SIZES = [
  { cols: 80, rows: 24, cw: 10 },
  { cols: 320, rows: 120, cw: 8 },
];
const CASES = [];
for (const s of SIZES) {
  CASES.push({ kind: "density", model: "rgb", renderer: "noop", ...s });
  for (const kind of ["density", "scatter"]) {
    for (const model of ["pal64", "pal256", "rgb"]) {
      CASES.push({ kind, model, renderer: "atlas", ...s });
      CASES.push({ kind, model, renderer: "tint", ...s });
    }
    CASES.push({ kind, model: "pal256", renderer: "atlas", slots: 4096, ...s });
    CASES.push({ kind, model: "rgb", renderer: "atlas", slots: 4096, ...s });
    CASES.push({ kind, model: "rgb", renderer: "direct", ...s });
  }
  for (const model of ["pal256", "rgb"]) {
    CASES.push({ kind: "blocks", model, renderer: "rects", ...s });
    CASES.push({ kind: "blocks", model, renderer: "image", ...s });
  }
}

const browser = await chromium.launch({ executablePath: CHROME, headless: true });
console.error(`chrome ${browser.version()}`);
for (let run = 1; run <= RUNS; run++) {
  for (const c of CASES) {
    const page = await browser.newPage();
    await page.setContent("<!doctype html><body style='margin:0;background:#111'></body>");
    await page.evaluate(async (b64) => {
      const bytes = Uint8Array.from(atob(b64), (ch) => ch.charCodeAt(0));
      const face = new FontFace("bench mono", bytes.buffer);
      document.fonts.add(await face.load());
    }, font);
    await page.addScriptTag({ content: page_js });
    const r = await page.evaluate((o) => window.runCase(o), c);
    console.log(JSON.stringify({ run, ...c, ...r }));
    await page.close();
  }
  if (run === 1) {
    const page = await browser.newPage();
    await page.setContent("<!doctype html><body></body>");
    await page.evaluate(async (b64) => {
      const bytes = Uint8Array.from(atob(b64), (ch) => ch.charCodeAt(0));
      document.fonts.add(await new FontFace("bench mono", bytes.buffer).load());
    }, font);
    await page.addScriptTag({ content: page_js });
    for (const dpr of [1, 2]) for (const px of [8, 9, 10, 11, 12]) console.log(JSON.stringify({ digits: await page.evaluate(([p, d]) => window.digits(p, d), [px, dpr]) }));
    await page.close();
  }
}
await browser.close();
