// Block, quadrant, sextant, octant and wedge glyphs as exact shapes, filled
// on the canvas instead of drawn from the font, so they tile without seams
// whatever the font's metrics (REQ-RENDER-020). Each shape is computed from
// the character's definition in the Unicode Standard (16.0.0): the Block
// Elements chart (U+2580–259F), Symbols for Legacy Computing (U+1FB00–1FBAF:
// sextants by the dot numbers in their names, the smooth mosaic wedges by the
// diagonal their names give, the eighth blocks), and Symbols for Legacy
// Computing Supplement (U+1CD00–1CDE5 octants by their names' dot numbers,
// U+1CEA0–1CEAF quarter blocks). Coordinates are fractions of the cell, x
// rightward and y downward; the cell is twice as tall as wide.

/** An axis-aligned rectangle [x0, y0, x1, y1] in cell fractions. */
export type Rect = readonly [number, number, number, number];
/** A polygon's vertices [x, y] in cell fractions. */
export type Poly = readonly (readonly [number, number])[];

/** A glyph's exact shape: rectangles and polygons filled in the ink, at `alpha` (the shades). */
export interface Shape {
  readonly rects: readonly Rect[];
  readonly polys: readonly Poly[];
  readonly alpha: number;
}

const shapes = new Map<number, Shape>();
const put = (cp: number, rects: Rect[], polys: Poly[] = [], alpha = 1): void => {
  shapes.set(cp, { rects, polys, alpha });
};

/**
 * Rectangles covering the lit dots of a `cols × rows` dot grid, dot `d`
 * (1-based, row by row) lit when bit d − 1 of `mask` is set: each row's runs
 * of lit dots, merged downward where the run below is the same, so shared
 * edges never meet inside a filled area.
 */
export function dotRects(mask: number, cols: number, rows: number): Rect[] {
  const runs: { c0: number; c1: number; r0: number; r1: number }[] = [];
  let open: { c0: number; c1: number; r0: number; r1: number }[] = [];
  for (let r = 0; r < rows; r++) {
    const next: typeof open = [];
    let c = 0;
    while (c < cols) {
      if (!(mask & (1 << (r * cols + c)))) {
        c++;
        continue;
      }
      const c0 = c;
      while (c < cols && mask & (1 << (r * cols + c))) c++;
      const above = open.find((o) => o.c0 === c0 && o.c1 === c);
      if (above) {
        above.r1 = r + 1;
        next.push(above);
      } else {
        const run = { c0, c1: c, r0: r, r1: r + 1 };
        runs.push(run);
        next.push(run);
      }
    }
    open = next;
  }
  return runs.map((o) => [o.c0 / cols, o.r0 / rows, o.c1 / cols, o.r1 / rows] as const);
}

// ---------------------------------------------------------------- Block Elements U+2580–259F

put(0x2580, [[0, 0, 1, 1 / 2]]); // UPPER HALF BLOCK
for (let k = 1; k <= 7; k++) put(0x2580 + k, [[0, 1 - k / 8, 1, 1]]); // LOWER k/8 BLOCK
put(0x2588, [[0, 0, 1, 1]]); // FULL BLOCK
for (let cp = 0x2589; cp <= 0x258f; cp++) put(cp, [[0, 0, (0x2590 - cp) / 8, 1]]); // LEFT 7/8 … 1/8 BLOCK
put(0x2590, [[1 / 2, 0, 1, 1]]); // RIGHT HALF BLOCK
put(0x2591, [[0, 0, 1, 1]], [], 1 / 4); // LIGHT SHADE
put(0x2592, [[0, 0, 1, 1]], [], 2 / 4); // MEDIUM SHADE
put(0x2593, [[0, 0, 1, 1]], [], 3 / 4); // DARK SHADE
put(0x2594, [[0, 0, 1, 1 / 8]]); // UPPER ONE EIGHTH BLOCK
put(0x2595, [[7 / 8, 0, 1, 1]]); // RIGHT ONE EIGHTH BLOCK

