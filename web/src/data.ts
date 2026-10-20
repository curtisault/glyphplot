// Data in the shell: the data object, the columns shorthand and the rows
// shorthand of spec/schemas/data.schema.json, and the binary form
// `{descriptor, columns: {name: TypedArray}}`, normalised into a descriptor
// without numeric values plus one writer a column (REQ-DATA-120). A writer
// fills the buffer the chart reserved, so a typed array reaches wasm memory
// with one copy and a JSON array with none in between (REQ-MEM-020,
// REQ-DATA-050). Shorthand types are inferred as the schema and the core's
// reader infer them. Label text is NFC-normalised here, before it crosses
// (REQ-I18N-050). A `us` or `ns` timestamp column given exactly (a
// BigInt64Array, integer strings, RFC 3339 strings) crosses as an `origin`,
// its least value, and f64 offsets from it, so a span under 2^53 units
// arrives exactly (REQ-DATA-020). Pure: no DOM, no wasm.

import { ColumnKind } from "./api.ts";

export type Scalar = number | string | boolean | null;
export type NumericArray =
  | Float64Array
  | Float32Array
  | Int32Array
  | Uint32Array
  | Int16Array
  | Uint16Array
  | Int8Array
  | Uint8Array
  | Uint8ClampedArray
  | BigInt64Array
  | BigUint64Array;
/** A column's values: JSON values, Dates (timestamps), or a typed array. */
export type ValuesInput = ArrayLike<Scalar | Date | undefined> | NumericArray;

export type ColumnType = "f64" | "f32" | "i64" | "u32" | "bool" | "timestamp" | "duration" | "category" | "string";
export type TimeUnit = "s" | "ms" | "us" | "ns";

/** A column of the full form (data.schema.json `column`). */
export interface ColumnDesc {
  name: string;
  type: ColumnType;
  role?: string;
  unit?: string;
  scale?: number;
  offset?: number;
  time_unit?: TimeUnit;
  /**
   * For a `us` or `ns` timestamp column given as a buffer: the instant, in
   * `time_unit`, its values are offsets from (REQ-DATA-020). A number must be
   * a whole one; a bigint holds any i64.
   */
  origin?: number | bigint;
  zone?: string;
  categories?: string[];
  values?: ValuesInput;
}

/** A table of the full form. */
export interface TableDesc {
  length?: number;
  sorted?: string;
  frame?: string;
  epoch?: string;
  columns: ColumnDesc[];
}

/** A matrix of the full form. */
export interface MatrixDesc {
  rows: number;
  cols: number;
  values?: ArrayLike<number | null> | NumericArray;
  row_labels?: string[];
  col_labels?: string[];
  unit?: string;
  symmetric?: boolean;
}

/** The full form of the data contract. */
export interface DataObject {
  version?: 1;
  tables: Record<string, TableDesc>;
  matrices?: Record<string, MatrixDesc>;
}

/** The binary form: a descriptor without values and the values by column name (or `table/column`). */
export interface BinaryData {
  descriptor: DataObject;
  columns: Record<string, ValuesInput>;
}

export type Row = Record<string, Scalar | Date | undefined>;

/** Anything `data` accepts: the full form, the binary form, either shorthand, or their JSON text. */
export type DataInput = DataObject | BinaryData | Record<string, ValuesInput> | Row[] | string;

/** Fills a reserved buffer with a column's values. */
export interface Source {
  readonly kind: 0 | 1;
  readonly length: number;
  /** Writes `length` values into `dst` (whose length is at least `length`); throws on a bad value. */
  write(dst: Float64Array | Uint32Array): void;
}

/** Data ready to cross: a descriptor without numeric values and a writer for each reserved column. */
export interface Prepared {
  /** The full form, `version` and every table's `length` set; numeric `values` left out, string values kept. */
  descriptor: DataObject;
  /** By column number (tables' columns in order, then matrices); null where nothing is reserved. */
  sources: (Source | null)[];
}

/** The shorthand table's name, as the core names it. */
export const SHORTHAND_TABLE = "data";
/** The most categories a column may hold. */
export const MAX_CATEGORIES = 65535;
/** A missing category code. */
export const MISSING_CODE = 0xffffffff;

