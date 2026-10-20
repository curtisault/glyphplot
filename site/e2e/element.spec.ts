// REQ-E2E-030: attributes and properties, `view` set and read, a module
// fetched once for many tags, calls queued before load, and teardown and
// freeing on attribute changes and removal; with them REQ-EL-010 to 150,
// REQ-LOAD-010 to 030, REQ-VIEW-030, 040, REQ-MOUNT-010, 020, plot-ready,
// plot-error, and no new chart on a theme change (REQ-EL-040, REQ-THEME-030).

import { expect, test, type Page } from "@playwright/test";
import type { GlyphPlot } from "glyphplot/element";
import { addChart, consoleErrors, frames, openBlank, sine, takeEvents, text } from "./helpers.ts";

type El = GlyphPlot;

test.beforeEach(async ({ page }) => {
  await openBlank(page);
});

/** Counts Chart.free() calls by type from now on, in window.__freed. */
async function countFrees(page: Page): Promise<void> {
  await page.evaluate(() => {
    const w = window as unknown as { __freed: string[] };
    w.__freed = [];
    const proto = window.gp.Chart.prototype;
    const free = proto.free;
    proto.free = function (this: InstanceType<typeof window.gp.Chart>) {
      w.__freed.push(this.type);
      return free.call(this);
    };
  });
}
const freed = (page: Page): Promise<string[]> => page.evaluate(() => (window as unknown as { __freed: string[] }).__freed);

test("the element is defined once with one style, observes the plan's attributes, and is not form-associated", async ({ page }) => {
  const r = await page.evaluate(() => {
    const C = customElements.get("glyph-plot") as unknown as { observedAttributes: string[]; formAssociated?: boolean };
    const fresh = document.createElement("glyph-plot");
    return {
      observed: C.observedAttributes,
      formAssociated: C.formAssociated ?? false,
      styles: document.querySelectorAll("style#glyphplot-style").length,
      fontFace: document.getElementById("glyphplot-style")?.textContent?.includes('@font-face{font-family:"glyphplot mono"') ?? false,
      props: ["data", "options", "view"].every((p) => p in fresh),
    };
  });
  // REQ-EL-020: the plan's element table as of v0.3.0.
  expect(r.observed.sort()).toEqual(["cols", "data", "label", "loading", "locale", "mono", "motion", "options", "rows", "theme", "time", "type", "view"]);
  expect(r.formAssociated).toBe(false); // REQ-EL-150
  expect(r.styles).toBe(1); // REQ-EL-010
  expect(r.fontFace).toBe(true);
  expect(r.props).toBe(true); // REQ-EL-060
});

test("plot-ready fires once after the first frame and resolves ready; role figure, a drawing, light DOM", async ({ page }) => {
  const el = await addChart(page, { attrs: { label: "Ready test" } });
  const ready = await takeEvents(el, "plot-ready");
  expect(ready).toHaveLength(1);
  expect(ready[0]?.detail).toMatchObject({ type: "line" });
  expect(ready[0]?.detail?.["cols"]).toBeGreaterThan(0);
  await expect(el).toHaveAttribute("role", "figure");
  await expect(el).toHaveAttribute("aria-label", "Ready test");
  expect(await el.evaluate((e) => e.shadowRoot)).toBeNull(); // REQ-EL-130
  await expect(el.locator("canvas.gp-drawing")).toHaveCount(1);
  expect(await el.evaluate((e) => (e as El).chart !== null)).toBe(true);
  await frames(page, 3);
  expect(await takeEvents(el, "plot-ready")).toHaveLength(0);
});

test("a server-rendered still stays until the first frame replaces it", async ({ page }) => {
  const r = await page.evaluate(async () => {
    const el = document.createElement("glyph-plot") as El;
    el.setAttribute("type", "line");
    el.setAttribute("loading", "eager");
    el.innerHTML = "<pre class='still'>a still</pre>";
    el.data = { x: [0, 1, 2], y: [1, 3, 2] };
    document.getElementById("stage")?.append(el);
    const before = el.querySelector(".still") !== null;
    await el.ready;
    return { before, after: el.querySelector(".still") !== null };
  });
  expect(r).toEqual({ before: true, after: false });
});

