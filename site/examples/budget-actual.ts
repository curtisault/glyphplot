// 48 · Budget against actual: two value columns by quarter, drawn as grouped
// bars (several series not stacked are grouped, SPEC §7.3), each bar's end
// to an eighth of a cell (REQ-BAR-010). Hover a group for both values.

import type { Example } from "./api.ts";
import { rng } from "../src/data/rng.ts";

const SEED = 48;

export const example: Example = {
  slug: "budget-actual",
  seed: SEED,
  start() {
    const r = rng(SEED);
    const quarter = ["2025 Q4", "2026 Q1", "2026 Q2", "2026 Q3"];
    const budget = [420, 450, 480, 500];
    const actual = budget.map((b) => Math.round(b * (0.85 + r() * 0.3)));
    return { type: "bar", data: { quarter, budget, actual }, options: { title: "Spend by quarter, k (simulated)" } };
  },
};
