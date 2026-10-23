// 24 · Themes from CSS: the chart takes its colours from custom properties
// (`--glyphplot-ground`, `--glyphplot-ink`, `--glyphplot-series-1` …,
// REQ-THEME-010), so a page's own stylesheet themes it. The buttons set two
// palettes as inline custom properties, or remove them; the element repaints
// from the last frame without drawing a new one (REQ-COLOR-030). It also
// follows the page's `data-theme` on <html> and `prefers-color-scheme`
// (REQ-THEME-030): the site's theme button changes it with the rest of the
// page.

import type { Example } from "./api.ts";
import { walk } from "../src/data/gen.ts";
import { rng, round } from "../src/data/rng.ts";
import { DAY, EPOCH, table, times } from "../src/data/table.ts";
import { button, controls, Scope } from "./support/ui.ts";

const SEED = 24;

const PRESETS: Record<string, Record<string, string>> = {
  Harbour: {
    "--glyphplot-ground": "#0f2430",
    "--glyphplot-ink": "#e6f1f5",
    "--glyphplot-muted": "#9fb8c4",
    "--glyphplot-rule": "#24414f",
    "--glyphplot-accent": "#ffd166",
    "--glyphplot-series-1": "#6fd3ff",
    "--glyphplot-series-2": "#ff9f6e",
    "--glyphplot-series-3": "#b6e388",
  },
  Ember: {
    "--glyphplot-ground": "#fff8f0",
    "--glyphplot-ink": "#2b1d14",
    "--glyphplot-muted": "#6b5546",
    "--glyphplot-rule": "#ead8c8",
    "--glyphplot-accent": "#a4161a",
    "--glyphplot-series-1": "#b3541e",
    "--glyphplot-series-2": "#1f6f8b",
    "--glyphplot-series-3": "#5c3d99",
  },
};

export const example: Example = {
  slug: "css-themes",
  seed: SEED,
  start() {
    const n = 60;
    const r = rng(SEED);
    return {
      type: "line",
      data: table("visits", [
        { name: "date", type: "timestamp", values: times(EPOCH, n, DAY) },
        ...["web", "app", "api"].map((name, k) => ({ name, values: walk(r, n, 30 + 10 * k, 3).map((v) => round(v)) })),
      ]),
    };
  },
  live(el, host) {
    const s = new Scope();
    const row = controls(s, host, "Theme");
    const names = Object.keys(PRESETS["Harbour"] as Record<string, string>);
    const apply = (preset: Record<string, string> | null): void => {
      for (const k of names) el.style.removeProperty(k);
      for (const [k, v] of Object.entries(preset ?? {})) el.style.setProperty(k, v);
      el.theme = null; // read the custom properties again
    };
    for (const [name, preset] of Object.entries(PRESETS)) button(s, row, name, () => apply(preset));
    button(s, row, "The page's own", () => apply(null));
    s.add(() => apply(null));
    return s.stop;
  },
};
