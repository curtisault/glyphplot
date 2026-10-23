// REQ-E2E-060: every chart type's page on the site (`/<type>`) and every
// example's own page (`/examples/<slug>`) draws a first frame with nothing on
// the console. The types are web/pkg/modules.js's, the examples
// site/examples/manifest.json's, so a new one is tested without an edit here.
//
// The line page and the first example are in the smoke set (REQ-E2E-070).
//
// Each worker keeps one browser context for all its pages, so the wasm and the
// site's chunks it has fetched once stay in that context's cache. Requests to
// any host but the preview server are refused: no test reaches a live feed,
// whose examples must then show their message, not an error (REQ-SITE-050).
// Chromium's own "Failed to load resource" line for a refused third-party
// request is not counted as the page's error.

import { type BrowserContext, expect, type Page, test as base } from "@playwright/test";
import type { GlyphPlot } from "glyphplot/element";
import { charts } from "../../web/pkg/modules.js";
import examples from "../examples/manifest.json" with { type: "json" };
import { DOCS } from "./helpers.ts";

/** A worker's one context, third-party requests refused. */
const test = base.extend<{ sitePage: { page: Page; errors: string[] } }, { siteContext: BrowserContext }>({
  siteContext: [
    async ({ browser }, use, info) => {
      const baseURL = info.project.use.baseURL ?? "";
      const own = new URL(baseURL).origin;
      const context = await browser.newContext({ baseURL, viewport: { width: 1000, height: 800 } });
      await context.route(
        (url) => url.origin !== own,
        (route) => route.abort("blockedbyclient"),
      );
      await use(context);
      await context.close();
    },
    { scope: "worker" },
  ],
  sitePage: async ({ siteContext }, use) => {
    const page = await siteContext.newPage();
    const errors: string[] = [];
    page.on("console", (m) => {
      if (m.type() !== "error") return;
      if (/Failed to load resource/.test(m.text()) && !/^http:\/\/localhost[:/]/.test(m.location().url)) return;
      errors.push(m.text());
    });
    page.on("pageerror", (e) => errors.push(`uncaught: ${e.message}`));
    await use({ page, errors });
    await page.close();
  },
});

/**
 * Waits for a chart inside `scope` (a selector) to draw its first frame: the
 * first `<glyph-plot>` that has made its chart, until its `ready`, or a canvas
 * the JS API drew into (`canvas.gp-ex-canvas`, the typed-arrays example).
 * A chart loads within a screen of the viewport and draws only while it
 * intersects it, so the first `<glyph-plot>` is scrolled into view while none
 * has a chart. Returns the frame's non-blank cell count (-1 for a canvas).
 */
async function firstFrame(page: Page, scope: string): Promise<number> {
  await page.waitForFunction(
    (scope) => {
      const root = document.querySelector(scope);
      const c = root?.querySelector("canvas.gp-ex-canvas") as HTMLCanvasElement | null | undefined;
      if (c && c.width > 0) return true;
      const plots = [...(root?.querySelectorAll("glyph-plot") ?? [])] as GlyphPlot[];
      if (plots.some((e) => e.chart !== null)) return true;
      plots[0]?.scrollIntoView({ block: "center" });
      return false;
    },
    scope,
    { timeout: 20_000, polling: 100 },
  );
  return page.evaluate(async (scope) => {
    const root = document.querySelector(scope) as Element;
    const made = ([...root.querySelectorAll("glyph-plot")] as GlyphPlot[]).find((e) => e.chart !== null);
    if (!made) return -1;
    made.scrollIntoView({ block: "center" });
    await made.ready;
    return [...made.text()].filter((ch) => ch.trim() !== "").length;
  }, scope);
}

for (const c of charts) {
  test(`the ${c.slug} page draws a first frame with no console error${c.slug === "line" ? " @smoke" : ""}`, async ({ sitePage: { page, errors } }) => {
    await page.goto(c.slug);
    await expect(page.locator("h1")).toContainText(c.slug);
    const ink = await firstFrame(page, "main");
    expect(ink).toBeGreaterThan(10);
    expect(errors).toEqual([]);
  });
}

for (const d of DOCS) {
  test(`the doc page ${d} draws a live chart with no console error`, async ({ sitePage: { page, errors } }) => {
    await page.goto(`docs/${d}`);
    await expect(page.locator("main h1")).toBeVisible();
    const ink = await firstFrame(page, "main");
    expect(ink).toBeGreaterThan(10);
    expect(errors).toEqual([]);
  });
}

for (const e of examples) {
  test(`the example ${e.slug} draws a first frame on its page with no console error${e.slug === "live-market" ? " @smoke" : ""}`, async ({ sitePage: { page, errors } }) => {
    await page.goto(`examples/${e.slug}`);
    await expect(page.locator("h1")).toHaveText(e.title);
    await expect(page.locator("gp-example[full]")).toHaveCount(1);
    const ink = await firstFrame(page, "gp-example[full]");
    // Wikimedia's feed is refused here, so its chart waits with no candles; every other example draws ink.
    if (e.slug !== "wikimedia-edits") expect(ink === -1 || ink > 0, `ink ${ink}`).toBe(true);
    // Let the example's live part run a few ticks: a timer that throws shows here.
    await page.waitForTimeout(300);
    if (e.feed) {
      // A live feed names its source and licence, and with the feed down (refused here) shows a message, not an error (REQ-SITE-050).
      await expect(page.locator("main")).toContainText(e.feed.source);
      await expect(page.locator("main")).toContainText(e.feed.licence);
      for (const b of await page.locator("gp-example button", { hasText: /^Load/ }).all()) await b.click();
      await expect(page.locator("gp-example .gp-ex-status").filter({ hasText: /not (available|reachable) right now/ })).toHaveCount(1, { timeout: 10_000 });
    }
    expect(errors).toEqual([]);
  });
}
