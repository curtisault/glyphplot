// Throwaway (see ../README.md). Bytes of escape codes for one full frame,
// written the plain way a terminal painter would: a cursor move to the start
// of each row, a colour code only when the colour changes along the row, and
// a reset at the end of any row that set a colour. Deterministic: no timing.
//   node sgr.mjs
const STOPS = [[68, 1, 84], [59, 82, 139], [33, 145, 140], [94, 201, 98], [253, 231, 37]];
const scale = (v) => { v = Math.min(1, Math.max(0, v)) * 4; const i = Math.min(3, Math.floor(v)), f = v - i, a = STOPS[i], b = STOPS[i + 1]; return a.map((x, k) => Math.round(x + (b[k] - x) * f)); };
// xterm 256: the 6x6x6 cube and the 24 greys, nearest by squared distance.
const LEVELS = [0, 95, 135, 175, 215, 255];
const x256 = ([r, g, b]) => {
  const q = (v) => LEVELS.reduce((best, l, i) => (Math.abs(l - v) < Math.abs(LEVELS[best] - v) ? i : best), 0);
  const [qr, qg, qb] = [q(r), q(g), q(b)];
  const cube = 16 + 36 * qr + 6 * qg + qb, cc = [LEVELS[qr], LEVELS[qg], LEVELS[qb]];
  const gi = Math.max(0, Math.min(23, Math.round(((r + g + b) / 3 - 8) / 10))), gv = 8 + 10 * gi;
  const d = (c) => (c[0] - r) ** 2 + (c[1] - g) ** 2 + (c[2] - b) ** 2;
  return d([gv, gv, gv]) < d(cc) ? { n: 232 + gi, rgb: [gv, gv, gv] } : { n: cube, rgb: cc };
};
const BRAILLE_LEVEL = [0, 0x01, 0x09, 0x0b, 0x1b, 0x1f, 0x3f, 0x7f, 0xff];

function frame(kind, cols, rows) {
  const cells = [];
  for (let y = 0; y < rows; y++) for (let x = 0; x < cols; x++) {
    let v = Math.min(1, Math.max(0, 0.5 + 0.25 * Math.sin(x * 0.11) + 0.25 * Math.cos(y * 0.23 + x * 0.03)));
    if (kind === "density") cells.push({ ch: String.fromCodePoint(0x2800 + BRAILLE_LEVEL[Math.round(v * 8)]), v, layer: 38 });
    else if (kind === "blocks") cells.push({ ch: " ", v, layer: 48 });
    else {
      // a line chart: three series in three colours, an axis, mostly empty
      const ys = [0.3, 0.5, 0.7].map((o, s) => Math.round(rows * (o + 0.15 * Math.sin(x * 0.07 + s))));
      const s = ys.findIndex((yy) => yy === y);
      if (x === 6) cells.push({ ch: "│", v: null, layer: 38 });
      else if (x < 6 && y % 4 === 0) cells.push({ ch: "1", v: null, layer: 38 });
      else if (s >= 0) cells.push({ ch: "⠒", v: [0.1, 0.5, 0.9][s], layer: 38 });
      else cells.push({ ch: " ", v: null, layer: 38 });
    }
  }
  return cells;
}

// The colour a cell asks for, as [key, SGR sequence], or null when it asks for
// none (an empty foreground cell). Equal keys mean the same sequence.
function sgrFor(c, model) {
  const blank = c.ch === " ";
  if (c.v === null || (blank && c.layer === 38)) {
    if (blank) return null;
    return ["default", c.layer === 48 ? "\x1b[49m" : "\x1b[39m"];
  }
  const levels = model === "pal64" ? 64 : model === "pal255" ? 255 : 0;
  const rgb = scale(levels ? Math.round(c.v * (levels - 1)) / (levels - 1) : c.v);
  if (model === "x256") {
    const n = x256(rgb).n;
    return [n, `\x1b[${c.layer};5;${n}m`];
  }
  const [r, g, b] = rgb;
  return [`${r},${g},${b}`, `\x1b[${c.layer};2;${r};${g};${b}m`];
}

// UTF-8 bytes of one frame, and how many colour sequences it holds.
function bytes(cells, cols, rows, model) {
  const parts = [];
  let codes = 0;
  for (let y = 0; y < rows; y++) {
    parts.push(`\x1b[${y + 1};1H`);
    let last; // the key in force on this row; undefined until one is set
    let set = false; // whether a non-default colour was set on this row
    for (const c of cells.slice(y * cols, (y + 1) * cols)) {
      const want = sgrFor(c, model);
      if (want && want[0] !== last) {
        parts.push(want[1]);
        codes += 1;
        last = want[0];
        if (last !== "default") set = true;
      }
      parts.push(c.ch);
    }
    if (set) parts.push("\x1b[0m");
  }
  return { out: Buffer.byteLength(parts.join(""), "utf8"), codes };
}

// colour error of 256-colour against the scale: mean and max Euclidean RGB distance over 1001 samples
let sum = 0, max = 0;
for (let i = 0; i <= 1000; i++) { const c = scale(i / 1000), q = x256(c).rgb; const d = Math.hypot(c[0] - q[0], c[1] - q[1], c[2] - q[2]); sum += d; max = Math.max(max, d); }
const distinct256 = new Set(Array.from({ length: 1001 }, (_, i) => x256(scale(i / 1000)).n)).size;
console.log(`xterm-256 against the scale: mean error ${(sum / 1001).toFixed(1)}, max ${max.toFixed(1)} (RGB units), ${distinct256} distinct colours of 256`);
console.log("kind      size      model    bytes    colour codes");
for (const [cols, rows] of [[80, 24], [320, 120]]) for (const kind of ["density", "blocks", "lines"]) for (const model of ["rgb", "pal255", "pal64", "x256"]) {
  const r = bytes(frame(kind, cols, rows), cols, rows, model);
  console.log(`${kind.padEnd(9)} ${(cols + "x" + rows).padEnd(9)} ${model.padEnd(8)} ${String(r.out).padStart(7)}  ${String(r.codes).padStart(6)}`);
}
