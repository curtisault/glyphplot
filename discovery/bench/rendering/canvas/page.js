// Throwaway (see ../README.md). Runs in the page: synthetic chart frames and
// four ways of drawing them on a canvas, timed per frame. The atlas renderer
// caches each glyph once per (code point, colour) on an offscreen canvas,
// clears it when full, and draws glyphs from the font registered as "bench mono".

const FONT = '"bench mono", monospace';

// A continuous scale (viridis-like, 5 stops) as 0xRRGGBB.
const STOPS = [[68, 1, 84], [59, 82, 139], [33, 145, 140], [94, 201, 98], [253, 231, 37]];
function scale(v) {
  v = Math.min(1, Math.max(0, v)) * (STOPS.length - 1);
  const i = Math.min(STOPS.length - 2, Math.floor(v)), f = v - i, a = STOPS[i], b = STOPS[i + 1];
  return (Math.round(a[0] + (b[0] - a[0]) * f) << 16) | (Math.round(a[1] + (b[1] - a[1]) * f) << 8) | Math.round(a[2] + (b[2] - a[2]) * f);
}
const hex = (c) => "#" + c.toString(16).padStart(6, "0");
// A palette of n colours quantised from the scale, and the index for a value.
const paletteOf = (n) => Array.from({ length: n }, (_, i) => scale(i / (n - 1)));
const BRAILLE_LEVEL = [0, 0x01, 0x09, 0x0b, 0x1b, 0x1f, 0x3f, 0x7f, 0xff]; // 0..8 dots lit

// Workloads: fill cells (code points) and a value per cell in [0, 1].
function frameData(kind, cols, rows, t, cells, values) {
  for (let y = 0; y < rows; y++) for (let x = 0; x < cols; x++) {
    const i = y * cols + x;
    let v = 0.5 + 0.25 * Math.sin(x * 0.11 + t) + 0.25 * Math.cos(y * 0.23 - t * 0.7 + x * 0.03);
    v = Math.min(1, Math.max(0, v));
    values[i] = v;
    if (kind === "density") cells[i] = 0x2800 + BRAILLE_LEVEL[Math.round(v * 8)]; // braille density heatmap
    else if (kind === "scatter") {
      // a braille scatter coloured by value: arbitrary dot masks, a third of cells empty
      let h = Math.imul(x * 374761393 + y * 668265263 + Math.floor(t * 7) * 2246822519, 2654435761) >>> 0;
      h ^= h >>> 15;
      cells[i] = h % 3 === 0 ? 0x20 : 0x2800 + (h >>> 8 & 255 || 1);
    } else cells[i] = 0x2588; // full-block heatmap
  }
}

function makeCanvas(cols, rows, cw) {
  const ch = cw * 2;
  const c = document.createElement("canvas");
  c.width = cols * cw; c.height = rows * ch;
  document.body.appendChild(c);
  return c;
}

