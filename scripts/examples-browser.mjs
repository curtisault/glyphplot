// Every example in headless Chromium (REQ-SITE-040, REQ-SITE-120): Vite's dev
// server over site/ (the wasm in web/pkg as `mise run build:wasm` left it),
// then on site/blank.html:
//
// 1. each example mounted alone as `<gp-example>` (and once more with
//    `full`): its `<glyph-plot>` must fire `plot-ready` (a first frame) with
//    no console error and no uncaught exception;
// 2. the gallery: all examples as cards, no wasm fetched before a scroll,
//    then scrolled to the end, each family's module fetched at most once.
//
// Requests to any host but the dev server are refused, so no third-party feed
// is contacted: the live-feed examples must then show their message, not an
// error (REQ-SITE-050). Chromium's own "Failed to load resource" line for a
// refused third-party request is not counted as the page's error.
//
//   node scripts/examples-browser.mjs [slug …]     (mise run examples:browser)
//
// Exits 1 naming each failure.
import { createRequire } from "node:module";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const require = createRequire(join(ROOT, "site/package.json"));
const { createServer } = await import(pathToFileURL(require.resolve("vite")).href);
const pw = await import(pathToFileURL(require.resolve("@playwright/test")).href);
const chromium = pw.chromium ?? pw.default.chromium;
const { examples } = await import(pathToFileURL(join(ROOT, "site/examples/index.ts")).href);

// `--live` lets the page reach third-party feeds, waits for them, and presses each "Load …" button: the owner's check of the live examples.
const LIVE = process.argv.includes("--live");
const only = process.argv.slice(2).filter((a) => a !== "--live");
const slugs = examples.map((e) => e.slug).filter((s) => only.length === 0 || only.includes(s));
const READY_MS = 30_000;

const server = await createServer({ configFile: join(ROOT, "site/vite.config.ts"), logLevel: "warn", server: { port: 5390, strictPort: false } });
await server.listen();
const base = server.resolvedUrls.local[0];
const browser = await chromium.launch();
const failures = [];
const began = performance.now();

/** A page on blank.html with <gp-example> defined, third-party requests refused, its errors collected. */
async function open() {
  const page = await browser.newPage({ viewport: { width: 900, height: 900 } });
  const errors = [];
  const wasm = [];
  const refused = [];
  await page.route("**/*", (route) => {
    const u = new URL(route.request().url());
    if (u.origin === new URL(base).origin || LIVE) return route.continue();
    refused.push(u.href);
    return route.abort("blockedbyclient");
  });
  page.on("request", (r) => {
    if (r.url().endsWith(".wasm")) wasm.push(r.url());
  });
  page.on("console", (m) => {
    if (m.type() !== "error") return;
    if (/Failed to load resource/.test(m.text()) && refused.length > 0) return;
    errors.push(m.text());
  });
  page.on("pageerror", (e) => errors.push(`uncaught: ${e.message}`));
  await page.goto(`${base}blank.html`);
  await page.waitForFunction(() => window.gpReady === true);
  await page.addScriptTag({ type: "module", content: 'import "/examples/element.ts"; window.gpExampleReady = true;' });
  await page.waitForFunction(() => window.gpExampleReady === true, null, { timeout: 120_000 });
  return { page, errors, wasm, refused };
}