const nfc = (s: string): string => s.normalize("NFC");

/** Every string in `v` NFC-normalised, keys kept (REQ-I18N-050). */
export function nfcDeep<T>(v: T): T {
  if (typeof v === "string") return nfc(v) as T;
  if (Array.isArray(v)) return v.map((x) => nfcDeep(x)) as T;
  if (v !== null && typeof v === "object" && !ArrayBuffer.isView(v)) {
    const out: Record<string, unknown> = {};
    for (const [k, x] of Object.entries(v as Record<string, unknown>)) out[k] = nfcDeep(x);
    return out as T;
  }
  return v;
}

const isTyped = (v: unknown): v is NumericArray => ArrayBuffer.isView(v) && !(v instanceof DataView);
const isBig = (v: NumericArray): v is BigInt64Array | BigUint64Array => v instanceof BigInt64Array || v instanceof BigUint64Array;

// ---------------------------------------------------------------- RFC 3339

function daysFromCivil(y: number, m: number, d: number): number {
  const yy = m <= 2 ? y - 1 : y;
  const era = Math.floor(yy / 400);
  const yoe = yy - era * 400;
  const doy = Math.floor((153 * (m > 2 ? m - 3 : m + 9) + 2) / 5) + d - 1;
  const doe = yoe * 365 + Math.floor(yoe / 4) - Math.floor(yoe / 100) + doy;
  return era * 146097 + doe - 719468;
}

function daysInMonth(y: number, m: number): number {
  if (m === 2) return (y % 4 === 0 && y % 100 !== 0) || y % 400 === 0 ? 29 : 28;
  return m === 4 || m === 6 || m === 9 || m === 11 ? 30 : 31;
}

const RFC3339 = /^(\d{4})-(\d{2})-(\d{2})[Tt](\d{2}):(\d{2}):(\d{2})(?:\.(\d+))?(?:([Zz])|([+-])(\d{2}):(\d{2}))$/;

/**
 * An RFC 3339 date-time as [whole seconds since the epoch, nanoseconds], or
 * null when `s` is not one. The offset written is applied; digits past
 * nanoseconds are dropped; a leap second reads as the next second.
 */
export function parseRfc3339(s: string): [number, number] | null {
  const m = RFC3339.exec(s);
  if (!m) return null;
  const n = (i: number): number => Number(m[i]);
  const [y, mo, d, h, mi, se] = [n(1), n(2), n(3), n(4), n(5), n(6)];
  if (mo < 1 || mo > 12 || d < 1 || d > daysInMonth(y, mo) || h > 23 || mi > 59 || se > 60) return null;
  const frac = m[7] ?? "";
  const nanos = frac === "" ? 0 : Number((frac + "00000000").slice(0, 9));
  let offset = 0;
  if (m[9] !== undefined) {
    const oh = n(10);
    const om = n(11);
    if (oh > 23 || om > 59) return null;
    offset = (oh * 3600 + om * 60) * (m[9] === "-" ? -1 : 1);
  }
  const secs = daysFromCivil(y, mo, d) * 86400 + h * 3600 + mi * 60 + se - offset;
  return [secs, nanos];
}

/** An RFC 3339 date-time in `unit` since the epoch (f64: exact for spans under 2^53 of the unit). */
export function stampIn(s: string, unit: TimeUnit): number | null {
  const p = parseRfc3339(s);
  if (!p) return null;
  const [secs, nanos] = p;
  switch (unit) {
    case "s":
      return secs + nanos / 1e9;
    case "ms":
      return secs * 1000 + nanos / 1e6;
    case "us":
      return secs * 1e6 + nanos / 1e3;
    case "ns":
      return secs * 1e9 + nanos;
  }
}

const UNIT_MS: Record<TimeUnit, number> = { s: 1000, ms: 1, us: 1e-3, ns: 1e-6 };

/** Whether a timestamp in `unit` crosses as an origin and offsets (REQ-DATA-020). */
const hasOrigin = (unit: TimeUnit | undefined): unit is "us" | "ns" => unit === "us" || unit === "ns";

