// The theme: the `--glyphplot-*` custom properties read from the element's
// computed style (REQ-THEME-010), a theme object that wins over them, and the
// choice of default set by the computed text colour's luma when there is no
// ground (REQ-THEME-020). The module's `resolve_palette(theme_json)` turns
// the result into the 255 entries (REQ-COLOR-030); the element re-resolves
// and repaints, drawing no new frame, when the colour scheme, `<html>`'s
// class, style or data-theme, or `theme` change (REQ-THEME-030). Under
// `forced-colors: active` a chart draws in one ink, CanvasText on Canvas, the
// accent in Highlight; under `@media print` it draws on paper (REQ-THEME-100).

import { PALETTE_LEN, Token } from "./api.ts";

/** A theme object (spec/schemas/theme.schema.json); colours as #rrggbb. */
export interface Theme {
  ground?: string;
  ink?: string;
  muted?: string;
  rule?: string;
  accent?: string;
  selection?: string;
  up?: string;
  down?: string;
  series?: string[];
  scale?: string[];
  diverging?: string[];
  paper?: boolean;
}

/** The `theme` attribute's values. */
export type ThemeMode = "auto" | "dark" | "light" | "paper";

/** The single-colour tokens and their custom properties. */
export const TOKENS = ["ground", "ink", "muted", "rule", "accent", "selection", "up", "down"] as const;

type Rgba = [number, number, number, number];

const clamp255 = (v: number): number => Math.max(0, Math.min(255, Math.round(v)));

/** Parses #rgb, #rgba, #rrggbb, #rrggbbaa, rgb() and rgba() (comma or space syntax); null for anything else. */
export function parseColor(text: string): Rgba | null {
  const s = text.trim().toLowerCase();
  let m = /^#([0-9a-f]{3,4}|[0-9a-f]{6}|[0-9a-f]{8})$/.exec(s);
  if (m) {
    let h = m[1] as string;
    if (h.length <= 4) h = [...h].map((c) => c + c).join("");
    const n = (i: number): number => parseInt(h.slice(i, i + 2), 16);
    return [n(0), n(2), n(4), h.length === 8 ? n(6) : 255];
  }
  m = /^rgba?\(\s*([^)]*)\)$/.exec(s);
  if (!m) return null;
  const args = (m[1] as string).split(/\s*[,/]\s*|\s+/).filter((x) => x !== "");
  if (args.length < 3 || args.length > 4) return null;
  const chan = (x: string): number => (x.endsWith("%") ? (parseFloat(x) * 255) / 100 : parseFloat(x));
  const alpha = (x: string | undefined): number => (x === undefined ? 255 : x.endsWith("%") ? (parseFloat(x) * 255) / 100 : parseFloat(x) * 255);
  const out: Rgba = [clamp255(chan(args[0] as string)), clamp255(chan(args[1] as string)), clamp255(chan(args[2] as string)), clamp255(alpha(args[3]))];
  return out.some((v) => Number.isNaN(v)) ? null : out;
}

/** `#rrggbb` of a colour. */
export function toHex(c: readonly number[]): string {
  return "#" + [c[0], c[1], c[2]].map((v) => clamp255(v ?? 0).toString(16).padStart(2, "0")).join("");
}

/** The luma REQ-THEME-020 decides by: 0.2126 r + 0.7152 g + 0.0722 b, 0–255. */
export function luma(c: readonly number[]): number {
  return 0.2126 * (c[0] ?? 0) + 0.7152 * (c[1] ?? 0) + 0.0722 * (c[2] ?? 0);
}

