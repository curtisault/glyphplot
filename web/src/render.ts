// Drawing a grid on a canvas: only the cells whose glyph, ink or back changed
// since the last frame drawn (REQ-RENDER-030), block, sextant, octant and
// wedge glyphs as exact shapes (REQ-RENDER-020), colours from the palette
// resolved once a theme (REQ-COLOR-030). Two renderers behind one flag
// (REQ-RENDER-050), whose cost does not depend on the number of distinct
// (glyph, colour) pairs (REQ-RENDER-010, ADR-119):
//
// - `text`: fillText per changed cell, in its ink (the default);
// - `atlas`: white glyph masks from an atlas keyed by code point alone,
//   tinted by compositing a one-pixel-a-cell colour image over them.
//
// The owner measures both on Chrome and Safari and the plan records the
// choice; until then `text` is the default.

import { NO_BACK, PALETTE_LEN, Token, WIDE_TAIL } from "./api.ts";
import { fillBlock, isBlock } from "./blocks.ts";
import { paletteCss } from "./theme.ts";
import type { GridViews } from "./wasm.ts";

export type RendererName = "text" | "atlas";

/** The font stack every surface puts first (REQ-FONT-030). */
export const FONT_STACK = '"glyphplot mono", ui-monospace, monospace';

/** The cells a frame redrew, for tests and the debug overlay. */
export interface DrawStats {
  cells: number;
  full: boolean;
}

/** Marks cell `i` and, around double-width glyphs, its partner. */
function markWide(dirty: Uint8Array, cells: Uint32Array, cols: number): void {
  const n = dirty.length;
  for (let i = 0; i < n; i++) {
    if (dirty[i] !== 1) continue;
    const col = i % cols;
    if (cells[i] === WIDE_TAIL && col > 0 && dirty[i - 1] === 0) dirty[i - 1] = 2;
    if (col + 1 < cols && cells[i + 1] === WIDE_TAIL && dirty[i + 1] === 0) dirty[i + 1] = 2;
  }
}

/**
 * The cells to redraw: 1 where glyph, ink or back differ from the previous
 * frame (or every cell when `full`), 2 for a double-width glyph's partner.
 * Pure, for tests.
 */
export function diffCells(cur: Pick<GridViews, "cells" | "colors" | "backs" | "cols">, prev: Pick<GridViews, "cells" | "colors" | "backs"> | null, full: boolean): Uint8Array {
  const n = cur.cells.length;
  const dirty = new Uint8Array(n);
  if (full || !prev || prev.cells.length !== n) dirty.fill(1);
  else {
    for (let i = 0; i < n; i++) {
      if (cur.cells[i] !== prev.cells[i] || cur.colors[i] !== prev.colors[i] || cur.backs[i] !== prev.backs[i]) dirty[i] = 1;
    }
  }
  markWide(dirty, cur.cells, cur.cols);
  return dirty;
}

/** What both renderers share: the canvas, the cell, the palette and the last frame drawn. */
abstract class Base {
  readonly canvas: HTMLCanvasElement;
  readonly ctx: CanvasRenderingContext2D;
  cw = 8;
  ch = 16;
  cols = 0;
  rows = 0;
  protected css: string[] = new Array<string>(PALETTE_LEN).fill("#000");
  protected rgba = new Uint8Array(PALETTE_LEN * 4);
  protected ground: string | null = null;
  protected prev: { cells: Uint32Array; colors: Uint8Array; backs: Uint8Array } | null = null;
  /** The next draw redraws every cell (a palette or font changed); the last frame is kept to repaint from. */
  protected stale = false;
  protected baseline = 0;
  protected fontPx = 13;

  constructor(canvas: HTMLCanvasElement) {
    this.canvas = canvas;
    const ctx = canvas.getContext("2d", { alpha: true });
    if (!ctx) throw new Error("glyphplot: no 2D canvas context");
    this.ctx = ctx;
  }