// A glyph atlas written for this bench. An offscreen 2D canvas is cut into
// `capacity` slots, each one cell (cw x 2cw) inside a 1 px gutter on every side,
// packed row by row in a near-square grid of ceil(sqrt(capacity)) slots across
// (1024 slots: 32 x 32; 4096: 64 x 64). Slots fill in order under whatever key
// the caller gives; a miss with every slot taken wipes the canvas and the key
// map and starts again from slot 0.
class GlyphAtlas {
  constructor(cw, capacity) {
    this.cw = cw;
    this.ch = cw * 2;
    this.capacity = capacity;
    this.across = Math.ceil(Math.sqrt(capacity));
    this.slotW = cw + 2;
    this.slotH = this.ch + 2;
    this.canvas = document.createElement("canvas");
    this.canvas.width = this.across * this.slotW;
    this.canvas.height = Math.ceil(capacity / this.across) * this.slotH;
    this.ctx = this.canvas.getContext("2d");
    this.ctx.font = `${cw / 0.6}px ${FONT}`;
    this.ctx.textAlign = "center";
    this.ctx.textBaseline = "middle";
    this.keys = new Map();
    this.stats = { hits: 0, misses: 0, clears: 0 };
  }
  // Top left of the cell inside slot `n`.
  left(n) { return (n % this.across) * this.slotW + 1; }
  top(n) { return Math.floor(n / this.across) * this.slotH + 1; }
  // The slot holding `cp` in `css` under `key`, drawing it on a miss.
  slot(key, cp, css) {
    const known = this.keys.get(key);
    if (known !== undefined) { this.stats.hits++; return known; }
    this.stats.misses++;
    if (this.keys.size >= this.capacity) {
      this.ctx.clearRect(0, 0, this.canvas.width, this.canvas.height);
      this.keys.clear();
      this.stats.clears++;
    }
    const n = this.keys.size;
    this.ctx.fillStyle = css;
    this.ctx.fillText(String.fromCodePoint(cp), this.left(n) + this.cw / 2, this.top(n) + this.ch / 2);
    this.keys.set(key, n);
    return n;
  }
  // Copies slot `n`'s cell to (dx, dy) on `dst`.
  blit(dst, n, dx, dy) {
    dst.drawImage(this.canvas, this.left(n), this.top(n), this.cw, this.ch, dx, dy, this.cw, this.ch);
  }
}

// The atlas keyed by (code point, colour); full redraw, as a streaming heatmap changes every cell.
function atlasRenderer(canvas, cols, rows, cw, slotsMax) {
  const ctx = canvas.getContext("2d");
  const ch = cw * 2;
  const atlas = new GlyphAtlas(cw, slotsMax);
  return {
    stats: atlas.stats,
    draw(cells, keyOf, cssOf, ground) {
      ctx.fillStyle = ground; ctx.fillRect(0, 0, canvas.width, canvas.height);
      for (let y = 0, i = 0; y < rows; y++) {
        for (let x = 0; x < cols; x++, i++) {
          const cp = cells[i];
          if (cp === 0x20) continue;
          atlas.blit(ctx, atlas.slot(keyOf(i, cp), cp, cssOf(i)), x * cw, y * ch);
        }
      }
    },
  };
}

// Tint by composite: glyph masks from an atlas keyed by code point alone, a
// colour layer of one pixel a cell scaled up, and one destination-in.
function tintRenderer(canvas, cols, rows, cw) {
  const ctx = canvas.getContext("2d");
  const ch = cw * 2;
  const mask = document.createElement("canvas"); mask.width = canvas.width; mask.height = canvas.height;
  const mctx = mask.getContext("2d");
  const small = document.createElement("canvas"); small.width = cols; small.height = rows;
  const sctx = small.getContext("2d");
  const img = sctx.createImageData(cols, rows);
  const px = new Uint32Array(img.data.buffer);
  // a white glyph per code point: the mask
  const atlas = new GlyphAtlas(cw, 1024);
  return {
    stats: atlas.stats,
    draw(cells, rgbOf, ground) {
      const n = cols * rows;
      mctx.clearRect(0, 0, mask.width, mask.height);
      for (let i = 0; i < n; i++) {
        const cp = cells[i];
        const c = rgbOf(i);
        px[i] = 0xff000000 | ((c & 0xff) << 16) | (c & 0xff00) | ((c >> 16) & 0xff); // ABGR little-endian
        if (cp === 0x20) continue;
        const x = i % cols, y = (i - x) / cols;
        atlas.blit(mctx, atlas.slot(cp, cp, "#fff"), x * cw, y * ch);
      }
      sctx.putImageData(img, 0, 0);
      ctx.globalCompositeOperation = "copy";
      ctx.imageSmoothingEnabled = false;
      ctx.drawImage(small, 0, 0, canvas.width, canvas.height);
      ctx.globalCompositeOperation = "destination-in";
      ctx.drawImage(mask, 0, 0);
      ctx.globalCompositeOperation = "destination-over";
      ctx.fillStyle = ground; ctx.fillRect(0, 0, canvas.width, canvas.height);
      ctx.globalCompositeOperation = "source-over";
    },
  };
}

