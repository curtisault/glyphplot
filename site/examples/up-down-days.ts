// 39 · Up and down days: the EIA's WTI price as daily changes, each day's
// close less the day before's (computed here from the checked-in closes, to
// the cent), as columns above and below zero in a `sparkline` (mode
// `column`).

import type { Example } from "./api.ts";
import { col, wti } from "../src/data/real.ts";
import { round } from "../src/data/rng.ts";
import { table } from "../src/data/table.ts";

export const example: Example = {
  slug: "up-down-days",
  seed: null,
  start() {
    const date = col(wti, "date");
    const price = col(wti, "price");
    const change = price.slice(1).map((p, i) => round(p - (price[i] as number)));
    return {
      type: "sparkline",
      data: table("wti", [
        { name: "date", type: "timestamp", values: date.slice(1) },
        { name: "change", unit: "USD", values: change },
      ]),
      options: { mode: "column" },
      attrs: { rows: "6", label: "WTI daily change, USD a barrel (U.S. EIA)" },
    };
  },
};
