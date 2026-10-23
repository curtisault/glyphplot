// 1 · Live candles, simulated market (REQ-SITE-040, ADR-136): the owner's
// one-line test. A seeded walk folded into 5-second candles, a tick every
// 250 ms appended with `append`, the forming candle replaced in place; the
// volume pane under the price; the view follows the live end until the reader
// pans away (End or the button brings it back); one finger scrubs the
// crosshair on a phone. Labelled "simulated market": no exchange, no real
// price.

import type { Example } from "./api.ts";
import { columns, Market, row } from "../src/data/market.ts";
import { EPOCH, HOUR } from "../src/data/table.ts";
import { button, controls, pauseButton, Scope, status } from "./support/ui.ts";

const SEED = 2026;
const PERIOD = 5000;
/** The simulated clock starts here; it then runs at the reader's pace. */
const T0 = EPOCH + 14 * HOUR;

/** The market and the history every run starts from. */
function market(): { m: Market; history: ReturnType<typeof columns>; last: number } {
  const m = new Market(SEED, PERIOD);
  const h = m.history(240, T0);
  return { m, history: columns(h), last: h[h.length - 1]?.t ?? T0 };
}

export const example: Example = {
  slug: "live-market",
  seed: SEED,
  start() {
    const { history, last } = market();
    return {
      type: "candles",
      data: history,
      view: { x: { domain: [last - 60 * PERIOD, last], follow: true } },
      attrs: { label: "Simulated market, live 5-second candles with volume" },
    };
  },
  live(el, host) {
    const s = new Scope();
    // A fresh market replays the start's history, then ticks on from it.
    const { m } = market();
    m.history(240, T0);
    const began = performance.now();
    s.every(250, () => el.append(row(m.tick(T0 + (performance.now() - began)))));
    const row1 = controls(s, host, "Feed");
    pauseButton(s, row1, el);
    button(s, row1, "Follow the live end", () => {
      el.view = { x: { follow: true } };
    });
    const out = status(s, host, "simulated market: waiting for the first tick");
    s.on(el, "plot-data", (e: CustomEvent<{ rows: number; version: number }>) => {
      const x = el.view["x"] as { follow?: boolean } | undefined;
      out.textContent = `simulated market: ${e.detail.rows} candles held; ${el.paused ? "paused" : "live"}; following the live end: ${x?.follow ? "yes" : "no"}`;
    });
    return s.stop;
  },
};
