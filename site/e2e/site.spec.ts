// The Elm site in headless Chromium, over `vite preview` of the release build
// (REQ-SITE-010): axe on the index, a chart page, the gallery and an example
// in both themes (REQ-SITE-070); the theme button (REQ-SITE-060); the
// gallery's loading, no wasm before the reader acts and each family's module
// once after (REQ-SITE-120, Phase 6's acceptance); its filters (REQ-SITE-040);
// and an example's source, copied (REQ-SITE-040). Requests to any host but
// the preview server are refused, so no test reaches a live feed.

import AxeBuilder from "@axe-core/playwright";
import { expect, type Page, test } from "@playwright/test";
import type { GlyphPlot } from "glyphplot/element";
import examples from "../examples/manifest.json" with { type: "json" };
import { DOCS } from "./helpers.ts";

test.beforeEach(async ({ page, baseURL }) => {
  const own = new URL(baseURL ?? "http://localhost/").origin;
  await page.route(
    (url) => url.origin !== own,
    (route) => route.abort("blockedbyclient"),
  );
});

/** Waits until every `<glyph-plot>` under `scope` that has begun loading has drawn its first frame. */
async function chartsDrawn(page: Page, scope = "main"): Promise<void> {
  await page.waitForFunction((scope) => {
    const plots = [...document.querySelectorAll(`${scope} glyph-plot`)] as GlyphPlot[];
    return plots.length > 0 && plots.some((e) => e.chart !== null);
  }, scope);
  await page.evaluate(async (scope) => {
    const plots = ([...document.querySelectorAll(`${scope} glyph-plot`)] as GlyphPlot[]).filter((e) => e.chart !== null);
    await Promise.all(plots.map((e) => e.ready));
  }, scope);
}

/** Serious and critical axe violations on the page, one line each. */
async function axe(page: Page): Promise<string[]> {
  const result = await new AxeBuilder({ page }).withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa", "best-practice"]).analyze();
  return result.violations
    .filter((v) => v.impact === "serious" || v.impact === "critical")
    .map((v) => `${v.impact} ${v.id}: ${v.help} (${v.nodes.length}): ${v.nodes.slice(0, 3).map((n) => n.target.join(" ")).join("; ")}`);
}

const PAGES = [
  { name: "the index", path: "", charts: true },
  { name: "a chart page", path: "candles", charts: true },
  { name: "the gallery", path: "examples", charts: false },
  { name: "an example", path: "examples/live-market", charts: true },
] as const;

for (const scheme of ["light", "dark"] as const) {
  for (const p of PAGES) {
    test(`axe finds no serious or critical violation on ${p.name}, ${scheme}${p.path === "examples/live-market" ? " @smoke" : ""}`, async ({ page }) => {
      await page.emulateMedia({ colorScheme: scheme });
      await page.goto(p.path);
      await expect(page.locator("main h1")).toBeVisible();
      if (p.charts) await chartsDrawn(page);
      // The site follows the scheme with no choice stored (REQ-SITE-060).
      expect(await page.evaluate(() => document.documentElement.dataset["theme"] ?? null)).toBeNull();
      const report = await axe(page);
      expect(report, report.join("\n")).toEqual([]);
    });
  }
}

test("the index shows before any wasm is fetched, and the docs sidebar lists every doc page", async ({ page }) => {
  // Each wasm request is held until the page is checked for its heading (REQ-SITE-010).
  const shownFirst: boolean[] = [];
  await page.route("**/*.wasm", async (route) => {
    shownFirst.push((await page.locator("main h1").count()) > 0);
    await route.continue();
  });
  await page.goto("");
  await chartsDrawn(page);
  expect(shownFirst.length).toBeGreaterThan(0);
  expect(shownFirst.every(Boolean)).toBe(true);
  await page.goto("docs/getting-started");
  const links = await page.locator("nav.sidebar a").evaluateAll((as) => as.map((a) => new URL((a as HTMLAnchorElement).href).pathname));
  expect(links).toEqual(DOCS.map((d) => `/docs/${d}`));
});

test("the theme button sets data-theme and localStorage, the charts follow, and a reload keeps it @smoke", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "light" });
  await page.goto("");
  await chartsDrawn(page);
  const plot = page.locator("#home-plot");
  const ground = (): Promise<string> => plot.evaluate((e) => (e as HTMLElement).style.getPropertyValue("--gp-ground"));
  const light = await ground();
  expect(light).not.toBe("");
  const button = page.locator(".theme-button");
  await expect(button).toHaveAccessibleName(/Switch to dark/);
  await button.click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  expect(await page.evaluate(() => localStorage.getItem("glyphplot-theme"))).toBe("dark");
  // The chart repaints in the dark theme's ground.
  await expect.poll(ground).not.toBe(light);
  const dark = await ground();
  await page.reload();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await chartsDrawn(page);
  await expect.poll(ground).toBe(dark);
  // Follow system clears the choice; the page follows the scheme again.
  await page.getByRole("button", { name: "Follow system" }).click();
  await expect(page.locator("html")).not.toHaveAttribute("data-theme", /.+/);
  expect(await page.evaluate(() => localStorage.getItem("glyphplot-theme"))).toBeNull();
  await expect.poll(ground).toBe(light);
});

