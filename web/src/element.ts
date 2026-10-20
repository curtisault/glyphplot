// `<glyph-plot>`: importing glyphplot/element defines it once and injects one
// `<style>` with the bundled @font-face and the element's rules, each wrapped
// in `:where()`; where there is no DOM or no customElements it does nothing
// (REQ-EL-010, REQ-FONT-030). Light DOM (REQ-EL-130), not form-associated
// (REQ-EL-150), lazy until within a screen height of the viewport
// (REQ-LOAD-020). `define(tag, { type })` defines a tag whose chart type is
// fixed (REQ-EL-030). A `type` change carries the data and view when the new
// chart takes the data's shape (REQ-EL-050); `fit` scales a fixed grid
// (REQ-EL-110); charts sharing a `link` share `x.domain` and `cursor.x`
// (REQ-LINK-010); `timeZone` gives the core its zone table (REQ-TZ-020, 050).

import { InputKind } from "./api.ts";
import {
  type A11yParts,
  Formats,
  REWRITE_MS,
  TABLE_ROWS,
  figureName,
  makeParts,
  throttle,
  valueText,
  writeTable,
} from "./a11y.ts";
import { chart, type Chart, type View } from "./chart.ts";
import { describe, emptyLike, prepare, type AppendInput, type DataInput, type DataObject, type Prepared } from "./data.ts";
import { fontFace } from "./font.ts";
import { keyCode } from "./input.ts";
import { localeOf, localePack } from "./locale.ts";
import { mount, type Mount } from "./mount.ts";
import { FONT_STACK, type RendererName } from "./render.ts";
import { encodeOf, shapeOf } from "./shape.ts";
import {
  browserColor,
  chooseTheme,
  entry,
  fillSeries,
  lowContrastSeries,
  seriesOf,
  themeFromStyle,
  toHex,
  watchTheme,
  type Theme,
  type ThemeMode,
} from "./theme.ts";
import { manifest, messageOf } from "./wasm.ts";
import { coveringFor, covers, resolveZone, zoneTable, type ZoneTable } from "./zone.ts";
import type { Overlay } from "./debug.ts";

/** The tag. */
export const TAG = "glyph-plot";

/** The attributes the element observes: the plan's element table, as of v0.5.0 (REQ-EL-020), and `timeZone` (REQ-TZ-020), lower-cased as HTML gives attribute names. */
export const OBSERVED = ["type", "data", "options", "view", "theme", "locale", "mono", "motion", "label", "cols", "rows", "fit", "loading", "link", "time", "timezone"] as const;

/** Rows without a height of the element's own or a `rows` attribute (REQ-EL-100). */
export const DEFAULT_ROWS = 12;

/** The least font size drawn, CSS px, by device pixel ratio (REQ-EL-085). */
export function fontFloor(dpr: number): number {
  return dpr < 2 ? 11 : 9;
}

/** The cell width in device px for a computed font size (REQ-EL-080, REQ-EL-085). */
export function cellWidth(fontPx: number, dpr: number): number {
  return Math.max(1, Math.round(0.6 * Math.max(fontPx, fontFloor(dpr)) * dpr));
}

/** The grid for a content box in device px and a cell width: whole cells, at least one, within a pixel of rounding. */
export function gridFor(w: number, h: number, cw: number): { cols: number; rows: number } {
  return { cols: Math.max(1, Math.floor((w + 1) / cw)), rows: Math.max(1, Math.floor((h + 1) / (cw * 2))) };
}

/** The `fit` values (REQ-EL-110); any other value is no fit. */
export type Fit = "width" | "contain";

/** The `fit` attribute's value, or null for none or any other value (REQ-EL-110). */
export function fitOf(v: string | null): Fit | null {
  return v === "width" || v === "contain" ? v : null;
}

/**
 * Where a fixed grid of `cols` × `rows` is drawn in a content box `w` × `h`
 * (device px), and its cell's width in device px, possibly fractional
 * (REQ-EL-110): `width` makes the cell w / cols; `contain` the largest cell
 * whose grid fits inside w × h, keeping the cell's 1:2 aspect, acting as
 * `width` when h is 0; no fit keeps `cw`. The box is centred.
 */
export function fitBox(fit: Fit | null, w: number, h: number, cols: number, rows: number, cw: number): { cell: number; x: number; y: number; width: number; height: number } {
  let cell = cw;
  if (fit === "width" || (fit === "contain" && h <= 0)) cell = w / cols;
  else if (fit === "contain") cell = Math.min(w / cols, h / (2 * rows));
  const width = cols * cell;
  const height = rows * 2 * cell;
  return { cell, x: Math.max(0, (w - width) / 2), y: Math.max(0, (h - height) / 2), width, height };
}

/** The renderer the element uses; the owner's measurement picks it (REQ-RENDER-050). */
export const config: { renderer: RendererName } = { renderer: "text" };

const where = (sel: string, body: string): string => `:where(${sel}){${body}}`;

