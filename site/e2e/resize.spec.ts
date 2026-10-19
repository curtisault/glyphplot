// REQ-E2E-040: a change of container width changes `cols`, keeps the view in
// data units and logs no console error; with it REQ-EL-080 to 100 (the cell
// from the font, the font floor, a relayout within a frame, width 0 ignored,
// 12 rows without a height).

import { expect, test } from "@playwright/test";
import type { GlyphPlot } from "glyphplot/element";
import { addChart, consoleErrors, domain, frames, openBlank } from "./helpers.ts";

type El = GlyphPlot;

test.beforeEach(async ({ page }) => {
  await openBlank(page);
  await page.evaluate(() => {
    const box = document.createElement("div");
    box.id = "box";
    box.style.width = "600px";
    box.style.fontSize = "16px";
    document.getElementById("stage")?.replaceWith(box);
    box.id = "stage";
  });
});

const sizeOf = (el: import("@playwright/test").Locator): Promise<[number, number]> => el.evaluate((e) => [(e as El).cols, (e as El).rows]);

test("a container width change changes cols, keeps the view in data units, and logs no console error", async ({ page }) => {
  const errors = consoleErrors(page);
  const el = await addChart(page);
  // 16 px at a pixel ratio of 1: a cell 10 px wide and 20 tall (REQ-EL-080).
  expect(await sizeOf(el)).toEqual([60, 12]);
  await el.evaluate((e) => ((e as El).view = { x: { domain: [40, 120] } }));
  await frames(page, 2);
  const textBefore = await el.evaluate((e) => (e as El).text());
  for (const [w, cols] of [
    [300, 30],
    [455, 45],
    [1000 - 2 * 16, 96],
  ] as const) {
    await page.evaluate((w) => ((document.getElementById("stage") as HTMLElement).style.width = `${w}px`), w);
    // Within one animation frame (REQ-EL-090): the ResizeObserver runs before the next paint.
    await frames(page, 2);
    expect((await sizeOf(el))[0]).toBe(cols);
    expect(await domain(el)).toEqual([40, 120]);
    expect(await el.locator("canvas").evaluate((c) => (c as HTMLCanvasElement).width)).toBe(cols * 10);
  }
  await page.evaluate(() => ((document.getElementById("stage") as HTMLElement).style.width = "600px"));
  await frames(page, 2);
  expect(await el.evaluate((e) => (e as El).text())).toBe(textBefore);
  expect(errors).toEqual([]);
});

test("a box of width 0 is ignored", async ({ page }) => {
  const errors = consoleErrors(page);
  const el = await addChart(page);
  const before = await sizeOf(el);
  await page.evaluate(() => ((document.getElementById("stage") as HTMLElement).style.width = "0px"));
  await frames(page, 3);
  expect(await sizeOf(el)).toEqual(before);
  await page.evaluate(() => ((document.getElementById("stage") as HTMLElement).style.width = "300px"));
  await frames(page, 2);
  expect((await sizeOf(el))[0]).toBe(30);
  expect(errors).toEqual([]);
});

test("without a height the chart takes 12 rows, or the rows attribute's count", async ({ page }) => {
  const a = await addChart(page);
  expect((await sizeOf(a))[1]).toBe(12);
  expect(await a.evaluate((e) => e.getBoundingClientRect().height)).toBe(240);
  const b = await addChart(page, { attrs: { rows: "5" } });
  expect((await sizeOf(b))[1]).toBe(5);
  expect(await b.evaluate((e) => e.getBoundingClientRect().height)).toBe(100);
  // A height of its own wins: 300 px holds 15 rows of 20 px.
  const c = await addChart(page, { style: "height: 300px" });
  expect(await sizeOf(c)).toEqual([60, 15]);
});

test("a 6 px font is drawn at the floor of 11 px at a pixel ratio of 1", async ({ page }) => {
  const el = await addChart(page, { style: "font-size: 6px" });
  // round(0.6 × 11) = 7 px cells: 600 / 7 → 85 columns (REQ-EL-085).
  expect((await sizeOf(el))[0]).toBe(85);
  expect(await el.locator("canvas").evaluate((c) => (c as HTMLCanvasElement).width)).toBe(85 * 7);
});

test.describe("at a pixel ratio of 2", () => {
  test.use({ deviceScaleFactor: 2 });

  test("the cell is round(0.6 × font size × ratio) device pixels, the remainder split as padding", async ({ page }) => {
    const el = await addChart(page);
    // round(0.6 × 16 × 2) = 19 device px: 1200 / 19 → 63 columns, 480 / 38 → 12 rows.
    expect(await sizeOf(el)).toEqual([63, 12]);
    const r = await el.evaluate((e) => {
      const c = e.querySelector("canvas") as HTMLCanvasElement;
      const f = e.querySelector(".gp-frame") as HTMLElement;
      return { w: c.width, h: c.height, left: f.style.left };
    });
    expect(r.w).toBe(63 * 19);
    expect(r.h).toBe(12 * 38);
    // (1200 − 1197) / 2 = 1 device px of padding each side: 0.5 CSS px.
    expect(r.left).toBe("0.5px");
  });
});