// fillText per cell, no atlas.
function directRenderer(canvas, cols, rows, cw) {
  const ctx = canvas.getContext("2d");
  const ch = cw * 2;
  return {
    stats: { hits: 0, misses: 0, clears: 0 },
    draw(cells, cssOf, ground) {
      ctx.fillStyle = ground; ctx.fillRect(0, 0, canvas.width, canvas.height);
      ctx.font = `${cw / 0.6}px ${FONT}`; ctx.textAlign = "center"; ctx.textBaseline = "middle";
      const n = cols * rows;
      for (let i = 0; i < n; i++) {
        const cp = cells[i];
        if (cp === 0x20) continue;
        const x = i % cols, y = (i - x) / cols;
        ctx.fillStyle = cssOf(i);
        ctx.fillText(String.fromCodePoint(cp), x * cw + cw / 2, y * ch + ch / 2);
      }
    },
  };
}

// Full blocks: a rectangle a cell, or one image of a pixel a cell.
function blockRects(canvas, cols, rows, cw) {
  const ctx = canvas.getContext("2d"), ch = cw * 2;
  return { stats: {}, draw(cssOf) { const n = cols * rows; for (let i = 0; i < n; i++) { const x = i % cols, y = (i - x) / cols; ctx.fillStyle = cssOf(i); ctx.fillRect(x * cw, y * ch, cw, ch); } } };
}
function blockImage(canvas, cols, rows) {
  const ctx = canvas.getContext("2d");
  const small = document.createElement("canvas"); small.width = cols; small.height = rows;
  const sctx = small.getContext("2d"), img = sctx.createImageData(cols, rows), px = new Uint32Array(img.data.buffer);
  return { stats: {}, draw(rgbOf) { const n = cols * rows; for (let i = 0; i < n; i++) { const c = rgbOf(i); px[i] = 0xff000000 | ((c & 0xff) << 16) | (c & 0xff00) | ((c >> 16) & 0xff); } sctx.putImageData(img, 0, 0); ctx.imageSmoothingEnabled = false; ctx.drawImage(small, 0, 0, canvas.width, canvas.height); } };
}

