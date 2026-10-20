// The module's palette for a theme (REQ-COLOR-030): up and down are tokens at
// entries 5 and 6, the core's defaults unless the theme sets them
// (REQ-THEME-060). That the defaults are a blue and an orange at least 50
// CIEDE2000 apart under normal vision and three simulated deficiencies
// (REQ-THEME-065) is theme.rs's `default_colours_stay_apart_under_cvd`.
import { test } from "node:test";
import assert from "node:assert/strict";
import { BUILD, moduleOfType } from "./support/wasm.mjs";

const UP = 5;
const DOWN = 6;
const at = (bytes, i) => [...bytes.slice(i * 4, i * 4 + 4)];

test(`${BUILD}: up and down are palette tokens 5 and 6, a blue and an orange by default, set by the theme`, async () => {
  const { glue } = await moduleOfType("line");
  const dark = glue.resolve_palette("{}");
  assert.deepEqual([at(dark, UP), at(dark, DOWN)], [[0x56, 0xb4, 0xe9, 255], [0xe6, 0x9f, 0x00, 255]]);
  const light = glue.resolve_palette('{"paper":true}');
  assert.deepEqual([at(light, UP), at(light, DOWN)], [[0x07, 0x84, 0xff, 255], [0xb3, 0x5c, 0x00, 255]]);
  const set = glue.resolve_palette('{"up":"#123456","down":"#654321"}');
  assert.deepEqual([at(set, UP), at(set, DOWN)], [[0x12, 0x34, 0x56, 255], [0x65, 0x43, 0x21, 255]]);
  // Nothing else moves.
  for (let i = 0; i < 255; i++) if (i !== UP && i !== DOWN) assert.deepEqual(at(set, i), at(dark, i), `entry ${i}`);
});
