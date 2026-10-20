// theme.ts: custom properties to a theme object (REQ-THEME-010), the
// default set by the text colour's luma (REQ-THEME-020), and colour helpers.
import { test } from "node:test";
import assert from "node:assert/strict";
import { parseColor, toHex, luma, contrast, splitList, themeFromStyle, fillSeries, chooseTheme, paletteCss, lowContrastSeries, seriesOf } from "../src/theme.ts";

test("parseColor reads hex and rgb() forms", () => {
  assert.deepEqual(parseColor("#fff"), [255, 255, 255, 255]);
  assert.deepEqual(parseColor("#11223380"), [17, 34, 51, 128]);
  assert.deepEqual(parseColor("rgb(1, 2, 3)"), [1, 2, 3, 255]);
  assert.deepEqual(parseColor("rgba(1,2,3,0.5)"), [1, 2, 3, 128]);
  assert.deepEqual(parseColor("rgb(10% 20% 30% / 50%)"), [26, 51, 77, 128]);
  assert.equal(parseColor("oklch(0.5 0.1 200)"), null);
  assert.equal(toHex([255, 0, 16]), "#ff0010");
});

test("luma and contrast", () => {
  assert.ok(Math.abs(luma([255, 255, 255]) - 255) < 1e-9);
  assert.ok(luma([0x33, 0x33, 0x33]) < 128);
  assert.ok(Math.abs(contrast([0, 0, 0], [255, 255, 255]) - 21) < 1e-9);
});

test("splitList splits on top-level commas, else whitespace", () => {
  assert.deepEqual(splitList("#111, rgb(1, 2, 3) , #222"), ["#111", "rgb(1, 2, 3)", "#222"]);
  assert.deepEqual(splitList("#111 #222  #333"), ["#111", "#222", "#333"]);
});

const style = (props, color = "rgb(0, 0, 0)") => ({ getPropertyValue: (n) => props[n] ?? "", color });

test("themeFromStyle reads the --glyphplot-* properties", () => {
  const t = themeFromStyle(style({ "--glyphplot-ground": " #101010", "--glyphplot-accent": "rgb(255,0,0)", "--glyphplot-series-2": "#00f", "--glyphplot-scale": "#000, #fff", "--glyphplot-ink": "transparent-ish" }));
  assert.deepEqual(t, { ground: "#101010", accent: "#ff0000", series: [undefined, "#0000ff", undefined, undefined, undefined, undefined, undefined, undefined], scale: ["#000000", "#ffffff"] });
  assert.deepEqual(fillSeries(t.series, ["#a", "#b", "#c"]), ["#a", "#0000ff"]);
});

test("chooseTheme: the object wins; without a ground the luma picks the set; modes", () => {
  const dark = [230, 230, 230, 255];
  const black = [20, 20, 20, 255];
  assert.deepEqual(chooseTheme({ ink: "#000000" }, { ink: "#ffffff" }, "auto", black).theme.ink, "#ffffff");
  const lightPage = chooseTheme({}, null, "auto", black);
  assert.deepEqual(lightPage, { theme: { paper: true }, paintGround: false, paper: false }, "dark text: the light set, no ground");
  assert.deepEqual(chooseTheme({}, null, "auto", dark), { theme: {}, paintGround: false, paper: false }, "light text: the dark set");
  assert.equal(chooseTheme({ ground: "#000000" }, null, "auto", black).paintGround, true);
  assert.deepEqual(chooseTheme({}, null, "paper", dark), { theme: { paper: true }, paintGround: true, paper: true });
  assert.deepEqual(chooseTheme({}, null, "light", dark).theme, { paper: true });
  assert.deepEqual(chooseTheme({}, null, "dark", black).theme, {});
});

test("palette helpers", () => {
  const bytes = new Uint8Array(1020);
  bytes.set([255, 255, 255, 255], 4 * 4); // ground white
  bytes.set([250, 250, 250, 255], 8 * 4); // series 1, near white
  bytes.set([1, 2, 3, 128], 9 * 4);
  const css = paletteCss(bytes);
  assert.equal(css.length, 255);
  assert.equal(css[4], "#ffffff");
  assert.equal(css[9], "rgba(1,2,3,0.502)");
  assert.equal(seriesOf(bytes)[0], "#fafafa");
  assert.equal(lowContrastSeries({ series: ["#fafafa", "#000000"] }, bytes).length, 1);
});

test("up and down are tokens: --glyphplot-up and --glyphplot-down, the theme object over them (REQ-THEME-060)", () => {
  const t = themeFromStyle(style({ "--glyphplot-up": "#0784ff", "--glyphplot-down": "rgb(179, 92, 0)" }));
  assert.deepEqual(t, { up: "#0784ff", down: "#b35c00" });
  const choice = chooseTheme(t, { down: "#ff0000" }, "dark", null);
  assert.equal(choice.theme.up, "#0784ff");
  assert.equal(choice.theme.down, "#ff0000");
});
