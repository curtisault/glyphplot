// 25 · A brand theme: a theme object set from script (`el.theme = {…}`,
// spec/schemas/theme.schema.json), which wins over the custom properties
// token by token (REQ-THEME-010): a ground, ink and three series colours.
// The button switches between it and the default.

import type { Example } from "./api.ts";
import { rng } from "../src/data/rng.ts";
import { Scope, button, controls } from "./support/ui.ts";

const SEED = 25;

const BRAND = {
  ground: "#14213d",
  ink: "#f1f1f1",
  muted: "#b8c0d0",
  rule: "#2a3a5e",
  accent: "#fca311",
  series: ["#fca311", "#8ecae6", "#e5e5e5"],
};

export const example: Example = {
  slug: "brand-theme",
  seed: SEED,
  start() {
    const r = rng(SEED);
    const months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun"];
    const v = (base: number) => months.map(() => Math.round(base * (0.7 + r() * 0.6)));
    return { type: "bar", data: { month: months, north: v(120), south: v(90), online: v(160) }, options: { title: "Orders a month (simulated)" } };
  },
  live(el, host) {
    const s = new Scope();
    const row = controls(s, host, "Theme");
    el.theme = BRAND;
    button(s, row, "Brand theme", () => {
      const on = !(typeof el.theme === "object" && el.theme !== null && "series" in el.theme);
      el.theme = on ? BRAND : null;
      return on;
    }, true);
    return s.stop;
  },
};
