// 28 · Linked charts: two charts of different widths with the same `link`
// value share the x domain and the cursor in data units (REQ-LINK-010), with
// no event loop between them. Pan, zoom or hover either one.

import type { Example } from "./api.ts";
import { cpu, requests } from "../src/data/gen.ts";
import { EPOCH, MINUTE, table, times } from "../src/data/table.ts";
import { make, plot, Scope } from "./support/ui.ts";

const SEED = 28;
const t = (): Float64Array => times(EPOCH, 3 * 24 * 12, 5 * MINUTE);

export const example: Example = {
  slug: "linked-charts",
  seed: SEED,
  start() {
    const x = t();
    return { type: "line", data: table("requests", [{ name: "t", type: "timestamp", values: x }, { name: "requests", unit: "req/s", values: requests(SEED, x) }]), attrs: { link: "linked-charts" } };
  },
  live(_el, host) {
    const s = new Scope();
    const narrow = s.put(host, make("div", "gp-ex-narrow"));
    const x = t();
    plot(s, narrow, { type: "line", rows: "10", link: "linked-charts", label: "CPU, linked" }, {
      data: table("cpu", [{ name: "t", type: "timestamp", values: x }, { name: "cpu", unit: "%", values: cpu(SEED + 1, x) }]),
      options: { mode: "area" },
    });
    return s.stop;
  },
};
