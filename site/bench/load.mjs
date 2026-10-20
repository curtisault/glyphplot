// mise run bench:load (REQ-PERF-080, REQ-BENCH-020): bench/load.html, one
// <glyph-plot> line chart of 1,000 points, loaded cold 7 times a profile in
// headless Chromium over the release build (site/dist and web/pkg, served
// gzip -9 by bench/serve.mjs), each load in a fresh browser context with
// the cache disabled. The time is ms from navigation start to the chart's
// first frame: the page's `plot-ready` listener reads performance.now()
// (the `glyphplot:first-frame` mark). Profiles, through CDP
// Network.emulateNetworkConditions and Emulation.setCPUThrottlingRate (the
// successor of discovery/bench/budgets/js/load-bench.mjs, with its units:
// kbps × 1024 / 8 bytes a second, as Lighthouse's devtools throttling):
//
// - "4G": 170 ms latency, 9 Mbps down, 1.5 Mbps up, CPU 1×. Gated: the
//   median MUST be at most 1,000 ms (REQ-PERF-080, ADR-133).
// - "slow 4G": Lighthouse 12's mobileSlow4G in its request-level form
//   (core/config/constants.js: 150 ms × 3.75 = 562.5 ms, 1.6 Mbps × 0.9 =
//   1.44 Mbps down, 750 kbps × 0.9 up, CPU 4×). Reported, not gated.
// - "none": localhost, no throttling. Reported, for reference.
//
// Then the same page with preload hints for the module's glue and its wasm
// (and the manifest, should it be a request of its own), on the two
// throttled profiles: reported, not gated, to show what the hints a page
// author can add are worth. The shell itself puts the manifest in its own
// chunk and fetches the wasm alongside the glue (REQ-PERF-080).
//
//   node bench/load.mjs [--loads N]
import { chromium } from "@playwright/test";
import { load, machine, median, needBuild, table } from "./common.mjs";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { serve, SITE_DIST } from "./serve.mjs";

const args = process.argv.slice(2);
const at = args.indexOf("--loads");
const LOADS = at >= 0 ? Number(args[at + 1]) : 7;
const BUDGET_MS = 1000;
const PROFILES = [
  { name: "4G (170 ms, 9 Mbps)", latency: 170, down: 9000, up: 1500, cpu: 1, gate: true },
  { name: "Lighthouse slow 4G (562.5 ms, 1.44 Mbps, CPU 4×)", latency: 562.5, down: 1.6 * 1024 * 0.9, up: 750 * 0.9, cpu: 4, gate: false },
  { name: "none (localhost)", latency: 0, down: 0, up: 0, cpu: 1, gate: false, off: true },
];

needBuild("bench/load.html");
const server = await serve();
const browser = await chromium.launch({ headless: true });
const rows = [];
let over = false;
const started = Date.now();
let resources = [];

/** One cold load of `path` under profile `p`: its first frame, the module's arrival and the bytes sent. */
async function once(path, p) {
  const ctx = await browser.newContext({ viewport: { width: 1000, height: 800 } });
  try {
    const page = await ctx.newPage();
    const errors = [];
    page.on("pageerror", (e) => errors.push(e.message));
    page.on("console", (m) => m.type() === "error" && errors.push(m.text()));
    const cdp = await ctx.newCDPSession(page);
    await cdp.send("Network.enable");
    await cdp.send("Network.setCacheDisabled", { cacheDisabled: true });
    if (!p.off) {
      await cdp.send("Network.emulateNetworkConditions", {
        offline: false,
        latency: p.latency,
        downloadThroughput: (p.down * 1024) / 8,
        uploadThroughput: (p.up * 1024) / 8,
      });
    }
    await cdp.send("Emulation.setCPUThrottlingRate", { rate: p.cpu });
    await page.goto(`${server.url}${path}`, { waitUntil: "commit" });
    const r = await page
      .waitForFunction(() => window.firstFrame, null, { timeout: 60_000, polling: 50 })
      .then((h) => h.jsonValue())
      .catch((e) => {
        throw new Error(`${path}: no first frame (${e.message}); page errors: ${errors.join(" | ") || "none"}`);
      });
    const nav = await page.evaluate(() => performance.getEntriesByType("navigation")[0]?.transferSize ?? 0);
    const w = r.resources.find((e) => e.name.endsWith(".wasm"));
    return { ms: r.ms, wasm: w ? w.end : NaN, transfer: nav + r.resources.reduce((a, e) => a + e.transfer, 0), resources: r.resources };
  } finally {
    await ctx.close();
  }
}