const INTEGER = /^[+-]?\d+$/;
const I64_MIN = -(1n << 63n);
const I64_MAX = (1n << 63n) - 1n;

/** An RFC 3339 date-time, exactly, as a whole number of `unit` since the epoch (sub-unit digits dropped toward the past). */
export function exactStamp(s: string, unit: "us" | "ns"): bigint | null {
  const p = parseRfc3339(s);
  if (!p) return null;
  const [secs, nanos] = p;
  return unit === "ns" ? BigInt(secs) * 1_000_000_000n + BigInt(nanos) : BigInt(secs) * 1_000_000n + BigInt(Math.floor(nanos / 1000));
}

// ---------------------------------------------------------------- writers

class DataError extends Error {
  override name = "DataError";
}

const bad = (column: string, row: number, expected: string): DataError =>
  new DataError(`column \`${column}\` row ${row}: expected ${expected}`);

/** A writer copying a typed array (one copy, REQ-MEM-020). */
function typedSource(kind: 0 | 1, values: NumericArray, length = values.length): Source {
  return {
    kind,
    length,
    write(dst) {
      if (isBig(values)) {
        for (let i = 0; i < length; i++) dst[i] = Number(values[i]);
      } else if (values.length === length) {
        dst.set(values);
      } else {
        dst.set(values.subarray(0, length));
      }
    },
  };
}

/** A writer converting JSON values to f64 as `type` reads them. */
function jsonF64(column: string, type: ColumnType, unit: TimeUnit, values: ArrayLike<unknown>, length: number): Source {
  return {
    kind: ColumnKind.F64,
    length,
    write(dst) {
      for (let i = 0; i < length; i++) {
        const v = values[i];
        let out: number;
        if (v === null || v === undefined) out = NaN;
        else if (type === "bool") {
          if (typeof v !== "boolean") throw bad(column, i, "a boolean or null");
          out = v ? 1 : 0;
        } else if (typeof v === "number") out = v;
        else if (typeof v === "bigint") out = Number(v);
        else if (v instanceof Date && type === "timestamp") out = v.getTime() / UNIT_MS[unit];
        else if (typeof v === "string" && type === "timestamp") {
          const t = stampIn(v, unit);
          if (t === null) throw bad(column, i, "an RFC 3339 date-time");
          out = t;
        } else {
          throw bad(column, i, type === "timestamp" ? "a number, an RFC 3339 string or null" : "a number or null");
        }
        dst[i] = out;
      }
    },
  };
}

/** A writer of category codes from JSON integers. */
function jsonCodes(column: string, values: ArrayLike<unknown>, length: number, categories: number): Source {
  return {
    kind: ColumnKind.U32,
    length,
    write(dst) {
      for (let i = 0; i < length; i++) {
        const v = values[i];
        if (v === null || v === undefined) dst[i] = MISSING_CODE;
        else if (typeof v === "number" && Number.isInteger(v) && v >= 0 && v < categories) dst[i] = v;
        else throw bad(column, i, `a category index below ${categories} or null`);
      }
    },
  };
}

/** Whether `values` must cross exactly: a BigInt array, or JSON values with a string or a bigint among them. */
function wantsExact(values: ValuesInput, length: number): boolean {
  if (isTyped(values)) return isBig(values);
  for (let i = 0; i < length; i++) {
    const v = (values as ArrayLike<unknown>)[i];
    if (typeof v === "string" || typeof v === "bigint") return true;
  }
  return false;
}

/**
 * A `us` or `ns` timestamp column's values as whole units, exactly, each
 * `base` plus its value; null where missing. Integers, integer strings,
 * RFC 3339 strings, bigints and Dates are read; anything else is an error.
 */
