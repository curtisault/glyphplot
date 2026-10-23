// Shared steps of the browser tests: the bare page, a chart added to it with
// its events recorded, frames waited for, console errors collected, and touch
// through CDP's Input.dispatchTouchEvent (Playwright's own touch helpers fire
// no Pointer Events, testing.md).

import type { CDPSession, Locator, Page } from "@playwright/test";
import type { GlyphPlot } from "glyphplot/element";

/** One recorded event. */
export interface Recorded {
  type: string;
  detail: Record<string, unknown> | null;
}

declare global {
  interface Window {
    __events: Record<string, Recorded[]>;
  }
}

/** The reader-action and loading events the element fires at v0.5.0. */
export const EVENT_TYPES = ["plot-ready", "plot-error", "plot-hover", "plot-select", "plot-view", "plot-cursor", "plot-brush", "plot-legend"];

/** Console errors and uncaught exceptions on `page`, from now on. */
export function consoleErrors(page: Page): string[] {
  const out: string[] = [];
  page.on("console", (m) => {
    if (m.type() === "error") out.push(m.text());
  });
  page.on("pageerror", (e) => out.push(`uncaught: ${e.message}`));
  return out;
}

/** Opens the bare page and waits for its module script. */
export async function openBlank(page: Page): Promise<void> {
  await page.goto("blank.html");
  await page.waitForFunction(() => window.gpReady === true);
}

/** What {@link addChart} makes. */
export interface ChartSpec {
  id?: string;
  /** Attributes; `type` defaults to `line`, `loading` to `eager`. */
  attrs?: Record<string, string>;
  /** Data set through the property (null: none); default {@link sine} of `n` points. */
  data?: unknown;
  n?: number;
  /** Inline style of the element. */
  style?: string;
  /** Selector of the parent; default #stage. */
  parent?: string;
  /** Wait for `ready`; default true. */
  wait?: boolean;
}

/** `n` points of sin(i / 10) and, with `two`, cos(i / 13), over x = 0 … n−1. */
export function sine(n: number, two = false): Record<string, number[]> {
  const x: number[] = [];
  const a: number[] = [];
  const b: number[] = [];
  for (let i = 0; i < n; i++) {
    x.push(i);
    a.push(Math.sin(i / 10));
    b.push(Math.cos(i / 13));
  }
  return two ? { x, a, b } : { x, a };
}

let counter = 0;

/** Adds a `<glyph-plot>` to the bare page, records its events in window.__events[id], and waits for its first frame. */
export async function addChart(page: Page, spec: ChartSpec = {}): Promise<Locator> {
  const id = spec.id ?? `c${++counter}`;
  const data = "data" in spec ? spec.data : sine(spec.n ?? 200);
  const attrs = { type: "line", loading: "eager", ...spec.attrs };
  await page.evaluate(
    async ({ id, data, attrs, style, parent, wait, types }) => {
      const el = document.createElement("glyph-plot") as GlyphPlot;
      el.id = id;
      for (const [k, v] of Object.entries(attrs)) el.setAttribute(k, v);
      if (style) el.setAttribute("style", style);
      if (data !== null && data !== undefined) el.data = data as never;
      window.__events ??= {};
      const list: Recorded[] = (window.__events[id] = []);
      for (const t of types) el.addEventListener(t, (e) => list.push({ type: t, detail: (e as CustomEvent).detail as Recorded["detail"] }));
      (document.querySelector(parent) ?? document.body).append(el);
      if (wait) await el.ready;
    },
    { id, data, attrs, style: spec.style ?? "", parent: spec.parent ?? "#stage", wait: spec.wait ?? true, types: EVENT_TYPES },
  );
  return page.locator(`#${id}`);
}

/** The events `el` fired since the last call, emptied. */
export async function takeEvents(el: Locator, type?: string): Promise<Recorded[]> {
  const id = await el.getAttribute("id");
  const all = await el.page().evaluate((id) => {
    return (window.__events[id ?? ""] ?? []).splice(0);
  }, id);
  return type ? all.filter((e) => e.type === type) : all;
}

/** Waits `n` animation frames in the page. */
export async function frames(page: Page, n = 2): Promise<void> {
  await page.evaluate(async (n) => {
    for (let i = 0; i < n; i++) await new Promise((r) => requestAnimationFrame(() => r(null)));
  }, n);
}

/** The element's view's x domain (null when fitted). */
export async function domain(el: Locator): Promise<[number, number] | null> {
  return el.evaluate((e) => ((e as GlyphPlot).view["x"] as { domain: [number, number] | null }).domain);
}

/** The element's frame as text. */
export async function text(el: Locator): Promise<string> {
  return el.evaluate((e) => (e as GlyphPlot).text());
}

/** The drawing's box in page coordinates. */
export async function drawingBox(el: Locator): Promise<{ x: number; y: number; width: number; height: number }> {
  const b = await el.locator(".gp-drawing").boundingBox();
  if (!b) throw new Error("the chart has no drawing");
  return b;
}

/** Touch through CDP Input.dispatchTouchEvent: each call gives every finger down, in order (id = index). */
export class Touch {
  readonly #cdp: CDPSession;
  constructor(cdp: CDPSession) {
    this.#cdp = cdp;
  }
  static async on(page: Page): Promise<Touch> {
    return new Touch(await page.context().newCDPSession(page));
  }
  #send(type: string, points: [number, number][]): Promise<unknown> {
    return this.#cdp.send("Input.dispatchTouchEvent", { type: type as "touchStart", touchPoints: points.map(([x, y], id) => ({ x, y, id })) });
  }
  start(points: [number, number][]): Promise<unknown> {
    return this.#send("touchStart", points);
  }
  move(points: [number, number][]): Promise<unknown> {
    return this.#send("touchMove", points);
  }
  end(): Promise<unknown> {
    return this.#send("touchEnd", []);
  }
  /** Fingers from `from` to `to` in `steps` moves, a frame apart. */
  async drag(page: Page, from: [number, number][], to: [number, number][], steps = 6): Promise<void> {
    await this.start(from);
    for (let s = 1; s <= steps; s++) {
      const f = s / steps;
      await this.move(from.map(([x, y], i) => [x + ((to[i]?.[0] ?? x) - x) * f, y + ((to[i]?.[1] ?? y) - y) * f]));
      await frames(page, 1);
    }
    await this.end();
    await frames(page, 2);
  }
}

/** The site's doc pages, as site/src/Route.elm's docPages lists them (REQ-SITE-030); site.spec.ts checks the sidebar holds the same. */
export const DOCS = ["getting-started", "data", "view-and-events", "theming", "accessibility", "performance", "cli"];
