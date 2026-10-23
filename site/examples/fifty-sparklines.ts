// 40 · Fifty live sparklines: fifty `<glyph-plot type="sparkline">` tags,
// each appended a sample every half second, each keeping its last 120 in a
// ring (`retain.rows`, REQ-STREAM-020). However many tags there are, the page
// loads the `line` module once and shares it (REQ-LOAD-010); the network
// panel shows one wasm file.

import type { Example } from "./api.ts";
import { normal, rng, round, type Rng } from "../src/data/rng.ts";
import { EPOCH, SECOND, table, times } from "../src/data/table.ts";
import { grid, plot, Scope } from "./support/ui.ts";

const SEED = 40;
const COUNT = 50;
const KEEP = 120;

/** One host's load as a mean-reverting walk around `level`. */
function next(r: Rng, v: number, level: number): number {
  return round(Math.max(0, v + (level - v) * 0.1 + normal(r) * 3), 1);
}

function history(k: number): { t: Float64Array; load: Float64Array } {
  const r = rng(SEED + k);
  const level = 20 + (k % 7) * 9;
  const load = new Float64Array(60);
  let v = level;
  for (let i = 0; i < load.length; i++) load[i] = v = next(r, v, level);
  return { t: times(EPOCH, load.length, SECOND), load };
}

export const example: Example = {
  slug: "fifty-sparklines",
  seed: SEED,
  start() {
    const { t, load } = history(0);
    return {
      type: "sparkline",
      data: table("host00", [{ name: "t", type: "timestamp", values: t }, { name: "host00", unit: "%", values: load }]),
      options: { retain: { rows: KEEP } },
      attrs: { rows: "1", label: "host00 load (simulated)" },
    };
  },
  live(el, host) {
    const s = new Scope();
    const g = grid(s, host, 2);
    g.classList.add("gp-ex-dense");
    const plots = [el];
    for (let k = 1; k < COUNT; k++) {
      const name = `host${String(k).padStart(2, "0")}`;
      const { t, load } = history(k);
      plots.push(plot(s, g, { type: "sparkline", rows: "1", label: `${name} load (simulated)` }, { data: table(name, [{ name: "t", type: "timestamp", values: t }, { name, unit: "%", values: load }]), options: { retain: { rows: KEEP } } }));
    }
    // Each host's walk carries on from its history, one sample a tick.
    const walks = plots.map((_, k) => {
      const r = rng(SEED + k);
      const level = 20 + (k % 7) * 9;
      let v = level;
      for (let i = 0; i < 60; i++) v = next(r, v, level);
      return { r, level, v };
    });
    let t = EPOCH + 60 * SECOND;
    s.every(500, () => {
      plots.forEach((p, k) => {
        const w = walks[k] as (typeof walks)[number];
        w.v = next(w.r, w.v, w.level);
        p.append({ t: new Date(t), [`host${String(k).padStart(2, "0")}`]: w.v });
      });
      t += SECOND;
    });
    return s.stop;
  },
};
