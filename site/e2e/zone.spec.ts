// REQ-TZ-020, REQ-TZ-050: the `timeZone` attribute takes UTC, a fixed offset
// or an IANA name, the page's zone by default, and the element gives the core
// a table of offset transitions (zone.schema.json) covering the data's time
// extent and the view, built again when the view moves past it.

import { expect, test } from "@playwright/test";
import type { GlyphPlot } from "glyphplot/element";
import type { ZoneTable } from "glyphplot";
import { addChart, frames, openBlank } from "./helpers.ts";

type El = GlyphPlot;

const HOUR = 3_600_000;
/** Hourly values over 2026-04-03 to 2026-04-06 UTC: Lord Howe leaves daylight time on 2026-04-04 at 15:00 UTC. */
function hourly(): { t: Date[]; y: number[] } {
  const t: Date[] = [];
  const y: number[] = [];
  for (let i = 0; i < 72; i++) {
    t.push(new Date(Date.UTC(2026, 3, 3) + i * HOUR));
    y.push(Math.sin(i / 6));
  }
  return { t, y };
}

const zoneOf = (el: import("@playwright/test").Locator): Promise<ZoneTable | null> => el.evaluate((e) => (e as El).chart?.zone ?? null);

test.beforeEach(async ({ page }) => {
  await openBlank(page);
});

test.describe("in a page in Lord Howe's zone", () => {
  test.use({ timezoneId: "Australia/Lord_Howe" });

  test("the default is the page's zone, passed explicitly with its half-hour change", async ({ page }) => {
    const el = await addChart(page, { data: hourly() });
    const z = await zoneOf(el);
    expect(z?.name).toBe("Australia/Lord_Howe");
    expect(z?.transitions[0]?.offset).toBe(660);
    expect(z?.transitions).toContainEqual({ at: Date.UTC(2026, 3, 4, 15), offset: 630 });
    // Covering the data's extent.
    expect(z?.covering[0]).toBeLessThanOrEqual(Date.UTC(2026, 3, 3));
    expect(z?.covering[1]).toBeGreaterThanOrEqual(Date.UTC(2026, 3, 5, 23));
  });
});

test("UTC, a fixed offset, an IANA name; a name Intl does not know warns and is UTC", async ({ page }) => {
  const warnings: string[] = [];
  page.on("console", (m) => {
    if (m.type() === "warning") warnings.push(m.text());
  });
  const el = await addChart(page, { data: hourly(), attrs: { timezone: "UTC" } });
  expect((await zoneOf(el))?.transitions).toEqual([{ at: (await zoneOf(el))?.covering[0], offset: 0 }]);
  await el.evaluate((e) => e.setAttribute("timeZone", "+05:30"));
  await frames(page, 2);
  let z = await zoneOf(el);
  expect(z?.name).toBe("+05:30");
  expect(z?.transitions.map((t) => t.offset)).toEqual([330]);
  await el.evaluate((e) => e.setAttribute("timezone", "America/New_York"));
  await frames(page, 2);
  z = await zoneOf(el);
  expect(z?.name).toBe("America/New_York");
  expect(z?.transitions.map((t) => t.offset)).toEqual([-240]);
  await el.evaluate((e) => e.setAttribute("timezone", "Mars/Olympus_Mons"));
  await frames(page, 2);
  z = await zoneOf(el);
  expect(z?.name).toBe("UTC");
  expect(warnings.filter((w) => w.includes("Mars/Olympus_Mons"))).toHaveLength(1);
});

test("a view moved past the table's span gets a new table covering it", async ({ page }) => {
  const el = await addChart(page, { data: hourly(), attrs: { timezone: "America/New_York" } });
  const before = await zoneOf(el);
  // November 2026, past the span: New York leaves daylight time on 1 November at 06:00 UTC.
  const later: [number, number] = [Date.UTC(2026, 9, 31), Date.UTC(2026, 10, 3)];
  await el.evaluate((e, d) => ((e as El).view = { x: { domain: d } }), later);
  await frames(page, 2);
  const after = await zoneOf(el);
  expect(after).not.toEqual(before);
  expect(after?.covering[0]).toBeLessThanOrEqual(Date.UTC(2026, 3, 3));
  expect(after?.covering[1]).toBeGreaterThanOrEqual(later[1]);
  expect(after?.transitions).toContainEqual({ at: Date.UTC(2026, 10, 1, 6), offset: -300 });
  // A view inside the span keeps the table.
  await el.evaluate((e) => ((e as El).view = { x: { domain: [Date.UTC(2026, 3, 4), Date.UTC(2026, 3, 5)] } }));
  await frames(page, 2);
  expect(await zoneOf(el)).toEqual(after);
});
