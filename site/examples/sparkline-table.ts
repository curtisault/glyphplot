// 37 · Sparklines in a table: twenty services' latency a minute for two
// hours, one row band a series in one `sparkline` (REQ-SPARK-010), each with
// its name, its latest value and its minimum (↓) and maximum (↑) beside it
// (`label`, `latest`, `extent`). One chart, one module, twenty rows.

import type { Example } from "./api.ts";
import { latency } from "../src/data/gen.ts";
import { EPOCH, MINUTE, table, times } from "../src/data/table.ts";

const SEED = 37;
const SERVICES = ["auth", "billing", "cart", "catalog", "checkout", "search", "images", "inventory", "mail", "maps", "orders", "pay", "profile", "quotes", "ratings", "recs", "sessions", "ship", "tax", "users"];

export const example: Example = {
  slug: "sparkline-table",
  seed: SEED,
  start() {
    const n = 120;
    return {
      type: "sparkline",
      data: table("services", [
        { name: "t", type: "timestamp", values: times(EPOCH, n, MINUTE) },
        ...SERVICES.map((name, k) => ({ name, unit: "ms", values: latency(SEED + k, n).p90 })),
      ]),
      options: { label: true, latest: true, extent: true },
      attrs: { rows: "20", label: "p90 latency of twenty services (simulated)" },
    };
  },
};
