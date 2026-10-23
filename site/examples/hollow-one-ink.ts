// 42 · Hollow candles in one ink: `mode: "hollow"` with the `mono`
// attribute. With no colour, a rising candle is drawn hollow and a falling
// one solid (REQ-CANDLE-030, REQ-THEME-060), and no candle is narrower than
// four cells: the chart resamples instead. Simulated daily sessions.

import type { Example } from "./api.ts";
import { columns, daily } from "../src/data/market.ts";
import { DAY, EPOCH } from "../src/data/table.ts";

const SEED = 42;

export const example: Example = {
  slug: "hollow-one-ink",
  seed: SEED,
  start() {
    return {
      type: "candles",
      data: columns(daily(SEED, 30, EPOCH - 45 * DAY)),
      options: { mode: "hollow", volume: false },
      attrs: { mono: "", label: "Hollow candles in one ink, simulated market" },
    };
  },
};