/** The element's rules for `tag`, each wrapped in :where() so any page rule wins. */
export function cssFor(tag: string): string {
  return [
    where(tag, `display:block;position:relative;overflow:hidden;height:var(--gp-auto-height,auto);font-family:${FONT_STACK};font-variant-ligatures:none`),
    where(`${tag} .gp-frame`, "position:absolute;left:0;top:0;touch-action:pan-y;-webkit-user-select:none;user-select:none;-webkit-touch-callout:none"),
    where(`${tag} .gp-drawing`, "display:block;position:absolute;left:0;top:0;margin:0;padding:0;border:0;white-space:pre;overflow:hidden;font-family:inherit;font-kerning:none;font-variant-ligatures:none;letter-spacing:0"),
    where(`${tag} .gp-cursor`, "position:absolute;inset:0;outline:none"),
    where(`${tag} .gp-cursor:focus-visible`, "outline:2px solid var(--gp-accent,Highlight);outline-offset:-2px"),
    where(`${tag} .gp-controls`, "position:absolute;right:2px;top:2px;display:flex;gap:2px;opacity:0;pointer-events:none;transition:opacity .15s"),
    where(`${tag}:hover .gp-controls, ${tag}:focus-within .gp-controls`, "opacity:1;pointer-events:auto"),
    where(`${tag} .gp-controls button`, "font:inherit;min-width:24px;min-height:24px;padding:0 4px;color:var(--gp-ink,CanvasText);background:var(--gp-ground,Canvas);border:1px solid var(--gp-muted,GrayText);border-radius:2px;cursor:pointer"),
    where(`${tag} .gp-sr`, "position:absolute;width:1px;height:1px;overflow:hidden;clip-path:inset(50%);white-space:nowrap"),
    where(`${tag} .gp-loading`, "visibility:hidden"),
    `@media (prefers-reduced-motion: reduce){${where(`${tag} .gp-controls`, "transition:none")}}`,
  ].join("\n");
}

/** The element's rules. */
export const CSS = cssFor(TAG);

/**
 * Injects the one `<style>` (REQ-EL-010): the @font-face and the element's
 * rules, and the rules for `tag` once when it is an alias `define` made.
 */
export function injectStyle(doc: Document, tag: string = TAG): void {
  let style = doc.getElementById("glyphplot-style");
  if (!style) {
    style = doc.createElement("style");
    style.id = "glyphplot-style";
    style.textContent = `${fontFace()}\n${CSS}`;
    style.dataset["tags"] = TAG;
    (doc.head ?? doc.documentElement).append(style);
  }
  const tags = (style.dataset["tags"] ?? TAG).split(" ");
  if (tags.includes(tag)) return;
  style.textContent += `\n${cssFor(tag)}`;
  style.dataset["tags"] = [...tags, tag].join(" ");
}

const Base: typeof HTMLElement = typeof HTMLElement === "undefined" ? (class {} as unknown as typeof HTMLElement) : HTMLElement;

interface Deferred {
  promise: Promise<GlyphPlot>;
  resolve(v: GlyphPlot): void;
  reject(e: Error): void;
  settled: boolean;
}

function deferred(): Deferred {
  const d = { settled: false } as Deferred;
  d.promise = new Promise<GlyphPlot>((res, rej) => {
    d.resolve = (v) => {
      d.settled = true;
      res(v);
    };
    d.reject = (e) => {
      d.settled = true;
      rej(e);
    };
  });
  d.promise.catch(() => {}); // a failure is reported by plot-error
  return d;
}

const intAttr = (v: string | null): number | null => {
  if (v === null) return null;
  const n = Number.parseInt(v, 10);
  return Number.isFinite(n) && n > 0 ? Math.min(n, 4096) : null;
};

/** The connected elements: those sharing a `link` value are a group (REQ-LINK-010). */
const linked = new Set<GlyphPlot>();

/** The `<glyph-plot>` element. */
export class GlyphPlot extends Base {
  static get observedAttributes(): string[] {
    return [...OBSERVED];
  }

  /** The chart type of a tag `define` made, whatever its `type` attribute says (REQ-EL-030); null for `<glyph-plot>`. */
  static readonly fixedType: string | null = null;

  #chart: Chart | null = null;
  #made = false;
  #mount: Mount | null = null;
  #parts: A11yParts | null = null;
  #drawing: HTMLCanvasElement | HTMLPreElement | null = null;
  #gen = 0;
  #started = false;
  /** The drawn element has replaced the server-rendered still. */
  #shown = false;
  /** The current chart has drawn its first frame. */
  #framedOnce = false;
  #laid = "";
  #still: Node[] = [];
  #ready = deferred();
  #data: DataInput | null = null;
  #prepared: Prepared | null = null;
  #appends: AppendInput[] = [];
  #options: Record<string, unknown> = {};
  #view: View = {};
  #theme: Theme | null = null;
  #io: IntersectionObserver | null = null;
  #ro: ResizeObserver | null = null;
  #stopTheme: (() => void) | null = null;
  #stopDpr: (() => void) | null = null;
  #unsubscribe: (() => void) | null = null;
  #box: { w: number; h: number } | null = null;
  #cw = 0;
  #fmt = new Formats("en-US");
  #a11y = throttle(() => this.#writeA11y(), REWRITE_MS);
  #warned = new Set<string>();
  #paper = false;
  /** The shape the chart before a type change took its data as, when that data is carried (REQ-EL-050). */
  #carriedShape: string | null = null;
  /** The zone `timeZone` names, resolved (REQ-TZ-050), and the table last given the chart (REQ-TZ-020). */
  #zoneName = "UTC";
  #zone: ZoneTable | null = null;
  /** The time extent of the data last committed, by its prepared object. */
  #extent: { of: unknown; span: [number, number] | null } | null = null;
  /** The debug overlay, in a debug build only. */
  #overlay: Overlay | null = null;

