// `chart(type, init)`: the JS Chart over one wasm chart (REQ-JS-010). It
// works with no DOM. Calls made before the module has loaded are queued and
// applied in order once the chart exists; a data set discards the appends
// queued before it (REQ-LOAD-030). Data crosses as typed columns through
// reserve_column and commit, a typed array copied once (REQ-MEM-020,
// REQ-DATA-050); label text is NFC-normalised first (REQ-I18N-050).
//
// Appends (REQ-STREAM-010) go into the shell's tail of the first table at
// once and are written to wasm once an animation frame: through the
// module's `staging(n)` and `append(n)` when it has them, else by committing
// the tail whole (the fallback until v0.6.0's modules). Each commit is
// reported as a `DataCommit`, `plot-data`'s detail (REQ-STREAM-070).
// `paused` keeps appends in the tail without writing them (REQ-A11Y-080).

import type { InputKindName, Meta, TableJson, WasmEvent } from "./api.ts";
import { arrowRows } from "./arrow.ts";
import { describe, descriptorJson, isColumnar, nfcDeep, prepare, type AppendInput, type DataInput, type DataObject, type Prepared, type Row } from "./data.ts";
import { encode, type Encoded, type InputEvent } from "./input.ts";
import { localePack, type LocalePack } from "./locale.ts";
import { encodeOf } from "./shape.ts";
import { readAppended, retainOf, Tail, type DataCommit } from "./stream.ts";
import { Guarded, loadFor, messageOf, type GridViews } from "./wasm.ts";
import type { ZoneTable } from "./zone.ts";

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

/** What changed, for whoever draws the chart; `append` is rows waiting for the next frame's write. */
export type Change = "data" | "append" | "options" | "view" | "locale" | "zone" | "mono" | "size";

/** Arrow IPC bytes, as `data` takes them from v0.6.0 (REQ-DATA-150). */
export type ArrowBytes = ArrayBuffer | Uint8Array;

/** What `data` takes: {@link DataInput}, or Arrow IPC bytes. */
export type ChartData = DataInput | ArrowBytes;

const isArrow = (v: unknown): v is ArrowBytes => v instanceof ArrayBuffer || v instanceof Uint8Array;

/** Calls `f` on the next animation frame, or the next task where there are none (Node). */
function nextFrame(f: () => void): void {
  if (typeof requestAnimationFrame === "function") requestAnimationFrame(() => f());
  else setTimeout(f, 0);
}

/** The patch that returns every part of the view to its default. */
export const VIEW_RESET: View = { x: null, y: null, axes: null, cursor: null, selection: null, hidden: null, time: null, motion: null };

