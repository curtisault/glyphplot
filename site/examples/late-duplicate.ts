// 55 · Late and duplicate rows: a stream of ten readings a second in which
// some rows repeat the last row's time (a correction) and some arrive with a
// time before it (late). A row whose x equals the last row's replaces it; a
// row whose x is less is dropped, and the next `plot-data` counts it in `late`
// (REQ-STREAM-040, REQ-STREAM-070). The line under the chart sets what the
// page sent against what the chart reported.

import type { Example } from "./api.ts";
import { normal, rng } from "../src/data/rng.ts";
import { EPOCH, table, times } from "../src/data/table.ts";
import { Scope, status } from "./support/ui.ts";

const SEED = 55;
const STEP = 100;

export const example: Example = {
  slug: "late-duplicate",
  seed: SEED,
  start() {
    const n = 300;
    const r = rng(SEED);
    let v = 20;
    return {
      type: "line",
      data: table("meter", [
        { name: "t", type: "timestamp", values: times(EPOCH, n, STEP) },
        { name: "reading", values: Float64Array.from({ length: n }, () => (v = Math.round((v + normal(r) * 0.3) * 100) / 100)) },
      ]),
      options: { retain: { rows: 600 } },
      view: { x: { follow: true } },
    };
  },
  live(el, host) {
    const s = new Scope();
    const r = rng(SEED + 1);
    let t = EPOCH + 300 * STEP;
    let v = 20;
    const sent = { rows: 0, duplicates: 0, late: 0 };
    let reportedLate = 0;
    const out = status(s, host, "Waiting for the stream.");
    const show = (): void => {
      out.textContent = `Sent ${sent.rows} rows: ${sent.duplicates} repeating the last time (each replaced the last row), ${sent.late} late. The chart reported ${reportedLate} late rows dropped.`;
    };
    s.every(STEP, () => {
      v = Math.round((v + normal(r) * 0.3) * 100) / 100;
      const roll = r();
      if (roll < 0.1) {
        el.append({ t: new Date(t - STEP), reading: v }); // the last row's time again: a correction
        sent.duplicates++;
      } else if (roll < 0.17) {
        el.append({ t: new Date(t - STEP * (2 + Math.floor(r() * 5))), reading: v }); // older than the last row: late
        sent.late++;
      } else {
        el.append({ t: new Date(t), reading: v });
        t += STEP;
      }
      sent.rows++;
    });
    s.on(el, "plot-data", (e: CustomEvent<{ late: number }>) => {
      reportedLate += e.detail.late;
      show();
    });
    return s.stop;
  },
};