async function measure(label, path, p, gated) {
  const runs = [];
  for (let i = 0; i < LOADS; i++) runs.push(await once(path, p));
  const firsts = runs.map((r) => r.ms);
  const med = median(firsts);
  const wasm = median(runs.map((r) => r.wasm));
  const bad = gated && med > BUDGET_MS;
  over ||= bad;
  rows.push([
    label,
    String(runs.length),
    Math.min(...firsts).toFixed(0),
    med.toFixed(0),
    Math.max(...firsts).toFixed(0),
    wasm.toFixed(0),
    (med - wasm).toFixed(1),
    Math.max(...runs.map((r) => r.transfer)).toLocaleString("en-US"),
    gated ? `${BUDGET_MS}` : "not gated",
    bad ? "OVER" : "",
  ]);
  return runs;
}

try {
  console.log(`bench:load: headless Chromium ${browser.version()} (Playwright), ${machine()}, ${new Date().toISOString()}, ${LOADS} cold loads a row`);
  for (const p of PROFILES) {
    const runs = await measure(p.name, "/bench/load.html", p, p.gate);
    resources = runs[0].resources;
  }
  // The same page with preload hints for the requests the shell makes after
  // its own scripts (the module's glue and its wasm, and the manifest when it
  // is a request of its own), so they leave with the page's own scripts: what
  // a page author can add. Reported, not gated.
  const asset = (re) => resources.map((r) => r.name).find((n) => re.test(n));
  const hints = [
    [asset(/^modules-.*\.js$/), "modulepreload"],
    [asset(/^glyphplot-.*\.js$/), "modulepreload"],
    [asset(/\.wasm$/), "preload"],
  ].filter(([n]) => n);
  if (hints.some(([n]) => n.endsWith(".wasm")) && hints.some(([n]) => /^glyphplot-.*\.js$/.test(n))) {
    const html = readFileSync(join(SITE_DIST, "bench/load.html"), "utf8").replace(
      "</head>",
      `${hints.map(([n, rel]) => `  <link rel="${rel}"${rel === "preload" ? ' as="fetch"' : ""} crossorigin href="../assets/${n}">`).join("\n")}\n</head>`,
    );
    server.page("/bench/load-preload.html", html);
    for (const p of PROFILES.filter((q) => !q.off)) await measure(`${p.name.split(" (")[0]}, preload hints`, "/bench/load-preload.html", p, false);
  } else console.log(`bench:load: the preload rows need the glue and the wasm among the requests; found ${hints.map(([n]) => n).join(", ") || "neither"}`);
} finally {
  await browser.close();
  await server.close();
}
console.log(table(["profile", "loads", "min ms", "median ms", "max ms", "wasm in ms", "after wasm ms", "transfer B", "budget (median)", ""], rows));
console.log(
  "ms from navigation start to the first frame (plot-ready); \"wasm in\" is the median time the module finished arriving, \"after wasm\" the median first frame less it (compile, instantiate, chart and first frame); transfer is the HTML and every resource fetched before the first frame, as sent (gzip -9), the largest of the loads.",
);
console.log(`requests before the first frame: ${resources.map((r) => r.name).join(", ")}`);
console.log(`bench:load: ${((Date.now() - started) / 1000).toFixed(1)} s, load averages after ${load()}`);
if (over) {
  console.log("bench:load: OVER budget");
  process.exit(1);
}
