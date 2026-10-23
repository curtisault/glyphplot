// 56 · The last hour: retention by age. `retain: { age: "1h" }` keeps the
// rows whose time is within an hour of the newest and drops the rest as rows
// arrive (REQ-STREAM-030), so the chart always shows one hour. The clock is
// simulated and runs a hundred times fast: a reading every ten simulated
// seconds, appended every 100 ms, so the hour turns over in 36 seconds.

import type { Example } from "./api.ts";
import { normal, rng, round, type Rng } from "../src/data/rng.ts";
import { EPOCH, HOUR, SECOND, table, times } from "../src/data/table.ts";
import { Scope, status } from "./support/ui.ts";

const SEED = 56;
const STEP = 10 * SECOND;
const N = HOUR / STEP;

function reading(r: Rng, i: number): number {
  return round(Math.max(0, 40 + 25 * Math.sin(i / 50) + normal(r) * 4), 1);
}

export const example: Example = {
  slug: "last-hour",
  seed: SEED,
  start() {
    const r = rng(SEED);
    return {
      type: "line",
      data: table("queue", [
        { name: "t", type: "timestamp", values: times(EPOCH, N, STEP) },
        { name: "depth", values: Float64Array.from({ length: N }, (_, i) => reading(r, i)) },
      ]),
      options: { mode: "area", retain: { age: "1h" } },
    };
  },
  live(el, host) {
    const s = new Scope();
    const r = rng(SEED);
    for (let i = 0; i < N; i++) reading(r, i);
    let i = N;
    const out = status(s, host);
    let dropped = 0;
    s.every(100, () => {
      el.append({ t: new Date(EPOCH + i * STEP), depth: reading(r, i) });
      i++;
    });
    s.on(el, "plot-data", (e: CustomEvent<{ rows: number; dropped: number }>) => {
      dropped += e.detail.dropped;
      out.textContent = `${e.detail.rows} rows held, the last hour of simulated time; ${dropped} older rows dropped so far.`;
    });
    return s.stop;
  },
};
