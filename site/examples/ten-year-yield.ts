// 9 · The ten-year yield: the Treasury's daily par yield at ten years, 2026
// to 2026-10-20, picked out of the whole curve with `encode` (REQ-DATA-080).
// The view starts on the last quarter; zoom out (−, or Ctrl or ⌘ with the
// wheel) for the year. The catalogue's series since 1962 (Federal Reserve
// H.15) waits for that dataset in datasets/; this year is what the
// repository holds.

import type { Example } from "./api.ts";
import { col, treasury } from "../src/data/real.ts";

export const example: Example = {
  slug: "ten-year-yield",
  seed: null,
  start() {
    const date = col(treasury, "date");
    const last = date[date.length - 1] as number;
    return {
      type: "line",
      data: treasury,
      options: { encode: { x: "date", y: "y10" }, title: "10-year Treasury par yield, %" },
      view: { x: { domain: [last - 91 * 86_400_000, last] } },
    };
  },
};
