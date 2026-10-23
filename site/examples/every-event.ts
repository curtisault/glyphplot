// 30 · Every event: a live log of the `plot-*` events the element dispatches
// (REQ-EVENT-010): bubbling, composed CustomEvents whose `detail` has the shape
// of spec/schemas/events.schema.json. Hover, drag, Shift+drag, Ctrl or ⌘ with
// the wheel, click a legend key, use the keys: the newest event is on top.
// `plot-hover` fires only when the hovered datum changes (REQ-EVENT-020) and
// `plot-view` at most once a frame, then once with `final: true`
// (REQ-EVENT-030).

import type { Example } from "./api.ts";
import { walk } from "../src/data/gen.ts";
import { rng, round } from "../src/data/rng.ts";
import { DAY, EPOCH, table, times } from "../src/data/table.ts";
import { make, Scope } from "./support/ui.ts";

const SEED = 30;
const TYPES = ["plot-ready", "plot-error", "plot-hover", "plot-select", "plot-brush", "plot-view", "plot-legend", "plot-cursor", "plot-data"];
const KEEP = 12;

/** A detail in one line: numbers that are epoch ms within this data's years written as dates. */
function brief(detail: unknown): string {
  return JSON.stringify(detail, (_k, v: unknown) => (typeof v === "number" && v > 1.7e12 && v < 1.9e12 ? new Date(v).toISOString().slice(0, 16) : v)) ?? "null";
}

export const example: Example = {
  slug: "every-event",
  seed: SEED,
  start() {
    const n = 60;
    const r = rng(SEED);
    return {
      type: "line",
      data: table("two", [
        { name: "date", type: "timestamp", values: times(EPOCH, n, DAY) },
        { name: "a", values: walk(r, n, 10, 1).map((v) => round(v)) },
        { name: "b", values: walk(r, n, 14, 1).map((v) => round(v)) },
      ]),
    };
  },
  live(el, host) {
    const s = new Scope();
    const log = s.put(host, make("ol", "gp-ex-log"));
    log.setAttribute("aria-label", "Events, newest first");
    for (const type of TYPES) {
      s.on(el, type, (e: CustomEvent<unknown>) => {
        const li = make("li");
        li.append(make("b", "", type), ` ${brief(e.detail)}`);
        log.prepend(li);
        while (log.children.length > KEEP) log.lastElementChild?.remove();
      });
    }
    return s.stop;
  },
};
