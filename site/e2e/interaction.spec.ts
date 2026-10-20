// REQ-E2E-020: pointer hover with its event, drag pan, wheel zoom, keyboard
// cursor, pan, zoom, reset and Escape, a brush, and touch scrub, pan and
// pinch through CDP Input.dispatchTouchEvent; with them REQ-EVENT-010 to 030,
// REQ-INPUT-040, REQ-KEY-010, REQ-TOUCH-010 and 020, REQ-A11Y-040; and the
// v0.5.0 events, `plot-brush` and `plot-legend`, in events.schema.json's
// shapes.

import { expect, test } from "@playwright/test";
import { addChart, consoleErrors, domain, drawingBox, frames, openBlank, sine, takeEvents, text, Touch } from "./helpers.ts";

const SOURCES = ["pointer", "touch", "keyboard", "wheel"];
const isRange = (v: unknown): boolean => Array.isArray(v) && v.length === 2 && v.every((n) => typeof n === "number");

/** events.schema.json's `plot-brush` detail: exactly its keys, of its types. */
function expectBrush(d: Record<string, unknown> | null): void {
  expect(d).not.toBeNull();
  expect(Object.keys(d ?? {}).sort()).toEqual(["count", "final", "source", "x", "y"]);
  expect(SOURCES).toContain(d?.["source"]);
  expect(d?.["x"] === null || isRange(d?.["x"])).toBe(true);
  expect(d?.["y"] === null || isRange(d?.["y"])).toBe(true);
  expect(Number.isInteger(d?.["count"]) && (d?.["count"] as number) >= 0).toBe(true);
  expect(typeof d?.["final"]).toBe("boolean");
}

/** events.schema.json's `plot-legend` detail: exactly its keys, of its types. */
function expectLegend(d: Record<string, unknown> | null): void {
  expect(d).not.toBeNull();
  expect(Object.keys(d ?? {}).sort()).toEqual(["hidden", "series", "source", "visible"]);
  expect(SOURCES).toContain(d?.["source"]);
  expect(typeof d?.["series"]).toBe("string");
  expect(typeof d?.["visible"]).toBe("boolean");
  const hidden = d?.["hidden"] as unknown[];
  expect(Array.isArray(hidden) && hidden.every((h) => typeof h === "string") && new Set(hidden).size === hidden.length).toBe(true);
}

test.beforeEach(async ({ page }) => {
  await openBlank(page);
});

test("hover fires plot-hover once a datum, with the datum, and null on leave @smoke", async ({ page }) => {
  const errors = consoleErrors(page);
  const el = await addChart(page);
  const b = await drawingBox(el);
  await page.mouse.move(b.x + b.width * 0.5, b.y + b.height * 0.4);
  await frames(page, 3);
  let hovers = await takeEvents(el, "plot-hover");
  expect(hovers).toHaveLength(1);
  const d = hovers[0]?.detail as { source: string; series: string; index: number; x: number; y: number; datum: Record<string, number> };
  expect(d.source).toBe("pointer");
  expect(d.series).toBe("a");
  expect(d.datum["x"]).toBe(d.x);
  expect(d.y).toBeCloseTo(Math.sin(d.index / 10), 9);
  // A move within the same datum fires nothing (REQ-EVENT-020).
  await page.mouse.move(b.x + b.width * 0.5 + 0.5, b.y + b.height * 0.4);
  await frames(page, 3);
  expect(await takeEvents(el, "plot-hover")).toHaveLength(0);
  // Many moves in one frame: at most one plot-hover a frame.
  for (let i = 1; i <= 10; i++) await page.mouse.move(b.x + b.width * (0.5 + i * 0.02), b.y + b.height * 0.4);
  await frames(page, 3);
  hovers = await takeEvents(el, "plot-hover");
  expect(hovers.length).toBeGreaterThanOrEqual(1);
  expect(hovers.length).toBeLessThanOrEqual(10);
  await page.mouse.move(b.x + b.width / 2, b.y + b.height + 200);
  await frames(page, 3);
  hovers = await takeEvents(el, "plot-hover");
  expect(hovers.at(-1)?.detail).toBeNull();
  expect(errors).toEqual([]);
});

