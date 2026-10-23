// 46 · The last price: a crosshair that snaps to the nearest of open, high,
// low and close (the magnet, REQ-CANDLE-050), with a readout of `o h l c`,
// volume and the signed change. The catalogue also draws a horizontal line at
// the last price; `candles` reads no `annotations` option at v0.7.0, so the
// last close is written beside the chart instead, from the data.

import type { Example } from "./api.ts";
import { columns, daily } from "../src/data/market.ts";
import { DAY, EPOCH } from "../src/data/table.ts";
import { Scope, status } from "./support/ui.ts";

const SEED = 46;
const candles = () => daily(SEED, 50, EPOCH - 70 * DAY);

export const example: Example = {
  slug: "last-price",
  seed: SEED,
  start() {
    return { type: "candles", data: columns(candles()), attrs: { label: "Simulated market with a magnet crosshair" } };
  },
  live(el, host) {
    const s = new Scope();
    const cs = candles();
    const last = cs[cs.length - 1];
    const prev = cs[cs.length - 2];
    const lastText = last && prev ? `Last close ${last.close.toFixed(2)} (${last.close >= prev.close ? "+" : "−"}${Math.abs(last.close - prev.close).toFixed(2)} on the day before), simulated.` : "";
    const out = status(s, host, lastText);
    s.on(el, "plot-hover", (e: CustomEvent<{ series?: string; y?: number | null } | null>) => {
      out.textContent = e.detail ? `Crosshair on ${e.detail.series}: ${e.detail.y ?? "no value"}. ${lastText}` : lastText;
    });
    return s.stop;
  },
};
