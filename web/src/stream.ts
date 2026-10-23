// Streaming in the shell (REQ-STREAM-010 to 070, ADR-107): the first table's
// rows as the chart holds them, appended to, with the ring's rules: a row
// whose x equals the last row's replaces it, a row whose x is less is dropped
// and counted as late (REQ-STREAM-040), and rows past `retain.rows` (default
// 1,000,000) or older than the newest x minus `retain.age` are trimmed from
// the front (REQ-STREAM-020, REQ-STREAM-030). The rows appended since the last
// write wait, as they came, in a staging area of one f64 column each, for the
// module's `staging(n)` and `append(n)` (the plan's boundary table, v0.6.0).
//
// The tail is the shell's copy of what the module holds: what a type change
// carries to the next chart, and what is committed whole when the staging
// area cannot carry a write. Pure: no DOM, no wasm.

import { ColumnKind } from "./api.ts";
import {
  batchColumns,
  encodeColumn,
  isColumnar,
  Labels,
  MISSING_CODE,
  scalarOf,
  type AppendInput,
  type ColumnDesc,
  type DataObject,
  type EncodedColumn,
  type Prepared,
  type Row,
  type Source,
  type TableDesc,
  type ValuesInput,
} from "./data.ts";

/** The ring's capacity when `retain.rows` is not given (REQ-STREAM-020). */
export const DEFAULT_RETAIN_ROWS = 1_000_000;

/** What a commit of data did: `plot-data`'s detail (events.schema.json, REQ-STREAM-070). */
export interface DataCommit {
  /** Rows held after the commit. */
  rows: number;
  /** Rows trimmed by `retain.rows` or `retain.age` in this commit. */
  dropped: number;
  /** Appended rows dropped because their x was less than the last row's. */
  late: number;
  /** The data version the module returned. */
  version: number;
}

/** `retain` as the options give it (options.schema.json). */
export interface Retain {
  rows?: number;
  age?: number | string;
}

const AGE_UNITS_MS: Record<string, number> = { ns: 1e-6, us: 1e-3, ms: 1, s: 1000, min: 60_000, h: 3_600_000, d: 86_400_000, w: 604_800_000 };
const UNIT_PER_MS: Record<string, number> = { s: 1e-3, ms: 1, us: 1e3, ns: 1e6 };

/** `retain.age` in the x column's own units: a number as given; a duration for a timestamp column in its `time_unit`; null for none. */
export function ageIn(age: Retain["age"], x: ColumnDesc | undefined): number | null {
  if (age === undefined) return null;
  if (typeof age === "number") return age > 0 ? age : null;
  const m = /^([0-9]+(?:\.[0-9]+)?)(ns|us|ms|s|min|h|d|w)$/.exec(age);
  if (!m || !x || x.type !== "timestamp") return null;
  return Number(m[1]) * (AGE_UNITS_MS[m[2] as string] as number) * (UNIT_PER_MS[x.time_unit ?? "ms"] as number);
}

/** The `retain` object of a chart's options. */
export function retainOf(options: Record<string, unknown>): Retain {
  const r = options["retain"];
  return r !== null && typeof r === "object" && !Array.isArray(r) ? (r as Retain) : {};
}

/**
 * The column the ring orders by: the one `encode.x` names, else the one with
 * role `x`, else the one named `x`, else the first timestamp, else the first
 * number (as the line chart picks it); -1 when there is none.
 */
export function xColumnOf(t: TableDesc, encode: Record<string, unknown>): number {
  const named = encode["x"];
  const name = typeof named === "string" ? named : Array.isArray(named) && typeof named[0] === "string" ? (named[0] as string) : null;
  const by = (f: (c: ColumnDesc) => boolean): number => t.columns.findIndex(f);
  const numeric = (c: ColumnDesc): boolean => c.type !== "category" && c.type !== "string";
  for (const i of [name !== null ? by((c) => c.name === name) : -1, by((c) => c.role === "x"), by((c) => c.name === "x")]) {
    if (i >= 0) return numeric(t.columns[i] as ColumnDesc) ? i : -1;
  }
  const ts = by((c) => c.type === "timestamp");
  return ts >= 0 ? ts : by((c) => numeric(c) && c.role !== "series");
}

type Store = Float64Array | Uint32Array | (string | null)[];

/** A growable column of the ring: `buf` from `start`, `len` rows. */
interface Column {
  readonly desc: ColumnDesc;
  readonly kind: 0 | 1 | 2;
  buf: Store;
  labels: Labels | null;
}