function exactValues(column: string, unit: "us" | "ns", values: ValuesInput, length: number, base: bigint): (bigint | null)[] {
  const out = new Array<bigint | null>(length);
  const perMs = unit === "ns" ? 1_000_000n : 1_000n;
  for (let i = 0; i < length; i++) {
    const v = (values as ArrayLike<unknown>)[i];
    let t: bigint | null;
    if (v === null || v === undefined || (typeof v === "number" && Number.isNaN(v))) t = null;
    else if (typeof v === "bigint") t = v;
    else if (typeof v === "number" && Number.isInteger(v)) t = BigInt(v);
    else if (typeof v === "string" && INTEGER.test(v)) t = BigInt(v);
    else if (typeof v === "string") {
      t = exactStamp(v, unit);
      if (t === null) throw bad(column, i, "an integer, an RFC 3339 string or null");
    } else if (v instanceof Date && !Number.isNaN(v.getTime())) t = BigInt(v.getTime()) * perMs;
    else throw bad(column, i, "an integer, an RFC 3339 string or null");
    if (t !== null) {
      t += base;
      if (t < I64_MIN || t > I64_MAX) throw bad(column, i, "an instant within an i64");
    }
    out[i] = t;
  }
  return out;
}

/** The least of `stamps`, or 0 when every one is missing. */
function leastOf(stamps: readonly (bigint | null)[]): bigint {
  let least: bigint | null = null;
  for (const t of stamps) if (t !== null && (least === null || t < least)) least = t;
  return least ?? 0n;
}

/** A writer of `stamps` as f64 offsets from `origin` (NaN where missing): exact while each offset is under 2^53. */
function offsetSource(stamps: readonly (bigint | null)[], origin: bigint): Source {
  return {
    kind: ColumnKind.F64,
    length: stamps.length,
    write(dst) {
      for (let i = 0; i < stamps.length; i++) {
        const t = stamps[i];
        dst[i] = t === null || t === undefined ? NaN : Number(t - origin);
      }
    },
  };
}

/** A column's `origin` as a bigint: 0 when absent; an error unless whole. */
function originOf(c: ColumnDesc): bigint {
  const o = c.origin;
  if (o === undefined) return 0n;
  if (typeof o === "bigint") return o;
  if (typeof o === "number" && Number.isInteger(o)) return BigInt(o);
  throw new DataError(`column \`${c.name}\`: \`origin\` must be an integer`);
}

/** A writer that writes `a` then `b`. */
export function concatSource(a: Source, b: Source): Source {
  return {
    kind: a.kind,
    length: a.length + b.length,
    write(dst) {
      a.write(dst.subarray(0, a.length));
      b.write(dst.subarray(a.length, a.length + b.length));
    },
  };
}

/** Writes a source into a fresh array: for tests and for callers without wasm. */
export function materialise(s: Source): Float64Array | Uint32Array {
  const out = s.kind === ColumnKind.U32 ? new Uint32Array(s.length) : new Float64Array(s.length);
  s.write(out);
  return out;
}

// ---------------------------------------------------------------- shorthand inference

/** One inferred shorthand column: its descriptor, writer and string values. */
interface Inferred {
  desc: ColumnDesc;
  source: Source | null;
}

/**
 * Infers a shorthand column's type as data.schema.json says: numbers f64,
 * booleans bool, RFC 3339 strings timestamps (ms, UTC), other strings
 * categories (strings past 65,535 distinct labels), all missing f64; Dates
 * are timestamps; a typed array is f64. Mixed types are an error.
 */