test("plot-error for a type no module plays, with the message, keeping the children", async ({ page }) => {
  const errors = consoleErrors(page);
  const r = await page.evaluate(async () => {
    const el = document.createElement("glyph-plot") as El;
    el.setAttribute("type", "nope");
    el.setAttribute("loading", "eager");
    el.textContent = "fallback text";
    const got = new Promise<unknown>((res) => el.addEventListener("plot-error", (e) => res((e as CustomEvent).detail)));
    document.getElementById("stage")?.append(el);
    const detail = await got;
    const rejected = await el.ready.then(
      () => "",
      (e: Error) => e.message,
    );
    return { detail, rejected, text: el.textContent, chart: el.chart };
  });
  expect(r.detail).toEqual({ type: "nope", message: 'there is no chart named "nope"' });
  expect(r.rejected).toBe('there is no chart named "nope"');
  expect(r.text).toBe("fallback text");
  expect(r.chart).toBeNull();
  expect(errors.some((e) => e.includes('there is no chart named "nope"'))).toBe(true);
});

test("attributes: data, options, label, mono, cols and rows", async ({ page }) => {
  const el = await addChart(page, { attrs: { data: JSON.stringify({ x: [0, 1, 2, 3], y: [1, 3, 2, 4] }) }, data: null });
  expect(await el.evaluate((e) => (e as El).data?.tables["data"]?.length)).toBe(4);
  // The data attribute replaces the data.
  await el.evaluate((e) => e.setAttribute("data", JSON.stringify({ x: [0, 1, 2, 3, 4, 5], y: [1, 2, 3, 4, 5, 6] })));
  expect(await el.evaluate((e) => (e as El).data?.tables["data"]?.length)).toBe(6);
  // options: set_options on the same chart (REQ-EL-040).
  const chart0 = await el.evaluateHandle((e) => (e as El).chart);
  await el.evaluate((e) => e.setAttribute("options", '{"mode":"step"}'));
  expect(await el.evaluate((e) => (e as El).options)).toEqual({ mode: "step" });
  expect(await el.evaluate((e, c) => (e as El).chart === c, chart0)).toBe(true);
  expect(await el.evaluate((e) => (e as El).data?.tables["data"]?.length)).toBe(6);
  // label names the figure.
  await el.evaluate((e) => e.setAttribute("label", "Renamed"));
  await expect(el).toHaveAttribute("aria-label", "Renamed");
  // cols and rows fix the grid.
  await el.evaluate((e) => {
    e.setAttribute("cols", "30");
    e.setAttribute("rows", "8");
  });
  await frames(page, 2);
  expect(await el.evaluate((e) => [(e as El).cols, (e as El).rows])).toEqual([30, 8]);
  // mono draws in a <pre>, on the same chart.
  await el.evaluate((e) => e.setAttribute("mono", ""));
  await frames(page, 3);
  await expect(el.locator("pre.gp-drawing")).toHaveCount(1);
  await expect(el.locator("canvas")).toHaveCount(0);
  expect((await el.locator("pre.gp-drawing").textContent())?.split("\n")).toHaveLength(8);
  expect(await el.evaluate((e, c) => (e as El).chart === c, chart0)).toBe(true);
});

test("properties: data replaces without writing the attribute and reads back the descriptor and lengths", async ({ page }) => {
  const el = await addChart(page, { data: sine(50, true) });
  const r = await el.evaluate((e) => {
    const p = e as El;
    p.data = { x: new Float64Array([0, 1, 2]), y: new Float64Array([2, 1, 3]) };
    return { attr: p.getAttribute("data"), data: p.data, type: p.type, theme: p.theme };
  });
  expect(r.attr).toBeNull(); // REQ-EL-070
  expect(r.data).toEqual({ version: 1, tables: { data: { length: 3, columns: [{ name: "x", type: "f64" }, { name: "y", type: "f64" }] } } });
  expect(r.type).toBe("line");
  expect(r.theme).toBe("auto");
});