test("a drag pans: plot-view steps with final false, then exactly one final true @smoke", async ({ page }) => {
  const el = await addChart(page);
  const b = await drawingBox(el);
  await page.mouse.move(b.x + b.width * 0.6, b.y + b.height * 0.5);
  await page.mouse.down();
  await page.mouse.move(b.x + b.width * 0.4, b.y + b.height * 0.5, { steps: 8 });
  await page.mouse.up();
  await frames(page, 3);
  const views = await takeEvents(el, "plot-view");
  const finals = views.filter((v) => v.detail?.["final"] === true);
  expect(finals).toHaveLength(1);
  expect(views.at(-1)?.detail?.["final"]).toBe(true);
  expect(views.length).toBeGreaterThan(1);
  for (const v of views) expect(v.detail).toMatchObject({ source: "pointer", cause: "pan" });
  const d = await domain(el);
  expect(d).not.toBeNull();
  // Dragged leftwards: the view moved to later x, keeping its span.
  expect(d?.[0]).toBeGreaterThan(0);
  expect((d?.[1] ?? 0) - (d?.[0] ?? 0)).toBeCloseTo(199, 0);
});

test("Ctrl+wheel zooms about the pointer with one final; the plain wheel scrolls the page @smoke", async ({ page }) => {
  await page.evaluate(() => document.body.style.setProperty("min-height", "300vh"));
  const el = await addChart(page);
  const b = await drawingBox(el);
  const px = b.x + b.width * 0.75;
  await page.mouse.move(px, b.y + b.height * 0.5);
  await frames(page, 2);
  await takeEvents(el);
  const before = await page.evaluate(() => scrollY);
  await page.mouse.wheel(0, 120);
  await page.waitForTimeout(250);
  expect(await page.evaluate(() => scrollY)).toBeGreaterThan(before);
  expect(await takeEvents(el, "plot-view")).toHaveLength(0);
  expect(await domain(el)).toBeNull();
  await page.evaluate(() => scrollTo(0, 0));
  await frames(page, 2);
  const b2 = await drawingBox(el);
  await page.mouse.move(b2.x + b2.width * 0.75, b2.y + b2.height * 0.5);
  await page.keyboard.down("Control");
  for (let i = 0; i < 3; i++) await page.mouse.wheel(0, -100);
  await page.keyboard.up("Control");
  // The wheel is final after 150 ms of rest (REQ-EVENT-030).
  await page.waitForTimeout(400);
  const d = await domain(el);
  expect(d).not.toBeNull();
  const [lo, hi] = d as [number, number];
  expect(hi - lo).toBeLessThan(199);
  // About the pointer: the x under it (about 3/4 across the plot) stays inside the zoomed view, right of centre.
  expect(lo).toBeGreaterThan(0);
  expect(hi).toBeLessThanOrEqual(200);
  const views = await takeEvents(el, "plot-view");
  expect(views.filter((v) => v.detail?.["final"] === true)).toHaveLength(1);
  for (const v of views) expect(v.detail).toMatchObject({ source: "wheel", cause: "zoom" });
  // The next input does not end the zoom a second time.
  await page.mouse.move(b2.x + b2.width * 0.3, b2.y + b2.height * 0.5);
  await frames(page, 3);
  expect((await takeEvents(el, "plot-view")).filter((v) => v.detail?.["final"] === true)).toHaveLength(0);
});

