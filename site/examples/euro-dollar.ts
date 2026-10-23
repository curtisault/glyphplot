// 10 · A currency rate: dollars a euro, one value a business day for two
// years, its column carrying the unit `USD` (REQ-DATA-100), which the readout
// and the table show beside each value. Hover, or focus the chart and use the
// arrows, for the readout. Simulated: it stands in for the Federal Reserve's
// H.10 rate until datasets/ holds it, so no value here is a real rate.

import type { Example } from "./api.ts";
import { fx } from "../src/data/gen.ts";
import { DAY, EPOCH, table } from "../src/data/table.ts";

const SEED = 10;

/** Business days (Monday to Friday, UTC) from `start`, `n` of them. */
function businessDays(start: number, n: number): Float64Array {
  const t = new Float64Array(n);
  for (let d = 0, i = 0; i < n; d++) {
    const day = start + d * DAY;
    const wd = new Date(day).getUTCDay();
    if (wd !== 0 && wd !== 6) t[i++] = day;
  }
  return t;
}

export const example: Example = {
  slug: "euro-dollar",
  seed: SEED,
  start() {
    const n = 520;
    return {
      type: "line",
      data: table("eurusd", [
        { name: "date", type: "timestamp", values: businessDays(EPOCH - 730 * DAY, n) },
        { name: "EURUSD", unit: "USD", values: fx(SEED, n, 1.1) },
      ]),
      options: { title: "Dollars a euro (simulated)" },
    };
  },
};