  /** A new grid or cell size: the backing store is cols·cw × rows·ch device pixels. */
  resize(cols: number, rows: number, cw: number): void {
    this.cols = cols;
    this.rows = rows;
    this.cw = cw;
    this.ch = cw * 2;
    this.canvas.width = cols * this.cw;
    this.canvas.height = rows * this.ch;
    this.fontPx = cw / 0.6;
    this.ctx.font = `${this.fontPx}px ${FONT_STACK}`;
    const m = this.ctx.measureText("█");
    const asc = m.fontBoundingBoxAscent || this.fontPx * 0.8;
    const desc = m.fontBoundingBoxDescent || this.fontPx * 0.2;
    // The em box centred in the cell.
    this.baseline = Math.round((this.ch - (asc + desc)) / 2 + asc);
    this.prev = null;
    this.onResize();
  }

  protected onResize(): void {}

  /**
   * A new palette; `paintGround` paints entry 4 behind every cell. The next
   * draw is full; the last frame is kept, so {@link repaint} can redraw it
   * in the new colours (forgetting it here left a theme change undrawn until
   * the next frame).
   */
  setPalette(bytes: Uint8Array, paintGround: boolean): void {
    this.rgba = bytes.slice(0, PALETTE_LEN * 4);
    this.css = paletteCss(bytes);
    this.ground = paintGround ? (this.css[Token.ground] ?? null) : null;
    this.stale = true;
  }

  /** The next draw redraws every cell (a font load, a palette). */
  invalidate(): void {
    this.stale = true;
  }

  /** Draws the changed cells of `g` and keeps a copy of it for the next diff. */
  draw(g: GridViews): DrawStats {
    const full = this.stale || this.prev === null || g.cols !== this.cols || g.rows !== this.rows;
    this.stale = false;
    const dirty = diffCells(g, this.prev, full);
    let count = 0;
    for (let i = 0; i < dirty.length; i++) if (dirty[i]) count++;
    if (count > 0) this.paint(g, dirty, full);
    this.prev = { cells: g.cells.slice(), colors: g.colors.slice(), backs: g.backs.slice() };
    return { cells: count, full };
  }

  /** Redraws the last frame with the current palette, drawing no new frame (REQ-THEME-030). */
  repaint(): void {
    const p = this.prev;
    if (!p) return;
    this.stale = true;
    this.draw({ cols: this.cols, rows: this.rows, cells: p.cells, colors: p.colors, backs: p.backs });
  }

  protected abstract paint(g: GridViews, dirty: Uint8Array, full: boolean): void;

  /** Clears a cell and fills its ground and back. */
  protected backOf(x: number, y: number, back: number): void {
    const ctx = this.ctx;
    ctx.clearRect(x, y, this.cw, this.ch);
    if (this.ground) {
      ctx.fillStyle = this.ground;
      ctx.fillRect(x, y, this.cw, this.ch);
    }
    if (back !== NO_BACK) {
      ctx.fillStyle = this.css[back] ?? "transparent";
      ctx.fillRect(x, y, this.cw, this.ch);
    }
  }
}

/** fillText per changed cell (the default renderer). */
export class TextRenderer extends Base {
  protected paint(g: GridViews, dirty: Uint8Array): void {
    const ctx = this.ctx;
    const { cols, cw, ch } = this;
    // Backs first, so a wide glyph drawn into its tail's cell is not cleared after it.
    for (let i = 0; i < dirty.length; i++) {
      if (!dirty[i]) continue;
      this.backOf((i % cols) * cw, Math.floor(i / cols) * ch, g.backs[i] ?? NO_BACK);
    }
    ctx.font = `${this.fontPx}px ${FONT_STACK}`;
    ctx.textAlign = "left";
    ctx.textBaseline = "alphabetic";
    let style = -1;
    for (let i = 0; i < dirty.length; i++) {
      if (!dirty[i]) continue;
      const cp = g.cells[i] ?? 32;
      if (cp === 32 || cp === WIDE_TAIL) continue;
      const ink = g.colors[i] ?? 0;
      if (ink !== style) {
        ctx.fillStyle = this.css[ink] ?? "#000";
        style = ink;
      }
      const x = (i % cols) * cw;
      const y = Math.floor(i / cols) * ch;
      if (!fillBlock(ctx, cp, x, y, cw, ch)) ctx.fillText(String.fromCodePoint(cp), x, y + this.baseline);
    }
  }
}

