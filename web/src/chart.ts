// `chart(type, init)`: the JS Chart over one wasm chart (REQ-JS-010). It
// works with no DOM. Calls made before the module has loaded are queued and
// applied in order once the chart exists; a data set discards the appends
// queued before it (REQ-LOAD-030). Data crosses as typed columns through
// reserve_column and commit, a typed array copied once (REQ-MEM-020,
// REQ-DATA-050); label text is NFC-normalised first (REQ-I18N-050).

import type { InputKindName, Meta, TableJson, WasmEvent } from "./api.ts";
import { appendTo, describe, descriptorJson, nfcDeep, prepare, type AppendInput, type DataInput, type DataObject, type Prepared } from "./data.ts";
import { encode, type Encoded, type InputEvent } from "./input.ts";
import { localePack, type LocalePack } from "./locale.ts";
import { Guarded, loadFor, messageOf, type GridViews } from "./wasm.ts";

/** A view state or a partial one (spec/schemas/view.schema.json). */
export type View = Record<string, unknown>;

/** What a chart starts with. */
export interface ChartInit {
  data?: DataInput;
  options?: Record<string, unknown> | string;
  view?: View | string | null;
  /** A BCP 47 tag or a locale pack. */
  locale?: string | LocalePack;
  mono?: boolean;
  cols?: number;
  rows?: number;
}

/** What changed, for whoever draws the chart. */
export type Change = "data" | "options" | "view" | "locale" | "mono" | "size";

/** The patch that returns every part of the view to its default. */
export const VIEW_RESET: View = { x: null, y: null, axes: null, cursor: null, selection: null, hidden: null, time: null, motion: null };

type Op =
  | { k: "data"; v: DataInput }
  | { k: "prepared"; v: Prepared }
  | { k: "append"; v: AppendInput }
  | { k: "options"; v: Record<string, unknown> }
  | { k: "view"; v: View }
  | { k: "locale"; v: LocalePack }
  | { k: "mono"; v: boolean }
  | { k: "resize"; cols: number; rows: number };

function parseObject(v: unknown, what: string): Record<string, unknown> {
  const o = typeof v === "string" ? (v.trim() === "" ? {} : (JSON.parse(v) as unknown)) : v;
  if (o === null || typeof o !== "object" || Array.isArray(o)) throw new Error(`${what} are not a JSON object`);
  return o as Record<string, unknown>;
}

/** Merges a view patch into a pending one key by key, as the module merges (for reads before load). */
function mergeView(into: View, patch: View): View {
  const out: View = { ...into };
  for (const [k, v] of Object.entries(patch)) {
    const prev = out[k];
    out[k] = v !== null && typeof v === "object" && !Array.isArray(v) && prev !== null && typeof prev === "object" && !Array.isArray(prev) ? { ...(prev as View), ...(v as View) } : v;
  }
  return out;
}

/** The JS Chart. Create it with {@link chart}. */
export class Chart {
  /** The chart type asked for. */
  readonly type: string;
  /** Resolves once the module has loaded and the queued calls are applied; rejects with the load's error (REQ-LOAD-010). */
  readonly ready: Promise<Chart>;
  #g: Guarded | null = null;
  #error: Error | null = null;
  #queue: Op[] = [];
  #options: Record<string, unknown>;
  #pendingView: View = {};
  #prepared: Prepared | null = null;
  #appends: AppendInput[] = [];
  #flushQueued = false;
  #version = 0;
  #meta: Meta | null = null;
  #listeners = new Set<(c: Change) => void>();
  #freed = false;

  constructor(type: string, init: ChartInit = {}) {
    this.type = type;
    this.#options = nfcDeep(parseObject(init.options ?? {}, "options"));
    if (init.data !== undefined) this.#queue.push({ k: "data", v: init.data });
    if (init.view !== undefined) this.view = init.view;
    if (init.locale !== undefined) this.locale = init.locale;
    if (init.mono !== undefined) this.mono = init.mono;
    if (init.cols !== undefined && init.rows !== undefined) this.resize(init.cols, init.rows);
    this.ready = this.#load();
    this.ready.catch(() => {}); // observed through `ready` or `error`
  }

