// REQ-E2E-050: appending at 1 kHz for a second ends on the frame a replace of
// the same rows draws. With it, in the browser: one write a frame and
// `plot-data` (REQ-STREAM-010, 070), End and follow (REQ-VIEW-060), `paused`
// and Space (REQ-A11Y-080), the table's rate and focus rule (REQ-A11Y-090),
// `announce` (REQ-A11Y-100), and `src` over files the site serves
// (REQ-DATA-150).
//
// Until the module has `staging(n)` and `append(n)` the shell commits the
// whole table each frame (its fallback); the equality holds on that path too,
// since the fallback commits exactly the rows a replace would.

import { expect, test, type Locator, type Page } from "@playwright/test";
import type { GlyphPlot } from "glyphplot/element";
import { addChart, consoleErrors, domain, frames, openBlank, text } from "./helpers.ts";

type El = GlyphPlot;

interface DataDetail {
  rows: number;
  dropped: number;
  late: number;
  version: number;
}

declare global {
  interface Window {
    __data: Record<string, DataDetail[]>;
  }
}

test.beforeEach(async ({ page }) => {
  await openBlank(page);
});

/** Records `el`'s plot-data details in window.__data[id] from now on. */
async function recordData(el: Locator): Promise<void> {
  await el.evaluate((e) => {
    window.__data ??= {};
    const list: DataDetail[] = (window.__data[e.id] = []);
    e.addEventListener("plot-data", (ev) => list.push((ev as CustomEvent<DataDetail>).detail));
  });
}

const dataSeen = (page: Page, id: string): Promise<DataDetail[]> => page.evaluate((id) => window.__data[id] ?? [], id);

/** Appends one row a millisecond for `ms` ms of the page's clock, each its own append() call; returns the rows and the animation frames that passed. */
async function stream(el: Locator, ms: number, from: number): Promise<{ rows: { x: number; y: number }[]; frames: number }> {
  return el.evaluate(
    async (e, [ms, from]) => {
      const p = e as El;
      const rows: { x: number; y: number }[] = [];
      let frames = 0;
      const t0 = performance.now();
      const y = (i: number): number => Math.round((Math.sin(i / 40) * 50 + Math.sin(i / 7) * 5) * 100) / 100;
      await new Promise<void>((done) => {
        const tick = (): void => {
          frames++;
          const due = Math.min(ms, Math.floor(performance.now() - t0));
          while (rows.length < due) {
            const i = from + rows.length;
            const row = { x: i, y: y(i) };
            rows.push(row);
            p.append(row);
          }
          if (rows.length < ms) requestAnimationFrame(tick);
          else done();
        };
        requestAnimationFrame(tick);
      });
      return { rows, frames };
    },
    [ms, from] as const,
  );
}

test("appending at 1 kHz for a second ends on the frame a replace of the same rows draws (REQ-E2E-050)", async ({ page }) => {
  const errors = consoleErrors(page);
  const seed = Array.from({ length: 20 }, (_, i) => ({ x: i - 20, y: 0 }));
  const a = await addChart(page, { id: "appended", data: seed, attrs: { rows: "12" } });
  const b = await addChart(page, { id: "replaced", data: null, attrs: { rows: "12" } });
  await recordData(a);
  const { rows, frames: passed } = await stream(a, 1000, 0);
  expect(rows).toHaveLength(1000);
  await frames(page, 3);
  await b.evaluate((e, all) => ((e as El).data = all), [...seed, ...rows]);
  await frames(page, 3);
  const frameA = await text(a);
  expect(frameA.trim().length).toBeGreaterThan(0);
  expect(frameA).toBe(await text(b));
  // The 1,000 appends were written a frame at a time, each write one plot-data.
  const seen = await dataSeen(page, "appended");
  expect(seen.length).toBeGreaterThan(0);
  expect(seen.length).toBeLessThanOrEqual(passed + 3);
  expect(seen[seen.length - 1]?.rows).toBe(1020);
  expect(seen.reduce((n, d) => n + d.late + d.dropped, 0)).toBe(0);
  for (let i = 1; i < seen.length; i++) expect(seen[i]?.version).toBe((seen[i - 1]?.version ?? 0) + 1);
  expect(errors).toEqual([]);
});

