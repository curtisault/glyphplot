// 38 · A sparkline in a sentence: an inline `sparkline` in area mode, one
// row high and twenty cells wide (`cols="20" rows="1"`), set in running
// text. Below 16 × 5 no chart draws axes (REQ-LAYOUT-020); a sparkline never
// does.

import type { Example } from "./api.ts";
import { cpu } from "../src/data/gen.ts";
import { EPOCH, HOUR, table, times } from "../src/data/table.ts";
import { make, plot, Scope } from "./support/ui.ts";

const SEED = 38;

export const example: Example = {
  slug: "sparkline-sentence",
  seed: SEED,
  start() {
    const t = times(EPOCH, 48, HOUR);
    return {
      type: "sparkline",
      data: table("cpu", [{ name: "t", type: "timestamp", values: t }, { name: "cpu", unit: "%", values: cpu(SEED, t) }]),
      options: { mode: "area", label: false, latest: false, extent: false },
      attrs: { cols: "20", rows: "1", label: "CPU over two days (simulated)" },
    };
  },
  live(el, host) {
    const s = new Scope();
    // The card's chart gives way to the same chart set in a sentence.
    const start = example.start();
    el.style.display = "none";
    s.add(() => el.style.removeProperty("display"));
    const p = s.put(host, make("p", "gp-ex-sentence"));
    p.append("Over two days the build server's CPU ");
    const inline = plot(s, p, { type: "sparkline", cols: "20", rows: "1", label: "CPU over two days (simulated)" }, { data: start.data, options: start.options ?? {} });
    Object.assign(inline.style, { display: "inline-block", width: "20ch", verticalAlign: "middle" });
    p.append(" rose each afternoon (simulated data).");
    return s.stop;
  },
};
