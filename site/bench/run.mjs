// mise run bench:render: serves bench/render.html with Vite for the owner's
// Chrome and Safari runs (REQ-RENDER-050), or, with --headless, runs it once
// in Playwright's headless Chromium and prints the table (REQ-RENDER-010).
//
//   node bench/run.mjs [--host] [--port N]       serve, until Ctrl-C
//   node bench/run.mjs --headless [--runs N]     one headless run, printed
import { createServer } from "vite";
import { chromium } from "@playwright/test";

const args = process.argv.slice(2);
const flag = (name) => args.includes(name);
const value = (name, d) => {
  const i = args.indexOf(name);
  return i >= 0 && args[i + 1] ? args[i + 1] : d;
};
const headless = flag("--headless");
const port = Number(value("--port", headless ? "5312" : "5311"));
const runs = Number(value("--runs", "3"));

const server = await createServer({
  configFile: new URL("../vite.config.ts", import.meta.url).pathname,
  server: { port, strictPort: true, host: flag("--host") ? true : "localhost" },
  logLevel: headless ? "error" : "info",
});
await server.listen();
const url = `http://localhost:${port}/bench/render.html`;

if (!headless) {
  server.printUrls();
  console.log(`\nThe renderer bench: ${url}\nOpen it in Chrome and in Safari, press Run, and copy the table into the plan (the page says how). Ctrl-C stops.`);
} else {
  const browser = await chromium.launch({ headless: true });
  try {
    const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
    page.on("pageerror", (e) => console.error(`page error: ${e.message}`));
    await page.goto(`${url}?auto&runs=${runs}`);
    await page.waitForFunction(() => window.benchDone !== undefined, null, { timeout: 600_000, polling: 500 });
    const done = await page.evaluate(() => window.benchDone);
    console.log(`headless Chromium ${browser.version()} (Playwright), software raster unless the machine's GPU is used headless`);
    console.log(done.table);
  } finally {
    await browser.close();
    await server.close();
  }
}