test("keys move the cursor, pan, zoom, reset, and Escape hides then blurs @smoke", async ({ page }) => {
  const el = await addChart(page);
  const cursor = el.locator(".gp-cursor");
  await cursor.focus();
  await page.keyboard.press("ArrowRight");
  await frames(page, 2);
  let hover = (await takeEvents(el, "plot-hover")).at(-1)?.detail;
  expect(hover).toMatchObject({ source: "keyboard", series: "a" });
  const first = hover?.["index"] as number;
  await page.keyboard.press("ArrowRight");
  await frames(page, 2);
  hover = (await takeEvents(el, "plot-hover")).at(-1)?.detail;
  expect(hover?.["index"]).toBe(first + 1);
  await expect(cursor).toHaveAttribute("aria-valuetext", new RegExp(`^${first + 1}, a `));

  await page.keyboard.press("+");
  await frames(page, 2);
  let views = await takeEvents(el, "plot-view");
  expect(views.at(-1)?.detail).toMatchObject({ source: "keyboard", cause: "zoom", final: true });
  const zoomed = (await domain(el)) as [number, number];
  expect(zoomed[1] - zoomed[0]).toBeLessThan(199);

  await page.keyboard.press("Shift+ArrowRight");
  await frames(page, 2);
  views = await takeEvents(el, "plot-view");
  expect(views.at(-1)?.detail).toMatchObject({ source: "keyboard", cause: "pan", final: true });
  const panned = (await domain(el)) as [number, number];
  expect(panned[0]).toBeGreaterThan(zoomed[0]);
  expect(panned[1] - panned[0]).toBeCloseTo(zoomed[1] - zoomed[0], 6);

  await page.keyboard.press("0");
  await frames(page, 2);
  views = await takeEvents(el, "plot-view");
  expect(views.at(-1)?.detail).toMatchObject({ source: "keyboard", cause: "reset", final: true });
  expect(await domain(el)).toBeNull();

  await page.keyboard.press("Escape");
  await frames(page, 2);
  expect((await takeEvents(el, "plot-hover")).at(-1)?.detail).toBeNull();
  expect(await el.evaluate((e) => (e as unknown as { view: { cursor: unknown } }).view.cursor)).toBeNull();
  await expect(cursor).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(cursor).not.toBeFocused();
});

test("a brush: Shift+drag, and Shift+Enter twice from the keyboard", async ({ page }) => {
  const el = await addChart(page);
  const b = await drawingBox(el);
  await page.keyboard.down("Shift");
  await page.mouse.move(b.x + b.width * 0.3, b.y + b.height * 0.5);
  await page.mouse.down();
  await page.mouse.move(b.x + b.width * 0.6, b.y + b.height * 0.5, { steps: 6 });
  await page.mouse.up();
  await page.keyboard.up("Shift");
  await frames(page, 3);
  let brushes = await takeEvents(el, "plot-brush");
  expect(brushes.length).toBeGreaterThan(0);
  for (const b of brushes) expectBrush(b.detail);
  expect(brushes.every((b) => b.detail?.["source"] === "pointer")).toBe(true);
  expect(brushes.at(-1)?.detail?.["count"]).toBeGreaterThan(0);
  expect(brushes.filter((e) => e.detail?.["final"] === true)).toHaveLength(1);
  const x = brushes.at(-1)?.detail?.["x"] as [number, number];
  expect(x[0]).toBeLessThan(x[1]);
  expect(await el.evaluate((e) => (e as unknown as { view: { selection: { x: unknown } } }).view.selection.x)).toEqual(x);
  // The view did not pan.
  expect(await domain(el)).toBeNull();

  await el.locator(".gp-cursor").focus();
  await page.keyboard.press("Home");
  await page.keyboard.press("Shift+Enter");
  await page.keyboard.press("ArrowRight");
  await page.keyboard.press("ArrowRight");
  await page.keyboard.press("ArrowRight");
  await page.keyboard.press("Shift+Enter");
  await frames(page, 2);
  brushes = await takeEvents(el, "plot-brush");
  for (const b of brushes) expectBrush(b.detail);
  expect(brushes.at(-1)?.detail).toMatchObject({ source: "keyboard", final: true, count: 4 });
  // Escape clears the brush: x and y null, count 0, final.
  await page.keyboard.press("Escape");
  await frames(page, 2);
  brushes = await takeEvents(el, "plot-brush");
  expect(brushes).toHaveLength(1);
  expectBrush(brushes[0]?.detail ?? null);
  expect(brushes[0]?.detail).toEqual({ source: "keyboard", x: null, y: null, count: 0, final: true });
});

