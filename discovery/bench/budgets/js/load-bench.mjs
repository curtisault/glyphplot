// Throwaway. Time to first frame of an 80x24 line chart (1000 points) from
// navigation start, in headless Chromium with CDP network (and CPU)
// throttling, a fresh browser context and no cache for each load. The server
// sends the wasm and the JS glue gzip -9.
//   node load-bench.mjs [reps]
import { chromium } from "playwright";
import { spawn } from "node:child_process";
import { loadavg } from "node:os";

const reps = Number(process.argv[2] ?? 7);
const onlyPreload = process.argv[3] === "preload";
// Lighthouse 12 core/config/constants.js mobileSlow4G, request-level (DevTools) form:
// 150 ms x 3.75 latency, 1.6 Mbps x 0.9 down, 750 kbps x 0.9 up, CPU x4.
const profiles = {
  "brief-4g": { latency: 170, down: 9000, up: 1500, cpu: 1 },
  "lighthouse-slow-4g": { latency: 562.5, down: 1.6 * 1024 * 0.9, up: 750 * 0.9, cpu: 4 },
  "none": null,
};
const cases = [
  ["1-line", null],
  ["4-serde", null],
  ["6-downsample", null],
  ["6-downsample", "ladder_bg_pad150.wasm"],
  ["6-downsample", "ladder_bg_pad300.wasm"],
  ["6-downsample", "ladder_bg_pad1000.wasm"],
  ["6-downsample", null, "page-preload.html"],
];
const port = 8900 + Math.floor(Math.random() * 90);
const server = spawn(process.execPath, [new URL("./server.mjs", import.meta.url).pathname, String(port)], { stdio: ["ignore", "pipe", "inherit"] });
await new Promise((r) => server.stdout.once("data", r));
const browser = await chromium.launch({ headless: true });
console.log(`chromium ${browser.version()}, load ${loadavg().map((l) => l.toFixed(2)).join(" ")}, ${new Date().toISOString()}, ${reps} loads each`);
const med = (a) => [...a].sort((x, y) => x - y)[Math.floor(a.length / 2)];
try {
  for (const [pname, p] of Object.entries(profiles)) {
    for (const [pkg, wasm, html = "page.html"] of cases) {
      if (onlyPreload && html === "page.html" && !(pkg === "6-downsample" && !wasm)) continue;
      const frames = [], wasmEnd = [], inits = [];
      let transfer = 0;
      for (let i = 0; i < reps; i++) {
        const ctx = await browser.newContext();
        const page = await ctx.newPage();
        const cdp = await ctx.newCDPSession(page);
        await cdp.send("Network.enable");
        await cdp.send("Network.setCacheDisabled", { cacheDisabled: true });
        if (p) {
          await cdp.send("Network.emulateNetworkConditions", { offline: false, latency: p.latency, downloadThroughput: (p.down * 1024) / 8, uploadThroughput: (p.up * 1024) / 8 });
          await cdp.send("Emulation.setCPUThrottlingRate", { rate: p.cpu });
        }
        await page.goto(`http://localhost:${port}/js/${html}?mode=load&pkg=${pkg}${wasm ? `&wasm=${wasm}` : ""}`);
        const r = await page.waitForFunction(() => window.__result, null, { timeout: 120_000 }).then((h) => h.jsonValue());
        frames.push(r.frame);
        inits.push(r.init - (r.res.find((e) => e.name.endsWith(".wasm"))?.end ?? 0));
        const w = r.res.find((e) => e.name.endsWith(".wasm"));
        wasmEnd.push(w?.end ?? NaN);
        transfer = r.res.reduce((a, e) => a + e.transfer, 0);
        await ctx.close();
      }
      console.log(`${pname.padEnd(19)} ${(pkg + (wasm ? "/" + wasm : "") + (html !== "page.html" ? " +preload" : "")).padEnd(36)} transfer ${String(transfer).padStart(8)} B | first frame ms min ${Math.min(...frames).toFixed(0).padStart(6)} med ${med(frames).toFixed(0).padStart(6)} | wasm received med ${med(wasmEnd).toFixed(0).padStart(6)} | compile+instantiate+frame after it med ${(med(frames) - med(wasmEnd)).toFixed(1)}`);
    }
  }
} finally {
  await browser.close();
  server.kill();
}
console.log(`load after ${loadavg().map((l) => l.toFixed(2)).join(" ")}`);
