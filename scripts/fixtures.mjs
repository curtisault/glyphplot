// Captures the pinned d3 packages' output for the core's ports to be compared
// with, bit for bit, by crates/glyphplot/tests/reference.rs (REQ-REF-010).
//
//   npm ci --prefix scripts/fixtures
//   node scripts/fixtures.mjs
//
// Writes crates/glyphplot/tests/fixtures/{ticks,nice,time-ticks,format,
// time-format}.txt. The cases come from a seeded SplitMix64, so a rerun writes
// the same bytes. Numbers are written as JavaScript's String(x), which is the
// shortest form that reads back as the same double (Rust's str::parse reads
// it back bitwise), with -0 as "-0".
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";

const PINS = {
  "d3-array": "3.2.4",
  "d3-format": "3.1.2",
  "d3-scale": "4.0.2",
  "d3-time": "3.1.0",
  "d3-time-format": "4.1.0",
};
const modules = new URL("./fixtures/node_modules/", import.meta.url);
for (const [name, version] of Object.entries(PINS)) {
  const found = JSON.parse(readFileSync(new URL(`${name}/package.json`, modules), "utf8")).version;
  if (found !== version) throw new Error(`${name} is ${found}, not ${version}: run npm ci --prefix scripts/fixtures`);
}
const load = (name) => import(new URL(`${name}/src/index.js`, modules).href);
const { ticks, tickIncrement, tickStep } = await load("d3-array");
const { formatDefaultLocale } = await load("d3-format");
const { scaleLinear, scaleUtc } = await load("d3-scale");
const { utcTicks } = await load("d3-time");
const { utcFormat } = await load("d3-time-format");

// The core's default locale (REQ-I18N-030): d3's en-US with "-" for the
// minus and an empty NaN. d3-scale's tickFormat uses this default too.
const { format } = formatDefaultLocale({ decimal: ".", thousands: ",", grouping: [3], currency: ["$", ""], minus: "-", nan: "" });

const out = new URL("../crates/glyphplot/tests/fixtures/", import.meta.url);
mkdirSync(out, { recursive: true });

const num = (x) => (Object.is(x, -0) ? "-0" : String(x));
const nums = (xs) => xs.map(num).join(" ");

// SplitMix64, seeded per file.
function rng(seed) {
  let s = BigInt(seed);
  const M = (1n << 64n) - 1n;
  const next = () => {
    s = (s + 0x9e3779b97f4a7c15n) & M;
    let z = s;
    z = ((z ^ (z >> 30n)) * 0xbf58476d1ce4e5b9n) & M;
    z = ((z ^ (z >> 27n)) * 0x94d049bb133111ebn) & M;
    return z ^ (z >> 31n);
  };
  const r = {
    bits: next,
    unit: () => Number(next() >> 11n) / 2 ** 53,
    int: (lo, hi) => lo + Math.floor(r.unit() * (hi - lo + 1)),
    pick: (xs) => xs[r.int(0, xs.length - 1)],
    // A double with its exponent spread from 10^lo to 10^hi.
    mag: (lo, hi) => (r.unit() * 9 + 1) * 10 ** r.int(lo, hi),
    // Any finite double, from its bits.
    double: () => {
      const view = new DataView(new ArrayBuffer(8));
      for (;;) {
        view.setBigUint64(0, next());
        const x = view.getFloat64(0);
        if (Number.isFinite(x)) return x;
      }
    },
  };
  return r;
}

const dv = new DataView(new ArrayBuffer(8));
const bitsOf = (x) => (dv.setFloat64(0, x), dv.getBigUint64(0));
const fromBits = (b) => (dv.setBigUint64(0, b), dv.getFloat64(0));
// The doubles k units from x.
const ulps = (x, k) => fromBits(bitsOf(x) + BigInt(k));

function write(name, header, lines) {
  const text = header.map((h) => `# ${h}\n`).join("") + lines.join("\n") + "\n";
  writeFileSync(new URL(name, out), text);
  console.log(`${name}: ${lines.length} cases, ${text.length} bytes`);
}

// Linear domains: picked by hand for their edges, then seeded.
const DOMAINS = [
  [0, 1], [0, 10], [-1, 1], [0, 0.001], [1e-9, 3e-9], [0, 1e9], [-37.5, 112.25], [99.5, 100.5],
  [123456, 123789], [0.1, 0.3], [0, 3.4e9], [-0.5, 0.75], [-100, -3], [-1e6, -999], [1e-300, 3e-300],
  [1e300, 1.7e308], [-1.7e308, 1.7e308], [0, 5e-324], [5e-324, 1e-323], [1, 1.0000000000000002],
  [5, 5], [0, 0], [-0, 0], [1e15, 1e15 + 1], [2 ** 53, 2 ** 53 + 4], [0.000001, 0.000002],
  [-2.5, 2.5], [0.49999999999999994, 0.5000000000000001], [1e21, 1e22], [-9.5, 9.5],
];
const withReversed = (ds) => ds.flatMap(([a, b]) => (a === b ? [[a, b]] : [[a, b], [b, a]]));

