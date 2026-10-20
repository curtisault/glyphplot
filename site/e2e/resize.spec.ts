// REQ-E2E-040: a change of container width changes `cols`, keeps the view in
// data units and logs no console error; with it REQ-EL-080 to 100 (the cell
// from the font, the font floor, a relayout within a frame, width 0 ignored,
// 12 rows without a height), and REQ-EL-110 (`fit` with fixed `cols` and
// `rows`).

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

test.describe("fit with cols and rows (REQ-EL-110)", () => {
  /** The frame's box in CSS px, the canvas's backing store, and the grid. */
  const layout = (el: import("@playwright/test").Locator): Promise<{ left: number; top: number; width: number; height: number; canvas: [number, number]; grid: [number, number]; box: number }> =>
    el.evaluate((e) => {
      const f = e.querySelector(".gp-frame") as HTMLElement;
      const c = e.querySelector("canvas") as HTMLCanvasElement;
      const r = f.getBoundingClientRect();
      const b = e.getBoundingClientRect();
      return { left: r.left - b.left, top: r.top - b.top, width: r.width, height: r.height, canvas: [c.width, c.height], grid: [(e as El).cols, (e as El).rows], box: b.height };
    });

  test("no fit: the grid is cols × rows of the font's cell, centred", async ({ page }) => {
    const el = await addChart(page, { attrs: { cols: "40", rows: "10" } });
    // 16 px: a 10 × 20 px cell; 400 × 200 in a 600 px box, 100 px each side.
    expect(await layout(el)).toEqual({ left: 100, top: 0, width: 400, height: 200, canvas: [400, 200], grid: [40, 10], box: 200 });
    // Any other value acts as no fit.
    await el.evaluate((e) => e.setAttribute("fit", "cover"));
    await frames(page, 3);
    expect(await layout(el)).toEqual({ left: 100, top: 0, width: 400, height: 200, canvas: [400, 200], grid: [40, 10], box: 200 });
  });

  test("width: a cell is W / cols wide, the height following, the grid unchanged", async ({ page }) => {
    const errors = consoleErrors(page);
    const el = await addChart(page, { attrs: { cols: "40", rows: "10", fit: "width" } });
    await frames(page, 3);
    // 600 / 40 = 15 px cells, 30 px tall: 600 × 300, the element's own height following.
    expect(await layout(el)).toEqual({ left: 0, top: 0, width: 600, height: 300, canvas: [600, 300], grid: [40, 10], box: 300 });
    await page.evaluate(() => ((document.getElementById("stage") as HTMLElement).style.width = "480px"));
    await frames(page, 3);
    // 480 / 40 = 12 px cells.
    expect(await layout(el)).toEqual({ left: 0, top: 0, width: 480, height: 240, canvas: [480, 240], grid: [40, 10], box: 240 });
    expect(errors).toEqual([]);
  });

  test("contain: the largest 1:2 cell inside W × H, centred; as width when H is 0", async ({ page }) => {
    const el = await addChart(page, { attrs: { cols: "40", rows: "10", fit: "contain" }, style: "height: 150px" });
    await frames(page, 3);
    // min(600 / 40, 150 / 20) = 7.5 px: 300 × 150, centred 150 px from the left;
    // the backing store in whole cells of round(7.5) = 8 px, scaled.
    expect(await layout(el)).toEqual({ left: 150, top: 0, width: 300, height: 150, canvas: [320, 160], grid: [40, 10], box: 150 });
    // A box wider than its height allows: min(600 / 40, 400 / 20) = 15 px, centred 50 px from the top.
    await el.evaluate((e) => ((e as HTMLElement).style.height = "400px"));
    await frames(page, 3);
    expect(await layout(el)).toEqual({ left: 0, top: 50, width: 600, height: 300, canvas: [600, 300], grid: [40, 10], box: 400 });
    // H = 0 acts as width.
    await el.evaluate((e) => ((e as HTMLElement).style.height = "0px"));
    await frames(page, 3);
    const zero = await layout(el);
    expect([zero.left, zero.top, zero.width, zero.height]).toEqual([0, 0, 600, 300]);
    // Without a height of its own it takes the width's height, so it acts as width too.
    await el.evaluate((e) => (e as HTMLElement).style.removeProperty("height"));
    await frames(page, 3);
    expect(await layout(el)).toEqual({ left: 0, top: 0, width: 600, height: 300, canvas: [600, 300], grid: [40, 10], box: 300 });
  });

  test("fit needs both cols and rows", async ({ page }) => {
    const el = await addChart(page, { attrs: { cols: "40", fit: "width" } });
    // cols alone is no fixed grid: the font's cell fills the box (60 × 12).
    expect((await layout(el)).grid).toEqual([60, 12]);
  });
});