export function inferColumn(name: string, values: ValuesInput): Inferred {
  const n = values.length;
  if (isTyped(values)) return { desc: { name, type: "f64" }, source: typedSource(ColumnKind.F64, values) };
  const arr = values as ArrayLike<unknown>;
  let numbers = 0;
  let bools = 0;
  let strings = 0;
  let dates = 0;
  let present = 0;
  for (let i = 0; i < n; i++) {
    const v = arr[i];
    if (v === null || v === undefined) continue;
    present++;
    if (typeof v === "number") numbers++;
    else if (typeof v === "boolean") bools++;
    else if (typeof v === "string") strings++;
    else if (v instanceof Date) dates++;
    else throw new DataError(`column \`${name}\` row ${i}: expected a number, string, boolean or null`);
  }
  if (numbers === present) return { desc: { name, type: "f64" }, source: jsonF64(name, "f64", "ms", arr, n) };
  if (bools === present) return { desc: { name, type: "bool" }, source: jsonF64(name, "bool", "ms", arr, n) };
  if (dates === present) return { desc: { name, type: "timestamp" }, source: jsonF64(name, "timestamp", "ms", arr, n) };
  if (strings !== present) throw new DataError(`column \`${name}\`: expected values of one type: numbers, booleans or strings`);
  const stamps = new Float64Array(n);
  let allStamps = true;
  for (let i = 0; i < n && allStamps; i++) {
    const v = arr[i];
    if (v === null || v === undefined) stamps[i] = NaN;
    else {
      const t = stampIn(v as string, "ms");
      if (t === null) allStamps = false;
      else stamps[i] = t;
    }
  }
  if (allStamps) return { desc: { name, type: "timestamp" }, source: typedSource(ColumnKind.F64, stamps) };
  const labels: string[] = [];
  const index = new Map<string, number>();
  const codes = new Uint32Array(n);
  for (let i = 0; i < n; i++) {
    const v = arr[i];
    if (v === null || v === undefined) {
      codes[i] = MISSING_CODE;
      continue;
    }
    const s = nfc(v as string);
    let c = index.get(s);
    if (c === undefined) {
      c = labels.length;
      labels.push(s);
      index.set(s, c);
    }
    codes[i] = c;
  }
  if (labels.length > MAX_CATEGORIES) {
    const text: (string | null)[] = Array.from({ length: n }, (_, i) => {
      const v = arr[i];
      return v === null || v === undefined ? null : nfc(v as string);
    });
    return { desc: { name, type: "string", values: text }, source: null };
  }
  return { desc: { name, type: "category", categories: labels }, source: typedSource(ColumnKind.U32, codes) };
}

/** The rows shorthand as columns: keys in first-seen order, a missing key null. */
export function rowsToColumns(rows: readonly Row[]): Record<string, (Scalar | Date)[]> {
  const out: Record<string, (Scalar | Date)[]> = {};
  const names: string[] = [];
  rows.forEach((row, r) => {
    if (row === null || typeof row !== "object" || Array.isArray(row)) throw new DataError(`row ${r}: expected an object`);
    for (const k of Object.keys(row)) {
      if (!Object.hasOwn(out, k)) {
        names.push(k);
        out[k] = new Array<Scalar | Date>(rows.length).fill(null);
      }
      const v = row[k];
      (out[k] as (Scalar | Date)[])[r] = v === undefined ? null : v;
    }
  });
  return out;
}

function shorthand(columns: Record<string, ValuesInput>): Prepared {
  const names = Object.keys(columns);
  if (names.length === 0) throw new DataError("a columns shorthand needs at least one column");
  let length: number | null = null;
  const descs: ColumnDesc[] = [];
  const sources: (Source | null)[] = [];
  for (const name of names) {
    const values = columns[name];
    if (!values || typeof values !== "object" || typeof (values as ArrayLike<unknown>).length !== "number") {
      throw new DataError(`column \`${name}\`: expected an array of values`);
    }
    if (length === null) length = values.length;
    else if (values.length !== length) throw new DataError(`table \`${SHORTHAND_TABLE}\`: column \`${name}\` has ${values.length} values, not ${length}`);
    const c = inferColumn(name, values);
    descs.push(c.desc);
    sources.push(c.source);
  }
  return { descriptor: { version: 1, tables: { [SHORTHAND_TABLE]: { length: length ?? 0, columns: descs } } }, sources };
}

// ---------------------------------------------------------------- the full form

const COLUMN_KEYS = ["role", "unit", "scale", "offset", "time_unit", "origin", "zone"] as const;
const NUMERIC: ReadonlySet<ColumnType> = new Set(["f64", "f32", "i64", "u32", "bool", "timestamp", "duration"]);

