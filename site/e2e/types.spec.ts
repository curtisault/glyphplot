// The Phase 4 types on the checklist page (test.html): `candles`, `bar` and
// `sparkline` each draw a first frame there from their own module and answer
// a hover with a plot-hover naming a datum (REQ-CANDLE-050, REQ-BAR-020,
// REQ-SPARK-010, REQ-HIT-010). The pixels and the finger are the owner's
// (cases 25 to 29).

import { expect, test } from "@playwright/test";
import type { GlyphPlot } from "glyphplot/element";
import { consoleErrors, frames } from "./helpers.ts";

const CASES = [
  { id: "candles-chart", type: "candles", module: "candles" },
  { id: "bar-vertical", type: "bar", module: "bar" },
  { id: "spark-line", type: "sparkline", module: "line" },
] as const;

for (const c of CASES) {
  test(`${c.type} draws a first frame on test.html and answers a hover @smoke`, async ({ page }) => {
    const errors = consoleErrors(page);
    await page.goto("test.html");
    await page.waitForFunction(() => window.gpReady === true);
    const el = page.locator(`#${c.id}`);
    await el.scrollIntoViewIfNeeded();
    await page.waitForFunction((id) => (document.getElementById(id) as GlyphPlot | null)?.chart != null, c.id);
    await frames(page, 2);
    // A first frame: the drawing is there and its text holds ink.
    const drawn = await el.evaluate((e) => {
      const g = e as GlyphPlot;
      return { type: g.type, cols: g.cols, rows: g.rows, ink: [...g.text()].filter((ch) => ch.trim() !== "").length };
    });
    expect(drawn.type).toBe(c.type);
    expect(drawn.cols).toBeGreaterThan(0);
    expect(drawn.ink).toBeGreaterThan(10);
    // From its own family's module, as the manifest maps it.
    expect(await page.evaluate(async (type) => (await window.gp.manifest()).modules[type], c.type)).toBe(c.module);
    // A hover over the plot names a datum.
    await el.evaluate((e) => {
      const w = window as unknown as { __hovers: unknown[] };
      w.__hovers = [];
      e.addEventListener("plot-hover", (ev) => w.__hovers.push((ev as CustomEvent).detail));
    });
    const b = await el.locator(".gp-drawing").boundingBox();
    if (!b) throw new Error("no drawing");
    // Left of centre and a third down: inside the plot of each (a sparkline's text column is on the right).
    await page.mouse.move(b.x + b.width * 0.3, b.y + b.height * 0.35);
    await frames(page, 3);
    await page.mouse.move(b.x + b.width * 0.35, b.y + b.height * 0.35);
    await frames(page, 3);
    const hovers = await page.evaluate(() => (window as unknown as { __hovers: Record<string, unknown>[] }).__hovers);
    const named = hovers.find((h) => h !== null);
    expect(named, JSON.stringify(hovers)).toBeTruthy();
    expect(typeof named?.["series"]).toBe("string");
    expect(typeof named?.["index"]).toBe("number");
    expect(named?.["source"]).toBe("pointer");
    expect(errors).toEqual([]);
  });
}
