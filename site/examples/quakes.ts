// 51 · Earthquakes by the hour, and by the day: the card counts the USGS's
// checked-in day of events (219 to 2026-10-20T20:04Z, every magnitude) by UTC
// hour. The button fetches the past 30 days from the USGS feed in the
// reader's browser (all_month.csv, 2,020,020 bytes when measured with curl
// on 2026-10-23) and counts it by UTC day: a month of daily counts. If the
// feed is down, the hourly counts stay and a message says so.

import type { Example } from "./api.ts";
import { col, usgsDay } from "../src/data/real.ts";
import { fetchFeed } from "./support/fetch.ts";
import { button, controls, Scope, status } from "./support/ui.ts";

const FEED = "https://earthquake.usgs.gov/earthquakes/feed/v1.0/summary/all_month.csv";
const SOURCE = "USGS Earthquake Hazards Program, all events of the past 30 days; public domain, credit USGS";

/** Counts of `times` (epoch ms) by bucket of `size` ms, labelled by `label`, every bucket from the first to the last. */
function counts(times: number[], size: number, label: (t: number) => string): { when: string[]; events: number[] } {
  const lo = Math.floor(Math.min(...times) / size);
  const hi = Math.floor(Math.max(...times) / size);
  const events = new Array<number>(hi - lo + 1).fill(0);
  for (const t of times) {
    const k = Math.floor(t / size) - lo;
    events[k] = (events[k] ?? 0) + 1;
  }
  return { when: events.map((_, i) => label((lo + i) * size)), events };
}

const HOUR = 3_600_000;
const DAY = 24 * HOUR;

export const example: Example = {
  slug: "quakes",
  seed: null,
  start() {
    return { type: "bar", data: counts(col(usgsDay, "time"), HOUR, (t) => `${new Date(t).toISOString().slice(11, 13)}h`), options: { title: "Events an hour, UTC (USGS)" } };
  },
  live(el, host) {
    const s = new Scope();
    const row = controls(s, host, "Data");
    const out = status(s, host, "One day, checked in. The button loads the past 30 days from the USGS in your browser.");
    button(s, row, "Load the past 30 days (USGS)", () => {
      out.textContent = `Loading from ${SOURCE}…`;
      void fetchFeed<string>(FEED, "text", 60_000).then((got) => {
        if (s.stopped) return;
        if (!got.ok) {
          out.textContent = `The USGS feed is not available right now (${got.message}); the chart still shows the checked-in day.`;
          return;
        }
        const lines = got.value.split("\n");
        const at = (lines[0] ?? "").split(",").indexOf("time");
        const times = lines.slice(1).flatMap((l) => {
          const t = Date.parse(l.split(",")[at] ?? "");
          return at >= 0 && Number.isFinite(t) ? [t] : [];
        });
        if (times.length === 0) {
          out.textContent = "The USGS feed answered with no events; the chart still shows the checked-in day.";
          return;
        }
        el.data = counts(times, DAY, (t) => new Date(t).toISOString().slice(5, 10));
        el.options = { title: "Events a day, UTC (USGS, live)" };
        out.textContent = `Live: ${SOURCE}. ${times.length} events.`;
      });
    });
    return s.stop;
  },
};