const KIND_STRING = 2;

function alloc(kind: 0 | 1 | 2, n: number): Store {
  return kind === ColumnKind.F64 ? new Float64Array(n) : kind === ColumnKind.U32 ? new Uint32Array(n) : new Array<string | null>(n).fill(null);
}

/** A source over `n` values of `buf` from `start`: one copy into the reserved buffer (REQ-MEM-020). */
function viewSource(kind: 0 | 1, buf: Float64Array | Uint32Array, start: number, n: number): Source {
  return {
    kind,
    length: n,
    write(dst) {
      dst.set(buf.subarray(start, start + n) as Float64Array & Uint32Array);
    },
  };
}

/**
 * The first table of a data set held for streaming. Build it from the data
 * last committed ({@link Tail.from}), push appends into it, and read
 * {@link Tail.prepared} to commit it whole or {@link Tail.takeStaged} to write
 * what came since through `staging(n)`.
 */
export class Tail {
  readonly #name: string;
  readonly #table: TableDesc;
  readonly #cols: Column[];
  readonly #rest: Prepared;
  readonly #restSources: (Source | null)[];
  readonly #x: number;
  readonly #capacity: number;
  readonly #age: number | null;
  #start = 0;
  #len = 0;
  #lastX = NaN;
  #late = 0;
  #dropped = 0;
  /** The rows appended since the last take, as they came: one f64 column each. */
  #staged: Float64Array[];
  #stagedLen = 0;
  /** A write the staging area cannot carry (a new label, a string column, an overflow): commit whole. */
  #whole = false;
  #cache: Prepared | null = null;
  readonly #scratch: Float64Array;

  private constructor(p: Prepared, retain: Retain, encode: Record<string, unknown>) {
    const entries = Object.entries(p.descriptor.tables);
    const first = entries[0];
    if (!first) throw new Error("append needs data with a table");
    const [name, table] = first;
    this.#name = name;
    this.#table = table;
    this.#rest = p;
    const n = table.length ?? 0;
    const k = table.columns.length;
    this.#restSources = p.sources.slice(k);
    this.#x = xColumnOf(table, encode);
    const rows = retain.rows;
    this.#capacity = typeof rows === "number" && Number.isInteger(rows) && rows >= 1 ? rows : DEFAULT_RETAIN_ROWS;
    this.#age = ageIn(retain.age, table.columns[this.#x]);
    const size = Math.max(16, n + this.#slack());
    this.#cols = table.columns.map((c, i) => {
      const kind = c.type === "string" ? KIND_STRING : c.type === "category" ? ColumnKind.U32 : ColumnKind.F64;
      const buf = alloc(kind, size);
      const src = p.sources[i];
      if (kind === KIND_STRING) {
        const v = (c.values as ArrayLike<string | null> | undefined) ?? [];
        for (let r = 0; r < n; r++) (buf as (string | null)[])[r] = v[r] ?? null;
      } else if (src) src.write((buf as Float64Array | Uint32Array).subarray(0, n));
      else (buf as Float64Array | Uint32Array).fill(kind === ColumnKind.U32 ? MISSING_CODE : NaN, 0, n);
      const { values: _drop, ...desc } = c;
      void _drop;
      if (c.type === "timestamp" && (c.time_unit === "us" || c.time_unit === "ns") && c.origin === undefined) {
        // Committed without an origin, the module took the least value, floored, as one and holds offsets
        // from it (Data::from_typed): the tail does the same, so what it stages and commits are those offsets.
        const b = buf as Float64Array;
        let least = Infinity;
        for (let r = 0; r < n; r++) {
          const v = b[r] as number;
          if (Number.isFinite(v) && v < least) least = v;
        }
        if (Number.isFinite(least)) {
          const origin = Math.floor(least);
          for (let r = 0; r < n; r++) b[r] = (b[r] as number) - origin;
          desc.origin = BigInt(origin);
        }
      }
      return { desc: { ...desc }, kind, buf, labels: kind === ColumnKind.U32 ? new Labels(c.categories ?? []) : null };
    });
    this.#len = n;
    this.#staged = this.#cols.map(() => new Float64Array(64));
    this.#scratch = new Float64Array(k);
    if (this.#cols.some((c) => c.kind === KIND_STRING)) this.#whole = true;
    this.#lastX = this.#findLastX();
  }

