// 59 · Today's yield curve: the Treasury's par yields by maturity, one month
// to thirty years, on the last day the checked-in file holds (2026-10-20). x
// is the maturity in years, a plain number axis; maturities with no value
// that day are left out.

import type { Example } from "./api.ts";
import { col, curveOn, treasury } from "../src/data/real.ts";
import { table } from "../src/data/table.ts";

export const example: Example = {
  slug: "yield-curve",
  seed: null,
  start() {
    const last = col(treasury, "date").length - 1;
    const day = new Date(col(treasury, "date")[last] as number).toISOString().slice(0, 10);
    const c = curveOn(last);
    return {
      type: "line",
      data: table("curve", [
        { name: "maturity", unit: "years", values: c.maturity },
        { name: "yield", unit: "%", values: c.yield },
      ]),
      options: { title: `Par yield by maturity, ${day}` },
    };
  },
};
