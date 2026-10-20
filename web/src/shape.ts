// The data's shape (REQ-EL-050, REQ-DATA-110): which of a chart's shapes
// (meta.schema.json `shapes`) a data set is, decided as the core's
// `Data::shape` decides it: the first shape whose channels all resolve, by
// `encode`, a column's role or a column's name, else the first whose kind the
// data is. A type change carries the old chart's data to the new chart when
// the new chart's meta lists the shape the old chart took it as.

import type { DataObject, TableDesc } from "./data.ts";

/** The `encode` option: channel to column name, or names. */
export type Encode = Record<string, string | readonly string[]>;

/** Each shape's channels, as alternatives (the core's `Shape::channels`). */
export const SHAPE_CHANNELS: Readonly<Record<string, readonly (readonly string[])[]>> = {
  series: [["x", "y"]],
  ohlcv: [["x", "open", "high", "low", "close"]],
  values: [["value"]],
  bins: [["lo", "hi", "count"]],
  counts: [["category", "value"]],
  points: [["x", "y"]],
  matrix: [[]],
  graph: [["source", "target"]],
  geo: [["lon", "lat"]],
  sky: [
    ["ra", "dec"],
    ["az", "alt"],
  ],
  orbits: [["a", "e", "i", "node", "peri", "anomaly", "epoch"]],
  ephemeris: [
    ["x", "ra", "dec"],
    ["x", "y", "z"],
  ],
};

/** The `encode` object of a chart's options; `{}` when it has none or it is not an object. */
export function encodeOf(options: Record<string, unknown>): Encode {
  const e = options["encode"];
  return e !== null && typeof e === "object" && !Array.isArray(e) ? (e as Encode) : {};
}

/** Whether `channel` resolves in `t`: the columns `encode` names (all present), else a column with that role, else a column of that name. */
function resolves(t: TableDesc, channel: string, encode: Encode): boolean {
  const named = Object.hasOwn(encode, channel) ? encode[channel] : undefined;
  const names = named === undefined ? [] : typeof named === "string" ? [named] : named;
  if (names.length > 0) return names.every((n) => t.columns.some((c) => c.name === n));
  return t.columns.some((c) => c.role === channel || c.name === channel);
}

const firstTable = (d: DataObject): TableDesc | undefined => Object.values(d.tables)[0];

function isKind(d: DataObject, shape: string): boolean {
  if (shape === "matrix") return Object.keys(d.matrices ?? {}).length > 0;
  if (shape === "graph") return Object.hasOwn(d.tables, "nodes") && Object.hasOwn(d.tables, "edges");
  return Object.keys(d.tables).length > 0;
}

function fits(d: DataObject, shape: string, encode: Encode): boolean {
  if (shape === "matrix") return isKind(d, shape);
  const t = shape === "graph" ? (Object.hasOwn(d.tables, "nodes") ? d.tables["edges"] : undefined) : firstTable(d);
  if (!t) return false;
  return (SHAPE_CHANNELS[shape] ?? []).some((set) => set.every((ch) => resolves(t, ch, encode)));
}

/**
 * The shape of `accepted` that `data` is, as the core picks it: the first
 * whose channels all resolve, else the first whose kind it is; null when it is
 * none of them (the core's data error).
 */
export function shapeOf(data: DataObject, accepted: readonly string[], encode: Encode = {}): string | null {
  return accepted.find((s) => fits(data, s, encode)) ?? accepted.find((s) => isKind(data, s)) ?? null;
}
