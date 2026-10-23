// 21 · For a screen reader: what assistive technology reads from the chart,
// shown. The element is a figure; its drawing has a label naming the type,
// the series and each axis's visible domain (REQ-A11Y-010); its one tab stop
// is a slider whose value text gives the cursor's x and each series' value
// (REQ-A11Y-030); and it holds a visually hidden table of the data in view,
// at most 500 rows (REQ-A11Y-020). The panel under the chart copies the three
// as they change (pan, zoom or step the cursor and watch), marked
// aria-hidden so a screen reader does not hear them twice.

import type { Example } from "./api.ts";
import { weather } from "../src/data/gen.ts";
import { EPOCH, HOUR, table, times } from "../src/data/table.ts";
import { make, Scope } from "./support/ui.ts";

const SEED = 21;

export const example: Example = {
  slug: "screen-reader",
  seed: SEED,
  start() {
    const t = times(EPOCH, 48, HOUR);
    const { temperature, humidity } = weather(SEED, t);
    return {
      type: "line",
      data: table("weather", [
        { name: "t", type: "timestamp", values: t },
        { name: "temperature", unit: "°C", values: temperature },
        { name: "humidity", unit: "%", values: humidity },
      ]),
      attrs: { label: "Temperature and humidity over two days (simulated)" },
    };
  },
  live(el, host) {
    const s = new Scope();
    const panel = s.put(host, make("div", "gp-ex-sr"));
    panel.setAttribute("aria-hidden", "true");
    const label = panel.appendChild(make("p"));
    const slider = panel.appendChild(make("p"));
    const box = panel.appendChild(make("div", "gp-ex-table"));
    const copy = (): void => {
      const drawing = el.querySelector(".gp-drawing");
      const cursor = el.querySelector('[role="slider"]');
      label.textContent = `Drawing (role img): ${drawing?.getAttribute("aria-label") ?? "not drawn yet"}`;
      slider.textContent = `Cursor (role slider): ${cursor?.getAttribute("aria-valuetext") ?? "none"}`;
      const t = el.querySelector(".gp-sr table");
      box.replaceChildren(...(t ? [t.cloneNode(true)] : []));
    };
    copy();
    const mo = new MutationObserver(() => copy());
    mo.observe(el, { subtree: true, childList: true, attributes: true, attributeFilter: ["aria-label", "aria-valuetext"] });
    s.add(() => mo.disconnect());
    return s.stop;
  },
};
