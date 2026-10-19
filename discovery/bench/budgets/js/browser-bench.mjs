// Throwaway. Runs bench-core.mjs in headless Chromium (Playwright) against
// pkg/6-downsample, served cross-origin isolated by server.mjs.
//   node browser-bench.mjs [runs] [chromium|webkit]
import { chromium, webkit } from "playwright";
import { spawn } from "node:child_process";
import { loadavg } from "node:os";

const [runs = "5", engine = "chromium"] = process.argv.slice(2);
const port = 8700 + Math.floor(Math.random() * 200);
const server = spawn(process.execPath, [new URL("./server.mjs", import.meta.url).pathname, String(port)], { stdio: ["ignore", "pipe", "inherit"] });
await new Promise((r) => server.stdout.once("data", r));
const browser = await (engine === "webkit" ? webkit : chromium).launch({ headless: true });
try {
  const page = await browser.newPage();
  console.log(`${engine} ${browser.version()}, load ${loadavg().map((l) => l.toFixed(2)).join(" ")}, ${new Date().toISOString()}`);
  await page.goto(`http://localhost:${port}/js/page.html?mode=bench&runs=${runs}&memory=1`);
  const iso = await page.evaluate(() => self.crossOriginIsolated);
  const r = await page.waitForFunction(() => window.__result, null, { timeout: 900_000, polling: 1000 }).then((h) => h.jsonValue());
  console.log(`crossOriginIsolated ${iso}`);
  console.log(r.text);
  console.log("memory pages", JSON.stringify(r.memory));
  console.log(`load after ${loadavg().map((l) => l.toFixed(2)).join(" ")}`);
} finally {
  await browser.close();
  server.kill();
}
