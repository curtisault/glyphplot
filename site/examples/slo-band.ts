// 14 · An SLO band: a week of a service's success rate an hour, filled to
// 99 %, with the 99.9 % objective as a line (`hline`) and the error budget
// between 99.9 % and 99.5 % shaded (`band` on the y scale), REQ-ANNOT-010.

import type { Example } from "./api.ts";
import { normal, rng, round } from "../src/data/rng.ts";
import { EPOCH, HOUR, table, times } from "../src/data/table.ts";

const SEED = 14;

export const example: Example = {
  slug: "slo-band",
  seed: SEED,
  start() {
    const n = 7 * 24;
    const r = rng(SEED);
    const ok = new Float64Array(n);
    let incident = 0;
    for (let i = 0; i < n; i++) {
      if (r() < 0.02) incident = 0.6 + r();
      incident *= 0.7;
      ok[i] = round(Math.min(100, 99.97 - Math.abs(normal(r)) * 0.03 - incident), 3);
    }
    return {
      type: "line",
      data: table("slo", [
        { name: "t", type: "timestamp", values: times(EPOCH, n, HOUR) },
        { name: "success", unit: "%", values: ok },
      ]),
      options: {
        mode: "area",
        baseline: 99,
        annotations: [
          { type: "band", scale: "y", from: 99.5, to: 99.9, label: "error budget" },
          { type: "hline", scale: "y", value: 99.9, label: "SLO 99.9" },
        ],
      },
    };
  },
};
