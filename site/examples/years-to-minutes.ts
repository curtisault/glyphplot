// 44 · From years to minutes: a million closes a minute apart (about 1.9
// years) given to `candles` as one value column. Fitted, the candles are
// months; zoom in (Ctrl or ⌘ with the wheel, a pinch, or + on the focused
// chart) and they step down the calendar ladder, weeks, days, hours, down to
// the minute (REQ-SAMPLE-040), each bucket aligned to its interval.

import type { Example } from "./api.ts";
import { minuteCloses } from "../src/data/market.ts";
import { EPOCH, table } from "../src/data/table.ts";

const SEED = 44;
const N = 1_000_000;

export const example: Example = {
  slug: "years-to-minutes",
  seed: SEED,
  start() {
    const { t, close } = minuteCloses(SEED, N, EPOCH - 700 * 86_400_000);
    return {
      type: "candles",
      data: table("ticks", [{ name: "t", type: "timestamp", values: t }, { name: "close", values: close }]),
      options: { xScale: "time" },
      attrs: { label: "A million simulated minute closes as calendar candles" },
    };
  },
};
