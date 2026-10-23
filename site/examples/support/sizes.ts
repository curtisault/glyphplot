// The grids a card and an example's page draw at, shared by `<gp-example>`
// (site/examples/element.ts) and the build-time stills
// (scripts/examples-stills.mjs, REQ-SITE-120), so a still has the rows of the
// chart that replaces it. A chart's columns follow its box; the still is
// drawn at `cols` and clipped or padded to the box.

export interface Grid {
  cols: number;
  rows: number;
}

/** A gallery card. */
export const CARD: Grid = { cols: 64, rows: 16 };

/** An example's own page (`<gp-example full>`). */
export const FULL: Grid = { cols: 120, rows: 28 };

/** The grid for an example's attributes (`cols`, `rows`) at a size. */
export function gridFor(attrs: Record<string, string> | undefined, size: Grid): Grid {
  const n = (v: string | undefined): number | null => {
    const x = v === undefined ? Number.NaN : Number.parseInt(v, 10);
    return Number.isFinite(x) && x > 0 ? x : null;
  };
  const rows = n(attrs?.["rows"]);
  const cols = n(attrs?.["cols"]);
  // A page's chart is drawn bigger than a card's, keeping a fixed grid as it is.
  return { cols: cols ?? size.cols, rows: rows === null ? size.rows : cols !== null || size === CARD ? rows : Math.round((rows * size.rows) / CARD.rows) };
}
