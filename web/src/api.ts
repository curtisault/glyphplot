// The wasm boundary as the shell sees it: what `crates/wasm` exports through
// wasm-bindgen `--target web` into web/pkg/<module>/glyphplot.js, and what
// web/pkg/modules.js exports. This file is the one place the shell states the
// boundary (the plan's "The boundary" table, REQ-WASM-010, REQ-MANIFEST-010);
// matching a built module against it is a one-file check.
//
// Nothing here is called directly by pages: the shell is the only consumer of
// the wasm API (REQ-ARCH-030).

/** One chart type's meta (spec/schemas/meta.schema.json). */
export interface Meta {
  slug: string;
  name: string;
  note: string;
  family: string;
  modes: string[];
  shapes: string[];
  options: Record<string, unknown> | null;
  min: { cols: number; rows: number };
  drop: string[];
  mono: string;
  charset: string;
  budget: { gzip: number; frameMs: number; worstMs: number; inputMs: number };
}

/** What web/pkg/modules.js exports (spec/schemas/manifest.schema.json). */
export interface Manifest {
  /** Every chart type's meta, in registry order. */
  readonly charts: readonly unknown[];
  /** Chart type → the module that plays it. */
  readonly modules: Readonly<Record<string, string>>;
  /** Module name → a function importing `./<module>/glyphplot.js`. */
  readonly imports: Readonly<Record<string, () => Promise<unknown>>>;
  /** The module for a type `modules` does not list: `all` in the debug build, null in release. */
  readonly fallback: string | null;
  /** Each module's budgets. */
  readonly budgets: Readonly<Record<string, unknown>>;
}

/**
 * The `Chart` object of one wasm module (the plan's "The wasm `Chart`" table).
 * Every member exists from v0.3.0 unless marked. Methods that fail throw (a
 * wasm-bindgen `Result::Err` becomes a thrown JS value; the shell reads its
 * message with `String(e)` or `e.message`). A panic traps with a
 * `WebAssembly.RuntimeError`; the shell then refuses every later call on
 * that chart (REQ-WASM-040).
 */
export interface WasmChart {
  /** New options; data and view kept (ADR-120). Throws `options are not a JSON object: …`. */
  set_options(json: string): void;
  /**
   * A buffer of `n` values for column `i`, returned as a byte address in the
   * module's memory, which the shell fills through a view made after this
   * call returns (REQ-DATA-060). See {@link ColumnKind} for `kind` and the
   * column numbering.
   */
  reserve_column(i: number, kind: number, n: number): number;
  /** The same as reserve plus fill, from a Float64Array (one copy by the glue). */
  set_column(i: number, values: Float64Array): void;
  /**
   * Applies the reserved columns as the new data, described by the full form
   * of data.schema.json without numeric `values` (see {@link ColumnKind});
   * returns the data version. Throws the data error's message.
   */
  commit(descriptor_json: string): number;
  /** The locale pack (spec/schemas/locale.schema.json). */
  set_locale(json: string): void;
  /** The zone table (spec/schemas/zone.schema.json). v0.5.0: optional until then. */
  set_zone?(json: string): void;
  /** The whole view state (spec/schemas/view.schema.json). */
  view_json(): string;
  /** A whole or partial view, merged key by key; a key set to null returns to its default. */
  set_view(json: string): void;
  /**
   * One input event in the numeric encoding of the plan's "Input encoding"
   * table ({@link InputKind}); returns the bitmask of {@link Changed}.
   */
  input(kind: number, x: number, y: number, a: number, b: number, buttons: number, mods: number, t: number): number;
  /** The events the last inputs produced, as `[{type, detail}]` (events.schema.json); `[]` when none. */
  events_json(): string;
  /** A new grid size; the view kept in data units. */
  resize(cols: number, rows: number): void;
  /** One ink for the frames that follow. */
  set_mono(on: boolean): void;
  /** Draws the frame at play time `t` (seconds) into the grid. */
  frame(t: number, paper: boolean): void;
  cols(): number;
  rows(): number;
  /** Address of `cols × rows` u32 code points, row by row; {@link WIDE_TAIL} follows a double-width glyph. */
  cells_ptr(): number;
  /** Address of `cols × rows` u8 ink palette indices. */
  colors_ptr(): number;
  /** Address of `cols × rows` u8 back palette indices; {@link NO_BACK} is none. */
  backs_ptr(): number;
  /** The frame as text: `rows` lines of `cols` cells joined by "\n". */
  text(): string;
  /** The drawing's accessible label (REQ-A11Y-010). */
  summary(): string;
  /** The hidden table's rows, as a JSON {@link TableJson}. */
  table_json(max_rows: number): string;
  /** This chart type's meta, as JSON ({@link Meta}). */
  meta_json(): string;
  /** Releases the chart (REQ-WASM-050). */
  free(): void;
}

/** `new Chart(type, options_json)`: throws `there is no chart named "<type>"` or `options are not a JSON object: …`. */
export type WasmChartClass = new (type: string, options_json: string) => WasmChart;

/** What wasm-bindgen's `default(init)` resolves to: the instance's exports, `memory` among them. */
export interface InitOutput {
  readonly memory: WebAssembly.Memory;
}

/** One module, web/pkg/<module>/glyphplot.js. */
export interface WasmModule {
  /** Instantiates the module; with no argument the glue fetches its own `glyphplot_bg.wasm`. */
  default(init?: { module_or_path: unknown }): Promise<InitOutput>;
  Chart: WasmChartClass;
  /** Every type's meta this module plays, as a JSON array. */
  charts_json(): string;
  /** WIDE_TAIL, the code point that follows a double-width glyph (0). */
  wide_tail(): number;
  /** The 255 RGBA entries for a theme object (theme.schema.json): 1,020 bytes. Throws `theme \`<field>\`: …`. */
  resolve_palette(theme_json: string): Uint8Array;
  /** Debug builds only: installs the panic hook. */
  start?(): void;
}

