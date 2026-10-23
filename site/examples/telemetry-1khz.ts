// 54 · 1 kHz telemetry: a sensor sampled every millisecond, appended as it
// comes (each timer tick appends the samples since the last, which the shell
// writes to wasm once a frame, REQ-STREAM-010). The view keeps a five-second
// span ending at the newest sample (`x.follow`, REQ-VIEW-060); a pan away
// stops following and End, or the button, starts it again. Space on the
// focused chart, or the button, pauses: data is kept, not drawn, and resuming
// shows the live end (REQ-A11Y-080). The ring keeps the last minute
// (`retain.rows: 60000`).

import type { Example } from "./api.ts";
import { normal, rng, type Rng } from "../src/data/rng.ts";
import { EPOCH, table } from "../src/data/table.ts";
import { button, controls, pauseButton, Scope, status } from "./support/ui.ts";

const SEED = 54;
const START = 10_000; // ms of history

/** The sensor: two sines and noise, a sample a millisecond. */
function sample(r: Rng, ms: number): number {
  return Math.round((Math.sin(ms / 400) * 2 + Math.sin(ms / 37) * 0.4 + normal(r) * 0.15) * 1000) / 1000;
}

export const example: Example = {
  slug: "telemetry-1khz",
  seed: SEED,
  start() {
    const r = rng(SEED);
    const t = new Float64Array(START);
    const v = new Float64Array(START);
    for (let i = 0; i < START; i++) {
      t[i] = EPOCH + i;
      v[i] = sample(r, i);
    }
    return {
      type: "line",
      data: table("sensor", [{ name: "t", type: "timestamp", values: t }, { name: "signal", unit: "V", values: v }]),
      options: { retain: { rows: 60_000 } },
      view: { x: { domain: [EPOCH + START - 5000, EPOCH + START - 1], follow: true } },
    };
  },
  live(el, host) {
    const s = new Scope();
    const r = rng(SEED);
    for (let i = 0; i < START; i++) sample(r, i); // the generator carries on after the start's history
    let ms = START;
    const began = performance.now();
    s.every(16, () => {
      const until = START + Math.floor(performance.now() - began);
      if (until <= ms) return;
      const n = until - ms;
      const t = new Float64Array(n);
      const v = new Float64Array(n);
      for (let i = 0; i < n; i++, ms++) {
        t[i] = EPOCH + ms;
        v[i] = sample(r, ms);
      }
      el.append({ t, signal: v });
    });
    const row = controls(s, host, "Stream");
    pauseButton(s, row, el);
    button(s, row, "Follow the live end", () => {
      el.view = { x: { follow: true } };
    });
    const out = status(s, host);
    let last = 0;
    s.on(el, "plot-data", (e: CustomEvent<{ rows: number }>) => {
      const now = performance.now();
      if (now - last < 500) return;
      last = now;
      const x = el.view["x"] as { follow?: boolean } | undefined;
      out.textContent = `${e.detail.rows.toLocaleString("en-US")} samples held (simulated); ${el.paused ? "paused" : "live"}; following: ${x?.follow ? "yes" : "no"}`;
    });
    return s.stop;
  },
};
