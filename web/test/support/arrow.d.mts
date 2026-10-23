// Types of arrow.mjs, for the site's TypeScript (site/vite.config.ts, site/e2e).

/** A column of {@link arrowStream}: float64, int64, or a millisecond timestamp (UTC). */
export interface ArrowColumn {
  type: "f64" | "i64" | "timestamp";
  values: ArrayLike<number>;
}

/** An Arrow IPC stream of one record batch of `columns`, every one the same length, none with nulls. */
export function arrowStream(columns: Record<string, ArrowColumn>): Uint8Array;

/** `n` rows of t (ms from 2026-01-01, a minute apart) and y. */
export function walk(n?: number): { t: number[]; y: number[] };
