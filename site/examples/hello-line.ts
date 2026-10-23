// 3 · Hello, line: the smallest page. One tag, and data in the columns
// shorthand (REQ-DATA-120): `t` as dates makes the x axis time, `y` the one
// series; nothing else is configured. In a page:
//
//   <glyph-plot type="line"></glyph-plot>
//   <script type="module">
//     import "glyphplot/element";
//     document.querySelector("glyph-plot").data = { t: [...dates], y: [...numbers] };
//   </script>

import type { Example } from "./api.ts";
import { walk } from "../src/data/gen.ts";
import { rng, round } from "../src/data/rng.ts";
import { DAY, EPOCH } from "../src/data/table.ts";

const SEED = 3;

export const example: Example = {
  slug: "hello-line",
  seed: SEED,
  start() {
    const y = walk(rng(SEED), 60, 50, 1.5);
    return {
      type: "line",
      data: { t: Array.from(y, (_, i) => new Date(EPOCH + i * DAY)), y: Array.from(y, (v) => round(v)) },
    };
  },
};
