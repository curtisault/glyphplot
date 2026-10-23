// 29 · Brush to zoom another: the year of WTI as an overview whose drag
// brushes (`drag: "brush"`, REQ-INPUT-040), and a detail chart under it. A
// finished brush (`plot-brush` with `final: true`) sets the detail's x domain
// to the brushed range; a double-click on the overview resets the detail.
// On a phone, a long press starts the brush (REQ-TOUCH-010).

import type { Example } from "./api.ts";
import { wti } from "../src/data/real.ts";
import { plot, Scope, status } from "./support/ui.ts";

export const example: Example = {
  slug: "brush-zoom",
  seed: null,
  start() {
    return { type: "line", data: wti, options: { mode: "area", drag: "brush", baseline: 50, title: "Overview: brush a range" }, attrs: { rows: "8" } };
  },
  live(el, host) {
    const s = new Scope();
    const detail = plot(s, host, { type: "line", rows: "12", label: "WTI detail" }, { data: wti, options: { title: "Detail" } });
    const out = status(s, host, "Drag across the overview to zoom the detail.");
    s.on(el, "plot-brush", (e: CustomEvent<{ x: [number, number] | null; final: boolean; count: number }>) => {
      if (!e.detail.final) return;
      detail.view = { x: { domain: e.detail.x } };
      out.textContent = e.detail.x ? `Detail: ${e.detail.count} days from ${new Date(e.detail.x[0]).toISOString().slice(0, 10)} to ${new Date(e.detail.x[1]).toISOString().slice(0, 10)}.` : "Detail: the whole year.";
    });
    s.on(el, "plot-view", (e: CustomEvent<{ cause?: string; final: boolean }>) => {
      if (e.detail.final && e.detail.cause === "reset") detail.view = null;
    });
    return s.stop;
  },
};