  async #load(): Promise<Chart> {
    try {
      const loaded = await loadFor(this.type);
      if (this.#freed) throw new Error("the chart was freed before it loaded");
      let raw;
      try {
        raw = new loaded.module.Chart(this.type, JSON.stringify(this.#options));
      } catch (e) {
        throw new Error(messageOf(e));
      }
      this.#g = new Guarded(raw, loaded);
      const queue = this.#queue;
      this.#queue = [];
      for (const op of queue) {
        try {
          this.#apply(op);
        } catch (e) {
          console.error(`glyphplot: ${op.k} for a ${this.type} chart failed: ${messageOf(e)}`);
        }
      }
      return this;
    } catch (e) {
      this.#error = e instanceof Error ? e : new Error(messageOf(e));
      throw this.#error;
    }
  }

  #apply(op: Op): void {
    const g = this.#g as Guarded;
    switch (op.k) {
      case "data":
        this.#commit(prepare(op.v));
        break;
      case "prepared":
        this.#commit(op.v);
        break;
      case "append":
        this.#appends.push(op.v);
        this.#flush();
        break;
      case "options":
        g.call((c) => c.set_options(JSON.stringify(op.v)));
        this.#drain();
        this.#notify("options");
        break;
      case "view":
        g.call((c) => c.set_view(JSON.stringify(op.v)));
        this.#drain();
        this.#notify("view");
        break;
      case "locale":
        g.call((c) => c.set_locale(JSON.stringify(op.v)));
        this.#notify("locale");
        break;
      case "mono":
        g.call((c) => c.set_mono(op.v));
        this.#notify("mono");
        break;
      case "resize":
        g.call((c) => c.resize(op.cols, op.rows));
        this.#notify("size");
        break;
    }
  }

  #run(op: Op): void {
    if (this.#g) this.#apply(op);
    else if (!this.#freed) this.#queue.push(op);
  }

  /** Discards events a script call produced: they are not the reader's (REQ-VIEW-040). */
  #drain(): void {
    try {
      this.#g?.call((c) => c.events_json());
    } catch {
      // nothing to discard
    }
  }

  #notify(c: Change): void {
    for (const f of this.#listeners) f(c);
  }

  /** Writes every reserved column, each right after its reservation, then commits (REQ-DATA-060). */
  #commit(p: Prepared): void {
    const g = this.#g as Guarded;
    p.sources.forEach((s, i) => {
      if (!s) return;
      const view = s.kind === 1 ? g.reserveU32(i, s.length) : g.reserveF64(i, s.length);
      s.write(view);
    });
    const json = descriptorJson(p);
    this.#version = g.call((c) => {
      try {
        return c.commit(json);
      } catch (e) {
        throw new Error(messageOf(e));
      }
    });
    this.#prepared = p;
    this.#drain();
    this.#notify("data");
  }

  #flush(): void {
    if (!this.#g || this.#appends.length === 0) return;
    const batches = this.#appends;
    this.#appends = [];
    let p = this.#prepared;
    for (const b of batches) p = p ? appendTo(p, b) : prepare(Array.isArray(b) ? b : [b as Record<string, never>]);
    if (p) this.#commit(p);
  }

  /** The data held, appends included, ready to commit to another chart (a type change, a reconnect). */
  prepared(): Prepared | null {
    this.#flush();
    if (this.#prepared) return this.#prepared;
    const queued = [...this.#queue].reverse().find((o) => o.k === "data" || o.k === "prepared");
    if (!queued) return null;
    return queued.k === "prepared" ? queued.v : queued.k === "data" ? prepare(queued.v) : null;
  }

  /** Replaces all data with data prepared before (see {@link Chart.prepared}). */
  setPrepared(p: Prepared): void {
    if (!this.#g) {
      this.#queue = this.#queue.filter((o) => o.k !== "append" && o.k !== "data" && o.k !== "prepared");
      this.#queue.push({ k: "prepared", v: p });
      return;
    }
    this.#appends = [];
    this.#commit(p);
  }

  /** Whether the chart exists in wasm. */
  get loaded(): boolean {
    return this.#g !== null;
  }

  /** The load's error, or null. */
  get error(): Error | null {
    return this.#error;
  }

  /** Why the chart refuses calls (a trap, or freed), or null. */
  get dead(): string | null {
    return this.#g?.dead ?? (this.#freed ? "the chart was freed" : null);
  }

  /** The data version the last commit returned. */
  get version(): number {
    return this.#version;
  }

  /** Calls `f` on every change made through this object; returns the function that stops it. */
  subscribe(f: (c: Change) => void): () => void {
    this.#listeners.add(f);
    return () => this.#listeners.delete(f);
  }

  /** The descriptor and lengths of the data held, without values (REQ-EL-070); null before any data. */
  get data(): DataObject | null {
    this.#flush();
    if (this.#prepared) return describe(this.#prepared);
    try {
      const p = this.prepared();
      return p ? describe(p) : null;
    } catch {
      return null;
    }
  }

  /** Replaces all data (REQ-DATA-130): the data object, either shorthand, the binary form, or JSON text. */
  set data(v: DataInput) {
    if (!this.#g) {
      this.#queue = this.#queue.filter((o) => o.k !== "append" && o.k !== "data" && o.k !== "prepared");
      this.#queue.push({ k: "data", v });
      return;
    }
    this.#appends = [];
    this.#commit(prepare(v));
  }

  /**
   * Appends rows (a row, rows, or typed columns by name) to the first table.
   * Appends made in one task are committed together. Until the module's
   * staging buffer (v0.6.0) they are committed as a replace.
   */
  append(rows: AppendInput): void {
    if (!this.#g) {
      this.#run({ k: "append", v: rows });
      return;
    }
    this.#appends.push(rows);
    if (!this.#flushQueued) {
      this.#flushQueued = true;
      queueMicrotask(() => {
        this.#flushQueued = false;
        try {
          this.#flush();
        } catch (e) {
          console.error(`glyphplot: append failed: ${messageOf(e)}`);
        }
      });
    }
  }

  /** The options given (not the defaults under them). */
  get options(): Record<string, unknown> {
    return structuredClone(this.#options);
  }

  /** New options, layered on the type's defaults; data and view kept (REQ-OPT-020). */
  set options(v: Record<string, unknown> | string) {
    this.#options = nfcDeep(parseObject(v, "options"));
    this.#run({ k: "options", v: this.#options });
  }

  /** The whole view state; before load, the patches set so far. */
  get view(): View {
    if (!this.#g) return structuredClone(this.#pendingView);
    this.#flush();
    return JSON.parse(this.#g.call((c) => c.view_json())) as View;
  }

  /** A whole or partial view, merged; null returns every part to its default (REQ-VIEW-030). Fires no event (REQ-VIEW-040). */
  set view(v: View | string | null) {
    const patch = v === null ? VIEW_RESET : nfcDeep(parseObject(v, "view"));
    if (!this.#g) this.#pendingView = mergeView(this.#pendingView, patch);
    this.#run({ k: "view", v: patch });
  }

  /** The locale: a tag (a pack is built from Intl) or a pack. */
  set locale(v: string | LocalePack) {
    const pack = typeof v === "string" ? localePack(v) : nfcDeep(v);
    this.#run({ k: "locale", v: pack });
  }

  /** One ink for the frames that follow. */
  set mono(on: boolean) {
    this.#run({ k: "mono", v: on });
  }

  /** A new grid size in cells; the view kept in data units. */
  resize(cols: number, rows: number): void {
    this.#run({ k: "resize", cols: Math.max(1, Math.floor(cols)), rows: Math.max(1, Math.floor(rows)) });
  }

  #need(): Guarded {
    if (!this.#g) throw new Error(this.#error ? this.#error.message : `the ${this.type} chart has not loaded yet; await chart.ready`);
    return this.#g;
  }

  /** One input event (input.schema.json, or already encoded) at play time `t`; returns the bitmask. */
  input(e: InputEvent | Encoded, t = (e as InputEvent).t ?? 0): number {
    const n = typeof (e as InputEvent).kind === "string" ? encode(e as InputEvent) : (e as Encoded);
    return this.inputRaw(n.kind, n.x, n.y, n.a, n.b, n.buttons, n.mods, t);
  }

  /** One input event in the numeric encoding: the hot path. */
  inputRaw(kind: number, x: number, y: number, a: number, b: number, buttons: number, mods: number, t: number): number {
    return this.#need().call((c) => c.input(kind, x, y, a, b, buttons, mods, t));
  }

  /** The events the last inputs produced. */
  events(): WasmEvent[] {
    const json = this.#need().call((c) => c.events_json());
    return json ? (JSON.parse(json) as WasmEvent[]) : [];
  }

  /** Draws the frame at play time `t` and returns views over the grid, valid until the next call that may allocate (REQ-WASM-030). */
  frame(t = 0, paper = false): GridViews {
    this.#flush();
    const g = this.#need();
    g.call((c) => c.frame(t, paper));
    return g.grid();
  }

  /** The last frame as text. */
  text(): string {
    this.#flush();
    return this.#need().call((c) => c.text());
  }

  /** The drawing's label. */
  summary(): string {
    return this.#need().call((c) => c.summary());
  }

  /** The data in the current view, for the hidden table (REQ-A11Y-020). */
  table(maxRows = 500): TableJson {
    this.#flush();
    return JSON.parse(this.#need().call((c) => c.table_json(maxRows))) as TableJson;
  }

  /** The type's meta. */
  get meta(): Meta {
    if (!this.#meta) this.#meta = JSON.parse(this.#need().call((c) => c.meta_json())) as Meta;
    return this.#meta;
  }

  /** The 255-entry palette (1,020 RGBA bytes) for a theme object, from the module (REQ-COLOR-030). */
  palette(theme: object): Uint8Array {
    const g = this.#need();
    try {
      return g.loaded.module.resolve_palette(JSON.stringify(theme));
    } catch (e) {
      throw new Error(messageOf(e));
    }
  }

  /** Releases the chart's memory (REQ-WASM-050); later calls throw. */
  free(): void {
    if (this.#freed) return;
    this.#freed = true;
    this.#queue = [];
    this.#appends = [];
    this.#listeners.clear();
    this.#g?.free();
  }
}

/**
 * A chart of `type`: its module is loaded at most once a page and shared
 * (REQ-LOAD-010). The returned Chart takes calls at once; they apply when it
 * has loaded. `ready` rejects with `there is no chart named "<type>"` for a
 * type no module plays.
 */
export function chart(type: string, init: ChartInit = {}): Chart {
  return new Chart(type, init);
}

/** The JSON form's kind names, for callers building events. */
export type { InputKindName };