function full(data: DataObject, binary: Record<string, ValuesInput> | null): Prepared {
  if (!data.tables || typeof data.tables !== "object") throw new DataError("missing field `tables`");
  const tableNames = Object.keys(data.tables);
  if (tableNames.length === 0) throw new DataError("`tables` needs at least one table");
  const descriptor: DataObject = { version: 1, tables: {} };
  const sources: (Source | null)[] = [];
  const valuesOf = (table: string, c: ColumnDesc): ValuesInput | undefined => {
    if (c.values !== undefined) return c.values;
    if (!binary) return undefined;
    const qualified = `${table}/${c.name}`;
    if (Object.hasOwn(binary, qualified)) return binary[qualified];
    return Object.hasOwn(binary, c.name) ? binary[c.name] : undefined;
  };
  for (const t of tableNames) {
    const table = data.tables[t] as TableDesc;
    if (!table || !Array.isArray(table.columns) || table.columns.length === 0) throw new DataError(`table \`${t}\` needs 1 to 256 columns`);
    let length = typeof table.length === "number" ? table.length : null;
    for (const c of table.columns) {
      const v = valuesOf(t, c);
      if (v === undefined) continue;
      if (length === null) length = v.length;
      else if (v.length !== length) throw new DataError(`table \`${t}\`: column \`${c.name}\` has ${v.length} values, not ${length}`);
    }
    if (length === null) throw new DataError(`table \`${t}\` has no \`length\` and no column with values`);
    const n = length;
    const columns: ColumnDesc[] = [];
    for (const c of table.columns) {
      const out: ColumnDesc = { name: c.name, type: c.type };
      for (const k of COLUMN_KEYS) if (c[k] !== undefined) (out as unknown as Record<string, unknown>)[k] = c[k];
      if (out.unit !== undefined) out.unit = nfc(out.unit);
      if (c.categories !== undefined) out.categories = c.categories.map(nfc);
      const v = valuesOf(t, c);
      let source: Source | null = null;
      if (v !== undefined) {
        if (c.type === "string") {
          if (isTyped(v)) throw new DataError(`column \`${c.name}\`: a string column takes strings, not a typed array`);
          out.values = Array.from({ length: n }, (_, i) => {
            const s = (v as ArrayLike<unknown>)[i];
            if (s === null || s === undefined) return null;
            if (typeof s !== "string") throw bad(c.name, i, "a string or null");
            return nfc(s);
          });
        } else if (c.type === "category") {
          if (!out.categories) throw new DataError(`column \`${c.name}\` is a category column without \`categories\``);
          source = isTyped(v) ? typedSource(ColumnKind.U32, v, n) : jsonCodes(c.name, v as ArrayLike<unknown>, n, out.categories.length);
        } else if (c.type === "timestamp" && hasOrigin(c.time_unit) && wantsExact(v, n)) {
          // Exact instants: the least is the origin, the rest offsets from it (REQ-DATA-020).
          const stamps = exactValues(c.name, c.time_unit, v, n, originOf(c));
          const origin = leastOf(stamps);
          out.origin = origin;
          source = offsetSource(stamps, origin);
        } else if (NUMERIC.has(c.type)) {
          if (c.origin !== undefined) out.origin = originOf(c);
          source = isTyped(v) ? typedSource(ColumnKind.F64, v, n) : jsonF64(c.name, c.type, c.time_unit ?? "ms", v as ArrayLike<unknown>, n);
        } else {
          throw new DataError(`column \`${c.name}\`: unknown type \`${String(c.type)}\``);
        }
      }
      columns.push(out);
      sources.push(source);
    }
    const td: TableDesc = { length: n, columns };
    if (table.sorted !== undefined) td.sorted = table.sorted;
    if (table.frame !== undefined) td.frame = table.frame;
    if (table.epoch !== undefined) td.epoch = table.epoch;
    descriptor.tables[t] = td;
  }
  if (data.matrices) {
    descriptor.matrices = {};
    for (const [name, m] of Object.entries(data.matrices)) {
      const out: MatrixDesc = { rows: m.rows, cols: m.cols };
      if (m.row_labels) out.row_labels = m.row_labels.map(nfc);
      if (m.col_labels) out.col_labels = m.col_labels.map(nfc);
      if (m.unit !== undefined) out.unit = nfc(m.unit);
      if (m.symmetric !== undefined) out.symmetric = m.symmetric;
      const v = m.values ?? (binary && Object.hasOwn(binary, name) ? binary[name] : undefined);
      const size = m.rows * m.cols;
      let source: Source | null = null;
      if (v !== undefined) {
        if (v.length !== size) throw new DataError(`matrix \`${name}\` has ${v.length} values, not ${m.rows} × ${m.cols}`);
        source = isTyped(v) ? typedSource(ColumnKind.F64, v) : jsonF64(name, "f64", "ms", v as ArrayLike<unknown>, size);
      }
      descriptor.matrices[name] = out;
      sources.push(source);
    }
  }
  return { descriptor, sources };
}