test("view: set partial, read whole, null resets a key, and the read view set back draws the same frame; no events from script", async ({ page }) => {
  const el = await addChart(page);
  await frames(page, 2);
  await takeEvents(el);
  await el.evaluate((e) => ((e as El).view = { x: { domain: [20, 60] } }));
  await frames(page, 2);
  const v = await el.evaluate((e) => (e as El).view);
  expect(v["x"]).toEqual({ domain: [20, 60], follow: false });
  expect(v["y"]).toMatchObject({ domain: null });
  const frameA = await text(el);
  // Round trip (REQ-VIEW-030).
  await el.evaluate((e) => {
    const p = e as El;
    const saved = JSON.parse(JSON.stringify(p.view)) as Record<string, unknown>;
    p.view = null;
    p.view = saved;
  });
  await frames(page, 2);
  expect(await text(el)).toBe(frameA);
  // The attribute form.
  await el.evaluate((e) => e.setAttribute("view", '{"x":{"domain":[10,30]}}'));
  await frames(page, 2);
  expect(await el.evaluate((e) => ((e as El).view["x"] as { domain: unknown }).domain)).toEqual([10, 30]);
  // A key set to null returns to its default.
  await el.evaluate((e) => ((e as El).view = { x: null }));
  await frames(page, 2);
  expect(await el.evaluate((e) => ((e as El).view["x"] as { domain: unknown }).domain)).toBeNull();
  // Script sets of view, data, options and time fire no reader event (REQ-VIEW-040).
  await el.evaluate((e) => {
    const p = e as El;
    p.view = { x: { domain: [5, 15] }, cursor: { x: 7 } };
    p.data = { x: [0, 1, 2, 3], y: [3, 1, 2, 0] };
    p.options = { mode: "area" };
    p.setAttribute("time", "1");
    p.resetView();
  });
  await frames(page, 4);
  expect((await takeEvents(el)).map((e) => e.type)).toEqual([]);
});

test("one module fetched and instantiated once for many tags @smoke", async ({ page }) => {
  const wasm: string[] = [];
  page.on("request", (r) => {
    if (r.url().endsWith(".wasm")) wasm.push(r.url());
  });
  // Each 120 px tall: a chart draws its first frame only once it is on screen (REQ-MOUNT-020).
  await Promise.all([1, 2, 3, 4, 5].map((i) => addChart(page, { id: `m${i}`, n: 20 * i, style: "height: 120px" })));
  expect(wasm).toHaveLength(1);
  expect(await page.evaluate(() => window.gp.instantiations.get("line"))).toBe(1);
  await expect(page.locator("glyph-plot canvas")).toHaveCount(5);
});

test("calls before load are queued in order; a data set discards the appends queued before it", async ({ page }) => {
  const r = await page.evaluate(async () => {
    const el = document.createElement("glyph-plot") as El;
    el.setAttribute("type", "line");
    el.setAttribute("loading", "eager");
    el.append({ x: 99, y: 99 }); // discarded by the data set after it
    el.data = { x: [0, 1, 2], y: [1, 2, 3] };
    el.append({ x: 3, y: 4 });
    el.append([
      { x: 4, y: 5 },
      { x: 5, y: 6 },
    ]);
    el.options = { mode: "step" };
    el.view = { x: { domain: [1, 4] } };
    const queuedView = el.view;
    document.getElementById("stage")?.append(el);
    await el.ready;
    const c = el.chart;
    return { queuedView, length: el.data?.tables["data"]?.length, mode: c?.options["mode"], view: el.view["x"] };
  });
  expect(r.queuedView).toEqual({ x: { domain: [1, 4] } });
  expect(r.length).toBe(6);
  expect(r.mode).toBe("step");
  expect(r.view).toEqual({ domain: [1, 4], follow: false });
});