test("plot-data after each commit: a replace, a frame's appends, late rows and retain.rows (REQ-STREAM-070)", async ({ page }) => {
  const el = await addChart(page, { id: "pd", data: { x: [0, 1, 2], y: [1, 2, 3] }, attrs: { options: '{"retain":{"rows":5}}' } });
  await recordData(el);
  await el.evaluate((e) => {
    const p = e as El;
    p.append({ x: 3, y: 1 });
    p.append([{ x: 1.5, y: 0 }, { x: 4, y: 2 }, { x: 5, y: 3 }]);
  });
  await frames(page, 2);
  await el.evaluate((e) => ((e as El).data = { x: [10, 11], y: [0, 0] }));
  const seen = await dataSeen(page, "pd");
  expect(seen.map(({ rows, dropped, late }) => ({ rows, dropped, late }))).toEqual([
    { rows: 5, dropped: 1, late: 1 },
    { rows: 2, dropped: 0, late: 0 },
  ]);
  expect(seen[1]?.version).toBe((seen[0]?.version ?? 0) + 1);
});

test("End sets follow; appends then keep the span and end at the newest x; a pan stops it (REQ-VIEW-060)", async ({ page }) => {
  const el = await addChart(page, { id: "follow", n: 200 });
  await el.evaluate((e) => ((e as El).view = { x: { domain: [50, 100] } }));
  await el.locator(".gp-cursor").focus();
  await page.keyboard.press("End");
  await frames(page, 2);
  expect(await el.evaluate((e) => (e as El).view["x"])).toMatchObject({ follow: true });
  await el.evaluate((e) => {
    for (let i = 200; i < 230; i++) (e as El).append({ x: i, a: 0 });
  });
  await frames(page, 3);
  const d = await domain(el);
  expect(d?.[1]).toBe(229);
  expect((d?.[1] ?? 0) - (d?.[0] ?? 0)).toBeCloseTo(50, 6);
  // Shift+← pans a tenth of the view away from the end.
  await page.keyboard.press("Shift+ArrowLeft");
  await frames(page, 2);
  expect(await el.evaluate((e) => (e as El).view["x"])).toMatchObject({ follow: false });
  const held = await domain(el);
  await el.evaluate((e) => (e as El).append({ x: 230, a: 1 }));
  await frames(page, 3);
  expect(await domain(el)).toEqual(held);
});

test("paused and Space keep appends undrawn; resume shows the live view (REQ-A11Y-080)", async ({ page }) => {
  const el = await addChart(page, { id: "pause", n: 100 });
  await el.locator(".gp-cursor").focus();
  await page.keyboard.press("Space");
  await expect(el).toHaveAttribute("paused", "");
  expect(await el.evaluate((e) => (e as El).paused)).toBe(true);
  await frames(page, 2);
  const still = await text(el);
  await el.evaluate((e) => {
    for (let i = 100; i < 160; i++) (e as El).append({ x: i, a: 5 });
  });
  await frames(page, 4);
  expect(await text(el)).toBe(still);
  expect(await el.evaluate((e) => (e as El).data?.tables["data"]?.length)).toBe(160); // kept
  await page.keyboard.press("Space");
  await expect(el).not.toHaveAttribute("paused");
  await frames(page, 3);
  expect(await text(el)).not.toBe(still);
  // The property and the attribute are one.
  await el.evaluate((e) => ((e as El).paused = true));
  await expect(el).toHaveAttribute("paused", "");
  await el.evaluate((e) => e.removeAttribute("paused"));
  expect(await el.evaluate((e) => (e as El).paused)).toBe(false);
});