/**
 * `kind` of `reserve_column(i, kind, n)`.
 *
 * Columns are numbered across the descriptor: every table's columns in table
 * order then column order (string columns counted though never reserved),
 * then each matrix's values as one more column, in matrix order.
 *
 * - `F64` (0): a column of `n` f64. Number, bool (1, 0), timestamp and
 *   duration columns. A timestamp or duration buffer holds values in the
 *   column's `time_unit` (default ms), as the JSON form's numbers would be;
 *   the core converts them as it holds them. Missing is NaN.
 * - `U32` (1): `n` u32 category codes into the column's `categories`;
 *   missing is 0xFFFFFFFF.
 *
 * A `string` column is not reserved: its labels travel in the descriptor's
 * `values`, NFC-normalised (REQ-I18N-050), as category labels do.
 */
export const ColumnKind = { F64: 0, U32: 1 } as const;

/** `kind` of `input(kind, …)`. */
export const InputKind = { move: 0, down: 1, up: 2, leave: 3, wheel: 4, pinch: 5, pan: 6, tap: 7, press: 8, key: 9 } as const;
export type InputKindName = keyof typeof InputKind;

/** `pointer` part of `buttons` (buttons + 256 × pointer). */
export const PointerCode = { mouse: 0, pen: 1, touch: 2 } as const;

/** `mods` bits. */
export const Mods = { shift: 1, ctrl: 2, alt: 4, meta: 8 } as const;

/** The bits `input` returns. */
export const Changed = { overlay: 1, base: 2, events: 4 } as const;

/** Key codes: the index of each `KeyboardEvent.key` (the plan's "Input encoding"). 25–30 are the CLI's. */
export const KEYS: readonly string[] = [
  "ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "Home", "End",
  "PageUp", "PageDown", "+", "=", "-", "0",
  "Enter", "Escape", " ", "?", "1", "2", "3", "4", "5", "6", "7", "8", "9",
  "h", "j", "k", "l", "f", "q",
];

/** The keys the browser shell sends: every key code below 25. */
export const SHELL_KEY_COUNT = 25;

/** The code point that follows a double-width glyph (`wide_tail()`). */
export const WIDE_TAIL = 0;

/** The back index for no back colour. */
export const NO_BACK = 255;

/** The palette's token indices (the plan's palette table). */
export const Token = { ink: 0, muted: 1, rule: 2, accent: 3, ground: 4, up: 5, down: 6, selection: 7, series1: 8 } as const;

/** Entries in a resolved palette. */
export const PALETTE_LEN = 255;

/**
 * What `table_json(max_rows)` returns: the data in the current view, at
 * most `max_rows` rows (downsampled, each bucket's minimum and maximum kept),
 * for the hidden table (REQ-A11Y-020). The first column is the row header
 * (x). Values are numbers as stored (timestamps in epoch ms), labels as
 * strings, missing as null; a string value is shown as it is.
 */
export interface TableJson {
  /** The caption's text, without the count. */
  caption: string;
  /** Column headings, x first. */
  columns: string[];
  /** Which columns hold epoch-ms timestamps, by index; optional. */
  time?: number[];
  rows: (number | string | boolean | null)[][];
  /** Data points the view holds; the caption states it when more than `rows.length`. */
  total: number;
}

/** One event the module reports (events.schema.json). */
export interface WasmEvent {
  type: string;
  detail: unknown;
}

/*
 * The input sequence the browser shell sends (REQ-INPUT-030, REQ-INPUT-040,
 * REQ-TOUCH-010), so the reducer can tell the gestures apart:
 *
 * - Mouse and pen: `down`, `move` (buttons held: a drag), `up` as the
 *   pointer events come. At most one `move` a frame, the latest, after the
 *   frame's other events; a move queued before a `down`, `up` or `leave` in
 *   the same frame is dropped, so `up` carries the drag's last position; a press and release within 5 px is followed by
 *   `tap` (count 1, or 2 within 500 ms and 30 px of the last tap: the
 *   double-click reset). Hover is `move` with buttons 0. `leave` on
 *   pointerleave and on blur.
 * - Wheel: only with Ctrl or ⌘ held, or with the option `wheel: "zoom"`;
 *   otherwise the page scrolls and nothing is sent. Deltas in lines, summed
 *   over a frame. A trackpad pinch arrives as a Ctrl wheel.
 * - Touch, one finger: `down`, `move` (scrub), `up` with pointer touch; a
 *   still finger held 500 ms sends `press` (a brush starts; later moves
 *   extend it); a release within 5 px sends `tap` (count 2 for a double
 *   tap). A vertical swipe scrolls the page (`touch-action: pan-y`): the
 *   browser cancels the pointer and the shell sends `up`.
 * - Touch, two fingers: the first finger's `up`, then `pan` (centroid
 *   movement in cells) and `pinch` (spread ratio about the centroid) until a
 *   finger lifts, which sends `pan` with end 1 and the release velocity in
 *   cells a second, and `pinch` with end 1 if the spread changed.
 * - Keys on the focused cursor: `key` for the keys of {@link KEYS} below 25,
 *   with mods. The control row's buttons send the keys `-`, `+`, `0`,
 *   `ArrowLeft` and `ArrowRight`.
 * - `t` is the shell's play time, which advances only while a `motion`
 *   record is in flight (REQ-MOUNT-020).
 */