// ---- ticks.txt ---------------------------------------------------------------
{
  const r = rng(1);
  const lines = [];
  const add = (a, b, n) => {
    // d3 throws where the tick count is not a number (subnormal steps whose
    // inverse is infinite); the core returns no ticks there.
    let t;
    try {
      t = ticks(a, b, n);
    } catch {
      t = null;
    }
    if (t) lines.push(`ticks ${num(a)} ${num(b)} ${num(n)} : ${nums(t)}`);
    lines.push(`step ${num(a)} ${num(b)} ${num(n)} : ${num(tickIncrement(a, b, n))} ${num(tickStep(a, b, n))}`);
  };
  for (const [a, b] of withReversed(DOMAINS)) for (let n = 0; n <= 50; n += n < 12 ? 1 : 7) add(a, b, n);
  for (const n of [0.5, 1.5, 2.7, -1, NaN, Infinity, 1e-9]) add(0, 1, n);
  for (const [a, b] of [[NaN, 1], [0, NaN], [0, Infinity], [-Infinity, Infinity], [Infinity, Infinity]]) add(a, b, 10);
  // Steps a few doubles either side of a power of ten, where Math.log10's
  // rounding decides the power.
  for (let m = -30; m <= 30; m++) for (let k = -3; k <= 3; k++) for (const n of [1, 10]) add(0, ulps(10 ** m, k) * n, n);
  for (let i = 0; i < 1500; i++) {
    const mag = 10 ** r.int(-12, 12);
    const a = (r.unit() * 2 - 1) * mag;
    const b = a + r.unit() * mag * 3;
    const n = r.int(0, 50);
    if (r.unit() < 0.5) add(a, b, n);
    else add(b, a, n);
  }
  write("ticks.txt", [
    "d3-array 3.2.4: `ticks start stop count : ticks…` and `step start stop count : tickIncrement tickStep`.",
    "Written by scripts/fixtures.mjs (REQ-REF-010); numbers as JavaScript's String(x), -0 as -0.",
  ], lines);
}

// ---- nice.txt ----------------------------------------------------------------
{
  const r = rng(2);
  const lines = [];
  const linear = (a, b, n) => lines.push(`linear ${num(a)} ${num(b)} ${num(n)} : ${nums(scaleLinear().domain([a, b]).nice(n).domain())}`);
  const utc = (a, b, n) => lines.push(`utc ${num(a)} ${num(b)} ${num(n)} : ${nums(scaleUtc().domain([a, b]).nice(n).domain().map(Number))}`);
  for (const [a, b] of withReversed(DOMAINS)) for (const n of [0, 1, 2, 3, 5, 7, 10, 13, 20, 50]) linear(a, b, n);
  // A count of 1 across zero does not settle in ten rounds: d3 gives the domain back.
  for (let i = 0; i < 100; i++) {
    const m = 10 ** r.int(-6, 6);
    linear(-r.unit() * m, r.unit() * m, r.pick([1, 1, 1, 0.5, 1.5]));
  }
  for (let i = 0; i < 1500; i++) {
    const mag = 10 ** r.int(-12, 12);
    const a = (r.unit() * 2 - 1) * mag;
    const b = a + r.unit() * mag * 3;
    const n = r.int(1, 50);
    if (r.unit() < 0.5) linear(a, b, n);
    else linear(b, a, n);
  }
  for (let i = 0; i < 600; i++) {
    const a = Math.floor(Date.UTC(1900, 0, 1) + r.unit() * 200 * 365 * 864e5);
    const span = Math.floor(10 ** (r.unit() * 12));
    const n = r.pick([0, 1, 2, 3, 5, 7, 10, 20]);
    if (r.unit() < 0.5) utc(a, a + span, n);
    else utc(a + span, a, n);
  }
  write("nice.txt", [
    "d3-scale 4.0.2: `linear start stop count : scaleLinear().domain([start, stop]).nice(count).domain()`",
    "and `utc start stop count : scaleUtc().domain([start, stop]).nice(count).domain()` in epoch ms.",
    "Written by scripts/fixtures.mjs (REQ-REF-010); numbers as JavaScript's String(x), -0 as -0.",
  ], lines);
}