/**
 * Normalises any accepted data input (REQ-DATA-120). JSON text is parsed
 * with `JSON.parse`. Throws a message naming the column at fault; a bad
 * value inside a JSON array is found when its writer runs.
 */
export function prepare(input: DataInput): Prepared {
  let v: unknown = input;
  if (typeof v === "string") v = JSON.parse(v);
  if (Array.isArray(v)) return shorthand(rowsToColumns(v as Row[]));
  if (v === null || typeof v !== "object") throw new DataError("data must be an object or an array");
  const o = v as Record<string, unknown>;
  if (Object.hasOwn(o, "descriptor") && Object.hasOwn(o, "columns") && !Array.isArray(o["columns"])) {
    return full(o["descriptor"] as DataObject, o["columns"] as Record<string, ValuesInput>);
  }
  if (Object.hasOwn(o, "tables")) {
    for (const k of Object.keys(o)) {
      if (k !== "tables" && k !== "matrices" && k !== "version") {
        throw new DataError(`unknown field \`${k}\` beside \`tables\`; a columns shorthand cannot use \`tables\`, \`matrices\` or \`version\``);
      }
    }
    return full(o as unknown as DataObject, null);
  }
  return shorthand(o as Record<string, ValuesInput>);
}

/** An empty table with `like`'s columns (the element's `clear()`), or x and y. */
export function emptyLike(like: DataObject | null): DataObject {
  const first = like ? Object.entries(like.tables)[0] : undefined;
  if (!first) return { version: 1, tables: { [SHORTHAND_TABLE]: { length: 0, columns: [{ name: "x", type: "f64" }, { name: "y", type: "f64" }] } } };
  const [name, t] = first;
  return {
    version: 1,
    tables: { [name]: { length: 0, columns: t.columns.map((c) => { const { values: _drop, ...rest } = c; void _drop; return rest; }) } },
  };
}

/**
 * The descriptor and lengths a `data` getter returns: no values at all
 * (REQ-EL-070), and no `origin`, which says how values crossed, not what
 * they are, so the result can describe new values.
 */
export function describe(p: Prepared): DataObject {
  const tables: Record<string, TableDesc> = {};
  for (const [name, t] of Object.entries(p.descriptor.tables)) {
    tables[name] = { ...t, columns: t.columns.map((c) => { const { values: _drop, origin: _origin, ...rest } = c; void _drop; void _origin; return { ...rest }; }) };
  }
  const out: DataObject = { version: 1, tables };
  if (p.descriptor.matrices) out.matrices = { ...p.descriptor.matrices };
  return out;
}

/** The descriptor as `commit` takes it: a bigint `origin` written as the integer it is (REQ-DATA-020). */
export function descriptorJson(p: Prepared): string {
  let big = false;
  const json = JSON.stringify(p.descriptor, (_k, v: unknown) => {
    if (typeof v !== "bigint") return v;
    big = true;
    return `\u0000glyphplot-i64:${v}`;
  });
  return big ? json.replace(/"\\u0000glyphplot-i64:(-?\d+)"/g, "$1") : json;
}

// ---------------------------------------------------------------- appends (until staging arrives in v0.6.0)

/** Rows or typed columns appended to the first table. */
export type AppendInput = Row | Row[] | Record<string, ValuesInput>;

/**
 * `prev` with `batch` appended to its first table, typed as that table's
 * columns: a new category label joins `categories`. The result is written
 * whole by `commit`; the core's staging buffer replaces this in v0.6.0.
 */
