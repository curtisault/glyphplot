// 22 · One ink: four series drawn with the `mono` attribute into a `<pre>`,
// with no colour at all. Each series is told apart by its dash pattern
// (solid, 3 on 1 off, 1 on 1 off, 3-1-1-1, counted in dots from the x
// origin, REQ-MONO-010), and the legend draws each pattern beside its name
// (REQ-MONO-040). Select the text: it is the chart.

import type { Example } from "./api.ts";
import { walk } from "../src/data/gen.ts";
import { rng, round } from "../src/data/rng.ts";
import { DAY, EPOCH, table, times } from "../src/data/table.ts";

const SEED = 22;

export const example: Example = {
  slug: "one-ink",
  seed: SEED,
  start() {
    const n = 120;
    const r = rng(SEED);
    const cols = ["alpha", "beta", "gamma", "delta"].map((name, k) => ({ name, values: walk(r, n, 10 + k * 6, 1.2).map((v) => round(v)) }));
    return {
      type: "line",
      data: table("four", [{ name: "date", type: "timestamp", values: times(EPOCH, n, DAY) }, ...cols]),
      attrs: { mono: "", label: "Four simulated series in one ink" },
    };
  },
};