  // ------------------------------------------------------------ properties (REQ-EL-060)

  /** The chart type: the `type` attribute, or the type a tag `define` made is fixed to (REQ-EL-030). */
  get type(): string {
    return (this.constructor as typeof GlyphPlot).fixedType ?? this.getAttribute("type") ?? "";
  }
  set type(v: string) {
    this.setAttribute("type", v);
  }

  /** Get: the descriptor and lengths, no values (REQ-EL-070). Set: replaces all data; the attribute is not written. */
  get data(): DataObject | null {
    if (this.#chart) return this.#chart.data;
    try {
      const p = this.#prepared ?? (this.#data !== null ? prepare(this.#data) : null);
      return p ? describe(p) : null;
    } catch {
      return null;
    }
  }
  set data(v: DataInput | null) {
    this.#setData(v);
  }

  /** The options given, as an object. Setting them keeps the chart, its data and its view (REQ-EL-040). */
  get options(): Record<string, unknown> {
    return structuredClone(this.#options);
  }
  set options(v: Record<string, unknown> | string | null) {
    try {
      const o = v === null ? {} : typeof v === "string" ? (JSON.parse(v || "{}") as Record<string, unknown>) : v;
      if (o === null || typeof o !== "object" || Array.isArray(o)) throw new Error("options are not a JSON object");
      this.#options = o;
      if (this.#chart) this.#chart.options = o;
    } catch (e) {
      console.error(`glyphplot: ${messageOf(e)}`);
    }
  }

  /** Get: the whole view. Set: a whole or partial view, merged; null resets (REQ-VIEW-030); fires no event (REQ-VIEW-040). */
  get view(): View {
    if (this.#chart) return this.#chart.view;
    return structuredClone(this.#view);
  }
  set view(v: View | string | null) {
    try {
      const patch = v === null ? null : typeof v === "string" ? (JSON.parse(v) as View) : v;
      this.#takeView(patch);
      const domain = (patch?.["x"] as { domain?: unknown } | null | undefined)?.domain;
      if (patch === null) this.#share({ x: { domain: null } });
      else if (domain !== undefined) this.#share({ x: { domain } });
    } catch (e) {
      console.error(`glyphplot: view: ${messageOf(e)}`);
    }
  }

  /** A theme object that wins over the custom properties, or the theme attribute's mode. */
  get theme(): Theme | ThemeMode {
    return this.#theme ? structuredClone(this.#theme) : this.#mode();
  }
  set theme(v: Theme | ThemeMode | null) {
    if (typeof v === "string") {
      this.#theme = null;
      this.setAttribute("theme", v);
      return;
    }
    this.#theme = v ? { ...v } : null;
    this.#resolveTheme();
  }

  /** The JS Chart once made, else null. */
  get chart(): Chart | null {
    return this.#made ? this.#chart : null;
  }

  /** Resolves after the chart's first frame (REQ-EL-120). */
  get ready(): Promise<GlyphPlot> {
    return this.#ready.promise;
  }

  get cols(): number {
    return this.#mount?.cols ?? 0;
  }

  get rows(): number {
    return this.#mount?.rows ?? 0;
  }

  /** Play time in seconds. */
  get currentTime(): number {
    return this.#mount?.t ?? 0;
  }

  /**
   * Appends rows to the data (a row, rows, or typed columns by name); queued
   * before load. Given DOM nodes or strings, it is the DOM's own `append`.
   */
  override append(...args: (Node | string | AppendInput)[]): void {
    if (args.every((a) => typeof a === "string" || (typeof Node !== "undefined" && a instanceof Node))) {
      super.append(...(args as (Node | string)[]));
      return;
    }
    const rows = (args.length === 1 ? args[0] : args) as AppendInput;
    if (this.#chart) this.#chart.append(rows);
    else this.#appends.push(rows);
  }

  /** Empties the data, keeping its columns. */
  clear(): void {
    this.#setData(emptyLike(this.data));
  }

  /** Returns the view to its fitted default. */
  resetView(): void {
    this.view = null;
  }

  /** The frame as text. */
  text(): string {
    return this.#made && this.#chart ? this.#chart.text() : "";
  }

  // ------------------------------------------------------------ lifecycle

  connectedCallback(): void {
    injectStyle(this.ownerDocument, this.localName);
    linked.add(this);
    for (const p of ["type", "data", "options", "view", "theme"] as const) {
      if (Object.hasOwn(this, p)) {
        const v = (this as unknown as Record<string, unknown>)[p];
        delete (this as unknown as Record<string, unknown>)[p];
        (this as unknown as Record<string, unknown>)[p] = v;
      }
    }
    if (!this.hasAttribute("role")) this.setAttribute("role", "figure");
    this.#label();
    this.#autoHeight();
    if (this.getAttribute("loading") === "eager" || typeof IntersectionObserver !== "function") {
      this.#start();
      return;
    }
    this.#io = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) {
          this.#io?.disconnect();
          this.#io = null;
          this.#start();
        }
      },
      { rootMargin: "100% 0px" },
    );
    this.#io.observe(this);
  }

  disconnectedCallback(): void {
    linked.delete(this);
    this.#io?.disconnect();
    this.#io = null;
    this.#teardown(true);
  }

  attributeChangedCallback(name: string, old: string | null, value: string | null): void {
    if (old === value) return;
    switch (name) {
      case "type":
        if ((this.constructor as typeof GlyphPlot).fixedType !== null) break; // an alias's type is fixed (REQ-EL-030)
        if (this.#started) {
          this.#teardown(false);
          this.#start();
        }
        break;
      case "data":
        if (value !== null) {
          try {
            this.#setData(JSON.parse(value) as DataInput);
          } catch (e) {
            console.error(`glyphplot: the data attribute is not JSON: ${messageOf(e)}`);
          }
        }
        break;
      case "options":
        this.options = value;
        break;
      case "view":
        if (value !== null) this.view = value;
        break;
      case "theme":
        this.#resolveTheme();
        break;
      case "locale":
        this.#setLocale();
        break;
      case "mono":
        if (this.#made && this.#chart) {
          this.#chart.mono = value !== null;
          this.#remount();
        }
        break;
      case "motion":
        this.#mount?.setMotion(value !== null);
        break;
      case "label":
        this.#label();
        break;
      case "cols":
      case "rows":
      case "fit":
        this.#autoHeight();
        this.#relayout();
        break;
      case "timezone":
        this.#setZone();
        break;
      case "loading":
        if (value === "eager" && !this.#started && this.isConnected) {
          this.#io?.disconnect();
          this.#io = null;
          this.#start();
        }
        break;
      case "time":
        this.#mount?.setTime(value === null || value === "" || !Number.isFinite(Number(value)) ? null : Math.max(0, Number(value)));
        break;
    }
  }

  // ------------------------------------------------------------ data, options, view

  #setData(v: DataInput | null): void {
    this.#data = v;
    this.#prepared = null;
    this.#carriedShape = null;
    this.#appends = [];
    if (v === null) return;
    if (this.#chart) {
      try {
        this.#chart.data = v;
      } catch (e) {
        console.error(`glyphplot: data: ${messageOf(e)}`);
      }
    }
  }

  /** Sets a view patch (null resets) on this element alone, firing no event (REQ-VIEW-040). */
  #takeView(patch: View | null): void {
    if (patch === null) this.#view = {};
    else this.#view = { ...this.#view, ...patch };
    if (this.#chart) this.#chart.view = patch;
  }

  // ------------------------------------------------------------ link (REQ-LINK-010)

  /** The other connected elements with this one's `link` value. */
  #peers(): GlyphPlot[] {
    const group = this.getAttribute("link");
    if (!group) return [];
    return [...linked].filter((p) => p !== this && p.getAttribute("link") === group);
  }

  /**
   * Gives every peer `patch` (an x domain or a cursor, in data units). A view
   * set fires no event (REQ-VIEW-040), so a peer's change never comes back:
   * there is no loop.
   */
  #share(patch: View): void {
    for (const p of this.#peers()) {
      try {
        p.#takeView(patch);
      } catch (e) {
        console.error(`glyphplot: link: ${messageOf(e)}`);
      }
    }
  }

