// 7 · CPU under the curve: a week of a CPU's utilisation every ten minutes,
// filled to a baseline of 0 (`mode: "area"`, `baseline`). Simulated: it
// stands in for NAB's realAWSCloudwatch CPU series until datasets/ holds
// them.

import type { Example } from "./api.ts";
import { cpu } from "../src/data/gen.ts";
import { DAY, EPOCH, MINUTE, table, times } from "../src/data/table.ts";

const SEED = 7;

export const example: Example = {
  slug: "cpu-area",
  seed: SEED,
  start() {
    const t = times(EPOCH, (7 * DAY) / (10 * MINUTE), 10 * MINUTE);
    return {
      type: "line",
      data: table("cpu", [
        { name: "t", type: "timestamp", values: t },
        { name: "cpu", unit: "%", values: cpu(SEED, t) },
      ]),
      options: { mode: "area", baseline: 0 },
    };
  },
};
