// 26 · From 4 × 1 to full screen: one chart in boxes of every size. The
// grid is the whole cells that fit the element's box (REQ-EL-080) and is laid
// out again within a frame of any change of size (REQ-EL-090); below 16 × 5
// a chart drops its axes, title and legend (REQ-LAYOUT-020). Live, the card's
// own chart takes a resize handle (drag its corner), and fixed grids of
// 4 × 1, 16 × 4 and 40 × 10 cells (`cols`, `rows`) sit under it.

import type { Example } from "./api.ts";
import { walk } from "../src/data/gen.ts";
import { rng, round } from "../src/data/rng.ts";
import { DAY, EPOCH, table, times } from "../src/data/table.ts";
import { cell, grid, Scope } from "./support/ui.ts";

const SEED = 26;

function data() {
  const n = 200;
  return table("walk", [
    { name: "date", type: "timestamp", values: times(EPOCH, n, DAY) },
    { name: "value", values: walk(rng(SEED), n, 100, 2).map((v) => round(v)) },
  ]);
}

export const example: Example = {
  slug: "sizes",
  seed: SEED,
  start() {
    return { type: "line", data: data() };
  },
  live(el, host) {
    const s = new Scope();
    el.style.resize = "both";
    el.style.minWidth = "2ch";
    s.add(() => {
      el.style.resize = "";
    });
    const g = grid(s, host, 3);
    for (const [cols, rows] of [
      [4, 1],
      [16, 4],
      [40, 10],
    ] as const) {
      cell(s, g, `${cols} × ${rows}`, { type: "line", cols: String(cols), rows: String(rows) }, { data: data() });
    }
    return s.stop;
  },
};
