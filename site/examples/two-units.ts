// 15 · Two units: temperature and humidity, an hour apart for five days. The
// catalogue draws them on one chart with a left and a right y axis; the core
// lays out dual axes (REQ-AXIS-010) but no chart reads the `axes` option at
// v0.7.0, so here each unit has its own chart, stacked, and the two share x
// and the crosshair through `link` (REQ-LINK-010): pan or hover one and the
// other follows.

import type { Example } from "./api.ts";
import { weather } from "../src/data/gen.ts";
import { EPOCH, HOUR, table, times } from "../src/data/table.ts";
import { plot, Scope } from "./support/ui.ts";

const SEED = 15;

function series() {
  const t = times(EPOCH, 5 * 24, HOUR);
  return { t, ...weather(SEED, t) };
}

export const example: Example = {
  slug: "two-units",
  seed: SEED,
  start() {
    const { t, temperature } = series();
    return {
      type: "line",
      data: table("weather", [
        { name: "t", type: "timestamp", values: t },
        { name: "temperature", unit: "°C", values: temperature },
      ]),
      attrs: { link: "two-units" },
    };
  },
  live(_el, host) {
    const s = new Scope();
    const { t, humidity } = series();
    plot(s, host, { type: "line", rows: "10", link: "two-units", label: "Relative humidity" }, {
      data: table("weather", [
        { name: "t", type: "timestamp", values: t },
        { name: "humidity", unit: "%", values: humidity },
      ]),
    });
    return s.stop;
  },
};
