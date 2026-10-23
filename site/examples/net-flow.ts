// 12 · Net flow: signed values, mostly small with rare moves a hundred to ten
// thousand times larger either way, an hour apart for two weeks, with a line
// at zero (an `hline` annotation). The catalogue asks for a symlog y axis;
// no chart reads a symlog option at v0.7.0 (the core has the scale, SPEC
// §5.1), so the axis is linear: zoom into a quiet stretch (drag a brush with
// Shift, or + on the focused chart) to see the small flows.

import type { Example } from "./api.ts";
import { netFlow } from "../src/data/gen.ts";
import { EPOCH, HOUR, table, times } from "../src/data/table.ts";

const SEED = 12;

export const example: Example = {
  slug: "net-flow",
  seed: SEED,
  start() {
    const n = 14 * 24;
    return {
      type: "line",
      data: table("flow", [
        { name: "t", type: "timestamp", values: times(EPOCH, n, HOUR) },
        { name: "flow", unit: "MB", values: netFlow(SEED, n) },
      ]),
      options: { annotations: [{ type: "hline", scale: "y", value: 0, label: "0" }] },
    };
  },
};
