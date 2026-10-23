// 5 · Gaps in telemetry: a CPU's utilisation every five minutes for two days,
// with seeded stretches of missing samples (NaN). A missing y breaks the line
// with no segment drawn across it (REQ-LINE-030); the button sets `spanGaps`,
// which joins across the gaps, without a new chart (REQ-OPT-020).
// Simulated: it stands in for NAB's realAWSCloudwatch series until
// datasets/ holds them.

import type { Example } from "./api.ts";
import { cpu, withGaps } from "../src/data/gen.ts";
import { rng } from "../src/data/rng.ts";
import { DAY, EPOCH, MINUTE, table, times } from "../src/data/table.ts";
import { button, controls, Scope } from "./support/ui.ts";

const SEED = 5;

export const example: Example = {
  slug: "telemetry-gaps",
  seed: SEED,
  start() {
    const t = times(EPOCH, (2 * DAY) / (5 * MINUTE), 5 * MINUTE);
    return {
      type: "line",
      data: table("cpu", [
        { name: "t", type: "timestamp", values: t },
        { name: "cpu", unit: "%", values: withGaps(cpu(SEED, t), rng(SEED + 1), 6, 12) },
      ]),
      options: { spanGaps: false },
    };
  },
  live(el, host) {
    const s = new Scope();
    const row = controls(s, host, "Gaps");
    button(s, row, "Join across gaps", () => {
      const on = !(el.options["spanGaps"] === true);
      el.options = { ...el.options, spanGaps: on };
      return on;
    }, false);
    return s.stop;
  },
};
