// REQ-A11Y-110: a "show data" control makes the table visible. The control
// row's "Show data" toggle (aria-pressed), T on the cursor and the showData
// property show the table over the drawing as a labelled, focusable region;
// Escape in it hides it and returns focus to the cursor. Hidden, the table
// is visually hidden as REQ-A11Y-020 says, and the chart keeps one tab stop
// (REQ-A11Y-030).

import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Locator } from "@playwright/test";
import type { GlyphPlot } from "glyphplot/element";
import { addChart, frames, openBlank } from "./helpers.ts";

/** The table box's rendered width, CSS px. */
const boxWidth = (el: Locator): Promise<number> => el.locator("table").evaluate((t) => (t.parentElement as HTMLElement).getBoundingClientRect().width);

test.beforeEach(async ({ page }) => {
  await openBlank(page);
});

test("T on the cursor shows the table as a region over the drawing; Escape hides it and returns to the cursor", async ({ page }) => {
  const el = await addChart(page, { n: 100 });
  const toggle = el.getByRole("button", { name: "Show data" });
  await expect(toggle).toHaveAttribute("aria-pressed", "false");
  expect(await boxWidth(el)).toBeLessThanOrEqual(1);
  await expect(el.getByRole("region", { name: "Chart data" })).toHaveCount(0);

  const cursor = el.getByRole("slider");
  await cursor.focus();
  await page.keyboard.press("t");
  const region = el.getByRole("region", { name: "Chart data" });
  await expect(region).toBeVisible();
  await expect(toggle).toHaveAttribute("aria-pressed", "true");
  expect(await boxWidth(el)).toBeGreaterThan(300);
  // The table is readable on the screen: its caption and first row header are inside the element's box and visible.
  await expect(region.locator("caption")).toBeVisible();
  await expect(region.locator("tbody tr").first().locator("th[scope=row]")).toBeVisible();
  expect(await el.evaluate((e) => (e as GlyphPlot).showData)).toBe(true);

  // The region scrolls and takes focus; Escape there hides it and focuses the cursor.
  await region.focus();
  await page.keyboard.press("PageDown");
  await expect.poll(() => region.evaluate((r) => r.scrollTop)).toBeGreaterThan(0);
  await page.keyboard.press("Escape");
  await expect(el.getByRole("region", { name: "Chart data" })).toHaveCount(0);
  await expect(toggle).toHaveAttribute("aria-pressed", "false");
  await expect(cursor).toBeFocused();
  expect(await boxWidth(el)).toBeLessThanOrEqual(1);
  // T again, and again: shown, hidden.
  await page.keyboard.press("T");
  await expect(el.getByRole("region", { name: "Chart data" })).toBeVisible();
  await page.keyboard.press("t");
  await expect(el.getByRole("region", { name: "Chart data" })).toHaveCount(0);
  // Ctrl+T is the browser's, not the chart's.
  await page.keyboard.press("Control+t");
  await expect(el.getByRole("region", { name: "Chart data" })).toHaveCount(0);
});

test("the control row's Show data toggle shows and hides the table with a click; it is labelled and pressed", async ({ page }) => {
  const el = await addChart(page, { n: 50 });
  await el.hover();
  const toggle = el.getByRole("button", { name: "Show data" });
  await expect(toggle).toBeVisible();
  await expect(toggle).toHaveAttribute("title", "Show data (T)");
  await expect(toggle).toHaveAttribute("aria-controls", /^gp-table-\d+$/);
  const controlled = await toggle.getAttribute("aria-controls");
  await toggle.click();
  const region = el.getByRole("region", { name: "Chart data" });
  await expect(region).toBeVisible();
  expect(await region.getAttribute("id")).toBe(controlled);
  // Every point of a small view, each a row.
  await expect(region.locator("tbody tr")).toHaveCount(50);
  // The toggle stays above the table, so the same click hides it.
  await toggle.click();
  await expect(el.getByRole("region", { name: "Chart data" })).toHaveCount(0);
  // The property does the same.
  await el.evaluate((e) => ((e as GlyphPlot).showData = true));
  await expect(el.getByRole("region", { name: "Chart data" })).toBeVisible();
  await el.evaluate((e) => ((e as GlyphPlot).showData = false));
  await expect(el.getByRole("region", { name: "Chart data" })).toHaveCount(0);
});

test("hidden, the chart has one tab stop; shown, the table is the only other one", async ({ page }) => {
  await page.evaluate(() => {
    const before = document.createElement("button");
    before.id = "before";
    before.textContent = "before";
    const after = document.createElement("button");
    after.id = "after";
    after.textContent = "after";
    document.querySelector("#stage")?.before(before);
    document.querySelector("#stage")?.after(after);
  });
  const el = await addChart(page, { n: 50 });
  await page.locator("#before").focus();
  await page.keyboard.press("Tab");
  await expect(el.getByRole("slider")).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(page.locator("#after")).toBeFocused();
  await el.evaluate((e) => ((e as GlyphPlot).showData = true));
  await page.locator("#before").focus();
  await page.keyboard.press("Tab");
  await expect(el.getByRole("slider")).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(el.getByRole("region", { name: "Chart data" })).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(page.locator("#after")).toBeFocused();
});

test("the shown table keeps the view's data as it changes, and axe finds no serious or critical violation", async ({ page }) => {
  const el = await addChart(page, { n: 1000 });
  await el.evaluate((e) => ((e as GlyphPlot).showData = true));
  const region = el.getByRole("region", { name: "Chart data" });
  await expect(region).toBeVisible();
  const caption = (await region.locator("caption").textContent()) ?? "";
  expect(caption).toContain("1,000");
  await el.evaluate((e) => ((e as GlyphPlot).view = { x: { domain: [10, 20] } }));
  await expect.poll(() => region.locator("tbody tr").count(), { timeout: 2500 }).toBeLessThanOrEqual(11);
  await frames(page, 2);
  for (const scheme of ["light", "dark"] as const) {
    await page.emulateMedia({ colorScheme: scheme });
    await frames(page, 2);
    const result = await new AxeBuilder({ page }).include("#stage").withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa", "best-practice"]).analyze();
    const bad = result.violations.filter((v) => v.impact === "serious" || v.impact === "critical");
    const report = bad.map((v) => `${v.impact} ${v.id}: ${v.help} (${v.nodes.length}): ${v.nodes.slice(0, 3).map((n) => n.target.join(" ")).join("; ")}`);
    expect(report, report.join("\n")).toEqual([]);
  }
});