type Op =
  | { k: "data"; v: DataInput }
  | { k: "arrow"; v: Uint8Array }
  | { k: "prepared"; v: Prepared }
  | { k: "append"; v: AppendInput }
  | { k: "options"; v: Record<string, unknown> }
  | { k: "view"; v: View }
  | { k: "locale"; v: LocalePack }
  | { k: "zone"; v: ZoneTable }
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
  /** The first table as appends left it; null until the first append after a replace. */
  #tail: Tail | null = null;
  /** Rows in the tail not yet written to wasm. */
  #pending = false;
  /** Appends are kept, not written (REQ-A11Y-080). */
  #held = false;
  /** The Arrow bytes the data came as, or null. */
  #arrow: Uint8Array | null = null;
  #lastCommit: DataCommit | null = null;
  #flushQueued = false;
  #version = 0;
  #meta: Meta | null = null;
  #zone: ZoneTable | null = null;
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
      try {
        this.#flush();
      } catch (e) {
        console.error(`glyphplot: append for a ${this.type} chart failed: ${messageOf(e)}`);
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
      case "arrow":
        this.#setArrow(op.v);
        break;
      case "prepared":
        this.#commit(op.v);
        break;
      case "append":
        this.#push(op.v);
        break;
      case "options":
        g.call((c) => c.set_options(JSON.stringify(op.v)));
        this.#drain();
        if (this.#tail) {
          // New `retain` or `encode`: the tail again under them, written whole at the next frame.
          this.#tail = Tail.from(this.#tail.prepared(), retainOf(op.v), encodeOf(op.v));
          this.#tail.markWhole();
          this.#pending = true;
        }
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
      case "zone":
        g.call((c) => {
          try {
            c.set_zone(JSON.stringify(op.v));
          } catch (e) {
            throw new Error(messageOf(e));
          }
        });
        this.#zone = op.v;
        this.#notify("zone");
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

  /** Writes every reserved column, each right after its reservation, then commits (REQ-DATA-060); returns the version. */
  #write(p: Prepared): number {
    const g = this.#g as Guarded;
    p.sources.forEach((s, i) => {
      if (!s) return;
      const view = s.kind === 1 ? g.reserveU32(i, s.length) : g.reserveF64(i, s.length);
      s.write(view);
    });
    const json = descriptorJson(p);
    const version = g.call((c) => {
      try {
        return c.commit(json);
      } catch (e) {
        throw new Error(messageOf(e));
      }
    });
    this.#version = version;
    this.#drain();
    return version;
  }

  /** Replaces the data with `p` (REQ-DATA-130): appends not yet written are dropped with the data they extended. */
  #commit(p: Prepared): void {
    const version = this.#write(p);
    this.#prepared = p;
    this.#tail = null;
    this.#pending = false;
    this.#arrow = null;
    const first = Object.values(p.descriptor.tables)[0];
    this.#lastCommit = { rows: first?.length ?? 0, dropped: 0, late: 0, version };
    this.#notify("data");
  }

  /** Arrow IPC bytes as the new data, through the module's `set_arrow` (REQ-DATA-150). */
  #setArrow(bytes: Uint8Array): void {
    const g = this.#g as Guarded;
    if (typeof g.raw.set_arrow !== "function") {
      throw new Error(`Arrow data needs a module with set_arrow (v0.6.0); this build's \`${g.loaded.name}\` module has none`);
    }
    const rows = arrowRows(bytes);
    const version = g.call((c) => {
      try {
        return (c.set_arrow as (b: Uint8Array) => number)(bytes);
      } catch (e) {
        throw new Error(messageOf(e));
      }
    });
    this.#version = version;
    this.#drain();
    this.#prepared = null;
    this.#tail = null;
    this.#pending = false;
    this.#arrow = bytes;
    this.#lastCommit = { rows, dropped: 0, late: 0, version };
    this.#notify("data");
  }

  /** Takes an append into the tail (REQ-STREAM-010); the write waits for the frame. Throws on a batch the table cannot take. */
  #push(rows: AppendInput): void {
    if (!this.#tail) {
      if (this.#arrow) throw new Error("append: the data came as Arrow bytes, which the shell cannot extend; set it as columns or rows to stream");
      const base = this.#prepared;
      if (base) {
        this.#tail = Tail.from(base, retainOf(this.#options), encodeOf(this.#options));
        this.#tail.push(rows);
      } else {
        // No data yet: the first append is the data.
        const first = Array.isArray(rows) || isColumnar(rows as Row) ? rows : [rows as Row];
        this.#tail = Tail.from(prepare(first as DataInput), retainOf(this.#options), encodeOf(this.#options));
        this.#tail.markWhole();
      }
    } else this.#tail.push(rows);
    this.#pending = true;
    if (this.#held) return;
    this.#notify("append");
    if (!this.#flushQueued) {
      this.#flushQueued = true;
      nextFrame(() => {
        this.#flushQueued = false;
        try {
          this.#flush();
        } catch (e) {
          console.error(`glyphplot: append failed: ${messageOf(e)}`);
        }
      });
    }
  }

  /** Whether the module writes appends through `staging(n)` and `append(n)` (v0.6.0). */
  #stages(): boolean {
    const raw = this.#g?.raw;
    return !!raw && typeof raw.staging === "function" && typeof raw.append === "function";
  }

  /**
   * Writes the appends waiting in the tail: the rows staged since the last
   * write through `staging(n)` and `append(n)`, or the tail whole by `commit`
   * (the fallback, and whenever a batch brought a new category label).
   */
  #flush(): void {
    if (!this.#g || this.#held || !this.#pending || !this.#tail) return;
    const g = this.#g;
    const tail = this.#tail;
    this.#pending = false;
    let commit: DataCommit;
    if (this.#stages() && !tail.whole && tail.staged > 0) {
      const n = tail.staged;
      const k = tail.width;
      const ptr = g.call((c) => (c.staging as (n: number) => number)(n));
      tail.takeStaged(new Float64Array(g.loaded.memory.buffer, ptr, n * k)); // made after the call (REQ-DATA-060)
      const r = g.call((c) => {
        try {
          return (c.append as (n: number) => unknown)(n);
        } catch (e) {
          throw new Error(messageOf(e));
        }
      });
      const counts = tail.takeCounts();
      commit = readAppended(r, { rows: tail.length, ...counts, version: this.#version + 1 });
      this.#version = commit.version;
      this.#drain();
    } else {
      const counts = tail.takeCounts();
      const version = this.#write(tail.prepared());
      tail.committedWhole();
      commit = { rows: tail.length, ...counts, version };
      if (!this.#stages()) this.#follow();
    }
    this.#lastCommit = commit;
    this.#notify("data");
  }

  /**
   * The fallback's `x.follow` (REQ-VIEW-060): with follow set and a domain,
   * the domain keeps its span and ends at the newest x. A module with
   * `staging` does this itself.
   */
  #follow(): void {
    const g = this.#g;
    const newest = this.#tail?.newestX ?? NaN;
    if (!g || !Number.isFinite(newest)) return;
    const x = (JSON.parse(g.call((c) => c.view_json())) as View)["x"] as { domain?: unknown; follow?: unknown } | undefined;
    if (!x || x.follow !== true || !Array.isArray(x.domain)) return;
    const [a, b] = x.domain as [number, number];
    if (b === newest) return;
    g.call((c) => c.set_view(JSON.stringify({ x: { domain: [newest - (b - a), newest], follow: true } })));
    this.#drain();
  }

  /** Writes the appends waiting now, not at the next frame; returns the last commit (null before any). Nothing is written while paused. */
  flush(): DataCommit | null {
    this.#flush();
    return this.lastCommit;
  }

  /** What the last commit of data did: `plot-data`'s detail (REQ-STREAM-070); null before any. */
  get lastCommit(): DataCommit | null {
    return this.#lastCommit ? { ...this.#lastCommit } : null;
  }

  /** Whether the module reads Arrow IPC bytes (`set_arrow`, v0.6.0); null before load. */
  get readsArrow(): boolean | null {
    return this.#g ? typeof this.#g.raw.set_arrow === "function" : null;
  }

  /** How appends reach wasm: `staging` (v0.6.0's modules), `commit` (the fallback), or null before load. */
  get streaming(): "staging" | "commit" | null {
    return this.#g ? (this.#stages() ? "staging" : "commit") : null;
  }

  /**
   * Whether appends are kept without being written or drawn (REQ-A11Y-080).
   * On resume they are written and, when the view has a domain of its own,
   * follow is set so the view shows the live end.
   */
  get paused(): boolean {
    return this.#held;
  }
  set paused(on: boolean) {
    if (on === this.#held) return;
    this.#held = on;
    if (on || !this.#g || this.#freed) return;
    try {
      if (this.#tail) {
        const g = this.#g;
        const x = (JSON.parse(g.call((c) => c.view_json())) as View)["x"] as { domain?: unknown; follow?: unknown } | undefined;
        if (x && Array.isArray(x.domain) && x.follow !== true) {
          g.call((c) => c.set_view(JSON.stringify({ x: { follow: true } })));
          this.#drain();
          this.#notify("view");
        }
      }
      if (this.#pending) this.#flush();
      else if (!this.#stages()) this.#follow();
    } catch (e) {
      console.error(`glyphplot: resume failed: ${messageOf(e)}`);
    }
  }

  /** The newest row, by column name (timestamps in epoch ms, labels as text), appends held while paused included; null without data. */
  latest(): Record<string, number | string | null> | null {
    if (this.#tail) return this.#tail.latest();
    const p = this.#prepared;
    if (!p) return null;
    try {
      return Tail.from(p).latest();
    } catch {
      return null;
    }
  }

  /** The data's x span when streaming over a timestamp x, from the tail without a scan; undefined when the caller must read the data. */
  streamSpan(): [number, number] | null | undefined {
    return this.#tail ? this.#tail.timeSpan() : undefined;
  }

  /** The Arrow bytes the data came as, or null. */
  get arrow(): Uint8Array | null {
    return this.#arrow;
  }

  /** The data held, appends included, ready to commit to another chart (a type change, a reconnect). */
  prepared(): Prepared | null {
    if (this.#tail) return this.#tail.prepared();
    if (this.#prepared) return this.#prepared;
    const queued = [...this.#queue].reverse().find((o) => o.k === "data" || o.k === "prepared");
    if (!queued) return null;
    return queued.k === "prepared" ? queued.v : queued.k === "data" ? prepare(queued.v) : null;
  }

  /** Replaces all data with data prepared before (see {@link Chart.prepared}). */
  setPrepared(p: Prepared): void {
    if (!this.#g) {
      this.#queue = this.#queue.filter((o) => o.k !== "append" && o.k !== "data" && o.k !== "prepared" && o.k !== "arrow");
      this.#queue.push({ k: "prepared", v: p });
      return;
    }
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

  /** The descriptor and lengths of the data held, appends included, without values (REQ-EL-070); null before any data or for Arrow data. */
  get data(): DataObject | null {
    this.#flush();
    try {
      const p = this.prepared();
      return p ? describe(p) : null;
    } catch {
      return null;
    }
  }

  /** Replaces all data (REQ-DATA-130): the data object, either shorthand, the binary form, JSON text, or Arrow IPC bytes (REQ-DATA-150). */
  set data(v: ChartData) {
    const op: Op = isArrow(v) ? { k: "arrow", v: v instanceof Uint8Array ? v : new Uint8Array(v) } : { k: "data", v };
    if (!this.#g) {
      this.#queue = this.#queue.filter((o) => o.k !== "append" && o.k !== "data" && o.k !== "prepared" && o.k !== "arrow");
      this.#queue.push(op);
      return;
    }
    if (op.k === "arrow") this.#setArrow(op.v);
    else this.#commit(prepare(op.v));
  }

  /**
   * Appends rows (a row, rows, or typed columns by name) to the first table
   * (REQ-STREAM-010): taken at once, typed as its columns (a bad batch
   * throws), and written to wasm once an animation frame, every append of
   * the frame together. An x equal to the last row's replaces it; a lesser x
   * is dropped and counted as late (REQ-STREAM-040).
   */
  append(rows: AppendInput): void {
    if (!this.#g) {
      this.#run({ k: "append", v: rows });
      return;
    }
    this.#push(rows);
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

  /** The zone table time is drawn in (zone.schema.json, REQ-TZ-020); the last one set, or null (the core's UTC). */
  get zone(): ZoneTable | null {
    return this.#zone;
  }
  set zone(v: ZoneTable) {
    this.#run({ k: "zone", v });
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
    this.#tail = null;
    this.#pending = false;
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
