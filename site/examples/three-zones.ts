// 16 · One series, three zones: the same 36 hours of data drawn in UTC, New
// York and Tokyo with the `timeZone` attribute (REQ-TZ-020). The instants are
// the same (REQ-DATA-090); only the ticks and labels move, placed at local
// midnights and hours from a zone table the shell builds with Intl. The
// three are linked, so the crosshair marks one instant in all of them.

import type { Example } from "./api.ts";
import { requests } from "../src/data/gen.ts";
import { EPOCH, HOUR, MINUTE, table, times } from "../src/data/table.ts";
import { cell, grid, Scope } from "./support/ui.ts";

const SEED = 16;

function data() {
  const t = times(EPOCH, 36 * 4, 15 * MINUTE);
  return table("requests", [
    { name: "t", type: "timestamp", values: t },
    { name: "requests", unit: "req/s", values: requests(SEED, t) },
  ]);
}

export const example: Example = {
  slug: "three-zones",
  seed: SEED,
  start() {
    return { type: "line", data: data(), options: { title: "UTC" }, view: { x: { domain: [EPOCH, EPOCH + 36 * HOUR] } }, attrs: { timezone: "UTC", link: "three-zones" } };
  },
  live(_el, host) {
    const s = new Scope();
    const g = grid(s, host, 2);
    for (const zone of ["America/New_York", "Asia/Tokyo"]) {
      cell(s, g, zone, { type: "line", rows: "10", timezone: zone, link: "three-zones" }, { data: data() });
    }
    return s.stop;
  },
};