test("the JS API: chart() queues calls before its module loads and mount() draws into a <pre> and a <canvas>", async ({ page }) => {
  const r = await page.evaluate(async () => {
    const c = window.gp.chart("line");
    c.data = { x: [0, 1, 2, 3], y: [0, 1, 0, 1] };
    c.view = { x: { domain: [0, 2] } };
    await c.ready;
    const pre = document.createElement("pre");
    const canvas = document.createElement("canvas");
    document.getElementById("stage")?.append(pre, canvas);
    const m1 = window.gp.mount(pre, c);
    const c2 = window.gp.chart("line", { data: { x: [0, 1, 2, 3], y: [0, 1, 0, 1] } });
    await c2.ready;
    const m2 = window.gp.mount(canvas, c2);
    m1.resize(30, 6, 8);
    m2.resize(30, 6, 8);
    for (let i = 0; i < 20 && !(pre.textContent && m2.frames > 0); i++) await new Promise((res) => requestAnimationFrame(res));
    const out = { pre: pre.textContent?.split("\n").length, canvas: [canvas.width, canvas.height], view: (c.view["x"] as { domain: unknown }).domain, frames: m2.frames };
    m1.stop();
    m2.stop();
    c.free();
    c2.free();
    return out;
  });
  expect(r.pre).toBe(6);
  expect(r.canvas).toEqual([240, 96]);
  expect(r.view).toEqual([0, 2]);
  expect(r.frames).toBeGreaterThan(0);
});

test("a type change frees the old chart and carries data and view; removal frees; a superseded load draws nothing", async ({ page }) => {
  await countFrees(page);
  const errors = consoleErrors(page);
  const el = await addChart(page);
  await el.evaluate((e) => ((e as El).view = { x: { domain: [30, 70] } }));
  // A type change: a new chart with the old data and view (REQ-EL-140).
  const before = await el.evaluateHandle((e) => (e as El).chart);
  await el.evaluate(async (e) => {
    e.setAttribute("type", "line ");
    e.setAttribute("type", "line");
    await (e as El).ready;
  });
  expect(await freed(page)).toEqual(["line", "line "]);
  expect(await el.evaluate((e, c) => (e as El).chart !== c && c?.dead === "the chart was freed", before)).toBe(true);
  expect(await el.evaluate((e) => (e as El).data?.tables["data"]?.length)).toBe(200);
  expect(await el.evaluate((e) => ((e as El).view["x"] as { domain: unknown }).domain)).toEqual([30, 70]);
  await expect(el.locator("canvas")).toHaveCount(1);
  // Removal stops drawing and frees (REQ-EL-140, REQ-MOUNT-010).
  const last = await el.evaluateHandle((e) => (e as El).chart);
  const handle = await el.elementHandle();
  await el.evaluate((e) => e.remove());
  expect(await page.evaluate((c) => c?.dead, last)).toBe("the chart was freed");
  expect(await freed(page)).toHaveLength(3);
  // Back in the page: a new chart, the data and view kept.
  await handle?.evaluate(async (e) => {
    document.getElementById("stage")?.append(e);
    await (e as El).ready;
  });
  expect(await el.evaluate((e) => (e as El).data?.tables["data"]?.length)).toBe(200);
  expect(await el.evaluate((e) => ((e as El).view["x"] as { domain: unknown }).domain)).toEqual([30, 70]);
  // Removed before its module loads: the load frees what it made and draws nothing.
  const r = await page.evaluate(async () => {
    const e = document.createElement("glyph-plot") as El;
    e.setAttribute("type", "line");
    e.setAttribute("loading", "eager");
    e.data = { x: [0, 1], y: [0, 1] };
    document.getElementById("stage")?.append(e);
    e.remove();
    await new Promise((res) => setTimeout(res, 300));
    return { children: e.children.length, chart: e.chart };
  });
  expect(r).toEqual({ children: 0, chart: null });
  expect(await freed(page)).toHaveLength(4);
  expect(errors).toEqual([]);
});