/** The quadrants U+2596–259F by name, as 2 × 2 dot masks (upper left 1, upper right 2, lower left 4, lower right 8). */
export const QUADRANTS: readonly (readonly [number, string, number])[] = [
  [0x2596, "QUADRANT LOWER LEFT", 4],
  [0x2597, "QUADRANT LOWER RIGHT", 8],
  [0x2598, "QUADRANT UPPER LEFT", 1],
  [0x2599, "QUADRANT UPPER LEFT AND LOWER LEFT AND LOWER RIGHT", 1 | 4 | 8],
  [0x259a, "QUADRANT UPPER LEFT AND LOWER RIGHT", 1 | 8],
  [0x259b, "QUADRANT UPPER LEFT AND UPPER RIGHT AND LOWER LEFT", 1 | 2 | 4],
  [0x259c, "QUADRANT UPPER LEFT AND UPPER RIGHT AND LOWER RIGHT", 1 | 2 | 8],
  [0x259d, "QUADRANT UPPER RIGHT", 2],
  [0x259e, "QUADRANT UPPER RIGHT AND LOWER LEFT", 2 | 4],
  [0x259f, "QUADRANT UPPER RIGHT AND LOWER LEFT AND LOWER RIGHT", 2 | 4 | 8],
];
for (const [cp, , mask] of QUADRANTS) put(cp, dotRects(mask, 2, 2));

// ---------------------------------------------------------------- sextants U+1FB00–1FB3B

/**
 * The 2 × 3 dot masks BLOCK SEXTANT-<digits> leaves out, as other characters
 * draw them: none, the left half (dots 135), the right half (246), full.
 */
export const SEXTANT_GAPS: readonly number[] = [0, 21, 42, 63];

/**
 * The dot mask of each BLOCK SEXTANT, U+1FB00 onward: the code points take
 * the masks 1 to 62 in increasing order, skipping {@link SEXTANT_GAPS}
 * (U+1FB00 BLOCK SEXTANT-1, U+1FB3B BLOCK SEXTANT-23456).
 */
export const SEXTANT_MASKS: readonly number[] = Array.from({ length: 64 }, (_, m) => m).filter((m) => !SEXTANT_GAPS.includes(m));
SEXTANT_MASKS.forEach((mask, i) => put(0x1fb00 + i, dotRects(mask, 2, 3)));

// ---------------------------------------------------------------- octants U+1CD00–1CDE5

/**
 * The 2 × 4 dot masks BLOCK OCTANT-<digits> leaves out because older
 * characters draw them (space, the halves, the quarter and three-quarter
 * blocks, the quadrants, the full block): 26 of the 256.
 */
export const OCTANT_GAPS: readonly number[] = [0, 1, 2, 3, 5, 10, 15, 20, 40, 63, 64, 80, 85, 90, 95, 128, 160, 165, 170, 175, 192, 240, 245, 250, 252, 255];

/**
 * The dot mask of each BLOCK OCTANT, U+1CD00 onward: the 230 code points
 * take the remaining masks in increasing order (U+1CD00 BLOCK OCTANT-3,
 * U+1CDE5 BLOCK OCTANT-2345678).
 */
export const OCTANT_MASKS: readonly number[] = Array.from({ length: 256 }, (_, m) => m).filter((m) => !OCTANT_GAPS.includes(m));
OCTANT_MASKS.forEach((mask, i) => put(0x1cd00 + i, dotRects(mask, 2, 4)));

// ---------------------------------------------------------------- quarter blocks U+1CEA0–1CEAF

