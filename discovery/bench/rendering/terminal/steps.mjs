// Throwaway (see ../README.md). The largest colour step between neighbouring
// entries of a palette of n colours quantised from a continuous scale, as CIE76
// delta E in Lab (D65); a delta E near 2.3 is about the smallest step people see.
// Two scales: the 5-stop viridis-like ramp of the benches, and real viridis' 9 stops.
const lab = ([r, g, b]) => {
  const l = (c) => { c /= 255; return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4; };
  const [R, G, B] = [l(r), l(g), l(b)];
  const X = (0.4124 * R + 0.3576 * G + 0.1805 * B) / 0.95047, Y = 0.2126 * R + 0.7152 * G + 0.0722 * B, Z = (0.0193 * R + 0.1192 * G + 0.9505 * B) / 1.08883;
  const f = (t) => (t > 216 / 24389 ? Math.cbrt(t) : (24389 / 27 * t + 16) / 116);
  return [116 * f(Y) - 16, 500 * (f(X) - f(Y)), 200 * (f(Y) - f(Z))];
};
const ramp = (stops) => (v) => { v = Math.min(1, Math.max(0, v)) * (stops.length - 1); const i = Math.min(stops.length - 2, Math.floor(v)), f = v - i; return stops[i].map((x, k) => x + (stops[i + 1][k] - x) * f); };
const SCALES = {
  bench5: [[68, 1, 84], [59, 82, 139], [33, 145, 140], [94, 201, 98], [253, 231, 37]],
  viridis9: [[68, 1, 84], [71, 44, 122], [59, 81, 139], [44, 113, 142], [33, 144, 141], [39, 173, 129], [92, 200, 99], [170, 220, 50], [253, 231, 37]],
};
for (const [name, stops] of Object.entries(SCALES)) {
  const s = ramp(stops);
  for (const n of [16, 32, 64, 128, 200, 255]) {
    let max = 0;
    for (let i = 1; i < n; i++) { const a = lab(s((i - 1) / (n - 1))), b = lab(s(i / (n - 1))); max = Math.max(max, Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2])); }
    console.log(`${name} n=${String(n).padStart(3)}  largest step dE76 ${max.toFixed(2)}`);
  }
}
