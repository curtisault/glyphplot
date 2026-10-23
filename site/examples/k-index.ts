// 50 · Geomagnetic storms: the planetary K-index every three hours for a
// week, a bar each, coloured by storm level: each level is its own series,
// stacked, so a bar takes its level's colour and, in one ink, its level's
// fill (REQ-MONO-030). Kp below 4 is quiet, 4 active, 5 and above a storm
// (NOAA's G1 at 5). The card draws a simulated week; the button loads the
// past week from NOAA SWPC in the reader's browser, and keeps the simulation
// with a message if SWPC is down.

import type { Example } from "./api.ts";
import { kIndex } from "../src/data/gen.ts";
import { EPOCH, HOUR } from "../src/data/table.ts";
import { fetchFeed } from "./support/fetch.ts";
import { button, controls, Scope, status } from "./support/ui.ts";

const SEED = 50;
const FEED = "https://services.swpc.noaa.gov/products/noaa-planetary-k-index.json";
const SOURCE = "NOAA SWPC planetary K-index; public domain (U.S. government work)";

/** Bars by level: a category a reading, one column a level, the reading in its level's column and 0 in the others. */
function byLevel(times: number[], kp: number[]): Record<string, (string | number)[]> {
  const level = (k: number) => (k >= 5 ? "storm" : k >= 4 ? "active" : "quiet");
  return {
    time: times.map((t) => `${new Date(t).getUTCDate()} ${new Date(t).toISOString().slice(11, 13)}h`),
    quiet: kp.map((k) => (level(k) === "quiet" ? k : 0)),
    active: kp.map((k) => (level(k) === "active" ? k : 0)),
    storm: kp.map((k) => (level(k) === "storm" ? k : 0)),
  };
}

export const example: Example = {
  slug: "k-index",
  seed: SEED,
  start() {
    const kp = Array.from(kIndex(SEED, 56));
    return { type: "bar", data: byLevel(kp.map((_, i) => EPOCH + i * 3 * HOUR), kp), options: { stack: true, title: "Kp every 3 hours" } };
  },
  live(el, host) {
    const s = new Scope();
    const row = controls(s, host, "Data");
    const out = status(s, host, "Simulated week. The button loads the past week from NOAA SWPC in your browser.");
    button(s, row, "Load the past week (NOAA SWPC)", () => {
      out.textContent = `Loading from ${SOURCE}…`;
      void fetchFeed<unknown>(FEED, "json").then((got) => {
        if (s.stopped) return;
        if (!got.ok || !Array.isArray(got.value)) {
          out.textContent = `The NOAA SWPC feed is not available right now (${got.ok ? "it answered with something else" : got.message}); the chart still shows the simulation.`;
          return;
        }
        const times: number[] = [];
        const kp: number[] = [];
        for (const r of got.value as { time_tag?: string; Kp?: number; kp_index?: number }[]) {
          const t = Date.parse(`${r.time_tag ?? ""}Z`);
          const k = r.Kp ?? r.kp_index;
          if (Number.isFinite(t) && typeof k === "number") {
            times.push(t);
            kp.push(k);
          }
        }
        if (kp.length === 0) {
          out.textContent = "NOAA SWPC answered with no readings; the chart still shows the simulation.";
          return;
        }
        el.data = byLevel(times.slice(-56), kp.slice(-56));
        out.textContent = `Live: ${SOURCE}. ${Math.min(56, kp.length)} readings to ${new Date(times[times.length - 1] as number).toISOString().slice(0, 16)}Z.`;
      });
    });
    return s.stop;
  },
};
