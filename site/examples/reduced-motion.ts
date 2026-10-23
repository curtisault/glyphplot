// 36 · Reduced motion: two charts with eased zooms (`motion: true` in the
// options), one with the `motion` attribute and one without. Under the
// reader's `prefers-reduced-motion: reduce`, the chart without the attribute
// jumps straight to each new view, and the one with it still eases
// (REQ-A11Y-050, REQ-VIEW-050). Without that preference both ease. The
// buttons zoom both charts by sending them the + and − keys.

import type { Example } from "./api.ts";
import { walk } from "../src/data/gen.ts";
import { rng, round } from "../src/data/rng.ts";
import { DAY, EPOCH, table, times } from "../src/data/table.ts";
import { button, cell, controls, grid, Scope, status, type Plot } from "./support/ui.ts";

const SEED = 36;

function data() {
  const n = 400;
  return table("walk", [
    { name: "date", type: "timestamp", values: times(EPOCH - n * DAY, n, DAY) },
    { name: "value", values: walk(rng(SEED), n, 100, 2).map((v) => round(v)) },
  ]);
}

/** Sends `key` to a chart's cursor, as the reader's key. */
function press(el: Plot, key: string): void {
  el.querySelector('[role="slider"]')?.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true }));
}

export const example: Example = {
  slug: "reduced-motion",
  seed: SEED,
  start() {
    return { type: "line", data: data(), options: { motion: true, title: "without the motion attribute" } };
  },
  live(el, host) {
    const s = new Scope();
    const g = grid(s, host, 1);
    const other = cell(s, g, "with the motion attribute", { type: "line", rows: "10", motion: "" }, { data: data(), options: { motion: true } });
    const reduced = typeof matchMedia === "function" && matchMedia("(prefers-reduced-motion: reduce)").matches;
    status(s, host, reduced ? "Your browser asks for reduced motion: the first chart jumps, the second eases." : "Your browser does not ask for reduced motion: both charts ease. Turn the setting on to see the first one jump.");
    const row = controls(s, host, "Zoom");
    button(s, row, "Zoom in", () => {
      press(el, "+");
      press(other, "+");
    });
    button(s, row, "Zoom out", () => {
      press(el, "-");
      press(other, "-");
    });
    return s.stop;
  },
};
