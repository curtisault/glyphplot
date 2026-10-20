// REQ-RENDER-040: a frame whose cells carry back colours is drawn as one
// cols × 2·rows image scaled without smoothing, the glyphs over it (the
// pixels are the owner's check). REQ-THEME-060: up and down are theme
// tokens, read from --glyphplot-up and --glyphplot-down and the theme object.

import { expect, test } from "@playwright/test";
import { addChart, frames, openBlank } from "./helpers.ts";

test.beforeEach(async ({ page }) => {
  await openBlank(page);
});

test("a heatmap-like back layer draws as one cols × 2·rows image, with both renderers", async ({ page }) => {
  const r = await page.evaluate(async () => {
    const gp = window.gp;
    const c = gp.chart("line");
    await c.ready;
    const palette = c.palette({});
    c.free();
    const out: Record<string, unknown> = {};
    for (const name of ["text", "atlas"] as const) {
      const canvas = document.createElement("canvas");
      document.body.append(canvas);
      const r = gp.makeRenderer(name, canvas);
      r.setPalette(palette, true);
      const cols = 40;
      const rows = 10;
      r.resize(cols, rows, 8);
      const images: [number, number, number, number, number, number][] = [];
      const fills = { n: 0 };
      const ctx = r.ctx;
      const drawImage = ctx.drawImage.bind(ctx) as (...a: unknown[]) => void;
      ctx.drawImage = ((...a: unknown[]) => {
        const src = a[0] as { width: number; height: number };
        if (a.length === 9) images.push([src.width, src.height, a[5] as number, a[6] as number, a[7] as number, a[8] as number]);
        drawImage(...a);
      }) as typeof ctx.drawImage;
      const fillRect = ctx.fillRect.bind(ctx);
      ctx.fillRect = (x: number, y: number, w: number, h: number) => {
        fills.n++;
        fillRect(x, y, w, h);
      };
      // Every cell a back from the scale steps (32–159), a half block over every fourth.
      const n = cols * rows;
      const grid = { cols, rows, cells: new Uint32Array(n).fill(32), colors: new Uint8Array(n), backs: new Uint8Array(n) };
      for (let i = 0; i < n; i++) {
        grid.backs[i] = 32 + ((i * 7) % 128);
        if (i % 4 === 0) grid.cells[i] = 0x2580;
      }
      const first = r.draw(grid);
      const firstImages = images.filter((im) => im[0] === cols && im[1] === 2 * rows).length;
      const firstFills = fills.n;
      // One cell changes: a cell's fill, not an image.
      images.length = 0;
      fills.n = 0;
      grid.backs[5] = 100;
      const one = r.draw(grid);
      const oneImages = images.filter((im) => im[0] === cols && im[1] === 2 * rows).length;
      // A pan changes every cell: one image again.
      images.length = 0;
      for (let i = 0; i < n; i++) grid.backs[i] = 32 + ((i * 7 + 3) % 128);
      const pan = r.draw(grid);
      const panImage = images.find((im) => im[0] === cols && im[1] === 2 * rows) ?? null;
      out[name] = { first, firstImages, firstFills, smoothing: ctx.imageSmoothingEnabled, one, oneImages, pan, panImage, count: (r as unknown as { backImages: number }).backImages };
      canvas.remove();
    }
    return out;
  });
  for (const name of ["text", "atlas"]) {
    const x = r[name] as Record<string, unknown>;
    expect(x["first"]).toEqual({ cells: 400, full: true, backImage: true });
    expect(x["firstImages"]).toBe(1);
    // The ground once, and the half blocks' exact fills: no fill a back.
    expect(x["firstFills"]).toBeLessThanOrEqual(1 + 100);
    expect(x["smoothing"]).toBe(false);
    expect(x["one"]).toEqual({ cells: 1, full: false, backImage: false });
    expect(x["oneImages"]).toBe(0);
    expect(x["pan"]).toEqual({ cells: 400, full: true, backImage: true });
    // Scaled to the whole canvas: 40 × 8 by 10 × 16 device px.
    expect(x["panImage"]).toEqual([40, 20, 0, 0, 320, 160]);
    expect(x["count"]).toBe(2);
  }
});

test("up and down are tokens: --glyphplot-up and --glyphplot-down, and the theme object over them", async ({ page }) => {
  await page.evaluate(() => {
    const w = window as unknown as { __themes: { theme: Record<string, unknown>; up: number[]; down: number[] }[] };
    w.__themes = [];
    const proto = window.gp.Chart.prototype;
    const palette = proto.palette;
    proto.palette = function (this: InstanceType<typeof window.gp.Chart>, theme: object) {
      const bytes = palette.call(this, theme);
      w.__themes.push({ theme: { ...(theme as Record<string, unknown>) }, up: [...bytes.slice(20, 23)], down: [...bytes.slice(24, 27)] });
      return bytes;
    };
  });
  const last = (): Promise<{ theme: Record<string, unknown>; up: number[]; down: number[] } | undefined> =>
    page.evaluate(() => (window as unknown as { __themes: { theme: Record<string, unknown>; up: number[]; down: number[] }[] }).__themes.at(-1));
  const el = await addChart(page, { attrs: { theme: "dark" } });
  // The defaults: the dark set's blue and orange (theme.rs, REQ-THEME-065).
  expect(await last()).toMatchObject({ up: [0x56, 0xb4, 0xe9], down: [0xe6, 0x9f, 0x00] });
  await el.evaluate((e) => {
    (e as HTMLElement).style.setProperty("--glyphplot-up", "#123456");
    (e as HTMLElement).style.setProperty("--glyphplot-down", "rgb(101, 67, 33)");
  });
  await page.evaluate(() => document.documentElement.classList.add("re-theme")); // a page's class change re-resolves (REQ-THEME-030)
  await frames(page, 2);
  expect(await last()).toMatchObject({ theme: { up: "#123456", down: "#654321" }, up: [0x12, 0x34, 0x56], down: [0x65, 0x43, 0x21] });
  // The theme object wins over the custom property, token by token.
  await el.evaluate((e) => ((e as unknown as { theme: object }).theme = { down: "#00ff00" }));
  await frames(page, 2);
  expect(await last()).toMatchObject({ theme: { up: "#123456", down: "#00ff00" }, up: [0x12, 0x34, 0x56], down: [0, 0xff, 0] });
});
