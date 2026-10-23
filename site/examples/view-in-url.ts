// 27 · A view in the URL: pan or zoom, and the view goes into the address as
// `?view=<JSON>` when the gesture ends (`plot-view` with `final: true`);
// reload, and the same view comes back, since setting
// `JSON.parse(JSON.stringify(el.view))` draws the same frame (REQ-VIEW-030).
// Only the x domain is kept, so the link stays short. On the example's own
// page the address bar is written with `history.replaceState`; on a gallery
// card the link is shown instead.

import type { Example } from "./api.ts";
import { walk } from "../src/data/gen.ts";
import { rng, round } from "../src/data/rng.ts";
import { DAY, EPOCH, table, times } from "../src/data/table.ts";
import { Scope, status } from "./support/ui.ts";

const SEED = 27;
const PARAM = "view";

export const example: Example = {
  slug: "view-in-url",
  seed: SEED,
  start() {
    const n = 365;
    return {
      type: "line",
      data: table("walk", [
        { name: "date", type: "timestamp", values: times(EPOCH - 365 * DAY, n, DAY) },
        { name: "value", values: walk(rng(SEED), n, 50, 1.5).map((v) => round(v)) },
      ]),
    };
  },
  live(el, host) {
    const s = new Scope();
    const page = host.closest("gp-example")?.hasAttribute("full") === true;
    const out = status(s, host, "Pan or zoom; the view's link appears here.");
    const url = new URL(location.href);
    const given = url.searchParams.get(PARAM);
    if (given) {
      try {
        el.view = { x: (JSON.parse(given) as { x?: unknown }).x ?? null };
        out.textContent = "The view came from the address.";
      } catch {
        out.textContent = "The address's view is not JSON; the fitted view is shown.";
      }
    }
    s.on(el, "plot-view", (e: CustomEvent<{ final: boolean; view: { x?: { domain?: unknown } } }>) => {
      if (!e.detail.final) return;
      const next = new URL(location.href);
      next.searchParams.set(PARAM, JSON.stringify({ x: { domain: e.detail.view.x?.domain ?? null } }));
      if (page) {
        history.replaceState(history.state, "", next);
        out.textContent = "The address now holds this view: reload to see it again.";
      } else out.textContent = `Link to this view: ${next.search}`;
    });
    return s.stop;
  },
};
