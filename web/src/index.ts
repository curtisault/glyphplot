// The `glyphplot` package: the JS API (`chart`, `mount`) and the pieces the
// element is made of. `glyphplot/element` defines `<glyph-plot>` on import.
// Full packaging is v0.11.0.

export { chart, Chart, VIEW_RESET, type ChartInit, type Change, type View } from "./chart.ts";
export { mount, prefersReducedMotion, type FrameTiming, type Mount, type MountOptions } from "./mount.ts";
export { manifest, moduleOf, setManifest, loadModule, loadFor, instantiations, type GridViews, type Loaded } from "./wasm.ts";
export {
  prepare,
  describe,
  inferColumn,
  rowsToColumns,
  parseRfc3339,
  type DataInput,
  type DataObject,
  type BinaryData,
  type ColumnDesc,
  type TableDesc,
  type MatrixDesc,
  type AppendInput,
  type Row,
} from "./data.ts";
export { encode, type InputEvent, type Encoded } from "./input.ts";
export { localePack, localeOf, BROWSER_MINUS, type LocalePack } from "./locale.ts";
export { chooseTheme, themeFromStyle, paletteCss, watchTheme, type Theme, type ThemeMode } from "./theme.ts";
export { blockShape, isBlock, fillBlock, type Shape } from "./blocks.ts";
export { FONT_STACK, type RendererName } from "./render.ts";
export { FONT_FAMILY, fontFace } from "./font.ts";
export type { Manifest, Meta, TableJson, WasmChart, WasmModule, WasmEvent } from "./api.ts";
