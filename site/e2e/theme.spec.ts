// REQ-THEME-100: under `forced-colors: active` the element draws in one ink,
// CanvasText on Canvas, with the accent in Highlight; under `@media print` it
// draws on paper. Emulated with page.emulateMedia in headless Chromium; the
// Windows high-contrast check is the owner's.

import { expect, test, type Locator, type Page } from "@playwright/test";
import type { GlyphPlot } from "glyphplot/element";
import { addChart, frames, openBlank } from "./helpers.ts";

type Rgb = [number, number, number];

/** Canvas, CanvasText and Highlight as the page resolves them now. */
async function system(page: Page): Promise<{ canvas: Rgb; ink: Rgb; highlight: Rgb }> {
  return page.evaluate(() => {
    const s = document.createElement("span");
    s.style.cssText = "position:absolute;forced-color-adjust:none;color:CanvasText;background-color:Canvas;border:1px solid Highlight";
    document.body.append(s);
    const cs = getComputedStyle(s);
    const rgb = (c: string): [number, number, number] => (c.match(/\d+/g) ?? []).slice(0, 3).map(Number) as [number, number, number];
    const out = { canvas: rgb(cs.backgroundColor), ink: rgb(cs.color), highlight: rgb(cs.borderTopColor) };
    s.remove();
    return out;
  });
}

/** The element's resolved token, as the shell sets it (--gp-ground, --gp-ink, --gp-accent). */
async function token(el: Locator, name: string): Promise<Rgb> {
  const hex = await el.evaluate((e, n) => (e as HTMLElement).style.getPropertyValue(`--gp-${n}`), name);
  return [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16)) as Rgb;
}

const luma = (c: Rgb): number => 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];

/**
 * The drawn pixels (alpha > 0) whose colour is not a blend of `ground` with
 * one of `inks` (anti-aliased glyph edges are such blends), more than `tol`
 * away in RGB; and how many pixels were drawn.
 */
async function offInk(el: Locator, ground: Rgb, inks: Rgb[], tol = 24): Promise<{ off: number; drawn: number }> {
  return el.locator("canvas.gp-drawing").evaluate(
    (cv, { ground, inks, tol }) => {
      const c = cv as HTMLCanvasElement;
      const d = (c.getContext("2d") as CanvasRenderingContext2D).getImageData(0, 0, c.width, c.height).data;
      const dist = (p: number[], a: number[], b: number[]): number => {
        const ab = [b[0]! - a[0]!, b[1]! - a[1]!, b[2]! - a[2]!];
        const ap = [p[0]! - a[0]!, p[1]! - a[1]!, p[2]! - a[2]!];
        const len = ab[0]! ** 2 + ab[1]! ** 2 + ab[2]! ** 2;
        const t = len === 0 ? 0 : Math.max(0, Math.min(1, (ap[0]! * ab[0]! + ap[1]! * ab[1]! + ap[2]! * ab[2]!) / len));
        return Math.hypot(ap[0]! - t * ab[0]!, ap[1]! - t * ab[1]!, ap[2]! - t * ab[2]!);
      };
      let off = 0;
      let drawn = 0;
      for (let i = 0; i < d.length; i += 4) {
        if (d[i + 3] === 0) continue;
        drawn++;
        const p = [d[i]!, d[i + 1]!, d[i + 2]!];
        if (inks.every((ink) => dist(p, ground, ink) > tol)) off++;
      }
      return { off, drawn };
    },
    { ground, inks, tol },
  );
}

/** Two series over 200 points, with the cursor shown so the accent is drawn. */
async function twoSeries(page: Page): Promise<Locator> {
  const el = await addChart(page, { data: twoSine(200) });
  const box = await el.locator(".gp-drawing").boundingBox();
  if (box) await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await frames(page, 3);
  return el;
}

function twoSine(n: number): Record<string, number[]> {
  const x: number[] = [];
  const a: number[] = [];
  const b: number[] = [];
  for (let i = 0; i < n; i++) {
    x.push(i);
    a.push(Math.sin(i / 10));
    b.push(Math.cos(i / 13));
  }
  return { x, a, b };
}

test.beforeEach(async ({ page }) => {
  await openBlank(page);
});