test("a theme change makes no new chart, loads nothing and draws no new frame, but repaints", async ({ page }) => {
  const el = await addChart(page, { data: sine(200, true) });
  await frames(page, 3);
  await el.evaluate((e) => {
    const c = (e as El).chart as NonNullable<El["chart"]>;
    const w = window as unknown as { __frames: number; __chart: unknown };
    w.__frames = 0;
    w.__chart = c;
    const frame = c.frame.bind(c);
    c.frame = (t?: number, paper?: boolean) => {
      w.__frames++;
      return frame(t, paper);
    };
  });
  const pixels = (): Promise<string> =>
    el.evaluate((e) => {
      const cv = e.querySelector("canvas") as HTMLCanvasElement;
      return cv.toDataURL();
    });
  const p0 = await pixels();
  await page.evaluate(() => (document.documentElement.dataset["theme"] = "dark"));
  await el.evaluate((e) => e.setAttribute("theme", "dark"));
  await frames(page, 3);
  const p1 = await pixels();
  await el.evaluate((e) => ((e as El).theme = { series: ["#ff0000", "#00ff00"] }));
  await el.evaluate((e) => e.style.setProperty("--glyphplot-series-1", "#0000ff"));
  await page.evaluate(() => document.documentElement.classList.add("x"));
  await frames(page, 3);
  const p2 = await pixels();
  const r = await el.evaluate((e) => {
    const w = window as unknown as { __frames: number; __chart: unknown };
    return { frames: w.__frames, same: (e as El).chart === w.__chart, inst: window.gp.instantiations.get("line") };
  });
  expect(r).toEqual({ frames: 0, same: true, inst: 1 });
  expect(p1).not.toBe(p0);
  expect(p2).not.toBe(p1);
});

test("loading is lazy: nothing is fetched until the element is within a screen of the viewport", async ({ page }) => {
  const wasm: string[] = [];
  page.on("request", (r) => {
    if (r.url().endsWith(".wasm")) wasm.push(r.url());
  });
  const el = await addChart(page, { attrs: { loading: "lazy" }, style: "margin-top: 3000px", wait: false });
  await page.waitForTimeout(400);
  expect(wasm).toHaveLength(0);
  expect(await el.evaluate((e) => (e as El).chart)).toBeNull();
  // Within one screen height (800 px) below the viewport, not yet on it: it loads.
  const top = await el.evaluate((e) => e.getBoundingClientRect().top + scrollY);
  await page.evaluate((y) => scrollTo(0, y), top - 800 - 400);
  await expect.poll(() => wasm.length).toBe(1);
  expect(await page.evaluate(() => window.gp.instantiations.get("line"))).toBe(1);
  // On screen it draws.
  await el.scrollIntoViewIfNeeded();
  await el.evaluate((e) => (e as El).ready);
});

test("a chart draws only while it intersects the viewport", async ({ page }) => {
  await page.evaluate(() => document.body.style.setProperty("min-height", "5000px"));
  const el = await addChart(page);
  await frames(page, 2);
  await el.evaluate((e) => {
    const c = (e as El).chart as NonNullable<El["chart"]>;
    const w = window as unknown as { __frames: number };
    w.__frames = 0;
    const frame = c.frame.bind(c);
    c.frame = (t?: number, paper?: boolean) => {
      w.__frames++;
      return frame(t, paper);
    };
  });
  await page.evaluate(() => scrollTo(0, 4000));
  await frames(page, 3);
  await el.evaluate((e) => ((e as El).data = { x: [0, 1, 2], y: [2, 0, 1] }));
  await frames(page, 4);
  expect(await page.evaluate(() => (window as unknown as { __frames: number }).__frames)).toBe(0);
  await page.evaluate(() => scrollTo(0, 0));
  await frames(page, 4);
  expect(await page.evaluate(() => (window as unknown as { __frames: number }).__frames)).toBeGreaterThan(0);
});