// ---- time-ticks.txt ------------------------------------------------------------
const SPANS = [1, 7, 500, 9e3, 6e4, 9e5, 36e5, 864e5, 3 * 864e5, 30 * 864e5, 120 * 864e5, 400 * 864e5, 3650 * 864e5, 29200 * 864e5, 365000 * 864e5];
const timeTicks = [];
{
  const r = rng(3);
  const lines = [];
  const add = (a, b, n) => {
    const t = utcTicks(a, b, n).map(Number);
    timeTicks.push(...t.slice(0, 3));
    lines.push(`${num(a)} ${num(b)} ${num(n)} : ${nums(t)}`);
  };
  const t0 = Date.UTC(2026, 9, 18, 13, 7, 21, 250);
  for (const span of SPANS) for (const n of [0, 1, 2, 3, 5, 10, 20, 50]) {
    add(t0, t0 + span, n);
    add(t0 + span, t0, n);
  }
  for (const n of [0.5, 2.7, -3, NaN]) add(t0, t0 + 864e5, n);
  add(t0, t0, 10);
  add(t0 + 0.75, t0 + 3000.25, 10);
  add(-8.64e15, -8.64e15 + 3650 * 864e5, 10);
  add(8.64e15 - 3650 * 864e5, 8.64e15, 10);
  add(-8.64e15, 8.64e15, 10);
  add(Date.UTC(-500, 5, 1), Date.UTC(1500, 0, 1), 7);
  for (let i = 0; i < 900; i++) {
    const a = Math.floor(Date.UTC(1900, 0, 1) + r.unit() * 200 * 365 * 864e5);
    const span = Math.floor(10 ** (r.unit() * 12));
    const n = r.int(0, 30);
    if (r.unit() < 0.5) add(a, a + span, n);
    else add(a + span, a, n);
  }
  write("time-ticks.txt", [
    "d3-time 3.1.0: `start stop count : utcTicks(start, stop, count)` in epoch ms.",
    "Written by scripts/fixtures.mjs (REQ-REF-010); numbers as JavaScript's String(x).",
  ], lines);
}

// ---- format.txt ----------------------------------------------------------------
{
  const r = rng(4);
  const lines = [];
  const tab = (...xs) => lines.push(xs.join("\t"));
  // JavaScript's own conversions, which d3-format's types call.
  for (let i = 0; i < 800; i++) {
    const x = Math.abs(r.unit() < 0.5 ? r.mag(-25, 22) : r.double());
    const f = r.int(0, 20);
    if (x < 1e21 || r.unit() < 0.1) tab("fixed", num(x), f, x.toFixed(f));
    tab("exponential", num(x), f, x.toExponential(f));
    tab("exponential", num(x), "-", x.toExponential());
    const p = r.int(1, 21);
    tab("precision", num(x), p, x.toPrecision(p));
    tab("string", num(x), String(x));
  }
  const TIES = [0, 0.5, 1.5, 2.5, 1.005, 1.25, 1.35, 0.045, 0.125, 999.9995, 9.995, 0.0000005, 1e21, 9.999999999999999e20, 1e-7, 1e-6, 123456789, 2 ** 53, 2 ** 53 + 2, 1.7976931348623157e308, 5e-324, 2.2250738585072014e-308, 0.1 + 0.2, 1e23, 9.5367431640625e-7];
  for (const x of TIES) {
    for (let f = 0; f <= 20; f++) {
      if (x < 1e21) tab("fixed", num(x), f, x.toFixed(f));
      tab("exponential", num(x), f, x.toExponential(f));
      if (f >= 1) tab("precision", num(x), f, x.toPrecision(f));
    }
    tab("exponential", num(x), "-", x.toExponential());
    tab("string", num(x), String(x));
  }
  // d3-format specifiers in the subset, on their own. `auto` is d3's "".
  const SPECS = [];
  for (const sign of ["", "-", "+", "(", " "]) for (const type of ["f", "s", "%", "e", "d"]) SPECS.push(sign + type);
  for (const type of ["f", "s", "%", "e", "d"]) {
    for (const p of [0, 1, 2, 6, 20, 21]) SPECS.push(`.${p}${type}`, `,.${p}~${type}`);
    SPECS.push(`$,${type}`, `~${type}`, `+$,.2~${type}`, `(,.1${type}`);
  }
  SPECS.push("auto", ".3~f", ",.0f", "$.2f", ".0%", ",.4s", ".1s", ".21s");
  const VALUES = [...TIES, ...TIES.map((x) => -x), -1e-9, -0.004, 0.0004, 1234.5, -1234567.891, 999999.5, 0.000999, 1e-24, 1e-27, 1e27, 999.95e3, 12345678901234567890];
  for (let i = 0; i < 16; i++) VALUES.push((r.unit() < 0.5 ? -1 : 1) * r.mag(-26, 26));
  const keep = (spec, x, label) => {
    // glyphplot drops the sign of a negative that rounds to zero under "+"
    // too, where d3 keeps it (REQ-FMT-030); NaN and ±∞ read as the locale's nan.
    if (!Number.isFinite(x) || label.includes("Infinity")) return false;
    if (spec.includes("+") && (x < 0 || Object.is(x, -0)) && !/[1-9]/.test(label)) return false;
    return true;
  };
  for (const spec of SPECS) {
    const f = format(spec === "auto" ? "" : spec);
    for (const x of VALUES) {
      const label = f(x);
      if (keep(spec, x, label)) tab("format", spec, num(x), label);
    }
  }
  // d3-scale's tickFormat over ticks: one format an axis (REQ-FMT-010).
  const TICK_SPECS = ["auto", "s", "~s", ".2s", "f", ".1f", "%", "~%", "e", ".3e", "d", ",d", "$,.2f", "(f", "+f", " ,~f"];
  const tick = (a, b, n, spec) => {
    const s = scaleLinear().domain([a, b]);
    const f = s.tickFormat(n, spec === "auto" ? undefined : spec);
    let t;
    try {
      t = s.ticks(n);
    } catch {
      return; // see ticks.txt: d3 throws on subnormal steps
    }
    const labels = t.map(f);
    if (t.every((x, i) => keep(spec, x, labels[i]))) tab("tick", num(a), num(b), num(n), spec, ...labels);
  };
  for (const [a, b] of withReversed(DOMAINS)) for (const n of [1, 5, 10]) for (const spec of TICK_SPECS) tick(a, b, n, spec);
  for (let i = 0; i < 500; i++) {
    const mag = 10 ** r.int(-12, 12);
    const a = (r.unit() * 2 - 1) * mag;
    const b = a + r.unit() * mag * 3;
    tick(a, b, r.int(1, 20), r.pick(TICK_SPECS));
  }
  write("format.txt", [
    "Tab-separated. JavaScript (Node 26): `fixed x f x.toFixed(f)`, `exponential x f|- x.toExponential(f)`, `precision x p x.toPrecision(p)`, `string x String(x)`.",
    "d3-format 3.1.2 with the plain locale (REQ-I18N-030): `format specifier x label` (auto is d3's \"\").",
    "d3-scale 4.0.2: `tick start stop count specifier labels…`, scaleLinear().domain([start, stop]).tickFormat(count, specifier) over its ticks(count).",
    "Written by scripts/fixtures.mjs (REQ-REF-010); numbers as JavaScript's String(x), -0 as -0.",
  ], lines);
}

