// font.ts: the @font-face matches web/fonts (REQ-FONT-030, REQ-EL-010).
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
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
