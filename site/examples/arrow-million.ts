// 32 · From Arrow: a million rows through `src`. The page writes an Arrow IPC
// stream of two columns (a millisecond timestamp and a float64), 1,000,000
// rows, from a seeded generator, and serves it to `src` as a `blob:` URL of
// type application/vnd.apache.arrow.stream; the shell hands the bytes to the
// module's own Arrow reader (REQ-DATA-140, REQ-DATA-150), with no JSON on
// the way. The card's first frame is the same rows given as typed arrays.

import type { Example } from "./api.ts";
import { walk } from "../src/data/gen.ts";
import { rng } from "../src/data/rng.ts";
import { EPOCH, SECOND, table, times } from "../src/data/table.ts";
import { Scope, status } from "./support/ui.ts";

const SEED = 32;
const N = 1_000_000;

function cols(): { t: Float64Array; y: Float64Array } {
  return { t: times(EPOCH, N, SECOND), y: walk(rng(SEED), N, 0, 1) };
}

export const example: Example = {
  slug: "arrow-million",
  seed: SEED,
  start() {
    const { t, y } = cols();
    return { type: "line", data: table("walk", [{ name: "t", type: "timestamp", values: t }, { name: "y", values: y }]) };
  },
  live(el, host) {
    const s = new Scope();
    const out = status(s, host, "Writing a million rows as Arrow…");
    let url: string | null = null;
    s.add(() => {
      if (url) URL.revokeObjectURL(url);
    });
    void import("../../web/test/support/arrow.mjs").then(({ arrowStream }) => {
      if (s.stopped) return;
      const { t, y } = cols();
      const bytes = arrowStream({ t: { type: "timestamp", values: t }, y: { type: "f64", values: y } });
      url = URL.createObjectURL(new Blob([bytes as Uint8Array<ArrayBuffer>], { type: "application/vnd.apache.arrow.stream" }));
      s.on(el, "plot-data", (e: CustomEvent<{ rows: number; version: number }>) => {
        out.textContent = `src: ${(bytes.byteLength / 1e6).toFixed(1)} MB of Arrow IPC; the chart holds ${e.detail.rows.toLocaleString("en-US")} rows (data version ${e.detail.version}).`;
      });
      el.src = url;
    });
    return s.stop;
  },
};
