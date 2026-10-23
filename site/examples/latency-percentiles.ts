// 11 · Latency percentiles: a service's p50, p90 and p99 every five minutes
// for two days, as `duration` columns (milliseconds), which the readout and
// the table write as durations (REQ-FMT-040). The catalogue asks for a log y
// axis here; no chart reads a log y option at v0.7.0 (the core has the scale,
// SPEC §5.1), so the axis is linear and the p99's bursts set its top. Toggle a
// series with 1, 2 or 3 on the focused chart, or by its legend key.

import type { Example } from "./api.ts";
import { latency } from "../src/data/gen.ts";
import { EPOCH, MINUTE, table, times } from "../src/data/table.ts";

const SEED = 11;

export const example: Example = {
  slug: "latency-percentiles",
  seed: SEED,
  start() {
    const n = 576;
    const { p50, p90, p99 } = latency(SEED, n);
    return {
      type: "line",
      data: table("latency", [
        { name: "t", type: "timestamp", values: times(EPOCH, n, 5 * MINUTE) },
        { name: "p50", type: "duration", values: p50 },
        { name: "p90", type: "duration", values: p90 },
        { name: "p99", type: "duration", values: p99 },
      ]),
    };
  },
};
