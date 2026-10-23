// 45 · Session gaps: six weeks of weekday sessions on the index time scale
// (the default `xScale: "index"`, REQ-SCALE-060, ADR-131), where each
// session sits next to the one before whatever the time between them, so the
// weekends leave no empty columns; labels still come from each candle's date.
// Live, the same sessions on a plain time scale (`xScale: "time"`) sit under
// it, with a gap every weekend.

import type { Example } from "./api.ts";
import { columns, daily } from "../src/data/market.ts";
import { DAY, EPOCH } from "../src/data/table.ts";
import { caption, plot, Scope } from "./support/ui.ts";

const SEED = 45;
const data = () => columns(daily(SEED, 30, EPOCH - 42 * DAY));

export const example: Example = {
  slug: "session-gaps",
  seed: SEED,
  start() {
    return { type: "candles", data: data(), options: { xScale: "index", volume: false, title: "xScale: index" } };
  },
  live(_el, host) {
    const s = new Scope();
    caption(s, host, "The same sessions on a time scale (xScale: time)");
    plot(s, host, { type: "candles", rows: "12", label: "Sessions on a time scale" }, { data: data(), options: { xScale: "time", volume: false } });
    return s.stop;
  },
};
