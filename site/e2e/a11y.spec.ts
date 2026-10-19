// REQ-A11Y-070: an axe-core scan of the page holding every chart (test.html),
// in light and dark, finds no violation of impact serious or critical. With it
// REQ-A11Y-010 to 060: the figure and its image, the hidden table, the slider
// cursor, the control row, the focus outline and reduced motion.

import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page } from "@playwright/test";
import type { GlyphPlot } from "glyphplot/element";
import { addChart, frames, openBlank } from "./helpers.ts";

type El = GlyphPlot;

/** Opens test.html in `scheme` and scrolls through it so every lazy chart loads and draws its first frame. */
async function openChecklist(page: Page, scheme: "light" | "dark"): Promise<void> {
  await page.emulateMedia({ colorScheme: scheme });
  await page.goto("test.html");
  await page.waitForFunction(() => window.gpReady === true);
  const height = await page.evaluate(() => document.documentElement.scrollHeight);
  for (let y = 0; y <= height; y += 600) {
    await page.evaluate((y) => scrollTo(0, y), y);
    await frames(page, 2);
  }
  await page.evaluate(() => scrollTo(0, 0));
  // Every <glyph-plot> on the page has drawn (or, in the debug build, the panic case has failed as it should).
  await page.waitForFunction(() =>
    [...document.querySelectorAll("glyph-plot")].every((e) => (e as GlyphPlot).chart !== null || e.getAttribute("type") === "debug-panic"),
  );
}

for (const scheme of ["light", "dark"] as const) {
  test(`axe finds no serious or critical violation on test.html, ${scheme} @smoke`, async ({ page }) => {
    await openChecklist(page, scheme);
    const result = await new AxeBuilder({ page }).withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa", "best-practice"]).analyze();
    const bad = result.violations.filter((v) => v.impact === "serious" || v.impact === "critical");
    const report = bad.map((v) => `${v.impact} ${v.id}: ${v.help} (${v.nodes.length}): ${v.nodes.slice(0, 3).map((n) => n.target.join(" ")).join("; ")}`);
    expect(report, report.join("\n")).toEqual([]);
    // The page holds every chart drawn.
    expect(await page.locator("glyph-plot .gp-drawing").count()).toBeGreaterThanOrEqual(19);
  });
}

