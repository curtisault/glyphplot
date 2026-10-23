// 31 · From a CSV file: `src` points at a CSV, which the shell fetches once,
// reads (a header row, RFC 3339 times read as timestamps, numbers as
// numbers, an empty field as missing) and gives as `data` (REQ-DATA-150). The
// file here is written in the page from a seeded generator (a day of CPU
// readings every five minutes, some missing) and served to `src` as a
// `blob:` URL of type text/csv, so the example needs no server; any URL the
// page may fetch works the same. The card's first frame is the same rows
// given as data.

import type { Example } from "./api.ts";
import { cpu, withGaps } from "../src/data/gen.ts";
import { rng } from "../src/data/rng.ts";
import { EPOCH, MINUTE, table, times } from "../src/data/table.ts";
import { make, Scope, status } from "./support/ui.ts";

const SEED = 31;

function rows(): { t: Float64Array; cpu: Float64Array } {
  const t = times(EPOCH, 288, 5 * MINUTE);
  return { t, cpu: withGaps(cpu(SEED, t), rng(SEED + 1), 3, 6) };
}

/** The CSV text: `time,cpu`, an empty field where a reading is missing. */
function csv(): string {
  const { t, cpu: v } = rows();
  const lines = ["time,cpu"];
  t.forEach((x, i) => lines.push(`${new Date(x).toISOString()},${Number.isNaN(v[i]) ? "" : v[i]}`));
  return `${lines.join("\n")}\n`;
}

export const example: Example = {
  slug: "csv-src",
  seed: SEED,
  start() {
    const { t, cpu: v } = rows();
    return { type: "line", data: table("data", [{ name: "time", type: "timestamp", values: t }, { name: "cpu", values: v }]) };
  },
  live(el, host) {
    const s = new Scope();
    const text = csv();
    const url = URL.createObjectURL(new Blob([text], { type: "text/csv" }));
    s.add(() => URL.revokeObjectURL(url));
    const out = status(s, host, `src: a CSV of ${text.length} characters.`);
    s.on(el, "plot-data", (e: CustomEvent<{ rows: number; version: number }>) => {
      out.textContent = `src: a CSV of ${text.length} characters; the chart holds ${e.detail.rows} rows (data version ${e.detail.version}).`;
    });
    el.src = url;
    const head = s.put(host, make("pre", "gp-ex-code", text.split("\n").slice(0, 4).join("\n") + "\n…"));
    head.setAttribute("aria-label", "The CSV's first lines");
    return s.stop;
  },
};
