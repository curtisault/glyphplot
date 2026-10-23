// 23 · On paper: `theme="paper"`, dark marks on a light ground in one ink,
// for print (spec/schemas/theme.schema.json, `paper`). An area of a month's
// daily rainfall, its fill told by shade rather than colour (REQ-MONO-030).

import type { Example } from "./api.ts";
import { rng, round } from "../src/data/rng.ts";
import { DAY, EPOCH, table, times } from "../src/data/table.ts";

const SEED = 23;

export const example: Example = {
  slug: "on-paper",
  seed: SEED,
  start() {
    const n = 31;
    const r = rng(SEED);
    // Mostly dry days, some showers, a few heavy days: a made-up month.
    const rain = Float64Array.from({ length: n }, () => (r() < 0.55 ? 0 : round(r() < 0.85 ? r() * 8 : 10 + r() * 25, 1)));
    return {
      type: "line",
      data: table("rain", [
        { name: "date", type: "timestamp", values: times(EPOCH, n, DAY) },
        { name: "rain", unit: "mm", values: rain },
      ]),
      options: { mode: "area", title: "Rain a day, mm (simulated)" },
      attrs: { theme: "paper" },
    };
  },
};