// One case: a renderer over a workload and colour model, `frames` frames after `warm`.
window.runCase = async function (o) {
  await document.fonts.ready;
  const { kind, model, renderer, cols, rows, cw, frames = 60, warm = 10, slots = 1024 } = o;
  const n = cols * rows, cells = new Uint32Array(n), values = new Float32Array(n);
  const canvas = makeCanvas(cols, rows, cw);
  const ctx = canvas.getContext("2d");
  // Colour models: index into a palette quantised from the scale, or RGB straight from it.
  const palN = model === "pal64" ? 64 : model === "pal256" ? 255 : 0;
  const pal = palN ? paletteOf(palN) : null, palCss = pal ? pal.map(hex) : null;
  const idx = new Uint8Array(n), rgb = new Uint32Array(n);
  const prep = () => { for (let i = 0; i < n; i++) { if (pal) { idx[i] = Math.round(values[i] * (palN - 1)); rgb[i] = pal[idx[i]]; } else rgb[i] = scale(values[i]); } };
  const cssCache = new Map();
  const rgbCss = (c) => { let s = cssCache.get(c); if (!s) { s = hex(c); if (cssCache.size < 65536) cssCache.set(c, s); } return s; };
  const cssOf = pal ? (i) => palCss[idx[i]] : (i) => rgbCss(rgb[i]);
  const rgbOf = (i) => rgb[i];
  const keyOf = pal ? (i, cp) => cp * 256 + idx[i] : (i, cp) => cp * 16777216 + rgb[i];
  let r;
  if (renderer === "atlas") r = atlasRenderer(canvas, cols, rows, cw, slots);
  else if (renderer === "tint") r = tintRenderer(canvas, cols, rows, cw);
  else if (renderer === "direct") r = directRenderer(canvas, cols, rows, cw);
  else if (renderer === "rects") r = blockRects(canvas, cols, rows, cw);
  else if (renderer === "image") r = blockImage(canvas, cols, rows);
  else if (renderer === "noop") r = { stats: {}, draw() { ctx.fillStyle = "#111"; ctx.fillRect(0, 0, canvas.width, canvas.height); } };
  const times = [];
  let distinctPairs = new Set();
  for (let f = 0; f < warm + frames; f++) {
    frameData(kind, cols, rows, f * 0.05, cells, values);
    prep();
    if (f === warm) { r.stats.hits = 0; r.stats.misses = 0; r.stats.clears = 0; }
    if (f >= warm && f < warm + 3) for (let i = 0; i < n; i++) if (cells[i] !== 0x20) distinctPairs.add(cells[i] * 16777216 + rgb[i]);
    const t0 = performance.now();
    if (renderer === "atlas") r.draw(cells, keyOf, cssOf, "#111");
    else if (renderer === "tint") r.draw(cells, rgbOf, "#111");
    else if (renderer === "direct") r.draw(cells, cssOf, "#111");
    else if (renderer === "rects") r.draw(cssOf);
    else if (renderer === "noop") r.draw();
    else r.draw(rgbOf);
    ctx.getImageData(0, 0, 1, 1); // wait for the raster: draw calls are otherwise deferred
    const dt = performance.now() - t0;
    if (f >= warm) times.push(dt);
  }
  canvas.remove();
  times.sort((a, b) => a - b);
  const q = (p) => times[Math.min(times.length - 1, Math.floor(p * times.length))];
  const s = r.stats, tot = (s.hits ?? 0) + (s.misses ?? 0);
  return { median: q(0.5), p95: q(0.95), min: times[0], hitRate: tot ? s.hits / tot : null, clears: s.clears ?? null, distinctPerFrame: Math.round(distinctPairs.size / 3) };
};

// Digit legibility: each digit drawn as the atlas draws it, at a cell width,
// binarised at half coverage; the closest pair of digits and how many pixels tell them apart.
window.digits = async function (fontPx, dpr) {
  await document.fonts.ready;
  const cw = Math.round(fontPx * 0.6 * dpr), ch = Math.round(fontPx * 1.2 * dpr);
  const c = document.createElement("canvas"); c.width = cw; c.height = ch;
  const x = c.getContext("2d", { willReadFrequently: true });
  const maps = [];
  for (let d = 0; d < 10; d++) {
    x.clearRect(0, 0, cw, ch); x.font = `${fontPx * dpr}px ${FONT}`; x.textAlign = "center"; x.textBaseline = "middle"; x.fillStyle = "#000";
    x.fillText(String(d), cw / 2, ch / 2);
    const a = x.getImageData(0, 0, cw, ch).data, m = new Uint8Array(cw * ch);
    for (let i = 0; i < m.length; i++) m[i] = a[i * 4 + 3] >= 128 ? 1 : 0;
    maps.push(m);
  }
  let best = { d: Infinity, pair: "" };
  for (let a = 0; a < 10; a++) for (let b = a + 1; b < 10; b++) {
    let diff = 0; for (let i = 0; i < maps[a].length; i++) diff += maps[a][i] ^ maps[b][i];
    if (diff < best.d) best = { d: diff, pair: `${a}/${b}` };
  }
  // ink rows: how many pixel rows the digit '8' spans
  const m8 = maps[8]; let top = ch, bot = -1;
  for (let yy = 0; yy < ch; yy++) for (let xx = 0; xx < cw; xx++) if (m8[yy * cw + xx]) { top = Math.min(top, yy); bot = Math.max(bot, yy); }
  return { fontPx, dpr, cell: `${cw}x${ch}`, digitHeightPx: bot - top + 1, closestPair: best.pair, pixelsApart: best.d };
};