test.describe("on the bare page", () => {
  test.beforeEach(async ({ page }) => {
    await openBlank(page);
  });

  test("a figure named by label, else type and series; its drawing an img with a summary", async ({ page }) => {
    const named = await addChart(page, { attrs: { label: "Prices" } });
    await expect(named).toHaveAttribute("role", "figure");
    await expect(named).toHaveAccessibleName("Prices");
    const plain = await addChart(page, { data: { x: [0, 1, 2, 3], a: [1, 2, 3, 4], b: [4, 3, 2, 1] } });
    await expect(plain).toHaveAccessibleName("line: a, b");
    const img = plain.getByRole("img");
    await expect(img).toHaveCount(1);
    const summary = (await img.getAttribute("aria-label")) ?? "";
    expect(summary).toContain("line");
    expect(summary).toMatch(/\ba\b.*\bb\b/);
    // Rewritten after a view change, at most once a second.
    await plain.evaluate((e) => ((e as El).view = { x: { domain: [1, 2] } }));
    await expect.poll(() => img.getAttribute("aria-label"), { timeout: 2500 }).not.toBe(summary);
  });

  test("a hidden table: caption, column headers, a row header a row, at most 500 rows, the count stated once", async ({ page }) => {
    const el = await addChart(page, { n: 1000 });
    const table = el.locator("table");
    await expect(table).toHaveCount(1);
    expect(await el.locator(".gp-sr").evaluate((b) => b.getBoundingClientRect().width <= 1)).toBe(true);
    const caption = (await table.locator("caption").textContent()) ?? "";
    expect(caption.split("1,000").length - 1, caption).toBe(1);
    await expect(table.locator("thead th[scope=col]")).toHaveText(["x", "a"]);
    const rows = await table.locator("tbody tr").count();
    expect(rows).toBeGreaterThan(0);
    expect(rows).toBeLessThanOrEqual(500);
    expect(await table.locator("tbody tr > th[scope=row]:first-child").count()).toBe(rows);
    // A view holding few points: every one, and no count appended.
    const small = await addChart(page, { n: 40 });
    const c2 = (await small.locator("caption").textContent()) ?? "";
    expect(await small.locator("tbody tr").count()).toBe(40);
    expect(c2).not.toContain("zoom in for more");
  });

  test("one tab stop: a slider cursor with key shortcuts and a value text", async ({ page }) => {
    const el = await addChart(page);
    const tabbable = await el.evaluate(
      (e) => [...e.querySelectorAll<HTMLElement>("a[href], button, input, select, textarea, [tabindex]")].filter((n) => n.tabIndex >= 0).length,
    );
    expect(tabbable).toBe(1);
    const slider = el.getByRole("slider");
    await expect(slider).toHaveCount(1);
    const keys = (await slider.getAttribute("aria-keyshortcuts")) ?? "";
    for (const k of ["ArrowLeft", "ArrowRight", "Home", "End", "PageUp", "PageDown", "Shift+ArrowLeft", "+", "-", "0", "Enter", "Shift+Enter", "Escape", "Space", "1", "9", "?"]) {
      expect(keys.split(" ")).toContain(k);
    }
    await page.keyboard.press("Tab");
    await expect(slider).toBeFocused();
    await page.keyboard.press("ArrowRight");
    await expect(slider).toHaveAttribute("aria-valuetext", /^\d+, a -?\d/);
    await page.keyboard.press("Tab");
    await expect(slider).not.toBeFocused();
  });

  test("a control row shown on focus and hover, with five named buttons", async ({ page }) => {
    const el = await addChart(page);
    const group = el.getByRole("group", { name: "Chart controls" });
    const names = await group.getByRole("button").evaluateAll((bs) => bs.map((b) => b.getAttribute("aria-label")));
    expect(names).toEqual(["Zoom out", "Zoom in", "Reset view", "Step back", "Step forward"]);
    const opacity = (): Promise<string> => group.evaluate((g) => getComputedStyle(g).opacity);
    expect(await opacity()).toBe("0");
    await el.getByRole("slider").focus();
    await expect.poll(opacity).toBe("1");
    await el.getByRole("slider").blur();
    await expect.poll(opacity).toBe("0");
    await el.hover();
    await expect.poll(opacity).toBe("1");
  });

  test("the focused cursor shows a 2 px outline in the accent, at least 3:1 against the ground", async ({ page }) => {
    for (const theme of ["auto", "dark", "light"]) {
      const el = await addChart(page, { attrs: { theme } });
      // Focus by keyboard, so :focus-visible applies: the cursor is the page's one tab stop.
      await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur());
      await page.keyboard.press("Tab");
      const r = await el.evaluate((e) => {
        const c = e.querySelector(".gp-cursor") as HTMLElement;
        const s = getComputedStyle(c);
        const ground = getComputedStyle(e).getPropertyValue("--gp-ground");
        const painted = e.getAttribute("theme") !== "auto";
        return { focused: document.activeElement === c, width: s.outlineWidth, style: s.outlineStyle, color: s.outlineColor, accent: getComputedStyle(e).getPropertyValue("--gp-accent"), ground: painted ? ground : getComputedStyle(document.body).backgroundColor };
      });
      expect(r.focused).toBe(true);
      expect(r.style).toBe("solid");
      expect(Number.parseFloat(r.width)).toBeGreaterThanOrEqual(2);
      expect(rgb(r.color)).toEqual(rgb(r.accent));
      expect(contrast(rgb(r.color), rgb(r.ground)), `${theme}: ${r.color} on ${r.ground}`).toBeGreaterThanOrEqual(3);
      await el.evaluate((e) => e.remove());
    }
  });

  test("under reduced motion a view change jumps, unless the motion attribute is set", async ({ page }) => {
    const motionAfterReset = async (attrs: Record<string, string>): Promise<unknown> => {
      const el = await addChart(page, { attrs: { options: '{"motion":true}', ...attrs } });
      await el.evaluate((e) => ((e as El).view = { x: { domain: [50, 90] } }));
      await el.getByRole("slider").focus();
      await page.keyboard.press("0");
      await frames(page, 2);
      const m = await el.evaluate((e) => (e as El).view["motion"]);
      await el.evaluate((e) => e.remove());
      return m;
    };
    expect(await motionAfterReset({})).not.toBeNull();
    await page.emulateMedia({ reducedMotion: "reduce" });
    expect(await motionAfterReset({})).toBeNull();
    expect(await motionAfterReset({ motion: "" })).not.toBeNull();
  });
});

/** A CSS colour (#rrggbb or rgb()) as [r, g, b]. */
function rgb(css: string): [number, number, number] {
  const s = css.trim();
  const hex = /^#([0-9a-f]{6})$/i.exec(s);
  if (hex?.[1]) return [0, 2, 4].map((i) => Number.parseInt(hex[1]?.slice(i, i + 2) ?? "0", 16)) as [number, number, number];
  const m = /rgba?\(([^)]+)\)/.exec(s);
  const parts = (m?.[1] ?? "0,0,0").split(/[ ,/]+/).map(Number);
  return [parts[0] ?? 0, parts[1] ?? 0, parts[2] ?? 0];
}

/** WCAG 2.2 contrast ratio. */
function contrast(a: [number, number, number], b: [number, number, number]): number {
  const lum = (c: [number, number, number]): number => {
    const [r, g, bl] = c.map((v) => {
      const s = v / 255;
      return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
    }) as [number, number, number];
    return 0.2126 * r + 0.7152 * g + 0.0722 * bl;
  };
  const [hi, lo] = [lum(a), lum(b)].sort((x, y) => y - x) as [number, number];
  return (hi + 0.05) / (lo + 0.05);
}