test("plot-legend: a key toggles the nth series, a click on its legend entry toggles it", async ({ page }) => {
  const errors = consoleErrors(page);
  const el = await addChart(page, { data: sine(100, true) });
  await el.locator(".gp-cursor").focus();
  await page.keyboard.press("2");
  await frames(page, 2);
  let legends = await takeEvents(el, "plot-legend");
  expect(legends).toHaveLength(1);
  expectLegend(legends[0]?.detail ?? null);
  expect(legends[0]?.detail).toEqual({ source: "keyboard", series: "b", visible: false, hidden: ["b"] });
  expect(await el.evaluate((e) => (e as unknown as { view: { hidden: string[] } }).view.hidden)).toEqual(["b"]);
  await page.keyboard.press("2");
  await frames(page, 2);
  expect((await takeEvents(el, "plot-legend"))[0]?.detail).toEqual({ source: "keyboard", series: "b", visible: true, hidden: [] });
  expect(errors).toEqual([]);
});

// A mouse click on a legend entry toggles its series once: on release
// (reduce.rs `end_drag`, DragKind::Legend); the tap the shell sends after a
// still click toggles nothing more (reduce.rs `Input::Tap` toggles for a
// finger only).
test("plot-legend: a click on a legend entry toggles its series once", async ({ page }) => {
  const errors = consoleErrors(page);
  const el = await addChart(page, { data: sine(100, true) });
  // The legend's first row: "─ a  ─ b"; a click on the cell holding "a".
  const first = (await text(el)).split("\n")[0] ?? "";
  const col = first.indexOf("a");
  expect(col).toBeGreaterThanOrEqual(0);
  const cols = await el.evaluate((e) => (e as unknown as { cols: number }).cols);
  const b = await drawingBox(el);
  const cell = b.width / cols;
  await page.mouse.click(b.x + (col + 0.5) * cell, b.y + cell); // row 0 is one cell wide and two tall: its middle
  await frames(page, 2);
  const legends = await takeEvents(el, "plot-legend");
  expectLegend(legends[0]?.detail ?? null);
  expect(legends[0]?.detail).toEqual({ source: "pointer", series: "a", visible: false, hidden: ["a"] });
  expect(errors).toEqual([]);
  // One plot-legend: the tap after the click toggles nothing more.
  expect(legends).toHaveLength(1);
  expect(await el.evaluate((e) => (e as unknown as { view: { hidden: string[] } }).view.hidden)).toEqual(["a"]);
});

test("the control row zooms, steps and resets with single clicks", async ({ page }) => {
  const el = await addChart(page);
  const b = await drawingBox(el);
  await page.mouse.move(b.x + b.width / 2, b.y + b.height / 2);
  const zoomIn = el.getByRole("button", { name: "Zoom in" });
  await expect(zoomIn).toBeVisible();
  await zoomIn.click();
  await frames(page, 2);
  const d = (await domain(el)) as [number, number];
  expect(d[1] - d[0]).toBeLessThan(199);
  await el.getByRole("button", { name: "Zoom out" }).click();
  await frames(page, 2);
  const d2 = (await domain(el)) as [number, number] | null;
  expect(d2 === null || d2[1] - d2[0] > d[1] - d[0]).toBe(true);
  await el.getByRole("button", { name: "Step forward" }).click();
  await frames(page, 2);
  expect((await takeEvents(el, "plot-hover")).at(-1)?.detail).toMatchObject({ source: "keyboard" });
  await el.getByRole("button", { name: "Reset view" }).click();
  await frames(page, 2);
  expect(await domain(el)).toBeNull();
});

