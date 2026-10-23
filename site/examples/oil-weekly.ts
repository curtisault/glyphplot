// 41 · Oil in weekly candles: the EIA's daily WTI closes given to `candles`
// as they are, one value a day. A table of one value column draws its
// closes as candles, resampled up the calendar ladder until each candle is at
// least a cell wide (REQ-SAMPLE-040): at a card's width a year of days
// becomes weekly candles, open the week's first close, high and low its
// extremes, close its last. Zoom in and the candles split into days.

import type { Example } from "./api.ts";
import { wti } from "../src/data/real.ts";

export const example: Example = {
  slug: "oil-weekly",
  seed: null,
  start() {
    return { type: "candles", data: wti, options: { title: "WTI, USD a barrel" }, attrs: { label: "WTI spot price as candles resampled from daily closes (U.S. EIA)" } };
  },
};
