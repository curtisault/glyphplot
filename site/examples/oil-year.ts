// 8 · A year of oil: the EIA's daily WTI spot price for the 250 trading days
// to 2026-10-14, as checked in (crates/glyphplot/tests/data/wti-daily.json).
// Two markers, found from the data when the example starts, name the year's
// lowest and highest close. The catalogue's "Oil since 1986" (decades of
// days, the negative day of 2020-04-20) waits for the full EIA series in
// datasets/; this year is what the repository holds.

import type { Example } from "./api.ts";
import { col, wti } from "../src/data/real.ts";

/** The row of the least (or greatest) value. */
function extreme(v: number[], most: boolean): number {
  let at = 0;
  v.forEach((x, i) => {
    if (most ? x > (v[at] as number) : x < (v[at] as number)) at = i;
  });
  return at;
}

export const example: Example = {
  slug: "oil-year",
  seed: null,
  start() {
    const date = col(wti, "date");
    const price = col(wti, "price");
    const lo = extreme(price, false);
    const hi = extreme(price, true);
    return {
      type: "line",
      data: wti,
      options: {
        title: "WTI spot, USD a barrel",
        annotations: [
          { type: "marker", x: date[lo], y: price[lo], glyph: "▼", label: `low ${price[lo]}` },
          { type: "marker", x: date[hi], y: price[hi], glyph: "▲", label: `high ${price[hi]}` },
        ],
      },
      attrs: { label: "WTI crude oil spot price, daily, the year to 2026-10-14 (U.S. EIA)" },
    };
  },
};