test("while streaming the table is rebuilt at most once a second, and never while focus is inside it (REQ-A11Y-090)", async ({ page }) => {
  const el = await addChart(page, { id: "table", n: 100 });
  const builds = await el.evaluate(async (e) => {
    const p = e as El;
    const box = p.querySelector(".gp-sr") as HTMLElement;
    const at: number[] = [];
    new MutationObserver(() => at.push(performance.now())).observe(box, { childList: true });
    const t0 = performance.now();
    let i = 100;
    while (performance.now() - t0 < 3200) {
      p.append({ x: i, a: Math.sin(i / 5) });
      i++;
      await new Promise((r) => requestAnimationFrame(r));
    }
    return at;
  });
  expect(builds.length).toBeGreaterThanOrEqual(2);
  expect(builds.length).toBeLessThanOrEqual(4);
  for (let k = 1; k < builds.length; k++) expect((builds[k] ?? 0) - (builds[k - 1] ?? 0)).toBeGreaterThanOrEqual(990);
  // Focus inside the table: no rebuild while it stays; one after it leaves.
  const r = await el.evaluate(async (e) => {
    const p = e as El;
    const box = p.querySelector(".gp-sr") as HTMLElement;
    const cell = box.querySelector("th") as HTMLElement;
    cell.tabIndex = -1;
    cell.focus();
    const focused = document.activeElement === cell;
    let during = 0;
    const mo = new MutationObserver(() => during++);
    mo.observe(box, { childList: true });
    const step = async (ms: number, k: number): Promise<number> => {
      const t0 = performance.now();
      while (performance.now() - t0 < ms) {
        p.append({ x: k++, a: 0 });
        await new Promise((r) => requestAnimationFrame(r));
      }
      return k;
    };
    let k = await step(2200, 10_000);
    const whileFocused = during;
    cell.blur();
    k = await step(1500, k);
    mo.disconnect();
    return { focused, whileFocused, after: during - whileFocused };
  });
  expect(r.focused).toBe(true);
  expect(r.whileFocused).toBe(0);
  expect(r.after).toBeGreaterThanOrEqual(1);
});

test("announce: no live region without it; with it, a polite region reads the latest value at most once in 5 s (REQ-A11Y-100)", async ({ page }) => {
  test.setTimeout(45_000);
  const plain = await addChart(page, { id: "quiet", n: 50 });
  expect(await plain.locator("[aria-live]").count()).toBe(0);
  const el = await addChart(page, { id: "said", n: 50, attrs: { announce: "1" } });
  const live = el.locator("[aria-live]");
  await expect(live).toHaveCount(1);
  await expect(live).toHaveAttribute("aria-live", "polite");
  await expect(live).toHaveAttribute("role", "status");
  // Streams for 6 s, recording each change of what the region says.
  const said = await el.evaluate(async (e) => {
    const p = e as El;
    const region = p.querySelector("[aria-live]") as HTMLElement;
    const out: { t: number; text: string }[] = [];
    const t0 = performance.now();
    new MutationObserver(() => out.push({ t: performance.now() - t0, text: region.textContent ?? "" })).observe(region, { childList: true, characterData: true, subtree: true });
    let i = 50;
    while (performance.now() - t0 < 6000) {
      p.append({ x: i, a: i });
      i++;
      await new Promise((r) => setTimeout(r, 50));
    }
    return out;
  });
  expect(said.length).toBeGreaterThanOrEqual(1);
  expect(said.length).toBeLessThanOrEqual(2); // announce="1" is held to 5 s
  for (let k = 1; k < said.length; k++) expect((said[k]?.t ?? 0) - (said[k - 1]?.t ?? 0)).toBeGreaterThanOrEqual(4900);
  expect(said[said.length - 1]?.text).toMatch(/^Latest: \d+, a \d+$/);
  // Silent while the cursor has focus, and while paused.
  for (const how of ["focus", "paused"] as const) {
    const quiet = await el.evaluate(async (e, how) => {
      const p = e as El;
      const region = p.querySelector("[aria-live]") as HTMLElement;
      const cursor = p.querySelector(".gp-cursor") as HTMLElement;
      if (how === "focus") cursor.focus();
      else p.paused = true;
      const before = region.textContent;
      const t0 = performance.now();
      let i = 10_000;
      while (performance.now() - t0 < 5600) {
        p.append({ x: i, a: i });
        i++;
        await new Promise((r) => setTimeout(r, 50));
      }
      const same = region.textContent === before;
      cursor.blur();
      p.paused = false;
      return same;
    }, how);
    expect(quiet, how).toBe(true);
  }
  // Removing the attribute removes the region.
  await el.evaluate((e) => e.removeAttribute("announce"));
  await expect(live).toHaveCount(0);
});

