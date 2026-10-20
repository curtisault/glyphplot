// REQ-LINK-010: charts sharing a `link` value share `x.domain` and `cursor.x`
// in data units, without event loops: two linked charts of different widths
// agree after a pan, a hover and a zoom; a chart of another group, or of
// none, keeps its own view.

import { expect, test } from "@playwright/test";
import type { GlyphPlot } from "glyphplot/element";
import { addChart, consoleErrors, domain, drawingBox, frames, openBlank, takeEvents } from "./helpers.ts";

type El = GlyphPlot;

test.beforeEach(async ({ page }) => {
  await openBlank(page);
});

const cursorX = (el: import("@playwright/test").Locator): Promise<number | null> =>
  el.evaluate((e) => ((e as El).view["cursor"] as { x?: number | null } | null)?.x ?? null);

test("two linked charts of different widths agree after a pan, with no event loop @smoke", async ({ page }) => {
  const errors = consoleErrors(page);
  // Four charts of 8 rows (160 px each) fit the 800 px viewport: a chart draws only while on screen.
  const a = await addChart(page, { attrs: { link: "prices", rows: "8" }, style: "width: 400px" });
  const b = await addChart(page, { attrs: { link: "prices", rows: "8" }, style: "width: 760px" });
  const other = await addChart(page, { attrs: { link: "volumes", rows: "8" }, style: "width: 500px" });
  const none = await addChart(page, { attrs: { rows: "8" }, style: "width: 500px" });
  expect(await a.evaluate((e) => (e as El).cols)).not.toBe(await b.evaluate((e) => (e as El).cols));
  for (const el of [a, b, other, none]) await takeEvents(el);

  // Drag-pan the narrow chart.
  const box = await drawingBox(a);
  await page.mouse.move(box.x + box.width * 0.7, box.y + box.height * 0.5);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width * 0.3, box.y + box.height * 0.5, { steps: 8 });
  await page.mouse.up();
  await frames(page, 3);
  const da = await domain(a);
  expect(da).not.toBeNull();
  expect(da?.[0]).toBeGreaterThan(0);
  expect(await domain(b)).toEqual(da);
  // Another group and no group keep their fitted views.
  expect(await domain(other)).toBeNull();
  expect(await domain(none)).toBeNull();
  // The pan's events are the dragged chart's; the linked one fires none, so nothing echoes back.
  expect((await takeEvents(a, "plot-view")).filter((e) => e.detail?.["final"] === true)).toHaveLength(1);
  expect(await takeEvents(b, "plot-view")).toHaveLength(0);

  // A hover on the wide chart puts the narrow one's cursor at the same x, in data units.
  const wide = await drawingBox(b);
  await page.mouse.move(wide.x + wide.width * 0.5, wide.y + wide.height * 0.5);
  await frames(page, 3);
  const hovered = (await takeEvents(b, "plot-hover")).at(-1)?.detail?.["x"] as number;
  expect(typeof hovered).toBe("number");
  expect(await cursorX(a)).toBe(hovered);
  expect(await cursorX(other)).toBeNull();
  // The narrow chart's only hover is its own leave as the pointer left it: the shared cursor fired none.
  expect((await takeEvents(a, "plot-hover")).filter((e) => e.detail !== null)).toHaveLength(0);
  // Leaving clears the peer's cursor too.
  await page.mouse.move(wide.x + wide.width / 2, wide.y + wide.height + 300);
  await frames(page, 3);
  expect(await cursorX(a)).toBeNull();

  // A keyboard zoom on the wide chart: the narrow one follows.
  await b.locator(".gp-cursor").focus();
  await page.keyboard.press("+");
  await frames(page, 3);
  const zoomed = await domain(b);
  expect(zoomed).not.toEqual(da);
  expect(await domain(a)).toEqual(zoomed);
  expect((await takeEvents(b, "plot-view")).at(-1)?.detail).toMatchObject({ source: "keyboard", cause: "zoom", final: true });
  expect(await takeEvents(a, "plot-view")).toHaveLength(0);
  // A view set by script on one is shared too, and fires nothing on either.
  await a.evaluate((e) => ((e as El).view = { x: { domain: [10, 50] } }));
  await frames(page, 2);
  expect(await domain(b)).toEqual([10, 50]);
  expect(await takeEvents(a, "plot-view")).toHaveLength(0);
  expect(await takeEvents(b, "plot-view")).toHaveLength(0);
  expect(errors).toEqual([]);
});

test("a chart joining a group takes the group's x domain; leaving it ends the link", async ({ page }) => {
  const a = await addChart(page, { attrs: { link: "g" } });
  await a.evaluate((e) => ((e as El).view = { x: { domain: [20, 60] } }));
  const b = await addChart(page, { attrs: { link: "g" }, style: "width: 300px" });
  expect(await domain(b)).toEqual([20, 60]);
  await b.evaluate((e) => e.removeAttribute("link"));
  await a.evaluate((e) => ((e as El).view = { x: { domain: [0, 10] } }));
  await frames(page, 2);
  expect(await domain(b)).toEqual([20, 60]);
});
