// Runs table.js in headless Chromium and WebKit through Playwright and prints the timings.
// Throwaway (see README.md). Needs any playwright-core install, found through NODE_PATH
// (or PW, the path of its index.js), with its Chromium downloaded:
//   NODE_PATH=<dir>/node_modules mise exec -- node discovery/bench/a11y-theming/table-run.mjs
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
const require = createRequire(process.env.PW ?? import.meta.url);
const pw = require("playwright-core");
const rows = process.env.ROWS ?? "1000, 10000, 100000";
const fn = readFileSync(new URL("./table.js", import.meta.url), "utf8").replace(/^\/\/.*$/gm, "").trim().replace(/;$/, "").replace("1000, 10000, 100000", rows);
for (const name of (process.env.BROWSERS ?? "chromium,webkit").split(",")) {
  try {
    const browser = await pw[name].launch({ timeout: 30000 });
    const page = await browser.newPage();
    await page.goto("about:blank");
    const r = await page.evaluate(`(${fn})()`);
    console.log(`## ${name} ${browser.version()}`);
    for (const o of r.out) console.log(`${o.rows} rows rep ${o.rep}: build+layout ${o.buildMs} ms, remove ${o.removeMs} ms, ${o.elements} elements`);
    await browser.close();
  } catch (e) {
    console.log(`## ${name}: ${e.message.split("\n")[0]}`);
  }
}