/** WCAG 2.2 relative luminance of an sRGB colour. */
export function luminance(c: readonly number[]): number {
  const lin = (v: number): number => {
    const s = v / 255;
    return s <= 0.04045 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * lin(c[0] ?? 0) + 0.7152 * lin(c[1] ?? 0) + 0.0722 * lin(c[2] ?? 0);
}

/** WCAG 2.2 contrast ratio of two colours. */
export function contrast(a: readonly number[], b: readonly number[]): number {
  const [la, lb] = [luminance(a), luminance(b)];
  return (Math.max(la, lb) + 0.05) / (Math.min(la, lb) + 0.05);
}

/** Splits a list of colours on top-level commas, else on top-level whitespace. */
export function splitList(text: string): string[] {
  const split = (sep: RegExp): string[] => {
    const out: string[] = [];
    let depth = 0;
    let cur = "";
    for (const ch of text) {
      if (ch === "(") depth++;
      if (ch === ")") depth--;
      if (depth === 0 && sep.test(ch)) {
        if (cur.trim()) out.push(cur.trim());
        cur = "";
      } else cur += ch;
    }
    if (cur.trim()) out.push(cur.trim());
    return out;
  };
  const byComma = split(/,/);
  return byComma.length > 1 ? byComma : split(/\s/);
}

/** Resolves any CSS colour to RGBA; the browser resolves what {@link parseColor} cannot. */
export type ColorResolver = (css: string) => Rgba | null;

let probe: CanvasRenderingContext2D | null | undefined;
const resolved = new Map<string, Rgba | null>();

/** A resolver that falls back to drawing one pixel with the colour on a canvas (oklch, named colours, color-mix…). */
export const browserColor: ColorResolver = (css) => {
  const own = parseColor(css);
  if (own) return own;
  if (resolved.has(css)) return resolved.get(css) ?? null;
  if (probe === undefined) {
    probe = typeof document === "undefined" ? null : document.createElement("canvas").getContext("2d", { willReadFrequently: true });
  }
  let out: Rgba | null = null;
  if (probe && typeof CSS !== "undefined" && CSS.supports("color", css)) {
    probe.clearRect(0, 0, 1, 1);
    probe.fillStyle = "#000";
    probe.fillStyle = css;
    probe.fillRect(0, 0, 1, 1);
    const d = probe.getImageData(0, 0, 1, 1).data;
    out = [d[0] ?? 0, d[1] ?? 0, d[2] ?? 0, d[3] ?? 0];
  }
  if (resolved.size < 1024) resolved.set(css, out);
  return out;
};

/** What the computed style gives: `getPropertyValue` of each custom property, and `color`. */
export interface StyleSource {
  getPropertyValue(name: string): string;
  color: string;
}

/** A theme read from custom properties: series may have gaps until {@link fillSeries} fills them. */
export type StyleTheme = Omit<Theme, "series"> & { series?: (string | undefined)[] };

/** The tokens the custom properties set (REQ-THEME-010); unset or unreadable ones left out. */
export function themeFromStyle(style: StyleSource, color: ColorResolver = browserColor): StyleTheme {
  const t: StyleTheme = {};
  const one = (name: string): string | undefined => {
    const v = style.getPropertyValue(`--glyphplot-${name}`).trim();
    if (!v) return undefined;
    const c = color(v);
    return c && c[3] > 0 ? toHex(c) : undefined;
  };
  for (const k of TOKENS) {
    const v = one(k);
    if (v !== undefined) t[k] = v;
  }
  const series: (string | undefined)[] = [];
  for (let i = 1; i <= 8; i++) series.push(one(`series-${i}`));
  if (series.some((s) => s !== undefined)) t.series = series;
  const scale = style.getPropertyValue("--glyphplot-scale").trim();
  if (scale) {
    const stops = splitList(scale)
      .map((s) => color(s))
      .filter((c): c is Rgba => c !== null)
      .map(toHex);
    if (stops.length >= 2) t.scale = stops;
  }
  return t;
}

/**
 * `series` with each unset entry taken from `defaults` (the default set's
 * series, read from a palette resolved without them), trimmed after the last
 * set one: a page may set `--glyphplot-series-3` alone.
 */
export function fillSeries(series: readonly (string | undefined)[], defaults: readonly string[]): string[] {
  let last = -1;
  series.forEach((s, i) => {
    if (s !== undefined) last = i;
  });
  return series.slice(0, last + 1).map((s, i) => s ?? defaults[i] ?? "#808080");
}

/** A resolved theme for one element: the JSON resolve_palette takes, and whether to paint the ground. */
export interface ThemeChoice {
  /** The theme object resolve_palette gets. */
  theme: Theme;
  /** Whether the drawing paints palette entry 4 behind every cell. */
  paintGround: boolean;
  /** Whether frames are drawn on paper (`frame(t, paper)`). */
  paper: boolean;
}

/**
 * The theme an element resolves (REQ-THEME-010, REQ-THEME-020): the
 * custom-property tokens, then the theme object over them, token by token.
 * With no ground and no ink, mode `auto` picks the light default set when
 * the computed text colour's luma is under 128, the dark set otherwise; it
 * asks for the light set with `paper: true`, which in theme.schema.json picks
 * the light defaults. A chart without a ground paints none; `dark`, `light`
 * and `paper` paint their set's ground.
 */
export function chooseTheme(fromStyle: Theme, override: Theme | null, mode: ThemeMode, textColor: Rgba | null): ThemeChoice {
  const theme: Theme = { ...fromStyle, ...(override ?? {}) };
  const own = theme.ground !== undefined;
  if (mode === "paper") return { theme: { ...theme, paper: true }, paintGround: true, paper: true };
  if (mode === "light") {
    return { theme: own ? theme : { ...theme, paper: true }, paintGround: true, paper: false };
  }
  if (mode === "dark") {
    delete theme.paper;
    return { theme, paintGround: true, paper: false };
  }
  if (!own && theme.ink === undefined && theme.paper === undefined && textColor !== null && luma(textColor) < 128) {
    return { theme: { ...theme, paper: true }, paintGround: false, paper: false };
  }
  return { theme, paintGround: own, paper: theme.paper === true };
}

/** The palette's entries as CSS colours, index by index. */
export function paletteCss(bytes: Uint8Array): string[] {
  const out: string[] = [];
  for (let i = 0; i < PALETTE_LEN; i++) {
    const [r, g, b, a] = [bytes[i * 4] ?? 0, bytes[i * 4 + 1] ?? 0, bytes[i * 4 + 2] ?? 0, bytes[i * 4 + 3] ?? 0];
    out.push(a === 255 ? toHex([r, g, b]) : `rgba(${r},${g},${b},${(a / 255).toFixed(3)})`);
  }
  return out;
}

/** Entry `i` of a palette as RGBA. */
export function entry(bytes: Uint8Array, i: number): Rgba {
  return [bytes[i * 4] ?? 0, bytes[i * 4 + 1] ?? 0, bytes[i * 4 + 2] ?? 0, bytes[i * 4 + 3] ?? 0];
}

/** The default series of a palette resolved without page series, as #rrggbb. */
export function seriesOf(bytes: Uint8Array): string[] {
  return Array.from({ length: 8 }, (_, i) => toHex(entry(bytes, Token.series1 + i)));
}

/**
 * Page-set series colours under 3:1 against the resolved ground, as
 * warnings (REQ-THEME-070, debug builds only): the colours are still used.
 */
export function lowContrastSeries(theme: Theme, palette: Uint8Array): string[] {
  const ground = entry(palette, Token.ground);
  const out: string[] = [];
  (theme.series ?? []).forEach((s, i) => {
    const c = parseColor(s);
    if (!c) return;
    const r = contrast(c, ground);
    if (r < 3) out.push(`glyphplot: --glyphplot-series-${i + 1} ${s} has contrast ${r.toFixed(2)}:1 against the ground ${toHex(ground)}, under 3:1`);
  });
  return out;
}

/** The media queries that change how a chart is themed: forced colours and print (REQ-THEME-100). */
export interface MediaState {
  /** `(forced-colors: active)`. */
  forced: boolean;
  /** `print`. */
  print: boolean;
}

/** Between beforeprint and afterprint: the print media query may not match yet when beforeprint fires. */
let printing = false;

/** What the media queries say now; both false where there is no `matchMedia`. */
export function mediaState(): MediaState {
  if (typeof matchMedia !== "function") return { forced: false, print: printing };
  return { forced: matchMedia("(forced-colors: active)").matches, print: printing || matchMedia("print").matches };
}

/** The system colours a forced-colours theme is made of, as #rrggbb. */
export interface SystemColors {
  canvas: string;
  canvasText: string;
  highlight: string;
}

/**
 * The system colours `Canvas`, `CanvasText` and `Highlight` as the browser
 * resolves them now, read from a hidden probe in `parent` (in forced colours
 * they are the reader's high-contrast colours); black on white with a blue
 * highlight for any it cannot read.
 */
export function systemColors(parent: Element): SystemColors {
  const doc = parent.ownerDocument;
  const probe = doc.createElement("span");
  probe.setAttribute("aria-hidden", "true");
  probe.style.cssText = "position:absolute;visibility:hidden;forced-color-adjust:none;color:CanvasText;background-color:Canvas;border:1px solid Highlight";
  parent.append(probe);
  try {
    const cs = getComputedStyle(probe);
    const hex = (css: string, fallback: string): string => {
      const c = parseColor(css);
      return c && c[3] > 0 ? toHex(c) : fallback;
    };
    return { canvas: hex(cs.backgroundColor, "#ffffff"), canvasText: hex(cs.color, "#000000"), highlight: hex(cs.borderTopColor, "#0000ff") };
  } finally {
    probe.remove();
  }
}

/**
 * The theme of forced colours (REQ-THEME-100): every token in `CanvasText`
 * on a `Canvas` ground, but the accent and the selection in `Highlight`. The
 * element draws it in one ink (mono), so series differ by dashes and fills,
 * not colour (REQ-MONO-010).
 */
export function forcedTheme(sys: SystemColors): Theme {
  const ink = sys.canvasText;
  return {
    ground: sys.canvas,
    ink,
    muted: ink,
    rule: ink,
    accent: sys.highlight,
    selection: sys.highlight,
    up: ink,
    down: ink,
    series: Array.from({ length: 8 }, () => ink),
    scale: [ink, ink],
    diverging: [ink, ink, ink],
  };
}

/**
 * The choice the media queries impose, or null to theme as usual
 * (REQ-THEME-100): forced colours draw {@link forcedTheme} with its ground
 * painted; print draws the default paper set (dark ink on a light ground,
 * painted), whatever the page's tokens, `theme` attribute or object say.
 */
export function mediaChoice(media: MediaState, sys: () => SystemColors): ThemeChoice | null {
  if (media.forced) return { theme: forcedTheme(sys()), paintGround: true, paper: false };
  if (media.print) return { theme: { paper: true }, paintGround: true, paper: true };
  return null;
}

/**
 * Calls `onChange` when the theme may have changed (REQ-THEME-030): a
 * `prefers-color-scheme` change, a change of `<html>`'s class, style or
 * data-theme, and forced colours or print turning on or off
 * (REQ-THEME-100). Returns the function that stops watching.
 */
export function watchTheme(onChange: () => void): () => void {
  if (typeof document === "undefined") return () => {};
  const stops: (() => void)[] = [];
  if (typeof matchMedia === "function") {
    for (const q of ["(prefers-color-scheme: dark)", "(forced-colors: active)", "print"]) {
      const mq = matchMedia(q);
      const f = (): void => onChange();
      mq.addEventListener("change", f);
      stops.push(() => mq.removeEventListener("change", f));
    }
  }
  if (typeof addEventListener === "function") {
    // A printout is laid out right after beforeprint: theme now, not on the media query's later change.
    const before = (): void => {
      printing = true;
      onChange();
    };
    const after = (): void => {
      printing = false;
      onChange();
    };
    addEventListener("beforeprint", before);
    addEventListener("afterprint", after);
    stops.push(() => {
      removeEventListener("beforeprint", before);
      removeEventListener("afterprint", after);
    });
  }
  if (typeof MutationObserver === "function") {
    const mo = new MutationObserver(() => onChange());
    mo.observe(document.documentElement, { attributes: true, attributeFilter: ["class", "style", "data-theme"] });
    stops.push(() => mo.disconnect());
  }
  return () => stops.forEach((s) => s());
}