/** U+1CEA0–1CEAF by name: a half or three quarters of one side, one quarter deep. */
const QUARTERS: readonly Rect[] = [
  [1 / 2, 3 / 4, 1, 1], // RIGHT HALF LOWER ONE QUARTER BLOCK
  [1 / 4, 3 / 4, 1, 1], // RIGHT THREE QUARTERS LOWER ONE QUARTER BLOCK
  [0, 3 / 4, 3 / 4, 1], // LEFT THREE QUARTERS LOWER ONE QUARTER BLOCK
  [0, 3 / 4, 1 / 2, 1], // LEFT HALF LOWER ONE QUARTER BLOCK
  [0, 1 / 2, 1 / 4, 1], // LOWER HALF LEFT ONE QUARTER BLOCK
  [0, 1 / 4, 1 / 4, 1], // LOWER THREE QUARTERS LEFT ONE QUARTER BLOCK
  [0, 0, 1 / 4, 3 / 4], // UPPER THREE QUARTERS LEFT ONE QUARTER BLOCK
  [0, 0, 1 / 4, 1 / 2], // UPPER HALF LEFT ONE QUARTER BLOCK
  [0, 0, 1 / 2, 1 / 4], // LEFT HALF UPPER ONE QUARTER BLOCK
  [0, 0, 3 / 4, 1 / 4], // LEFT THREE QUARTERS UPPER ONE QUARTER BLOCK
  [1 / 4, 0, 1, 1 / 4], // RIGHT THREE QUARTERS UPPER ONE QUARTER BLOCK
  [1 / 2, 0, 1, 1 / 4], // RIGHT HALF UPPER ONE QUARTER BLOCK
  [3 / 4, 0, 1, 1 / 2], // UPPER HALF RIGHT ONE QUARTER BLOCK
  [3 / 4, 0, 1, 3 / 4], // UPPER THREE QUARTERS RIGHT ONE QUARTER BLOCK
  [3 / 4, 1 / 4, 1, 1], // LOWER THREE QUARTERS RIGHT ONE QUARTER BLOCK
  [3 / 4, 1 / 2, 1, 1], // LOWER HALF RIGHT ONE QUARTER BLOCK
];
QUARTERS.forEach((r, i) => put(0x1cea0 + i, [r]));

// ---------------------------------------------------------------- wedges U+1FB3C–1FB67

/** The points the smooth mosaic names use: the sextant grid's edge points. */
const POINTS: Record<string, readonly [number, number]> = {
  "UPPER LEFT": [0, 0],
  "UPPER MIDDLE LEFT": [0, 1 / 3],
  "LOWER MIDDLE LEFT": [0, 2 / 3],
  "LOWER LEFT": [0, 1],
  "UPPER CENTRE": [1 / 2, 0],
  "LOWER CENTRE": [1 / 2, 1],
  "UPPER RIGHT": [1, 0],
  "UPPER MIDDLE RIGHT": [1, 1 / 3],
  "LOWER MIDDLE RIGHT": [1, 2 / 3],
  "LOWER RIGHT": [1, 1],
};

