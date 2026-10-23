// 35 · The hover event, as React 19 would bind it. React 19 passes custom
// elements' properties through and attaches `on<event>` props as listeners,
// so in JSX this chart is
//
//   <glyph-plot type="candles" options={{ mode: "hollow" }} data={candles}
//     onplot-hover={(e) => setPrice(e.detail?.datum ?? null)} />
//
// The site has no React (no JSX typings ship before packaging, v0.11.0), so
// the example attaches the same listener with addEventListener: each
// `plot-hover` carries the snapped candle's row in `detail.datum`, or null
// when the cursor leaves (REQ-EVENT-020).

import type { Example } from "./api.ts";
import { columns, daily } from "../src/data/market.ts";
import { DAY, EPOCH } from "../src/data/table.ts";
import { Scope, status } from "./support/ui.ts";

const SEED = 35;

export const example: Example = {
  slug: "hover-event",
  seed: SEED,
  start() {
    return { type: "candles", data: columns(daily(SEED, 60, EPOCH - 90 * DAY)), options: { mode: "hollow", volume: false }, attrs: { label: "Hollow candles of a simulated market" } };
  },
  live(el, host) {
    const s = new Scope();
    const out = status(s, host, "Hover a candle.");
    s.on(el, "plot-hover", (e: CustomEvent<{ datum?: Record<string, number | string | null> } | null>) => {
      const d = e.detail?.datum;
      out.textContent = d ? `${new Date(Number(d["t"])).toISOString().slice(0, 10)}: open ${d["open"]}, high ${d["high"]}, low ${d["low"]}, close ${d["close"]} (simulated)` : "Hover a candle.";
    });
    return s.stop;
  },
};
