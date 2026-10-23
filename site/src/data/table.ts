// Building the data contract's full form (spec/schemas/data.schema.json) from
// columns, so generated timestamps cross as timestamps (epoch ms in a
// Float64Array) without a Date a row, and units ride on their columns
// (REQ-DATA-100).

import type { DataObject } from "glyphplot";

/** One column of the full form. */
export interface Col {
  name: string;
  /** Default `f64`. */
  type?: "f64" | "timestamp" | "duration" | "category" | "string";
  unit?: string;
  role?: string;
  values: ArrayLike<number | string | null>;
}

/** The full form of one table named `name`. */
export function table(name: string, cols: Col[]): DataObject {
  return {
    version: 1,
    tables: {
      [name]: {
        columns: cols.map((c) => ({
          name: c.name,
          type: c.type ?? "f64",
          ...(c.unit ? { unit: c.unit } : {}),
          ...(c.role ? { role: c.role } : {}),
          values: c.values as ArrayLike<number>,
        })),
      },
    },
  };
}

/** `n` instants from `start`, `step` ms apart, as epoch ms. */
export function times(start: number, n: number, step: number): Float64Array {
  const t = new Float64Array(n);
  for (let i = 0; i < n; i++) t[i] = start + i * step;
  return t;
}

/** The epoch every generated series in the gallery starts from: 2026-10-01T00:00:00Z. */
export const EPOCH = Date.UTC(2026, 9, 1);

export const SECOND = 1000;
export const MINUTE = 60 * SECOND;
export const HOUR = 60 * MINUTE;
export const DAY = 24 * HOUR;