  /** A tail holding `p`'s first table, with the chart's `retain` and `encode` options. */
  static from(p: Prepared, retain: Retain = {}, encode: Record<string, unknown> = {}): Tail {
    return new Tail(p, retain, encode);
  }

  #slack(): number {
    return Math.max(1024, this.#capacity >> 3);
  }

  #findLastX(): number {
    const x = this.#cols[this.#x];
    if (!x || x.kind !== ColumnKind.F64) return NaN;
    const b = x.buf as Float64Array;
    for (let r = this.#start + this.#len - 1; r >= this.#start; r--) if (!Number.isNaN(b[r] as number)) return b[r] as number;
    return NaN;
  }

  /** Rows held. */
  get length(): number {
    return this.#len;
  }

  /** The ring's capacity (`retain.rows`). */
  get capacity(): number {
    return this.#capacity;
  }

  /** The table's name. */
  get name(): string {
    return this.#name;
  }

  /** The x column's index in the table, -1 when there is none. */
  get xIndex(): number {
    return this.#x;
  }

  /** The newest x in data units (epoch ms for a timestamp), NaN with none. */
  get newestX(): number {
    const c = this.#cols[this.#x];
    if (!c || Number.isNaN(this.#lastX)) return NaN;
    if (c.desc.type !== "timestamp") return this.#lastX;
    const per = UNIT_PER_MS[c.desc.time_unit ?? "ms"] as number;
    return (this.#lastX + Number(c.desc.origin ?? 0)) / per;
  }

  /** Rows waiting in the staging area. */
  get staged(): number {
    return this.#stagedLen;
  }

  /** Columns in the table: the staging area's width. */
  get width(): number {
    return this.#cols.length;
  }

  /** The x span held, oldest to newest, in data units (epoch ms), when x is a timestamp column; null otherwise or when empty. */
  timeSpan(): [number, number] | null {
    const c = this.#cols[this.#x];
    if (!c || c.desc.type !== "timestamp" || this.#len === 0 || Number.isNaN(this.#lastX)) return null;
    const b = c.buf as Float64Array;
    let lo = NaN;
    for (let r = this.#start; r < this.#start + this.#len && Number.isNaN(lo); r++) lo = b[r] as number;
    const per = UNIT_PER_MS[c.desc.time_unit ?? "ms"] as number;
    const o = Number(c.desc.origin ?? 0);
    return [(lo + o) / per, (this.#lastX + o) / per];
  }

  /** Marks the next write as a whole commit: the module holds none of these rows yet. */
  markWhole(): void {
    this.#whole = true;
    this.#stagedLen = 0;
  }

  /** Whether the next write must commit the whole table (the staging area cannot carry it). */
  get whole(): boolean {
    return this.#whole;
  }

  /** Makes the room for `add` more rows at the end, compacting or growing the buffers. */
  #room(add: number): void {
    const size = this.#cols[0]?.buf.length ?? 0;
    if (this.#start + this.#len + add <= size) return;
    const need = this.#len + add;
    const grow = need > size - this.#slack() / 2;
    const next = grow ? Math.max(need + this.#slack(), Math.min(size * 2, this.#capacity + this.#slack())) : size;
    for (const c of this.#cols) {
      if (grow || c.kind === KIND_STRING) {
        const b = alloc(c.kind, Math.max(next, need));
        if (c.kind === KIND_STRING) {
          const src = c.buf as (string | null)[];
          for (let r = 0; r < this.#len; r++) (b as (string | null)[])[r] = src[this.#start + r] ?? null;
        } else (b as Float64Array).set((c.buf as Float64Array).subarray(this.#start, this.#start + this.#len));
        c.buf = b;
      } else (c.buf as Float64Array).copyWithin(0, this.#start, this.#start + this.#len);
    }
    this.#start = 0;
    this.#cache = null;
  }

  /** Stages `n` rows of encoded columns, as they came (before the ring's rules: the module applies its own). */
  #stage(cols: readonly ArrayLike<number>[] | null, n: number): void {
    if (this.#whole) return;
    if (this.#stagedLen + n > this.#capacity) {
      // More than the ring holds came since the last write: commit whole instead.
      this.#whole = true;
      this.#stagedLen = 0;
      return;
    }
    const need = this.#stagedLen + n;
    if (need > (this.#staged[0]?.length ?? 0)) {
      const size = Math.max(need, (this.#staged[0]?.length ?? 0) * 2);
      this.#staged = this.#staged.map((s) => {
        const b = new Float64Array(size);
        b.set(s.subarray(0, this.#stagedLen));
        return b;
      });
    }
    this.#staged.forEach((s, j) => {
      if (cols) {
        const v = cols[j] as ArrayLike<number>;
        for (let r = 0; r < n; r++) s[this.#stagedLen + r] = v[r] as number;
      } else s[this.#stagedLen] = this.#scratch[j] as number;
    });
    this.#stagedLen = need;
  }

  /** Writes one row, `get(j)` giving column j's value, by the ring's rules. */
  #put(get: (j: number) => number | string | null, xv: number): void {
    const xi = this.#x;
    let at: number;
    if (xi >= 0 && this.#len > 0 && !Number.isNaN(xv) && !Number.isNaN(this.#lastX)) {
      const last = (this.#cols[xi] as Column).buf[this.#start + this.#len - 1] as number;
      if (xv === last) at = this.#start + this.#len - 1; // an equal x replaces the last row (REQ-STREAM-040)
      else if (xv < this.#lastX) {
        this.#late++;
        return;
      } else {
        this.#room(1);
        at = this.#start + this.#len++;
      }
    } else {
      this.#room(1);
      at = this.#start + this.#len++;
    }
    for (let j = 0; j < this.#cols.length; j++) (this.#cols[j] as Column).buf[at] = get(j) as never;
    if (!Number.isNaN(xv)) this.#lastX = xv;
  }

  /** Trims rows past the capacity and older than the newest x minus `retain.age` (REQ-STREAM-020, 030). */
  #trim(): void {
    let drop = Math.max(0, this.#len - this.#capacity);
    const x = this.#cols[this.#x];
    if (this.#age !== null && x && x.kind === ColumnKind.F64 && !Number.isNaN(this.#lastX)) {
      const from = this.#lastX - this.#age;
      const b = x.buf as Float64Array;
      while (drop < this.#len && !((b[this.#start + drop] as number) >= from)) drop++;
    }
    if (drop === 0) return;
    const strings = this.#cols.filter((c) => c.kind === KIND_STRING);
    for (const c of strings) (c.buf as (string | null)[]).fill(null, this.#start, this.#start + drop);
    this.#start += drop;
    this.#len -= drop;
    this.#dropped += drop;
  }

  /**
   * Appends a row, rows, or typed columns by name (REQ-STREAM-010), typed as
   * the table's columns. Throws, holding nothing of the batch, on a column the
   * table lacks or a value its column cannot take.
   */
  push(batch: AppendInput): void {
    if (!Array.isArray(batch) && !isColumnar(batch as Row) && this.#pushRow(batch as Row)) return;
    const cols = batchColumns(batch);
    let n: number | null = null;
    for (const [name, v] of Object.entries(cols)) {
      if (!this.#cols.some((c) => c.desc.name === name)) throw new Error(`append: no column \`${name}\` in table \`${this.#name}\``);
      if (n === null) n = v.length;
      else if (v.length !== n) throw new Error(`append: column \`${name}\` has ${v.length} values, not ${n}`);
    }
    const k = n ?? 0;
    if (k === 0) return;
    const before = this.#cols.map((c) => c.labels?.list.length ?? 0);
    const enc: EncodedColumn[] = this.#cols.map((c) => {
      const v: ValuesInput = Object.hasOwn(cols, c.desc.name) ? (cols[c.desc.name] as ValuesInput) : new Array<null>(k).fill(null);
      return encodeColumn(c.desc, v, k, c.labels);
    });
    if (this.#cols.some((c, j) => (c.labels?.list.length ?? 0) !== before[j])) this.#whole = true;
    this.#cache = null;
    this.#stage(this.#whole ? null : (enc.map((e) => (Array.isArray(e) ? [] : e)) as ArrayLike<number>[]), k);
    const xi = this.#x;
    const xs = xi >= 0 && !Array.isArray(enc[xi]) ? (enc[xi] as Float64Array) : null;
    // The common case, every x above the last and none missing: whole columns at once.
    let rising = xs !== null && !Array.isArray(enc[xi]);
    if (rising && xs) {
      let prev = Number.isNaN(this.#lastX) ? -Infinity : this.#lastX;
      if (this.#len > 0 && Number.isNaN(this.#lastX)) rising = false;
      for (let r = 0; r < k && rising; r++) {
        const v = xs[r] as number;
        if (!(v > prev)) rising = false;
        prev = v;
      }
    }
    if (rising && xs && !this.#cols.some((c) => c.kind === KIND_STRING)) {
      this.#room(k);
      const at = this.#start + this.#len;
      this.#cols.forEach((c, j) => (c.buf as Float64Array).set(enc[j] as Float64Array, at));
      this.#len += k;
      this.#lastX = xs[k - 1] as number;
    } else {
      const xv = (r: number): number => (xs ? (xs[r] as number) : NaN);
      for (let r = 0; r < k; r++) this.#put((j) => ((enc[j] as ArrayLike<number | string | null>)[r] ?? null) as number | string | null, xv(r));
    }
    this.#trim();
  }

  /** The one-row fast path: no allocation when every value is a number (or a Date in a timestamp column); false when it cannot take the row. */
  #pushRow(row: Row): boolean {
    if (row === null || typeof row !== "object") throw new Error("append: expected a row object, rows, or columns");
    for (const key in row) if (!this.#cols.some((c) => c.desc.name === key)) throw new Error(`append: no column \`${key}\` in table \`${this.#name}\``);
    const s = this.#scratch;
    for (let j = 0; j < this.#cols.length; j++) {
      const c = this.#cols[j] as Column;
      const v = scalarOf(c.desc, row[c.desc.name]);
      if (v === undefined) return false;
      s[j] = v;
    }
    this.#cache = null;
    this.#stage(null, 1);
    this.#put((j) => s[j] as number, this.#x >= 0 ? (s[this.#x] as number) : NaN);
    this.#trim();
    return true;
  }

  /** Zeroes the late and dropped counts: the module reported its own. */
  clearCounts(): void {
    this.#late = 0;
    this.#dropped = 0;
  }

  /** The late and dropped counts since the last take, zeroed. */
  takeCounts(): { late: number; dropped: number } {
    const out = { late: this.#late, dropped: this.#dropped };
    this.#late = 0;
    this.#dropped = 0;
    return out;
  }

  /** Writes the staged rows into `dst` (column j's at j × n), then empties the staging area; returns n. */
  takeStaged(dst: Float64Array): number {
    const n = this.#stagedLen;
    this.#staged.forEach((s, j) => dst.set(s.subarray(0, n), j * n));
    this.#stagedLen = 0;
    return n;
  }

  /** Empties the staging area: what it held is in a whole commit. */
  committedWhole(): void {
    this.#stagedLen = 0;
    this.#whole = this.#cols.some((c) => c.kind === KIND_STRING);
  }

  /** The table as it is now, ready to commit whole; the same object until the next push. */
  prepared(): Prepared {
    if (this.#cache) return this.#cache;
    const n = this.#len;
    const columns: ColumnDesc[] = this.#cols.map((c) => {
      const d: ColumnDesc = { ...c.desc };
      if (c.labels) d.categories = c.labels.list.slice();
      if (c.kind === KIND_STRING) d.values = (c.buf as (string | null)[]).slice(this.#start, this.#start + n);
      return d;
    });
    const sources: (Source | null)[] = this.#cols.map((c) => (c.kind === KIND_STRING ? null : viewSource(c.kind as 0 | 1, c.buf as Float64Array | Uint32Array, this.#start, n)));
    const table: TableDesc = { ...this.#table, length: n, columns };
    const descriptor: DataObject = { ...this.#rest.descriptor, tables: { ...this.#rest.descriptor.tables, [this.#name]: table } };
    this.#cache = { descriptor, sources: [...sources, ...this.#restSources] };
    return this.#cache;
  }

  /** The newest row by column name: numbers in data units (timestamps epoch ms), labels as text; null when empty. */
  latest(): Record<string, number | string | null> | null {
    if (this.#len === 0) return null;
    const at = this.#start + this.#len - 1;
    const out: Record<string, number | string | null> = {};
    for (const c of this.#cols) {
      const v = c.buf[at];
      if (c.kind === KIND_STRING) out[c.desc.name] = (v as string | null) ?? null;
      else if (c.labels) out[c.desc.name] = v === MISSING_CODE ? null : (c.labels.list[v as number] ?? null);
      else if (c.desc.type === "timestamp") out[c.desc.name] = ((v as number) + Number(c.desc.origin ?? 0)) / (UNIT_PER_MS[c.desc.time_unit ?? "ms"] as number);
      else out[c.desc.name] = v as number;
    }
    return out;
  }
}
