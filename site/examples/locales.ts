// 17 · Locales: one chart in en-US, de-DE, fr-FR, ar-EG and ja-JP with the
// `locale` attribute (REQ-I18N-010). The shell builds each locale's pack from
// Intl (decimal and group separators, digits, month names, the minus sign,
// REQ-I18N-020); the frames differ only in their labels. Values in the
// thousands with decimals, over six weeks, so groups, decimals and month
// names all show.

import type { Example } from "./api.ts";
import { walk } from "../src/data/gen.ts";
import { rng, round } from "../src/data/rng.ts";
import { DAY, EPOCH, table, times } from "../src/data/table.ts";
import { cell, grid, Scope } from "./support/ui.ts";

const SEED = 17;

function data() {
  const n = 42;
  return table("sales", [
    { name: "date", type: "timestamp", values: times(EPOCH, n, DAY) },
    { name: "sales", values: walk(rng(SEED), n, 12_000, 900).map((v) => round(v, 2)) },
  ]);
}

export const example: Example = {
  slug: "locales",
  seed: SEED,
  start() {
    return { type: "line", data: data(), options: { title: "en-US" }, attrs: { locale: "en-US", timezone: "UTC", link: "locales" } };
  },
  live(_el, host) {
    const s = new Scope();
    const g = grid(s, host, 2);
    for (const locale of ["de-DE", "fr-FR", "ar-EG", "ja-JP"]) {
      cell(s, g, locale, { type: "line", rows: "10", locale, timezone: "UTC", link: "locales" }, { data: data() });
    }
    return s.stop;
  },
};
