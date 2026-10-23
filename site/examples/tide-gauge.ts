// 18 · A tide gauge: water level against the tide prediction over two days.
// The card draws a simulated gauge (four harmonic constituents as the
// prediction, a seeded surge on top for the observation); the button loads
// the real one in the reader's browser from NOAA CO-OPS (The Battery, New
// York, station 8518750: the past 48 hours, six-minute values, metres above
// MLLW, UTC). If CO-OPS is down the chart keeps the simulation and says so.

import type { Example } from "./api.ts";
import { tide } from "../src/data/gen.ts";
import { DAY, EPOCH, MINUTE, table, times } from "../src/data/table.ts";
import { fetchFeed } from "./support/fetch.ts";
import { button, controls, Scope, status } from "./support/ui.ts";

const SEED = 18;
const API = "https://api.tidesandcurrents.noaa.gov/api/prod/datagetter";
const SOURCE = "NOAA CO-OPS, The Battery, NY (8518750); public domain (U.S. government work)";

/** One CO-OPS product for the past 48 hours, as `[epoch ms, metres]`. */
async function product(name: "water_level" | "predictions"): Promise<[number, number][] | string> {
  const q = new URLSearchParams({ product: name, application: "glyphplot", range: "48", station: "8518750", datum: "MLLW", units: "metric", time_zone: "gmt", format: "json" });
  const got = await fetchFeed<Record<string, unknown>>(`${API}?${q}`, "json");
  if (!got.ok) return got.message;
  const rows = got.value[name === "water_level" ? "data" : "predictions"];
  if (!Array.isArray(rows)) return "it answered without data";
  return rows.flatMap((r: { t?: string; v?: string }) => {
    const t = Date.parse(`${(r.t ?? "").replace(" ", "T")}:00Z`);
    const v = Number.parseFloat(r.v ?? "");
    return Number.isFinite(t) && Number.isFinite(v) ? [[t, v] as [number, number]] : [];
  });
}

export const example: Example = {
  slug: "tide-gauge",
  seed: SEED,
  start() {
    const t = times(EPOCH, (2 * DAY) / (6 * MINUTE), 6 * MINUTE);
    const { observed, predicted } = tide(SEED, t);
    return {
      type: "line",
      data: table("tide", [
        { name: "t", type: "timestamp", values: t },
        { name: "observed", unit: "m", values: observed },
        { name: "predicted", unit: "m", values: predicted },
      ]),
    };
  },
  live(el, host) {
    const s = new Scope();
    const row = controls(s, host, "Data");
    const out = status(s, host, "Simulated gauge. The button loads the real one from NOAA CO-OPS in your browser.");
    let busy = false;
    button(s, row, "Load the live gauge (NOAA CO-OPS)", () => {
      if (busy) return;
      busy = true;
      out.textContent = `Loading from ${SOURCE}…`;
      void Promise.all([product("water_level"), product("predictions")]).then(([obs, pred]) => {
        busy = false;
        if (typeof obs === "string" || typeof pred === "string") {
          out.textContent = `The NOAA CO-OPS feed is not available right now (${typeof obs === "string" ? obs : pred}); the chart still shows the simulation.`;
          return;
        }
        const level = new Map(obs);
        const t = pred.map(([x]) => x);
        el.data = table("tide", [
          { name: "t", type: "timestamp", values: t },
          { name: "observed", unit: "m", values: t.map((x) => level.get(x) ?? null) },
          { name: "predicted", unit: "m", values: pred.map(([, v]) => v) },
        ]);
        out.textContent = `Live: ${SOURCE}. Metres above mean lower low water, UTC; ${obs.length} observations.`;
      });
    });
    return s.stop;
  },
};
