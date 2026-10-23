// 34 · Plain HTML, no build: one element whose data is an attribute of JSON
// text (REQ-DATA-120) and one module script that defines the tag. No
// property is set from script: live, the card's chart is replaced by the
// markup below, parsed as HTML.
//
//   <script type="module" src="…/glyphplot/element.js"></script>
//   <glyph-plot type="line" label="Visitors a day"
//     data='{"day":["2026-10-01T00:00:00Z", …],"visitors":[312, …]}'></glyph-plot>
//
// The package is not published yet (packaging is v0.11.0), so the script's
// URL is wherever the page serves the shell's built `element.js` from.

import type { Example } from "./api.ts";
import { int, rng } from "../src/data/rng.ts";
import { DAY, EPOCH } from "../src/data/table.ts";
import { Scope } from "./support/ui.ts";

const SEED = 34;

function data(): { day: string[]; visitors: number[] } {
  const r = rng(SEED);
  const day = Array.from({ length: 28 }, (_, i) => new Date(EPOCH + i * DAY).toISOString().replace(".000Z", "Z"));
  return { day, visitors: day.map((_, i) => int(r, 250, 400) + (i % 7 >= 5 ? -120 : 0)) };
}

/** The markup, as a page would hold it. */
export function markup(): string {
  return `<glyph-plot type="line" rows="14" label="Visitors a day (simulated)" data='${JSON.stringify(data())}'></glyph-plot>`;
}

export const example: Example = {
  slug: "plain-html",
  seed: SEED,
  start() {
    return { type: "line", data: data(), attrs: { label: "Visitors a day (simulated)" } };
  },
  live(el, host) {
    const s = new Scope();
    const box = document.createElement("div");
    box.innerHTML = markup();
    el.style.display = "none";
    s.add(() => el.style.removeProperty("display"));
    s.put(host, box);
    host.insertBefore(box, el);
    return s.stop;
  },
};
