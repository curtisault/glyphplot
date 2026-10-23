// The real datasets the gallery draws, as checked in under
// crates/glyphplot/tests/data/ (REQ-FIX-020), in the data contract's full
// form: their sources, licences and retrieval are in that folder's README,
// and each has an entry in datasets/manifest.json (REQ-DATASET-010) whose id
// the examples name in site/examples/manifest.json. Nothing here
// is edited or generated: an example derives from these (a daily change, an
// hourly count) only by arithmetic it shows in its source.

import treasuryJson from "../../../crates/glyphplot/tests/data/treasury-par-yield-2026.json" with { type: "json" };
import usgsJson from "../../../crates/glyphplot/tests/data/usgs-earthquakes-day.json" with { type: "json" };
import wtiJson from "../../../crates/glyphplot/tests/data/wti-daily.json" with { type: "json" };

/** A checked-in dataset: one table of named columns. */
export interface RealTable {
  version: 1;
  tables: Record<string, { sorted?: string; columns: { name: string; type: string; role?: string; unit?: string; values: (number | string | null)[] }[] }>;
}

/** U.S. EIA, WTI spot price, daily, 2025-10-15 to 2026-10-14: `date`, `price` (USD/bbl). */
export const wti = wtiJson as unknown as RealTable;
/** U.S. Treasury, daily par yield curve, 2026-01-02 to 2026-10-20: `date` and a column a maturity, `m1` to `y30` (%). */
export const treasury = treasuryJson as unknown as RealTable;
/** USGS, every event of the day to 2026-10-20T20:04Z: `time`, `mag`, `depth`, `latitude`, `longitude`, `place`. */
export const usgsDay = usgsJson as unknown as RealTable;

/** The values of column `name` of the dataset's one table. */
export function col<T = number>(ds: RealTable, name: string): T[] {
  const table = Object.values(ds.tables)[0];
  const c = table?.columns.find((x) => x.name === name);
  if (!c) throw new Error(`no column ${name}`);
  return c.values as T[];
}

/** The Treasury's maturities: column name and years. */
export const MATURITIES: readonly [string, number][] = [
  ["m1", 1 / 12],
  ["m1.5", 1.5 / 12],
  ["m2", 2 / 12],
  ["m3", 3 / 12],
  ["m4", 4 / 12],
  ["m6", 6 / 12],
  ["y1", 1],
  ["y2", 2],
  ["y3", 3],
  ["y5", 5],
  ["y7", 7],
  ["y10", 10],
  ["y20", 20],
  ["y30", 30],
];

/** The yield curve on row `i` of the Treasury table: maturity in years and yield in %, maturities with no value left out. */
export function curveOn(i: number): { maturity: number[]; yield: number[] } {
  const maturity: number[] = [];
  const yields: number[] = [];
  for (const [name, years] of MATURITIES) {
    const v = col<number | null>(treasury, name)[i];
    if (v === null || v === undefined) continue;
    maturity.push(years);
    yields.push(v);
  }
  return { maturity, yield: yields };
}
