// font.ts: the @font-face matches web/fonts (REQ-FONT-030, REQ-EL-010); the
// font covers tier B and stays under 32 KB (REQ-FONT-020).
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, statSync } from "node:fs";
import { UNICODE_RANGE, fontFace, FONT_FAMILY } from "../src/font.ts";

test("UNICODE_RANGE is web/fonts/unicode-range.txt", () => {
  const file = readFileSync(new URL("../fonts/unicode-range.txt", import.meta.url), "utf8").trim();
  assert.equal(UNICODE_RANGE, file);
});

test("the @font-face names the family, swaps and loads the woff2 beside the package", () => {
  const css = fontFace();
  assert.match(css, /font-family:"glyphplot mono"/);
  assert.match(css, /font-display:swap/);
  assert.match(css, /fonts\/glyphplot-mono\.woff2/);
  assert.match(css, /unicode-range:U\+0020-007E/);
  assert.equal(FONT_FAMILY, "glyphplot mono");
});

/** The code points a unicode-range value covers. */
function covered(range) {
  const out = new Set();
  for (const part of range.split(", ")) {
    const [lo, hi] = part.slice(2).split("-").map((h) => parseInt(h, 16));
    for (let cp = lo; cp <= (hi ?? lo); cp++) out.add(cp);
  }
  return out;
}

test("tier B: U+00C0–00FF and U+0100–017F, in a woff2 of at most 32,768 bytes", () => {
  const cps = covered(UNICODE_RANGE);
  for (let cp = 0xc0; cp <= 0x17f; cp++) assert.ok(cps.has(cp), `U+${cp.toString(16).toUpperCase().padStart(4, "0")}`);
  const bytes = statSync(new URL("../fonts/glyphplot-mono.woff2", import.meta.url)).size;
  assert.ok(bytes <= 32_768, `${bytes} bytes`);
});
