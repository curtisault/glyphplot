// 19 · An ECG strip: thirty seconds at 360 samples a second (10,800 rows),
// the view on three seconds. On a phone one finger drags the cursor along the
// strip (scrubbing, REQ-TOUCH-010) and two fingers pan; with a mouse, drag
// to pan and hover to read. Simulated: P, QRS and T waves as Gaussians at
// about 72 beats a minute, standing in for PhysioNet's MIT-BIH records until
// datasets/ holds them. It is not a medical trace.

import type { Example } from "./api.ts";
import { ecg } from "../src/data/gen.ts";
import { table } from "../src/data/table.ts";

const SEED = 19;
const HZ = 360;

export const example: Example = {
  slug: "ecg-strip",
  seed: SEED,
  start() {
    const v = ecg(SEED, 30, HZ);
    return {
      type: "line",
      data: table("ecg", [
        { name: "seconds", unit: "s", values: Float64Array.from(v, (_, i) => i / HZ) },
        { name: "lead", unit: "mV", values: v },
      ]),
      view: { x: { domain: [0, 3] } },
      attrs: { label: "Simulated ECG strip, 360 samples a second" },
    };
  },
};