/** U+1FB3C–1FB67, the Unicode names: `<corner> BLOCK DIAGONAL <from> TO <to>`. */
export const WEDGE_NAMES: readonly string[] = [
  "LOWER LEFT BLOCK DIAGONAL LOWER MIDDLE LEFT TO LOWER CENTRE",
  "LOWER LEFT BLOCK DIAGONAL LOWER MIDDLE LEFT TO LOWER RIGHT",
  "LOWER LEFT BLOCK DIAGONAL UPPER MIDDLE LEFT TO LOWER CENTRE",
  "LOWER LEFT BLOCK DIAGONAL UPPER MIDDLE LEFT TO LOWER RIGHT",
  "LOWER LEFT BLOCK DIAGONAL UPPER LEFT TO LOWER CENTRE",
  "LOWER RIGHT BLOCK DIAGONAL UPPER MIDDLE LEFT TO UPPER CENTRE",
  "LOWER RIGHT BLOCK DIAGONAL UPPER MIDDLE LEFT TO UPPER RIGHT",
  "LOWER RIGHT BLOCK DIAGONAL LOWER MIDDLE LEFT TO UPPER CENTRE",
  "LOWER RIGHT BLOCK DIAGONAL LOWER MIDDLE LEFT TO UPPER RIGHT",
  "LOWER RIGHT BLOCK DIAGONAL LOWER LEFT TO UPPER CENTRE",
  "LOWER RIGHT BLOCK DIAGONAL LOWER MIDDLE LEFT TO UPPER MIDDLE RIGHT",
  "LOWER RIGHT BLOCK DIAGONAL LOWER CENTRE TO LOWER MIDDLE RIGHT",
  "LOWER RIGHT BLOCK DIAGONAL LOWER LEFT TO LOWER MIDDLE RIGHT",
  "LOWER RIGHT BLOCK DIAGONAL LOWER CENTRE TO UPPER MIDDLE RIGHT",
  "LOWER RIGHT BLOCK DIAGONAL LOWER LEFT TO UPPER MIDDLE RIGHT",
  "LOWER RIGHT BLOCK DIAGONAL LOWER CENTRE TO UPPER RIGHT",
  "LOWER LEFT BLOCK DIAGONAL UPPER CENTRE TO UPPER MIDDLE RIGHT",
  "LOWER LEFT BLOCK DIAGONAL UPPER LEFT TO UPPER MIDDLE RIGHT",
  "LOWER LEFT BLOCK DIAGONAL UPPER CENTRE TO LOWER MIDDLE RIGHT",
  "LOWER LEFT BLOCK DIAGONAL UPPER LEFT TO LOWER MIDDLE RIGHT",
  "LOWER LEFT BLOCK DIAGONAL UPPER CENTRE TO LOWER RIGHT",
  "LOWER LEFT BLOCK DIAGONAL UPPER MIDDLE LEFT TO LOWER MIDDLE RIGHT",
  "UPPER RIGHT BLOCK DIAGONAL LOWER MIDDLE LEFT TO LOWER CENTRE",
  "UPPER RIGHT BLOCK DIAGONAL LOWER MIDDLE LEFT TO LOWER RIGHT",
  "UPPER RIGHT BLOCK DIAGONAL UPPER MIDDLE LEFT TO LOWER CENTRE",
  "UPPER RIGHT BLOCK DIAGONAL UPPER MIDDLE LEFT TO LOWER RIGHT",
  "UPPER RIGHT BLOCK DIAGONAL UPPER LEFT TO LOWER CENTRE",
  "UPPER LEFT BLOCK DIAGONAL UPPER MIDDLE LEFT TO UPPER CENTRE",
  "UPPER LEFT BLOCK DIAGONAL UPPER MIDDLE LEFT TO UPPER RIGHT",
  "UPPER LEFT BLOCK DIAGONAL LOWER MIDDLE LEFT TO UPPER CENTRE",
  "UPPER LEFT BLOCK DIAGONAL LOWER MIDDLE LEFT TO UPPER RIGHT",
  "UPPER LEFT BLOCK DIAGONAL LOWER LEFT TO UPPER CENTRE",
  "UPPER LEFT BLOCK DIAGONAL LOWER MIDDLE LEFT TO UPPER MIDDLE RIGHT",
  "UPPER LEFT BLOCK DIAGONAL LOWER CENTRE TO LOWER MIDDLE RIGHT",
  "UPPER LEFT BLOCK DIAGONAL LOWER LEFT TO LOWER MIDDLE RIGHT",
  "UPPER LEFT BLOCK DIAGONAL LOWER CENTRE TO UPPER MIDDLE RIGHT",
  "UPPER LEFT BLOCK DIAGONAL LOWER LEFT TO UPPER MIDDLE RIGHT",
  "UPPER LEFT BLOCK DIAGONAL LOWER CENTRE TO UPPER RIGHT",
  "UPPER RIGHT BLOCK DIAGONAL UPPER CENTRE TO UPPER MIDDLE RIGHT",
  "UPPER RIGHT BLOCK DIAGONAL UPPER LEFT TO UPPER MIDDLE RIGHT",
  "UPPER RIGHT BLOCK DIAGONAL UPPER CENTRE TO LOWER MIDDLE RIGHT",
  "UPPER RIGHT BLOCK DIAGONAL UPPER LEFT TO LOWER MIDDLE RIGHT",
  "UPPER RIGHT BLOCK DIAGONAL UPPER CENTRE TO LOWER RIGHT",
  "UPPER RIGHT BLOCK DIAGONAL UPPER MIDDLE LEFT TO LOWER MIDDLE RIGHT",
];

const UNIT: Poly = [
  [0, 0],
  [1, 0],
  [1, 1],
  [0, 1],
];