/** A white mask a code point, packed into slots on an offscreen canvas; cleared when full. */
class MaskAtlas {
  readonly canvas: HTMLCanvasElement | OffscreenCanvas;
  readonly ctx: CanvasRenderingContext2D | OffscreenCanvasRenderingContext2D;
  readonly across: number;
  readonly slotW: number;
  readonly slotH: number;
  readonly capacity: number;
  readonly cw: number;
  readonly ch: number;
  readonly fontPx: number;
  readonly baseline: number;
  #slots = new Map<number, number>();

  constructor(cw: number, ch: number, fontPx: number, baseline: number, capacity = 1024) {
    this.cw = cw;
    this.ch = ch;
    this.fontPx = fontPx;
    this.baseline = baseline;
    this.capacity = capacity;
    this.across = Math.ceil(Math.sqrt(capacity));
    this.slotW = cw * 2 + 2; // room for a double-width glyph
    this.slotH = ch + 2;
    const w = this.across * this.slotW;
    const h = Math.ceil(capacity / this.across) * this.slotH;
    this.canvas = typeof OffscreenCanvas === "function" ? new OffscreenCanvas(w, h) : Object.assign(document.createElement("canvas"), { width: w, height: h });
    const ctx = this.canvas.getContext("2d") as CanvasRenderingContext2D | OffscreenCanvasRenderingContext2D | null;
    if (!ctx) throw new Error("glyphplot: no 2D canvas context for the atlas");
    this.ctx = ctx;
  }

  /** The slot's top-left inside the atlas. */
  at(n: number): [number, number] {
    return [(n % this.across) * this.slotW + 1, Math.floor(n / this.across) * this.slotH + 1];
  }

  /** The slot holding `cp`'s white mask, drawing it on a miss. */
  slot(cp: number): number {
    const known = this.#slots.get(cp);
    if (known !== undefined) return known;
    if (this.#slots.size >= this.capacity) {
      this.ctx.clearRect(0, 0, this.canvas.width, this.canvas.height);
      this.#slots.clear();
    }
    const n = this.#slots.size;
    const [x, y] = this.at(n);
    const ctx = this.ctx;
    ctx.fillStyle = "#fff";
    ctx.font = `${this.fontPx}px ${FONT_STACK}`;
    ctx.textAlign = "left";
    ctx.textBaseline = "alphabetic";
    ctx.fillText(String.fromCodePoint(cp), x, y + this.baseline);
    this.#slots.set(cp, n);
    return n;
  }
}

/**
 * The atlas renderer: per changed row span, glyph masks (white, from the
 * atlas or the exact block shapes) on a mask layer, a one-pixel-a-cell ink
 * image scaled over the span and kept where the mask is (`destination-in`),
 * then backs and the tinted glyphs onto the canvas. The atlas is keyed by
 * code point alone, so colours never multiply its entries.
 */
export class AtlasRenderer extends Base {
  #mask: HTMLCanvasElement | null = null;
  #mctx: CanvasRenderingContext2D | null = null;
  #tint: HTMLCanvasElement | null = null;
  #tctx: CanvasRenderingContext2D | null = null;
  #inks: HTMLCanvasElement | null = null;
  #ictx: CanvasRenderingContext2D | null = null;
  #img: ImageData | null = null;
  #atlas: MaskAtlas | null = null;