test("the gallery fetches no wasm before a scroll, then each family's module once @smoke", async ({ page }) => {
  const wasm: string[] = [];
  page.on("request", (r) => {
    if (r.url().endsWith(".wasm")) wasm.push(new URL(r.url()).pathname);
  });
  await page.goto("examples");
  await expect(page.locator(".card")).toHaveCount(examples.length);
  expect(examples.length).toBeGreaterThanOrEqual(50);
  // The cards show their build-time stills.
  const stills = await page.locator("gp-example .gp-ex-still").evaluateAll((els) => els.filter((e) => (e.textContent ?? "").trim() !== "").length);
  expect(stills).toBeGreaterThanOrEqual(examples.length - 2);
  await page.waitForTimeout(1500);
  expect(wasm, "wasm fetched before a scroll").toEqual([]);
  expect(await page.locator("gp-example glyph-plot").count()).toBe(0);
  // Scroll to the end with the wheel, as a reader would.
  await page.mouse.move(500, 400);
  const height = await page.evaluate(() => document.documentElement.scrollHeight);
  for (let y = 0; y <= height; y += 700) {
    await page.mouse.wheel(0, 700);
    await page.waitForTimeout(80);
  }
  await expect.poll(() => page.locator("gp-example glyph-plot").count()).toBeGreaterThan(10);
  await page.waitForTimeout(1500);
  const byPath = new Map<string, number>();
  for (const p of wasm) byPath.set(p, (byPath.get(p) ?? 0) + 1);
  // One module a family the gallery uses (line, candles, bar), each fetched once.
  expect([...byPath.values()].every((n) => n === 1), JSON.stringify([...byPath])).toBe(true);
  expect(byPath.size).toBe(3);
});

test("each filter narrows the cards to the manifest's", async ({ page }) => {
  await page.goto("examples");
  const cards = page.locator(".card");
  const has: Record<string, (e: (typeof examples)[number], v: string) => boolean> = {
    type: (e, v) => e.types.includes(v),
    mode: (e, v) => e.modes.includes(v),
    domain: (e, v) => e.domain === v,
    feature: (e, v) => e.features.includes(v),
    milestone: (e, v) => e.milestone === v,
  };
  for (const [key, match] of Object.entries(has)) {
    const select = page.locator(`#filter-${key}`);
    const values = await select.locator("option").evaluateAll((os) => os.map((o) => (o as HTMLOptionElement).value).filter((v) => v !== ""));
    expect(values.length, key).toBeGreaterThan(0);
    const v = values[0] as string;
    await select.selectOption(v);
    await expect(cards.locator("h2"), `${key}=${v}`).toHaveText(examples.filter((e) => match(e, v)).map((e) => e.title));
    await select.selectOption("");
    await expect(cards).toHaveCount(examples.length);
  }
});

test("each card says where its data comes from and links to its page, which shows the chart full size", async ({ page }) => {
  await page.goto("examples");
  const cards = page.locator(".card");
  await expect(cards).toHaveCount(examples.length);
  // Generated data says "simulated" and its seed on the card (REQ-SITE-110); a live feed names its source (REQ-SITE-050):
  // <gp-example>'s own line, the one the card shows, which goes with the example wherever it is used.
  for (const [i, e] of examples.entries()) {
    const card = cards.nth(i);
    if (e.datasets.includes("synthetic")) await expect(card.locator(".gp-ex-provenance")).toContainText(/Simulated (data|market), seed \d+/);
    if (e.feed) await expect(card.locator(".gp-ex-provenance")).toContainText(e.feed.source);
  }
  const e = examples.find((x) => x.datasets.includes("synthetic") && !x.feed) ?? examples[0];
  if (!e) throw new Error("no example");
  await cards.locator("h2 a", { hasText: e.title }).click();
  await expect(page).toHaveURL(new RegExp(`/examples/${e.slug}$`));
  await expect(page.locator("main h1")).toHaveText(e.title);
  await expect(page.locator("gp-example[full]")).toHaveAttribute("slug", e.slug);
  await expect(page.locator("main .gp-ex-provenance").first()).toContainText(/Simulated (data|market), seed \d+/);
});

test("a filter narrows the cards and keeps itself in the URL @smoke", async ({ page }) => {
  await page.goto("examples");
  const cards = page.locator(".card");
  await expect(cards).toHaveCount(examples.length);
  const candles = examples.filter((e) => e.types.includes("candles"));
  await page.locator("#filter-type").selectOption("candles");
  await expect(cards).toHaveCount(candles.length);
  expect(candles.length).toBeLessThan(examples.length);
  await expect(page.locator("main > p[role=status]")).toHaveText(`${candles.length} of ${examples.length} examples`);
  await expect(page).toHaveURL(/\/examples\?type=candles$/);
  // A second filter narrows further; the titles are the manifest's.
  const both = candles.filter((e) => e.modes.includes("hollow"));
  await page.locator("#filter-mode").selectOption("hollow");
  await expect(cards.locator("h2")).toHaveText(both.map((e) => e.title));
  // Clear shows every card again.
  await page.getByRole("button", { name: "Clear filters" }).click();
  await expect(cards).toHaveCount(examples.length);
  // A filtered URL opened afresh shows the same cards.
  await page.goto("examples?type=candles");
  await expect(cards).toHaveCount(candles.length);
});

test("an example's page shows its source, and the copy button copies it", async ({ page, context }) => {
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  await page.goto("examples/hello-line");
  const code = page.locator("figure.code pre code");
  await expect(code).toContainText('slug: "hello-line"');
  const source = (await code.textContent()) ?? "";
  await expect(page.locator("figure.code figcaption")).toContainText("site/examples/hello-line.ts");
  const copy = page.locator("figure.code button.copy");
  await copy.click();
  await expect(copy).toHaveText("Copied");
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(source);
});