test("a double-click resets the view", async ({ page }) => {
  const el = await addChart(page);
  await el.evaluate((e) => ((e as unknown as { view: unknown }).view = { x: { domain: [50, 90] } }));
  const b = await drawingBox(el);
  await page.mouse.dblclick(b.x + b.width / 2, b.y + b.height / 2);
  await frames(page, 3);
  expect(await domain(el)).toBeNull();
  const reset = (await takeEvents(el, "plot-view")).filter((v) => v.detail?.["cause"] === "reset");
  expect(reset).toHaveLength(1);
});

test("a double-click's reset names the pointer as its source", async ({ page }) => {
  // The tap carries its pointer type in `buttons` (256 × pointer, mouse 0), so the reset is the mouse's (REQ-INPUT-020).
  const el = await addChart(page);
  await el.evaluate((e) => ((e as unknown as { view: unknown }).view = { x: { domain: [50, 90] } }));
  const b = await drawingBox(el);
  await page.mouse.dblclick(b.x + b.width / 2, b.y + b.height / 2);
  await frames(page, 3);
  const reset = (await takeEvents(el, "plot-view")).filter((v) => v.detail?.["cause"] === "reset");
  expect(reset[0]?.detail?.["source"]).toBe("pointer");
});

test.describe("touch", () => {
  test.use({ hasTouch: true });

  test("one finger scrubs the cursor (Pointer Events, source touch)", async ({ page }) => {
    const el = await addChart(page);
    const b = await drawingBox(el);
    const t = await Touch.on(page);
    const y = b.y + b.height * 0.5;
    await t.drag(page, [[b.x + b.width * 0.4, y]], [[b.x + b.width * 0.7, y]]);
    const hovers = await takeEvents(el, "plot-hover");
    expect(hovers.length).toBeGreaterThan(1);
    for (const h of hovers) expect(h.detail?.["source"]).toBe("touch");
    const idx = hovers.map((h) => h.detail?.["index"] as number);
    expect(idx).toEqual([...idx].sort((p, q) => p - q));
    // A scrub does not pan.
    expect(await domain(el)).toBeNull();
  });

  test("two fingers pan, ending with one final plot-view", async ({ page }) => {
    const el = await addChart(page);
    const b = await drawingBox(el);
    const t = await Touch.on(page);
    const y = b.y + b.height * 0.5;
    const l = b.x + b.width * 0.5;
    const r = b.x + b.width * 0.7;
    await takeEvents(el);
    await t.drag(page, [[l, y], [r, y]], [[l - 120, y], [r - 120, y]]);
    const d = (await domain(el)) as [number, number];
    expect(d).not.toBeNull();
    expect(d[0]).toBeGreaterThan(0);
    // The span is kept: the fingers kept their spread.
    expect(d[1] - d[0]).toBeCloseTo(199, -1);
    const views = await takeEvents(el, "plot-view");
    // Each finger's pointermove changes the spread by a step (20 px) on its own; under the pinch threshold that is a pan, never a zoom.
    for (const v of views) expect(v.detail).toMatchObject({ source: "touch", cause: "pan" });
    expect(views.filter((v) => v.detail?.["final"] === true)).toHaveLength(1);
  });

  test("a pinch zooms about its centre", async ({ page }) => {
    const el = await addChart(page);
    const b = await drawingBox(el);
    const t = await Touch.on(page);
    const y = b.y + b.height * 0.5;
    const c = b.x + b.width * 0.5;
    await takeEvents(el);
    await t.drag(page, [[c - 40, y], [c + 40, y]], [[c - 160, y], [c + 160, y]]);
    const d = (await domain(el)) as [number, number];
    expect(d[1] - d[0]).toBeLessThan(120);
    // About the centre: the zoomed view still holds the x that was under it.
    const mid = (d[0] + d[1]) / 2;
    expect(mid).toBeGreaterThan(60);
    expect(mid).toBeLessThan(140);
    const views = await takeEvents(el, "plot-view");
    expect(views.some((v) => v.detail?.["cause"] === "zoom")).toBe(true);
    expect(views.filter((v) => v.detail?.["final"] === true)).toHaveLength(1);
  });
});
