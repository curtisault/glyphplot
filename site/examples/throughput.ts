// 57 · Network throughput: traffic in drawn above the axis and traffic out
// below it (stored negative), both as areas to a baseline of 0, a reading a
// quarter second, appended live with the view following the newest two
// minutes. Simulated traffic.

import type { Example } from "./api.ts";
import { normal, rng, round, type Rng } from "../src/data/rng.ts";
import { EPOCH, MINUTE, table, times } from "../src/data/table.ts";
import { Scope } from "./support/ui.ts";

const SEED = 57;
const STEP = 250;
const N = (5 * MINUTE) / STEP;

/** In and out in Mb/s: bursty in, steadier out. */
function reading(r: Rng, i: number): [number, number] {
  const burst = r() < 0.03 ? 300 * r() : 0;
  return [round(Math.max(0, 180 + 60 * Math.sin(i / 90) + normal(r) * 25 + burst), 1), round(-Math.max(0, 70 + 20 * Math.sin(i / 60 + 1) + normal(r) * 10), 1)];
}

export const example: Example = {
  slug: "throughput",
  seed: SEED,
  start() {
    const r = rng(SEED);
    const rows = Array.from({ length: N }, (_, i) => reading(r, i));
    const end = EPOCH + (N - 1) * STEP;
    return {
      type: "line",
      data: table("traffic", [
        { name: "t", type: "timestamp", values: times(EPOCH, N, STEP) },
        { name: "in", unit: "Mb/s", values: rows.map(([a]) => a) },
        { name: "out", unit: "Mb/s", values: rows.map(([, b]) => b) },
      ]),
      options: { mode: "area", baseline: 0, retain: { rows: 4 * 60 * 10 } },
      view: { x: { domain: [end - 2 * MINUTE, end], follow: true } },
    };
  },
  live(el) {
    const s = new Scope();
    const r = rng(SEED);
    for (let i = 0; i < N; i++) reading(r, i);
    let i = N;
    s.every(STEP, () => {
      const [a, b] = reading(r, i);
      el.append({ t: new Date(EPOCH + i * STEP), in: a, out: b });
      i++;
    });
    return s.stop;
  },
};