  /** On its first frame, a chart joining a group takes a peer's x domain. */
  #joinLink(): void {
    for (const p of this.#peers()) {
      const c = p.#chart;
      if (!c || !p.#made || c.dead) continue;
      const domain = (c.view["x"] as { domain?: unknown } | undefined)?.domain;
      if (Array.isArray(domain)) {
        this.#takeView({ x: { domain } });
        return;
      }
    }
  }

  /** Shares what a reader's event changed with the link group: the x domain of a view change, the x of a cursor. */
  #shareEvent(type: string, detail: unknown): void {
    if (!this.getAttribute("link")) return;
    if (type === "plot-view") {
      const view = (detail as { view?: View } | null)?.view;
      const domain = (view?.["x"] as { domain?: unknown } | undefined)?.domain;
      if (domain !== undefined) this.#share({ x: { domain } });
    } else if (type === "plot-hover" || type === "plot-cursor") {
      const x = (detail as { x?: unknown } | null)?.x;
      this.#share({ cursor: typeof x === "number" && Number.isFinite(x) ? { x, y: null, series: null, index: null, locked: false } : null });
    }
  }

  // ------------------------------------------------------------ time zone (REQ-TZ-020, REQ-TZ-050)

  /** The zone the `timeZone` attribute names, the page's when it has none; UTC, with a warning, for a name Intl does not know. */
  #resolveZone(): string {
    const attr = this.getAttribute("timezone");
    const z = resolveZone(attr);
    if (z !== null) return z;
    const key = `zone:${attr ?? ""}`;
    if (!this.#warned.has(key)) {
      this.#warned.add(key);
      console.warn(`glyphplot: timeZone "${attr ?? ""}" is not UTC, a fixed offset or an IANA name: UTC is used`);
    }
    return "UTC";
  }

  #setZone(): void {
    const z = this.#resolveZone();
    if (z === this.#zoneName && this.#zone !== null) return;
    this.#zoneName = z;
    this.#zone = null;
    this.#updateZone(true);
  }

  /** The data's time extent in epoch ms: its first table's first timestamp column, null when it has none. */
  #timeExtent(c: Chart): [number, number] | null {
    const p = c.prepared();
    if (!p) return null;
    if (this.#extent?.of === p) return this.#extent.span;
    let span: [number, number] | null = null;
    // The first table's columns are the first numbered (ColumnKind).
    const columns = Object.values(p.descriptor.tables)[0]?.columns ?? [];
    const i = columns.findIndex((col) => col.type === "timestamp");
    const col = columns[i];
    const src = p.sources[i];
    if (col && src && src.kind === 0) {
      const v = new Float64Array(src.length);
      src.write(v);
      const per = { s: 1e-3, ms: 1, us: 1e3, ns: 1e6 }[col.time_unit ?? "ms"];
      const origin = Number(col.origin ?? 0);
      let lo = Infinity;
      let hi = -Infinity;
      for (const x of v) {
        if (x < lo) lo = x;
        if (x > hi) hi = x;
      }
      if (lo <= hi) span = [(lo + origin) / per, (hi + origin) / per];
    }
    this.#extent = { of: p, span };
    return span;
  }

  /**
   * Gives the chart the zone table of `#zoneName` covering the data's time
   * extent and the view (REQ-TZ-020), when `force` or when the view has moved
   * past the span the last table covered.
   */
  #updateZone(force: boolean): void {
    const c = this.#chart;
    if (!c || !c.loaded || c.dead) return;
    try {
      const extent = this.#timeExtent(c);
      const domain = (c.view["x"] as { domain?: unknown } | undefined)?.domain;
      let lo = extent?.[0] ?? Infinity;
      let hi = extent?.[1] ?? -Infinity;
      if (Array.isArray(domain) && extent) {
        lo = Math.min(lo, Number(domain[0]));
        hi = Math.max(hi, Number(domain[1]));
      }
      if (!(lo <= hi)) lo = hi = 0; // no time: the zone is still passed explicitly (REQ-TZ-050)
      if (!force && covers(this.#zone, lo, hi)) return;
      const [from, to] = coveringFor(lo, hi);
      const table = zoneTable(this.#zoneName, from, to);
      this.#zone = table;
      c.zone = table;
    } catch (e) {
      console.error(`glyphplot: time zone: ${messageOf(e)}`);
    }
  }

  #mode(): ThemeMode {
    const m = this.getAttribute("theme");
    return m === "dark" || m === "light" || m === "paper" ? m : "auto";
  }

  #label(): void {
    const label = this.getAttribute("label");
    if (label) this.setAttribute("aria-label", label);
    else if (!this.#shown) this.setAttribute("aria-label", this.type || "chart");
  }

  #setLocale(): void {
    const tag = localeOf(this);
    this.#fmt = new Formats(tag);
    if (this.#chart) {
      try {
        this.#chart.locale = localePack(tag);
      } catch (e) {
        console.error(`glyphplot: locale: ${messageOf(e)}`);
      }
      this.#a11y.run();
    }
  }

  // ------------------------------------------------------------ loading (REQ-LOAD-020, REQ-EL-120, REQ-EL-140)

  #start(): void {
    if (!this.isConnected) return;
    this.#started = true;
    const gen = ++this.#gen;
    this.#framedOnce = false;
    if (this.#ready.settled) this.#ready = deferred();
    const type = this.type;
    const tag = localeOf(this);
    this.#fmt = new Formats(tag);
    let c: Chart;
    try {
      c = chart(type, { options: this.#options, locale: localePack(tag), mono: this.hasAttribute("mono") });
    } catch (e) {
      this.#fail(type, messageOf(e));
      return;
    }
    // Data carried from the chart before (a type change, a reconnect) stays in
    // #prepared until the new chart's meta is known: it is given only when the
    // meta takes its shape (REQ-EL-050); data set meanwhile replaces it.
    const carried = this.#prepared !== null;
    const appends = this.#appends;
    this.#appends = [];
    if (!carried) {
      if (this.#data !== null) c.data = this.#data;
      for (const a of appends) c.append(a);
      if (Object.keys(this.#view).length > 0) c.view = this.#view;
    }
    this.#data = null;
    this.#zoneName = this.#resolveZone();
    this.#zone = null;
    this.#chart = c;
    c.ready.then(
      () => {
        if (gen !== this.#gen) {
          c.free(); // superseded: free what it made and draw nothing (REQ-EL-140)
          return;
        }
        const p = this.#prepared;
        this.#prepared = null;
        if (p) this.#carry(c, p, appends);
        this.#updateZone(true);
        this.#made = true;
        this.#build(c);
      },
      (e: unknown) => {
        if (gen !== this.#gen) return;
        this.#fail(type, messageOf(e));
      },
    );
  }

  /**
   * Gives a new chart the data (and the view) of the chart before it when its
   * meta takes the shape that chart took the data as; otherwise warns once,
   * naming the shape, and leaves it empty (REQ-EL-050).
   */
  #carry(c: Chart, p: Prepared, appends: AppendInput[]): void {
    const shape = this.#carriedShape ?? shapeOf(p.descriptor, c.meta.shapes, encodeOf(this.#options));
    this.#carriedShape = null;
    if (shape !== null && !c.meta.shapes.includes(shape)) {
      const key = `shape:${c.type}:${shape}`;
      if (!this.#warned.has(key)) {
        this.#warned.add(key);
        console.warn(`glyphplot: a ${c.type} chart takes ${c.meta.shapes.join(" or ")} data, not ${shape}: it is drawn empty`);
      }
      this.#view = {};
      return;
    }
    try {
      c.setPrepared(p);
      for (const a of appends) c.append(a);
      if (Object.keys(this.#view).length > 0) c.view = this.#view;
    } catch (e) {
      console.error(`glyphplot: data: ${messageOf(e)}`);
    }
  }

  #fail(type: string, message: string): void {
    this.#removeParts();
    this.dispatchEvent(new CustomEvent("plot-error", { detail: { type, message }, bubbles: true, composed: true }));
    this.#ready.reject(new Error(message));
    console.error(`glyphplot: ${message}`);
  }

  /** Builds the drawing and its accessible parts, hidden until the first frame, and mounts the chart. */
  #build(c: Chart): void {
    this.#still = this.#shown ? [] : [...this.childNodes];
    const parts = makeParts(this.ownerDocument, (key) => {
      const code = keyCode(key);
      if (code >= 0) this.#mount?.send({ kind: InputKind.key, x: 0, y: 0, a: code, b: 0, buttons: 0, mods: 0 }, "key");
    });
    this.#parts = parts;
    for (const el of [parts.frame, parts.controls]) el.classList.add("gp-loading");
    super.append(parts.frame, parts.controls, parts.tableBox);
    this.#unsubscribe = c.subscribe((what) => {
      if (what === "data" || what === "view") this.#updateZone(false);
      if (what === "data" || what === "options" || what === "locale" || what === "zone" || what === "view") this.#a11y.run();
    });
    this.#remount();
    this.#ro = new ResizeObserver((entries) => {
      const e = entries[entries.length - 1];
      if (!e) return;
      const dpr = globalThis.devicePixelRatio || 1;
      const dev = e.devicePixelContentBoxSize?.[0];
      const css = e.contentBoxSize?.[0];
      // Safari has no devicePixelContentBoxSize: contentBoxSize × devicePixelRatio (REQ-EL-080).
      const cw = (css?.inlineSize ?? e.contentRect.width) * dpr;
      const chh = (css?.blockSize ?? e.contentRect.height) * dpr;
      // The device box is exact when it agrees with the ratio; an emulated
      // ratio (a headless test's deviceScaleFactor) leaves it in CSS px.
      const agrees = dev !== undefined && Math.abs(dev.inlineSize - cw) <= dpr + 1 && Math.abs(dev.blockSize - chh) <= dpr + 1;
      const w = agrees ? dev.inlineSize : cw;
      const h = agrees ? dev.blockSize : chh;
      this.#box = { w, h };
      this.#relayout();
    });
    try {
      this.#ro.observe(this, { box: "device-pixel-content-box" });
    } catch {
      this.#ro.observe(this);
    }
    this.#watchDpr();
    this.#stopTheme = watchTheme(() => this.#resolveTheme());
  }

  /** (Re)makes the drawing surface for the current mono setting and mounts the chart on it. */
  #remount(): void {
    const c = this.#chart;
    const parts = this.#parts;
    if (!c || !parts) return;
    this.#mount?.stop();
    this.#drawing?.remove();
    const mono = this.hasAttribute("mono");
    const drawing = this.ownerDocument.createElement(mono ? "pre" : "canvas");
    drawing.className = "gp-drawing";
    drawing.setAttribute("role", "img");
    drawing.setAttribute("aria-label", this.getAttribute("aria-label") ?? c.type);
    parts.frame.prepend(drawing);
    this.#drawing = drawing;
    const t = this.getAttribute("time");
    this.#mount = mount(drawing, c, {
      input: true,
      renderer: config.renderer,
      target: this,
      surface: parts.frame,
      keys: parts.cursor,
      motion: this.hasAttribute("motion"),
      time: t !== null && t !== "" && Number.isFinite(Number(t)) ? Math.max(0, Number(t)) : null,
      onFrame: (_g, _s, timing) => {
        this.#framed();
        this.#overlay?.update(timing);
      },
      onEvent: (e) => this.#onEvent(e.type, e.detail),
      waitForSize: true,
      onError: (m) => {
        if (!this.#framedOnce) this.#fail(c.type, m);
        else console.error(`glyphplot: the ${c.type} chart stopped: ${m}`);
      },
    });
    this.#resolveTheme();
    this.#laid = "";
    this.#relayout();
    this.#debugOverlay(c);
  }

  /** In a debug build (the manifest has a fallback), the frame-cost overlay; a release page never loads debug.ts. */
  #debugOverlay(c: Chart): void {
    this.#overlay?.remove();
    this.#overlay = null;
    const m = this.#mount;
    void manifest()
      .then(async (mf) => {
        if (mf.fallback === null) return;
        const { debugOverlay } = await import("./debug.ts");
        if (this.#mount !== m || !m) return;
        this.#overlay?.remove();
        this.#overlay = debugOverlay(this, c.meta.budget);
      })
      .catch((e: unknown) => console.warn(`glyphplot: debug overlay: ${messageOf(e)}`));
  }

  #framed(): void {
    if (this.#framedOnce) return;
    this.#framedOnce = true;
    // The server-rendered still stays until the drawn element replaces it (REQ-EL-130).
    for (const n of this.#still) if (n.parentNode === this) (n as ChildNode).remove();
    this.#still = [];
    this.#shown = true;
    this.#parts?.frame.classList.remove("gp-loading");
    this.#parts?.controls.classList.remove("gp-loading");
    this.#joinLink();
    this.#writeA11y();
    const c = this.#chart;
    this.dispatchEvent(new CustomEvent("plot-ready", { detail: { type: c?.type ?? this.type, cols: this.cols, rows: this.rows }, bubbles: true, composed: true }));
    this.#ready.resolve(this);
  }

  #onEvent(type: string, detail: unknown): void {
    this.#shareEvent(type, detail);
    if (type === "plot-view") this.#updateZone(false);
    const cursor = this.#parts?.cursor;
    if (!cursor) return;
    if (type === "plot-hover") cursor.setAttribute("aria-valuetext", valueText(detail, this.#chart?.data ?? null, this.#fmt));
    if (type === "plot-cursor" || type === "plot-hover") {
      const x = (detail as { x?: unknown } | null)?.x;
      if (typeof x === "number" && Number.isFinite(x)) cursor.setAttribute("aria-valuenow", String(x));
    }
    if (type === "plot-view" && (detail as { final?: boolean } | null)?.final) this.#a11y.run();
  }

  /** The summary, the table and the figure's name, at most once a second (REQ-A11Y-010, REQ-A11Y-020). */
  #writeA11y(): void {
    const c = this.#chart;
    const parts = this.#parts;
    if (!c || !this.#made || !parts || c.dead) return;
    try {
      const summary = c.summary();
      this.#drawing?.setAttribute("aria-label", summary);
      const table = c.table(TABLE_ROWS);
      writeTable(parts.tableBox, table, this.#fmt);
      if (!this.getAttribute("label")) this.setAttribute("aria-label", figureName(c.type, table.columns.slice(1)));
      // The slider's range is the view's x domain in data units: the view's own when set, else the fitted one the table reports.
      const set = (c.view["x"] as { domain?: unknown } | null | undefined)?.domain;
      const domain = Array.isArray(set) && set.length === 2 ? set : table.domain;
      const lo = Number(domain?.[0]);
      const hi = Number(domain?.[1]);
      if (Number.isFinite(lo) && Number.isFinite(hi)) {
        parts.cursor.setAttribute("aria-valuemin", String(lo));
        parts.cursor.setAttribute("aria-valuemax", String(hi));
        const now = Number(parts.cursor.getAttribute("aria-valuenow"));
        if (!(now >= lo && now <= hi)) parts.cursor.setAttribute("aria-valuenow", String(lo));
      }
    } catch (e) {
      console.error(`glyphplot: accessible text: ${messageOf(e)}`);
    }
  }

  // ------------------------------------------------------------ size (REQ-EL-080 to 100)

  #fontPx(): number {
    const fs = Number.parseFloat(getComputedStyle(this).fontSize);
    return Number.isFinite(fs) && fs > 0 ? fs : 13;
  }

  /** The height the element takes when it has none of its own: `rows` rows, default 12 (REQ-EL-100). */
  #autoHeight(): void {
    const dpr = globalThis.devicePixelRatio || 1;
    const rows = intAttr(this.getAttribute("rows")) ?? DEFAULT_ROWS;
    // With a fit, the cell is the content width's share once it is known (REQ-EL-110).
    const fit = this.#fit();
    const cols = intAttr(this.getAttribute("cols"));
    const cw = fit !== null && cols !== null && this.#box && this.#box.w > 0 ? this.#box.w / cols : cellWidth(this.#fontPx(), dpr);
    this.style.setProperty("--gp-auto-height", `${(rows * cw * 2) / dpr}px`);
  }

  /** The `fit` in force: only with both `cols` and `rows` (REQ-EL-110). */
  #fit(): Fit | null {
    return intAttr(this.getAttribute("cols")) !== null && intAttr(this.getAttribute("rows")) !== null ? fitOf(this.getAttribute("fit")) : null;
  }

  /** Lays the chart out for the measured box within this frame, keeping its view (REQ-EL-090); width 0 is ignored. */
  #relayout(): void {
    const box = this.#box;
    const m = this.#mount;
    const frame = this.#parts?.frame;
    const drawing = this.#drawing;
    if (!box || !m || !frame || !drawing || box.w <= 0) return;
    const dpr = globalThis.devicePixelRatio || 1;
    const cw = cellWidth(this.#fontPx(), dpr);
    if (cw !== this.#cw) {
      this.#cw = cw;
      this.#autoHeight();
    }
    const fixedCols = intAttr(this.getAttribute("cols"));
    const fixedRows = intAttr(this.getAttribute("rows"));
    const whole = gridFor(box.w, box.h, cw);
    const cols = fixedCols !== null && fixedRows !== null ? fixedCols : whole.cols;
    const rows = fixedCols !== null && fixedRows !== null ? fixedRows : fixedRows !== null && box.h <= 0 ? fixedRows : whole.rows;
    const fit = this.#fit();
    // Without a fit, whole device pixels: the cell from the font, the remainder
    // split as padding. With one, the drawing is scaled to the fitted box, its
    // backing store in whole cells of the nearest whole pixel (REQ-EL-110).
    const at = fitBox(fit, box.w, box.h, cols, rows, cw);
    const cell = fit === null ? cw : at.cell;
    const backing = fit === null ? cw : Math.max(1, Math.round(at.cell));
    const padX = fit === null ? Math.max(0, Math.floor((box.w - cols * cw) / 2)) : at.x;
    const padY = fit === null ? Math.max(0, Math.floor((box.h - rows * cw * 2) / 2)) : at.y;
    const px = (v: number): string => `${v / dpr}px`;
    Object.assign(frame.style, { left: px(padX), top: px(padY), width: px(cols * cell), height: px(rows * cell * 2) });
    Object.assign(drawing.style, { width: px(cols * cell), height: px(rows * cell * 2) });
    if (drawing instanceof HTMLPreElement) Object.assign(drawing.style, { fontSize: px(cell / 0.6), lineHeight: px(cell * 2) });
    if (fit !== null) this.#autoHeight();
    const laid = `${cols}x${rows}@${backing}`;
    if (laid !== this.#laid) {
      this.#laid = laid;
      m.resize(cols, rows, backing);
    }
  }

  /** Lays out again when the device pixel ratio changes (zoom, another screen). */
  #watchDpr(): void {
    this.#stopDpr?.();
    if (typeof matchMedia !== "function") return;
    const mq = matchMedia(`(resolution: ${globalThis.devicePixelRatio || 1}dppx)`);
    const f = (): void => {
      this.#cw = 0;
      this.#autoHeight();
      this.#relayout();
      this.#watchDpr();
    };
    mq.addEventListener("change", f, { once: true });
    this.#stopDpr = () => mq.removeEventListener("change", f);
  }

  // ------------------------------------------------------------ theme (REQ-THEME-010 to 030)

  #resolveTheme(): void {
    const c = this.#chart;
    const m = this.#mount;
    if (!c || !this.#made || !m || c.dead) return;
    try {
      const style = getComputedStyle(this);
      const fromStyle = themeFromStyle(style);
      const text = browserColor(style.color);
      const mode = this.#mode();
      const { series: styleSeries, ...base } = fromStyle;
      let tokens: Theme = base;
      if (styleSeries) {
        const gaps = styleSeries.some((s) => s === undefined);
        const defaults = gaps ? seriesOf(c.palette(chooseTheme(base, this.#theme, mode, text).theme)) : [];
        tokens = { ...base, series: fillSeries(styleSeries, defaults) };
      }
      const choice = chooseTheme(tokens, this.#theme, mode, text);
      const bytes = c.palette(choice.theme);
      m.setPalette(bytes, choice.paintGround);
      if (choice.paper !== this.#paper) {
        this.#paper = choice.paper;
        m.setPaper(choice.paper);
      }
      this.style.setProperty("--gp-accent", toHex(entry(bytes, 3)));
      this.style.setProperty("--gp-ink", toHex(entry(bytes, 0)));
      this.style.setProperty("--gp-muted", toHex(entry(bytes, 1)));
      this.style.setProperty("--gp-ground", toHex(entry(bytes, 4)));
      void manifest().then((mf) => {
        if (mf.fallback === null) return; // debug builds only (REQ-THEME-070)
        for (const w of lowContrastSeries(choice.theme, bytes)) {
          if (this.#warned.has(w)) continue;
          this.#warned.add(w);
          console.warn(w);
        }
      });
    } catch (e) {
      console.error(`glyphplot: theme: ${messageOf(e)}`);
    }
  }

  // ------------------------------------------------------------ teardown

  #removeParts(): void {
    this.#parts?.frame.remove();
    this.#parts?.controls.remove();
    this.#parts?.tableBox.remove();
    this.#parts = null;
    this.#drawing = null;
  }

  /** Stops drawing and frees the chart (REQ-EL-140); with `all`, forgets the size watchers too. Data and view are carried to the next chart. */
  #teardown(all: boolean): void {
    this.#gen++;
    this.#a11y.cancel();
    this.#mount?.stop();
    this.#mount = null;
    this.#overlay?.remove();
    this.#overlay = null;
    this.#unsubscribe?.();
    this.#unsubscribe = null;
    this.#ro?.disconnect();
    this.#ro = null;
    this.#stopTheme?.();
    this.#stopTheme = null;
    this.#stopDpr?.();
    this.#stopDpr = null;
    const c = this.#chart;
    if (c) {
      try {
        if (c.loaded && !c.dead) {
          this.#view = c.view;
          delete this.#view["motion"];
        }
        const p = c.prepared();
        if (p) {
          this.#prepared = p;
          this.#carriedShape = c.loaded && !c.dead ? shapeOf(p.descriptor, c.meta.shapes, encodeOf(this.#options)) : null;
        }
      } catch {
        // a dead chart keeps nothing
      }
      c.free();
    }
    this.#chart = null;
    this.#made = false;
    this.#removeParts();
    this.#cw = 0;
    this.#laid = "";
    if (all) {
      this.#started = false;
      this.#box = null;
    }
  }
}

const hasElements = (): boolean => typeof document !== "undefined" && typeof customElements !== "undefined" && typeof HTMLElement !== "undefined";

/** Defines `<glyph-plot>` once; does nothing without a DOM or customElements (REQ-EL-010). */
export function defineElement(): void {
  if (!hasElements()) return;
  injectStyle(document);
  if (!customElements.get(TAG)) customElements.define(TAG, GlyphPlot);
}

/**
 * Defines `tag` as a `<glyph-plot>` whose chart type is `type` whatever its
 * `type` attribute says, with the element's rules; does nothing for a tag
 * already defined, or without a DOM (REQ-EL-030, ADR-118):
 * `define("candle-chart", { type: "candles" })`.
 */
export function define(tag: string, { type }: { type: string }): void {
  if (!hasElements() || customElements.get(tag)) return;
  injectStyle(document, tag);
  customElements.define(
    tag,
    class extends GlyphPlot {
      static override readonly fixedType: string | null = type;
    },
  );
}

defineElement();