test("src: CSV, JSON and NDJSON from the site, each fetched once per change, drawn as the same data (REQ-DATA-150)", async ({ page }) => {
  const fetched: string[] = [];
  page.on("request", (r) => {
    if (r.url().includes("/data/walk.")) fetched.push(new URL(r.url()).pathname.replace(/^.*\/data\//, ""));
  });
  const el = await addChart(page, { id: "src", data: null, wait: false, attrs: { src: "data/walk.csv" } });
  await el.evaluate((e) => (e as El).ready);
  await expect.poll(() => el.evaluate((e) => (e as El).data?.tables["data"]?.length ?? 0)).toBe(50);
  expect(await el.evaluate((e) => (e as El).data?.tables["data"]?.columns.map((c) => `${c.name}:${c.type}`))).toEqual(["t:timestamp", "y:f64"]);
  await frames(page, 2);
  const csv = await text(el);
  for (const file of ["walk.json", "walk.ndjson"]) {
    await el.evaluate((e, f) => e.setAttribute("src", `data/${f}`), file);
    await expect.poll(() => el.evaluate((e) => (e as El).data?.tables["data"]?.columns.map((c) => c.type).join(","))).toBe("timestamp,f64");
    await frames(page, 3);
    expect(await text(el), file).toBe(csv);
  }
  // Moved in the page, the element keeps its data; nothing is fetched again.
  await el.evaluate(async (e) => {
    const parent = e.parentElement as HTMLElement;
    e.remove();
    parent.append(e);
    await (e as El).ready;
  });
  await frames(page, 3);
  expect(await text(el)).toBe(csv);
  expect(fetched).toEqual(["walk.csv", "walk.json", "walk.ndjson"]);
});

test("src: Arrow goes to the module's set_arrow, and without it plot-error says so; a missing file is plot-error (REQ-DATA-150)", async ({ page }) => {
  const el = await addChart(page, { id: "arrow", n: 20 });
  const reads = await el.evaluate((e) => (e as El).chart?.readsArrow ?? null);
  /** Sets `src` and resolves with the next plot-error's message. */
  const failing = (src: string): Promise<string> =>
    el.evaluate(
      (e, src) =>
        new Promise<string>((resolve) => {
          e.addEventListener("plot-error", (ev) => resolve((ev as CustomEvent<{ message: string }>).detail.message), { once: true });
          e.setAttribute("src", src);
        }),
      src,
    );
  if (reads) {
    await el.evaluate((e) => e.setAttribute("src", "data/walk.arrows"));
    await expect.poll(() => el.evaluate((e) => (e as El).chart?.lastCommit?.rows)).toBe(50);
  } else {
    expect(await failing("data/walk.arrows")).toMatch(/^src "data\/walk\.arrows": Arrow data needs a module with set_arrow \(v0\.6\.0\)/);
    expect(await el.evaluate((e) => (e as El).data?.tables["data"]?.length)).toBe(20); // the data there was is kept
  }
  // A missing file: a 404, or (vite preview, a single-page host) its HTML fallback page.
  expect(await failing("data/none.csv")).toMatch(/^src "data\/none\.csv": (HTTP 404|the server sent an HTML page)/);
});

test("src: Arrow drawn through set_arrow equals the same rows from CSV (REQ-DATA-150)", async ({ page }) => {
  const probe = await addChart(page, { id: "probe", n: 2 });
  const reads = await probe.evaluate((e) => (e as El).chart?.readsArrow ?? false);
  test.fixme(!reads, "the module has no set_arrow yet (the Arrow reader, v0.6.0): on at integration");
  const a = await addChart(page, { id: "from-arrow", data: null, wait: false, attrs: { src: "data/walk.arrows" } });
  const b = await addChart(page, { id: "from-csv", data: null, wait: false, attrs: { src: "data/walk.csv" } });
  await expect.poll(() => a.evaluate((e) => (e as El).chart?.lastCommit?.rows)).toBe(50);
  await expect.poll(() => b.evaluate((e) => (e as El).chart?.lastCommit?.rows)).toBe(50);
  await frames(page, 3);
  expect(await text(a)).toBe(await text(b));
});
