// 4 · A million points: 1,000,000 rows, one a second (about 11.6 days), as
// two Float64Array columns in the full form, copied once into wasm
// (REQ-MEM-020). Every frame draws from M4 aggregates of the rows in view, the
// first, last, least and greatest a dot column (REQ-SAMPLE-010), so a pan or a
// zoom (drag, Ctrl or ⌘ with the wheel, a pinch, or + and − on the focused
// chart) redraws in a few milliseconds whatever the zoom.

import type { Example } from "./api.ts";
import { walk } from "../src/data/gen.ts";
import { rng } from "../src/data/rng.ts";
import { EPOCH, SECOND, table, times } from "../src/data/table.ts";

const SEED = 4;
const N = 1_000_000;

export const example: Example = {
  slug: "million-points",
  seed: SEED,
  start() {
    return {
      type: "line",
      data: table("walk", [
        { name: "t", type: "timestamp", values: times(EPOCH, N, SECOND) },
        { name: "value", values: walk(rng(SEED), N, 0, 1) },
      ]),
      attrs: { label: "A million points of a simulated random walk" },
    };
  },
};
