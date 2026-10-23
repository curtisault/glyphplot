// 13 · Deploys on a request rate: a day of requests a second, a value a
// minute, with each deploy drawn as a vertical line labelled with its version
// (`vline` annotations, REQ-ANNOT-010) and a `label` naming the incident after
// the third. The deploy times are part of the simulation, seeded like the
// rate.

import type { Example } from "./api.ts";
import { requests } from "../src/data/gen.ts";
import { HOUR, MINUTE, EPOCH, table, times } from "../src/data/table.ts";

const SEED = 13;

export const example: Example = {
  slug: "deploys",
  seed: SEED,
  start() {
    const t = times(EPOCH, 24 * 60, MINUTE);
    const rate = requests(SEED, t);
    // The third deploy is followed by a dip, part of the simulation.
    const bad = EPOCH + 15 * HOUR;
    for (let i = 0; i < t.length; i++) {
      const dt = (t[i] as number) - bad;
      if (dt > 0 && dt < 40 * MINUTE) rate[i] = Math.round((rate[i] as number) * 0.55);
    }
    const deploys: [number, string][] = [
      [EPOCH + 3 * HOUR + 20 * MINUTE, "v41"],
      [EPOCH + 9 * HOUR + 5 * MINUTE, "v42"],
      [bad, "v43"],
      [EPOCH + 15 * HOUR + 40 * MINUTE, "v43.1"],
    ];
    return {
      type: "line",
      data: table("requests", [
        { name: "t", type: "timestamp", values: t },
        { name: "requests", unit: "req/s", values: rate },
      ]),
      options: {
        annotations: [
          ...deploys.map(([value, label]) => ({ type: "vline", value, label })),
          { type: "label", x: bad + 45 * MINUTE, y: 700, text: "rolled back" },
        ],
      },
    };
  },
};