export function appendTo(prev: Prepared, batch: AppendInput): Prepared {
  const entries = Object.entries(prev.descriptor.tables);
  const first = entries[0];
  if (!first) throw new DataError("append needs data with a table");
  const [tname, table] = first;
  let cols: Record<string, ValuesInput>;
  if (Array.isArray(batch)) cols = rowsToColumns(batch);
  else {
    const vals = Object.values(batch);
    const columnar = vals.length > 0 && vals.every((x) => x !== null && typeof x === "object" && !(x instanceof Date) && typeof (x as ArrayLike<unknown>).length === "number");
    cols = columnar ? (batch as Record<string, ValuesInput>) : rowsToColumns([batch as Row]);
  }
  let k: number | null = null;
  for (const [name, v] of Object.entries(cols)) {
    if (!table.columns.some((c) => c.name === name)) throw new DataError(`append: no column \`${name}\` in table \`${tname}\``);
    if (k === null) k = v.length;
    else if (v.length !== k) throw new DataError(`append: column \`${name}\` has ${v.length} values, not ${k}`);
  }
  const added = k ?? 0;
  const n = table.length ?? 0;
  const columns: ColumnDesc[] = [];
  const sources = prev.sources.slice();
  table.columns.forEach((c, i) => {
    const v: ValuesInput = Object.hasOwn(cols, c.name) ? (cols[c.name] as ValuesInput) : new Array<null>(added).fill(null);
    const out: ColumnDesc = { ...c };
    let tail: Source | null = null;
    if (c.type === "string") {
      const before = (c.values as ArrayLike<Scalar> | undefined) ?? new Array<null>(n).fill(null);
      const more = Array.from({ length: added }, (_, r) => {
        const s = (v as ArrayLike<unknown>)[r];
        return s === null || s === undefined ? null : nfc(String(s));
      });
      out.values = [...Array.from(before), ...more];
    } else if (c.type === "category") {
      const labels = (c.categories ?? []).slice();
      const index = new Map(labels.map((l, j) => [l, j] as const));
      const codes = new Uint32Array(added);
      for (let r = 0; r < added; r++) {
        const s = (v as ArrayLike<unknown>)[r];
        if (s === null || s === undefined) codes[r] = MISSING_CODE;
        else if (typeof s === "number") codes[r] = s;
        else {
          const l = nfc(String(s));
          let code = index.get(l);
          if (code === undefined) {
            code = labels.length;
            labels.push(l);
            index.set(l, code);
          }
          codes[r] = code;
        }
      }
      out.categories = labels;
      tail = typedSource(ColumnKind.U32, codes);
    } else if (c.type === "timestamp" && hasOrigin(c.time_unit) && c.origin !== undefined) {
      // Offsets from the column's origin, as its head holds them (REQ-DATA-020).
      const origin = originOf(c);
      // Appended values are instants, read exactly when they can be.
      tail = offsetSource(exactValues(c.name, c.time_unit, wantsExact(v, added) ? v : roundedOrMissing(v, added), added, 0n), origin);
    } else {
      tail = isTyped(v) ? typedSource(ColumnKind.F64, v) : jsonF64(c.name, c.type, c.time_unit ?? "ms", v as ArrayLike<unknown>, added);
    }
    if (tail) {
      const head = sources[i] ?? missingSource(tail.kind, n);
      sources[i] = concatSource(head, tail);
    }
    columns.push(out);
  });
  const descriptor: DataObject = { ...prev.descriptor, tables: { ...prev.descriptor.tables, [tname]: { ...table, length: n + added, columns } } };
  return { descriptor, sources };
}

/** Numbers as whole units (rounded), anything not a finite number missing: the inexact values a plain array or a Float64Array holds. */
function roundedOrMissing(v: ValuesInput, length: number): (number | null)[] {
  return Array.from({ length }, (_, i) => {
    const x = (v as ArrayLike<unknown>)[i];
    return typeof x === "number" && Number.isFinite(x) ? Math.round(x) : null;
  });
}

function missingSource(kind: 0 | 1, length: number): Source {
  return { kind, length, write: (dst) => dst.fill(kind === ColumnKind.U32 ? MISSING_CODE : NaN, 0, length) };
}
