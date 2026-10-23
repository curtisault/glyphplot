// 53 · A burn-down: a project's remaining budget at the end of each week, as
// bars, with a target line (an `hline`: the budget the project means to have
// left) and a band marking the contingency it must not spend into
// (REQ-ANNOT-010). Simulated spend.

import type { Example } from "./api.ts";
import { rng } from "../src/data/rng.ts";

const SEED = 53;

export const example: Example = {
  slug: "burn-down",
  seed: SEED,
  start() {
    const r = rng(SEED);
    const week = Array.from({ length: 12 }, (_, i) => `W${i + 1}`);
    let left = 120;
    const remaining = week.map(() => (left = Math.max(0, Math.round((left - 6 - r() * 8) * 10) / 10)));
    return {
      type: "bar",
      data: { week, remaining },
      options: {
        title: "Budget left, k (simulated)",
        annotations: [
          { type: "band", scale: "y", from: 0, to: 15, label: "contingency" },
          { type: "hline", scale: "y", value: 30, label: "target" },
        ],
      },
    };
  },
};