  protected override onResize(): void {
    const make = (w: number, h: number): [HTMLCanvasElement, CanvasRenderingContext2D] => {
      const c = document.createElement("canvas");
      c.width = w;
      c.height = h;
      const x = c.getContext("2d");
      if (!x) throw new Error("glyphplot: no 2D canvas context");
      return [c, x];
    };
    [this.#mask, this.#mctx] = make(this.canvas.width, this.canvas.height);
    // One row tall: compositing with `copy` and `destination-in` touches the
    // whole destination canvas, so a full-size tint canvas cost a full-canvas
    // pass for every changed row (about 700 ms a frame at 320 × 120 in
    // headless Chromium, bench:render); a row-sized one, clipped to the span,
    // costs the span.
    [this.#tint, this.#tctx] = make(this.canvas.width, this.ch);
    [this.#inks, this.#ictx] = make(Math.max(1, this.cols), Math.max(1, this.rows));
    this.#img = this.#ictx.createImageData(Math.max(1, this.cols), Math.max(1, this.rows));
    this.#atlas = new MaskAtlas(this.cw, this.ch, this.fontPx, this.baseline);
  }

  protected paint(g: GridViews, dirty: Uint8Array): void {
    const mctx = this.#mctx;
    const tctx = this.#tctx;
    const ictx = this.#ictx;
    const img = this.#img;
    const atlas = this.#atlas;
    if (!mctx || !tctx || !ictx || !img || !atlas || !this.#mask || !this.#inks || !this.#tint) return;
    const { cols, rows, cw, ch } = this;
    const px = img.data;
    // 1. Masks of the changed cells, and their inks into the small image.
    mctx.fillStyle = "#fff";
    for (let i = 0; i < dirty.length; i++) {
      if (!dirty[i]) continue;
      const x = (i % cols) * cw;
      const y = Math.floor(i / cols) * ch;
      mctx.clearRect(x, y, cw, ch);
      const ink = g.colors[i] ?? 0;
      px.set(this.rgba.subarray(ink * 4, ink * 4 + 4), i * 4);
    }
    for (let i = 0; i < dirty.length; i++) {
      if (!dirty[i]) continue;
      const cp = g.cells[i] ?? 32;
      if (cp === 32 || cp === WIDE_TAIL) continue;
      const x = (i % cols) * cw;
      const y = Math.floor(i / cols) * ch;
      if (isBlock(cp)) {
        fillBlock(mctx, cp, x, y, cw, ch);
      } else {
        const wide = i % cols < cols - 1 && g.cells[i + 1] === WIDE_TAIL ? 2 : 1;
        const [sx, sy] = atlas.at(atlas.slot(cp));
        mctx.drawImage(atlas.canvas, sx, sy, cw * wide, ch, x, y, cw * wide, ch);
      }
    }
    ictx.putImageData(img, 0, 0);
    // 2. Per row, the span of changed cells: backs, then tinted glyphs.
    for (let r = 0; r < rows; r++) {
      let c0 = -1;
      let c1 = -1;
      for (let c = 0; c < cols; c++) {
        if (dirty[r * cols + c]) {
          if (c0 < 0) c0 = c;
          c1 = c;
        }
      }
      if (c0 < 0) continue;
      const x = c0 * cw;
      const y = r * ch;
      const w = (c1 - c0 + 1) * cw;
      for (let c = c0; c <= c1; c++) {
        const i = r * cols + c;
        if (dirty[i]) this.backOf(c * cw, y, g.backs[i] ?? NO_BACK);
      }
      tctx.save();
      tctx.beginPath();
      tctx.rect(x, 0, w, ch);
      tctx.clip();
      tctx.globalCompositeOperation = "copy";
      tctx.imageSmoothingEnabled = false;
      tctx.drawImage(this.#inks, c0, r, c1 - c0 + 1, 1, x, 0, w, ch);
      tctx.globalCompositeOperation = "destination-in";
      tctx.drawImage(this.#mask, x, y, w, ch, x, 0, w, ch);
      tctx.restore();
      // Cells in the span that did not change keep their pixels: copy the tint of each run of changed cells.
      for (let c = c0; c <= c1; ) {
        if (!dirty[r * cols + c]) {
          c++;
          continue;
        }
        let e = c;
        while (e + 1 <= c1 && dirty[r * cols + e + 1]) e++;
        this.ctx.drawImage(this.#tint, c * cw, 0, (e - c + 1) * cw, ch, c * cw, y, (e - c + 1) * cw, ch);
        c = e + 1;
      }
    }
  }
}

/** A renderer by name. */
export function makeRenderer(name: RendererName, canvas: HTMLCanvasElement): TextRenderer | AtlasRenderer {
  return name === "atlas" ? new AtlasRenderer(canvas) : new TextRenderer(canvas);
}
