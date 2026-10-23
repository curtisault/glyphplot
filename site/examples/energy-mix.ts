// 49 · An energy mix: a year of a grid's generation by source, stacked
// (`stack: true`), a bar a month. Sources' shares move with the seasons: more
// solar in summer, more wind and gas in winter. Simulated: no country's grid.

import type { Example } from "./api.ts";
import { normal, rng } from "../src/data/rng.ts";

const SEED = 49;

export const example: Example = {
  slug: "energy-mix",
  seed: SEED,
  start() {
    const r = rng(SEED);
    const month = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    const season = month.map((_, i) => Math.cos(((i - 6) / 12) * 2 * Math.PI)); // 1 in July, −1 in January
    const gwh = (f: (s: number) => number) => season.map((s) => Math.max(0, Math.round(f(s) + normal(r) * 40)));
    return {
      type: "bar",
      data: {
        month,
        nuclear: gwh(() => 900),
        gas: gwh((s) => 700 - 250 * s),
        wind: gwh((s) => 600 - 200 * s),
        solar: gwh((s) => 300 + 250 * s),
        hydro: gwh((s) => 250 + 60 * s),
      },
      options: { stack: true, title: "Generation, GWh a month (simulated)" },
    };
  },
};
