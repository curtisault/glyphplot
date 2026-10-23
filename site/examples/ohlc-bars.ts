// 43 · OHLC bars: `mode: "ohlc"`, each session a vertical bar from low to
// high with the open as a tick to the left and the close to the right, the
// volume in its pane below (REQ-CANDLE-010, REQ-CANDLE-040). Simulated
// daily sessions.

import type { Example } from "./api.ts";
import { columns, daily } from "../src/data/market.ts";
import { DAY, EPOCH } from "../src/data/table.ts";

const SEED = 43;

export const example: Example = {
  slug: "ohlc-bars",
  seed: SEED,
  start() {
    return { type: "candles", data: columns(daily(SEED, 40, EPOCH - 60 * DAY)), options: { mode: "ohlc" }, attrs: { label: "OHLC bars, simulated market" } };
  },
};