// 1. Each example alone, as a card and as a page.
const solo = await open();
for (const slug of slugs) {
  for (const full of LIVE ? [false] : [false, true]) {
    const where = `${slug}${full ? " (full)" : ""}`;
    solo.errors.length = 0;
    const result = await solo.page.evaluate(
      async ({ slug, full, ms, live }) => {
        const stage = document.getElementById("stage");
        stage.replaceChildren();
        const ex = document.createElement("gp-example");
        ex.setAttribute("slug", slug);
        if (full) ex.setAttribute("full", "");
        const ready = new Promise((res) => {
          const t = setTimeout(() => res({ ok: false, why: `no plot-ready within ${ms} ms` }), ms);
          ex.addEventListener("plot-ready", (e) => {
            clearTimeout(t);
            res({ ok: true, detail: e.detail, target: e.target });
          });
          ex.addEventListener("plot-error", (e) => {
            clearTimeout(t);
            res({ ok: false, why: `plot-error: ${e.detail.message}` });
          });
        });
        stage.append(ex);
        // An example drawing through the JS API alone (typed-arrays) has no element to fire plot-ready: its canvas counts.
        const poll = setInterval(() => {
          const c = ex.querySelector("canvas.gp-ex-canvas");
          if (c && c.width > 0) ex.dispatchEvent(new CustomEvent("gp-canvas-ready", { detail: { cols: 0, rows: 0 } }));
        }, 100);
        ex.addEventListener("gp-canvas-ready", (e) => {
          clearInterval(poll);
          ex.dispatchEvent(new CustomEvent("plot-ready", { detail: e.detail }));
        }, { once: true });
        ready.then(() => clearInterval(poll));
        ex.dispatchEvent(new Event("pointerenter")); // wakes this card alone
        const r = await ready;
        await new Promise((res) => setTimeout(res, live ? 5000 : 400)); // let live() run a few ticks
        if (live) {
          for (const b of ex.querySelectorAll(".gp-ex-controls button")) if (/^Load/.test(b.textContent ?? "")) b.click();
          await new Promise((res) => setTimeout(res, 15_000));
        }
        // The chart that drew first (a canvas drawn by the JS API counts as drawn).
        const plot = r.target;
        const text = plot && typeof plot.text === "function" ? plot.text() : plot === ex ? "canvas" : "";
        delete r.target;
        const status = [...ex.querySelectorAll(".gp-ex-status")].map((o) => o.textContent).join(" | ");
        const provenance = ex.querySelector(".gp-ex-provenance")?.textContent ?? "";
        return { ...r, chars: text.replace(/\s/g, "").length, status, provenance };
      },
      { slug, full, ms: READY_MS, live: LIVE },
    );
    if (!result.ok) failures.push(`${where}: ${result.why}`);
    else if (result.chars === 0 && slug !== "wikimedia-edits") failures.push(`${where}: the first frame is blank`);
    if (solo.errors.length > 0) failures.push(`${where}: console: ${solo.errors.join(" / ")}`);
    if (!full) console.log(`${result.ok ? "ok  " : "FAIL"} ${slug}${result.detail ? ` ${result.detail.cols}×${result.detail.rows}` : ""}${result.status ? ` — ${result.status.slice(0, LIVE ? 400 : 140)}` : ""}`);
  }
}
await solo.page.evaluate(() => document.getElementById("stage").replaceChildren());
await solo.page.close();

// 2. The gallery: no wasm before a scroll, each module once after.
if (only.length === 0 && !LIVE) {
  const g = await open();
  await g.page.evaluate((slugs) => {
    const stage = document.getElementById("stage");
    for (const slug of slugs) {
      const ex = document.createElement("gp-example");
      ex.setAttribute("slug", slug);
      ex.style.marginBottom = "48px";
      stage.append(ex);
    }
  }, slugs);
  await g.page.waitForTimeout(2000);
  const before = g.wasm.length;
  if (before > 0) failures.push(`gallery: ${before} wasm fetched before a scroll: ${g.wasm.join(", ")}`);
  const height = await g.page.evaluate(() => document.documentElement.scrollHeight);
  for (let y = 0; y <= height; y += 600) {
    await g.page.mouse.wheel(0, 600);
    await g.page.waitForTimeout(150);
  }
  await g.page.waitForTimeout(2000);
  const byModule = {};
  for (const u of g.wasm) {
    const m = /\/([^/]+)\/glyphplot_bg\.wasm/.exec(u)?.[1] ?? u;
    byModule[m] = (byModule[m] ?? 0) + 1;
  }
  for (const [m, n] of Object.entries(byModule)) if (n > 1) failures.push(`gallery: module ${m} fetched ${n} times`);
  if (g.errors.length > 0) failures.push(`gallery: console: ${g.errors.slice(0, 5).join(" / ")}`);
  const live = await g.page.evaluate(() => document.querySelectorAll("gp-example glyph-plot").length);
  console.log(`gallery: ${slugs.length} cards; wasm before a scroll: ${before}; after scrolling to the end: ${JSON.stringify(byModule)}; ${live} charts made`);
  await g.page.close();
}

await browser.close();
await server.close();
console.log(`examples:browser: ${slugs.length} examples in ${((performance.now() - began) / 1000).toFixed(1)} s (performance.now())`);
if (failures.length > 0) {
  console.error(`examples:browser: ${failures.length} failure(s):\n  ${failures.join("\n  ")}`);
  process.exit(1);
}