/** The part of `poly` on the side of the line p→q where `keep` lies (Sutherland–Hodgman, one edge). */
export function clipHalf(poly: Poly, p: readonly [number, number], q: readonly [number, number], keep: readonly [number, number]): Poly {
  const side = (v: readonly [number, number]): number => (q[0] - p[0]) * (v[1] - p[1]) - (q[1] - p[1]) * (v[0] - p[0]);
  const sign = Math.sign(side(keep));
  const inside = (v: readonly [number, number]): boolean => side(v) * sign >= -1e-12;
  const out: [number, number][] = [];
  for (let i = 0; i < poly.length; i++) {
    const a = poly[i] as readonly [number, number];
    const b = poly[(i + 1) % poly.length] as readonly [number, number];
    const ia = inside(a);
    const ib = inside(b);
    if (ia) out.push([a[0], a[1]]);
    if (ia !== ib) {
      const sa = side(a);
      const sb = side(b);
      const t = sa / (sa - sb);
      out.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
    }
  }
  return out;
}

/** A wedge's polygon from its Unicode name, or null when the name is not of that form. */
export function wedgeFromName(name: string): Poly | null {
  const m = /^(UPPER|LOWER) (LEFT|RIGHT) BLOCK DIAGONAL (.+) TO (.+)$/.exec(name);
  if (!m) return null;
  const corner = POINTS[`${m[1]} ${m[2]}`];
  const p = POINTS[m[3] as string];
  const q = POINTS[m[4] as string];
  if (!corner || !p || !q) return null;
  return clipHalf(UNIT, p, q, corner);
}

WEDGE_NAMES.forEach((name, i) => {
  const poly = wedgeFromName(name);
  if (poly) put(0x1fb3c + i, [], [poly]);
});

// ---------------------------------------------------------------- triangular blocks U+1FB68–1FB6F, U+1FB9A–1FB9B

const C: readonly [number, number] = [1 / 2, 1 / 2];
const TRI = {
  left: [[0, 0], C, [0, 1]] as Poly,
  upper: [[0, 0], [1, 0], C] as Poly,
  right: [[1, 0], [1, 1], C] as Poly,
  lower: [[1, 1], [0, 1], C] as Poly,
};
put(0x1fb68, [], [[[0, 0], [1, 0], [1, 1], [0, 1], C]]); // UPPER AND RIGHT AND LOWER TRIANGULAR THREE QUARTERS BLOCK
put(0x1fb69, [], [[[0, 0], C, [1, 0], [1, 1], [0, 1]]]); // LEFT AND LOWER AND RIGHT TRIANGULAR THREE QUARTERS BLOCK
put(0x1fb6a, [], [[[0, 0], [1, 0], C, [1, 1], [0, 1]]]); // UPPER AND LEFT AND LOWER TRIANGULAR THREE QUARTERS BLOCK
put(0x1fb6b, [], [[[0, 0], [1, 0], [1, 1], C, [0, 1]]]); // LEFT AND UPPER AND RIGHT TRIANGULAR THREE QUARTERS BLOCK
put(0x1fb6c, [], [TRI.left]); // LEFT TRIANGULAR ONE QUARTER BLOCK
put(0x1fb6d, [], [TRI.upper]); // UPPER TRIANGULAR ONE QUARTER BLOCK
put(0x1fb6e, [], [TRI.right]); // RIGHT TRIANGULAR ONE QUARTER BLOCK
put(0x1fb6f, [], [TRI.lower]); // LOWER TRIANGULAR ONE QUARTER BLOCK
put(0x1fb9a, [], [TRI.upper, TRI.lower]); // UPPER AND LOWER TRIANGULAR HALF BLOCK
put(0x1fb9b, [], [TRI.left, TRI.right]); // LEFT AND RIGHT TRIANGULAR HALF BLOCK

// ---------------------------------------------------------------- eighth blocks U+1FB70–1FB8B

