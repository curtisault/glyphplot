// 20 · Keyboard only: every interaction from the keys (REQ-KEY-010). Tab to
// the chart (its one tab stop is a slider, REQ-A11Y-030), then ← and → step
// datum by datum, Shift+← → pan, + and − zoom about the cursor, 0 resets,
// Enter locks the cursor and selects, Shift+Enter starts and then commits a
// brush, Escape clears, 1–3 toggle a series and ? lists the keys. The line
// under the chart reports each action the chart reports with
// `source: "keyboard"`.

import type { Example } from "./api.ts";
import { walk } from "../src/data/gen.ts";
import { rng, round } from "../src/data/rng.ts";
import { DAY, EPOCH, table, times } from "../src/data/table.ts";
import { button, controls, make, Scope, status } from "./support/ui.ts";

const SEED = 20;

export const example: Example = {
  slug: "keyboard-only",
  seed: SEED,
  start() {
    const n = 90;
    const r = rng(SEED);
    return {
      type: "line",
      data: table("stock", [
        { name: "date", type: "timestamp", values: times(EPOCH, n, DAY) },
        { name: "north", values: walk(r, n, 40, 2).map((v) => round(v)) },
        { name: "south", values: walk(r, n, 30, 2).map((v) => round(v)) },
        { name: "west", values: walk(r, n, 20, 2).map((v) => round(v)) },
      ]),
      attrs: { label: "Stock in three warehouses (simulated); focus the chart and use the keys" },
    };
  },
  live(el, host) {
    const s = new Scope();
    const row = controls(s, host, "Keyboard");
    button(s, row, "Focus the chart", () => {
      el.querySelector<HTMLElement>('[role="slider"]')?.focus();
    });
    const keys = s.put(host, make("p", "gp-ex-note", "← → step · Shift+← → pan · + − zoom · 0 reset · Enter select · Shift+Enter brush · Esc clear · 1–3 series · Space pause · ? keys"));
    keys.setAttribute("aria-hidden", "true");
    const out = status(s, host, "Waiting for a key on the chart.");
    for (const type of ["plot-cursor", "plot-select", "plot-brush", "plot-view", "plot-legend"]) {
      s.on(el, type, (e: CustomEvent<{ source?: string; x?: unknown; final?: boolean } | null>) => {
        if (e.detail?.source !== "keyboard") return;
        if (type === "plot-view" && e.detail.final === false) return;
        out.textContent = `${type} from the keyboard${typeof e.detail.x === "number" ? ` at x ${new Date(e.detail.x).toISOString().slice(0, 10)}` : ""}`;
      });
    }
    return s.stop;
  },
};