// ---- time-format.txt -------------------------------------------------------------
{
  const r = rng(5);
  const lines = [];
  const label = scaleUtc().tickFormat();
  const times = [...timeTicks];
  for (let i = 0; i < 400; i++) times.push(Math.floor(Date.UTC(-2000, 0, 1) + r.unit() * 14000 * 365 * 864e5));
  for (let i = 0; i < 400; i++) {
    // Instants on round boundaries, which pick the coarser labels.
    const day = Date.UTC(1950 + r.int(0, 100), r.int(0, 11), r.int(1, 28));
    times.push(day + r.pick([0, 0, 0, r.int(1, 23) * 36e5, r.int(1, 59) * 6e4, r.int(1, 59) * 1e3, r.int(1, 999)]));
  }
  times.push(0, -1, 0.5, -0.5, 8.64e15, -8.64e15, Date.UTC(0, 0, 1), Date.UTC(12026, 0, 1), Date.UTC(-5, 6, 1));
  for (const t of times) lines.push(`label\t${num(t)}\t${label(new Date(t))}`);
  const PATTERNS = ["%Y-%m-%d %H:%M:%S.%L", "%a %A %b %B", "%d %e %-d %_d %0e", "%I %p %-I %H", "%j %-j", "%Z", "%%", "%Y", "at %H o'clock", "%I:%M", ".%L", ":%S"];
  for (const p of PATTERNS) {
    const f = utcFormat(p);
    for (let i = 0; i < 60; i++) {
      const t = r.pick(times);
      lines.push(`strftime\t${p}\t${num(t)}\t${f(new Date(t))}`);
    }
  }
  write("time-format.txt", [
    "Tab-separated. d3-scale 4.0.2: `label t scaleUtc().tickFormat()(new Date(t))`, the multi-scale UTC labels.",
    "d3-time-format 4.1.0: `strftime pattern t utcFormat(pattern)(new Date(t))`.",
    "Written by scripts/fixtures.mjs (REQ-REF-010); numbers as JavaScript's String(x).",
  ], lines);
}
