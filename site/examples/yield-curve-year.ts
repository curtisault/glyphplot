// 60 · The yield curve through the year: the ten-year yield over 2026 above,
// the whole curve on one day below. Scrubbing the date (hover, a finger, or
// the arrows on the focused upper chart) replaces the lower chart's data with
// that day's curve (REQ-DATA-130), and the upper chart keeps the chosen day
// marked with the view's time cursor (`view.time`). The card shows the first
// day of the year.

import type { Example } from "./api.ts";
import { col, curveOn, treasury } from "../src/data/real.ts";
import { table } from "../src/data/table.ts";
import { caption, plot, Scope, status } from "./support/ui.ts";

/** The lower chart's data for row `i`. */
function curveData(i: number) {
  const c = curveOn(i);
  return table("curve", [
    { name: "maturity", unit: "years", values: c.maturity },
    { name: "yield", unit: "%", values: c.yield },
  ]);
}

const dayOf = (i: number): string => new Date(col(treasury, "date")[i] as number).toISOString().slice(0, 10);

export const example: Example = {
  slug: "yield-curve-year",
  seed: null,
  start() {
    return { type: "line", data: curveData(0), options: { title: `Par yield by maturity, ${dayOf(0)}` } };
  },
  live(el, host) {
    const s = new Scope();
    const dates = col(treasury, "date");
    // The year chart goes above the curve.
    const box = document.createElement("div");
    host.insertBefore(box, el);
    s.add(() => box.remove());
    caption(s, box, "Scrub the date: the 10-year yield, 2026");
    const year = plot(s, box, { type: "line", rows: "8", label: "10-year Treasury par yield, 2026" }, { data: treasury, options: { encode: { x: "date", y: "y10" } }, view: { time: dates[0] as number } });
    const out = status(s, host, `Curve on ${dayOf(0)}.`);
    let shown = 0;
    const show = (x: unknown): void => {
      if (typeof x !== "number") return;
      // The row whose date is nearest x.
      let i = 0;
      for (let k = 1; k < dates.length; k++) if (Math.abs((dates[k] as number) - x) < Math.abs((dates[i] as number) - x)) i = k;
      if (i === shown) return;
      shown = i;
      el.data = curveData(i);
      el.options = { title: `Par yield by maturity, ${dayOf(i)}` };
      year.view = { time: dates[i] as number };
      out.textContent = `Curve on ${dayOf(i)}.`;
    };
    s.on(year, "plot-hover", (e: CustomEvent<{ x?: unknown } | null>) => show(e.detail?.x));
    s.on(year, "plot-cursor", (e: CustomEvent<{ x?: unknown }>) => show(e.detail.x));
    return s.stop;
  },
};