for (let n = 2; n <= 7; n++) put(0x1fb70 + n - 2, [[(n - 1) / 8, 0, n / 8, 1]]); // VERTICAL ONE EIGHTH BLOCK-n
for (let n = 2; n <= 7; n++) put(0x1fb76 + n - 2, [[0, (n - 1) / 8, 1, n / 8]]); // HORIZONTAL ONE EIGHTH BLOCK-n
const E = 1 / 8;
put(0x1fb7c, [[0, 0, E, 1], [E, 1 - E, 1, 1]]); // LEFT AND LOWER ONE EIGHTH BLOCK
put(0x1fb7d, [[0, 0, 1, E], [0, E, E, 1]]); // LEFT AND UPPER ONE EIGHTH BLOCK
put(0x1fb7e, [[0, 0, 1, E], [1 - E, E, 1, 1]]); // RIGHT AND UPPER ONE EIGHTH BLOCK
put(0x1fb7f, [[1 - E, 0, 1, 1 - E], [0, 1 - E, 1, 1]]); // RIGHT AND LOWER ONE EIGHTH BLOCK
put(0x1fb80, [[0, 0, 1, E], [0, 1 - E, 1, 1]]); // UPPER AND LOWER ONE EIGHTH BLOCK
put(0x1fb81, [1, 3, 5, 8].map((n) => [0, (n - 1) / 8, 1, n / 8] as Rect)); // HORIZONTAL ONE EIGHTH BLOCK-1358
[2, 3, 5, 6, 7].forEach((k, i) => put(0x1fb82 + i, [[0, 0, 1, k / 8]])); // UPPER 1/4, 3/8, 5/8, 3/4, 7/8 BLOCK
[2, 3, 5, 6, 7].forEach((k, i) => put(0x1fb87 + i, [[1 - k / 8, 0, 1, 1]])); // RIGHT 1/4, 3/8, 5/8, 3/4, 7/8 BLOCK

// ---------------------------------------------------------------- lookup and fill

/** The exact shape of `cp`, or undefined when the font draws it. */
export function blockShape(cp: number): Shape | undefined {
  return shapes.get(cp);
}

/** Whether `cp` is filled as a shape rather than drawn from the font. */
export function isBlock(cp: number): boolean {
  return shapes.has(cp);
}

/** Every code point with a shape, ascending. */
export function blockCodePoints(): number[] {
  return [...shapes.keys()].sort((a, b) => a - b);
}

/** The area of a shape in cell fractions (overlaps counted once only for rectangles and polygons that do not overlap). */
export function shapeArea(s: Shape): number {
  let a = 0;
  for (const [x0, y0, x1, y1] of s.rects) a += (x1 - x0) * (y1 - y0);
  for (const p of s.polys) {
    let twice = 0;
    for (let i = 0; i < p.length; i++) {
      const u = p[i] as readonly [number, number];
      const v = p[(i + 1) % p.length] as readonly [number, number];
      twice += u[0] * v[1] - v[0] * u[1];
    }
    a += Math.abs(twice) / 2;
  }
  return a;
}

/** The 2D context methods {@link fillBlock} uses. */
export interface FillContext {
  globalAlpha: number;
  fillRect(x: number, y: number, w: number, h: number): void;
  beginPath(): void;
  moveTo(x: number, y: number): void;
  lineTo(x: number, y: number): void;
  closePath(): void;
  fill(): void;
}

/**
 * Fills `cp`'s shape in the current fill style over the cell at device
 * pixels (x, y), w × h; rectangle edges are rounded to whole pixels, so
 * neighbouring shapes meet without gaps or overlaps. Returns false, drawing
 * nothing, when `cp` has no shape.
 */
export function fillBlock(ctx: FillContext, cp: number, x: number, y: number, w: number, h: number): boolean {
  const s = shapes.get(cp);
  if (!s) return false;
  const alpha = ctx.globalAlpha;
  if (s.alpha !== 1) ctx.globalAlpha = alpha * s.alpha;
  for (const [x0, y0, x1, y1] of s.rects) {
    const l = Math.round(x0 * w);
    const t = Math.round(y0 * h);
    ctx.fillRect(x + l, y + t, Math.round(x1 * w) - l, Math.round(y1 * h) - t);
  }
  if (s.polys.length > 0) {
    ctx.beginPath();
    for (const p of s.polys) {
      p.forEach(([px, py], i) => (i === 0 ? ctx.moveTo(x + px * w, y + py * h) : ctx.lineTo(x + px * w, y + py * h)));
      ctx.closePath();
    }
    ctx.fill();
  }
  if (s.alpha !== 1) ctx.globalAlpha = alpha;
  return true;
}
