// 58 · A service dashboard: a day of one service's request rate and p99
// latency as two line charts linked by `link` (REQ-LINK-010), so a pan, a
// zoom or the crosshair moves both, and its errors an hour as bars under
// them. A bar chart's x is its categories, not time, so the bars stand beside
// the linked pair rather than in the link. Simulated: it stands in for NAB's
// realAWSCloudwatch series until datasets/ holds them.

import type { Example } from "./api.ts";
import { latency, requests } from "../src/data/gen.ts";
import { int, rng } from "../src/data/rng.ts";
import { EPOCH, HOUR, MINUTE, table, times } from "../src/data/table.ts";
import { caption, plot, Scope } from "./support/ui.ts";

const SEED = 58;
const N = 24 * 12;
const t = (): Float64Array => times(EPOCH, N, 5 * MINUTE);

export const example: Example = {
  slug: "service-dashboard",
  seed: SEED,
  start() {
    const x = t();
    return {
      type: "line",
      data: table("rate", [{ name: "t", type: "timestamp", values: x }, { name: "requests", unit: "req/s", values: requests(SEED, x) }]),
      options: { title: "Requests" },
      attrs: { link: "service-dashboard", rows: "9" },
    };
  },
  live(_el, host) {
    const s = new Scope();
    const x = t();
    plot(s, host, { type: "line", rows: "9", link: "service-dashboard", label: "p99 latency" }, {
      data: table("latency", [{ name: "t", type: "timestamp", values: x }, { name: "p99", type: "duration", values: latency(SEED, N).p99 }]),
      options: { title: "p99" },
    });
    caption(s, host, "Errors an hour (not linked: a bar chart's x is its categories)");
    const r = rng(SEED + 1);
    const hours = Array.from({ length: 24 }, (_, h) => new Date(EPOCH + h * HOUR).toISOString().slice(11, 13) + "h");
    plot(s, host, { type: "bar", rows: "8", label: "Errors an hour" }, { data: { hour: hours, errors: hours.map((_, h) => int(r, 0, 6) + (h === 15 ? 40 : 0)) } });
    return s.stop;
  },
};