test("forced colours: CanvasText on Canvas, the accent in Highlight, every drawn pixel one ink or the highlight", async ({ page }) => {
  await page.emulateMedia({ forcedColors: "active" });
  const sys = await system(page);
  const el = await twoSeries(page);
  expect(await token(el, "ground")).toEqual(sys.canvas);
  expect(await token(el, "ink")).toEqual(sys.ink);
  expect(await token(el, "muted")).toEqual(sys.ink);
  expect(await token(el, "accent")).toEqual(sys.highlight);
  const { off, drawn } = await offInk(el, sys.canvas, [sys.ink, sys.highlight]);
  expect(drawn).toBeGreaterThan(1000);
  expect(off).toBe(0);
  // The ground is painted: a corner pixel is Canvas.
  const corner = await el.locator("canvas.gp-drawing").evaluate((cv) => [...((cv as HTMLCanvasElement).getContext("2d") as CanvasRenderingContext2D).getImageData(1, 1, 1, 1).data].slice(0, 3));
  expect(corner).toEqual(sys.canvas);
});

test("the same chart without forced colours draws colours other than one ink (the check above can fail)", async ({ page }) => {
  const el = await twoSeries(page);
  const ground = await token(el, "ground");
  const ink = await token(el, "ink");
  const accent = await token(el, "accent");
  const { off } = await offInk(el, ground, [ink, accent]);
  expect(off).toBeGreaterThan(100);
});

test("forced colours turning on and off re-theme a drawn chart in place, in one ink and back", async ({ page }) => {
  const el = await twoSeries(page);
  const before = await el.evaluate((e) => (e as GlyphPlot).text());
  const chart = await el.evaluate((e) => (e as GlyphPlot).chart !== null);
  expect(chart).toBe(true);
  const colourInk = await token(el, "ink");
  await page.emulateMedia({ forcedColors: "active" });
  const sys = await system(page);
  await expect.poll(() => token(el, "ink")).toEqual(sys.ink);
  await expect.poll(() => token(el, "accent")).toEqual(sys.highlight);
  // One ink: series told apart by dashes and marks, so the frame's text changes.
  await expect.poll(() => el.evaluate((e) => (e as GlyphPlot).text())).not.toBe(before);
  await frames(page, 3);
  expect((await offInk(el, sys.canvas, [sys.ink, sys.highlight])).off).toBe(0);
  await page.emulateMedia({ forcedColors: "none" });
  await expect.poll(() => token(el, "ink")).toEqual(colourInk);
  await expect.poll(() => el.evaluate((e) => (e as GlyphPlot).text())).toBe(before);
});

test("forced colours: the focus ring is Highlight", async ({ page }) => {
  await page.emulateMedia({ forcedColors: "active" });
  const sys = await system(page);
  const el = await addChart(page, { n: 100 });
  const cursor = el.getByRole("slider");
  await page.keyboard.press("Tab");
  await expect(cursor).toBeFocused();
  const outline = await cursor.evaluate((c) => getComputedStyle(c).outlineColor);
  expect((outline.match(/\d+/g) ?? []).slice(0, 3).map(Number)).toEqual(sys.highlight);
});

test("print: a dark chart draws on paper, dark ink on a light painted ground, and goes back after", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "dark" });
  const el = await addChart(page, { n: 200, attrs: { theme: "dark" } });
  await frames(page, 2);
  const darkGround = await token(el, "ground");
  expect(luma(darkGround)).toBeLessThan(80);
  await page.emulateMedia({ media: "print" });
  await expect.poll(async () => luma(await token(el, "ground"))).toBeGreaterThan(220);
  expect(luma(await token(el, "ink"))).toBeLessThan(80);
  await frames(page, 2);
  const ground = await token(el, "ground");
  const corner = await el.locator("canvas.gp-drawing").evaluate((cv) => [...((cv as HTMLCanvasElement).getContext("2d") as CanvasRenderingContext2D).getImageData(1, 1, 1, 1).data].slice(0, 3));
  expect(corner).toEqual(ground);
  // No control row on paper.
  expect(await el.locator(".gp-controls").evaluate((c) => getComputedStyle(c).display)).toBe("none");
  await page.emulateMedia({ media: "screen" });
  await expect.poll(() => token(el, "ground")).toEqual(darkGround);
});

test("print: beforeprint themes for paper at once, afterprint restores", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "dark" });
  const el = await addChart(page, { n: 200, attrs: { theme: "dark" } });
  const dark = await token(el, "ground");
  await page.evaluate(() => dispatchEvent(new Event("beforeprint")));
  expect(luma(await token(el, "ground"))).toBeGreaterThan(220);
  await page.evaluate(() => dispatchEvent(new Event("afterprint")));
  expect(await token(el, "ground")).toEqual(dark);
});
