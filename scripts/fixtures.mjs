// Captures the pinned d3 packages' output for the core's ports to be compared
// with, bit for bit, by crates/glyphplot/tests/reference.rs (REQ-REF-010).
//
//   npm ci --prefix scripts/fixtures
//   node scripts/fixtures.mjs
//
// Writes crates/glyphplot/tests/fixtures/{ticks,nice,time-ticks,format,
// time-format,log,zone-ticks,lttb}.txt. The cases come from a seeded
// SplitMix64, so a rerun writes the same bytes. Numbers are written as
// JavaScript's String(x), which is the shortest form that reads back as the
// same double (Rust's str::parse reads it back bitwise), with -0 as "-0".
//
// zone-ticks.txt is captured by child Node processes, one a zone, each run
// with TZ=<zone> (`node scripts/fixtures.mjs --zone <zone>`), so d3's
// scaleTime runs in that local time; each records the zone table it used,
// built from Intl as the shell builds it (REQ-TZ-020, REQ-TZ-030).
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";

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
const { scaleLinear, scaleUtc, scaleLog, scaleTime } = await load("d3-scale");
const { utcTicks } = await load("d3-time");
const { utcFormat } = await load("d3-time-format");

// The core's default locale (REQ-I18N-030): d3's en-US with "-" for the
// minus and an empty NaN. d3-scale's tickFormat uses this default too.
const { format } = formatDefaultLocale({ decimal: ".", thousands: ",", grouping: [3], currency: ["$", ""], minus: "-", nan: "" });

// The zones zone-ticks.txt covers: daylight saving in both hemispheres
// (Lord Howe's shift is half an hour), a half-hour offset without it, and UTC.
const ZONES = ["America/New_York", "Europe/London", "Australia/Lord_Howe", "Asia/Kolkata", "UTC"];
// The span every zone table covers, and every zone case lies in.
const ZONE_SPAN = [Date.UTC(2023, 0, 1), Date.UTC(2029, 0, 1)];
const child = process.argv[2] === "--zone";

const out =new URL("../crates/glyphplot/tests/fixtures/", import.meta.url);
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

// A zone's table of offset transitions over [from, to], as the shell builds
// it with Intl (REQ-TZ-020, discovery/scales.md): the offset read with
// `timeZoneName: "longOffset"` every day from `from`, each change bisected to
// the minute. The first transition is at `from`.
function zoneTable(name, from, to) {
  const f = new Intl.DateTimeFormat("en-US", { timeZone: name, timeZoneName: "longOffset" });
  const offset = (t) => {
    const s = f.formatToParts(t).find((p) => p.type === "timeZoneName").value; // "GMT-04:00", "GMT"
    if (s === "GMT") return 0;
    const sign = s[3] === "-" ? -1 : 1;
    return sign * (Number(s.slice(4, 6)) * 60 + Number(s.slice(7, 9)));
  };
  const transitions = [{ at: from, offset: offset(from) }];
  for (let t = from + 864e5; t <= to; t += 864e5) {
    const last = transitions[transitions.length - 1].offset;
    if (offset(t) === last) continue;
    let lo = t - 864e5, hi = t;
    while (hi - lo > 6e4) {
      const mid = Math.floor((lo + hi) / 2 / 6e4) * 6e4;
      if (offset(mid) === last) lo = mid;
      else hi = mid;
    }
    transitions.push({ at: hi, offset: offset(hi) });
  }
  return { name, transitions, covering: [from, to] };
}

// The cases of one zone, run in a child process whose TZ is that zone:
// d3-scale's scaleTime().ticks(count), .nice(count) and .tickFormat() under
// it, around each daylight-saving change of the table and at random.
function zoneCases(zone) {
  // The process's zone must be the one asked for (ICU may give Asia/Kolkata as Asia/Calcutta).
  const resolved = Intl.DateTimeFormat().resolvedOptions().timeZone;
  const asked = new Intl.DateTimeFormat("en-US", { timeZone: zone }).resolvedOptions().timeZone;
  if (resolved !== asked) throw new Error(`TZ=${zone} gave ${resolved}`);
  const table = zoneTable(zone, ...ZONE_SPAN);
  const r = rng(10 + ZONES.indexOf(zone));
  const lines = [`zone ${zone} ${JSON.stringify(table)}`];
  const label = scaleTime().tickFormat();
  const add = (a, b, n) => {
    const s = scaleTime().domain([a, b]);
    const t = s.ticks(n).map(Number);
    lines.push(`ticks ${num(a)} ${num(b)} ${num(n)} : ${nums(t)}`);
    lines.push(`nice ${num(a)} ${num(b)} ${num(n)} : ${nums(s.copy().nice(n).domain().map(Number))}`);
    for (const x of t.slice(0, 4)) lines.push(`label\t${num(x)}\t${label(new Date(x))}`);
  };
  const SPANS = [6e4, 9e5, 36e5, 3 * 36e5, 12 * 36e5, 864e5, 1.5 * 864e5, 3 * 864e5, 10 * 864e5, 45 * 864e5, 200 * 864e5, 800 * 864e5];
  const [lo, hi] = [ZONE_SPAN[0] + 30 * 864e5, ZONE_SPAN[1] - 1200 * 864e5];
  // Around each change: domains that start a little before it and cross it.
  for (const { at } of table.transitions.slice(1)) {
    if (at < lo || at > hi) continue;
    for (let k = 0; k < 6; k++) {
      const span = r.pick(SPANS);
      const a = Math.floor(at - span * r.unit());
      const n = r.pick([1, 2, 3, 5, 7, 10, 20, 50]);
      // Only domains inside the table's span: outside it the table does not hold the zone's offsets.
      if (a < lo || a + span > ZONE_SPAN[1]) continue;
      if (r.unit() < 0.5) add(a, a + span, n);
      else add(a + span, a, n);
    }
  }
  for (let i = 0; i < 300; i++) {
    const a = Math.floor(lo + r.unit() * (hi - lo));
    const span = Math.floor(10 ** (3 + r.unit() * 8)); // a second to about three years
    const n = r.int(0, 30);
    if (r.unit() < 0.5) add(a, a + span, n);
    else add(a + span, a, n);
  }
  return lines;
}

if (child) {
  // Written to fd 1 synchronously: process.stdout to a pipe is asynchronous and exit would cut it short.
  writeFileSync(1, zoneCases(process.argv[3]).join("\n") + "\n");
  process.exit(0);
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

// ---- log.txt -----------------------------------------------------------------------
{
  const r = rng(6);
  const lines = [];
  const LOG_SPECS = ["auto", "s", "~s", ".2s", "$,.2f", ".1e", "d", "~f"];
  const add = (base, a, b, n, spec) => {
    const s = scaleLog().base(base).domain([a, b]);
    let t;
    try {
      t = s.ticks(n);
    } catch {
      return; // see ticks.txt: d3-array throws on subnormal steps
    }
    lines.push(`ticks ${num(base)} ${num(a)} ${num(b)} ${num(n)} : ${nums(t)}`);
    lines.push(`nice ${num(base)} ${num(a)} ${num(b)} : ${nums(s.copy().nice().domain())}`);
    const f = s.tickFormat(n, spec === "auto" ? undefined : spec);
    lines.push(["format", num(base), num(a), num(b), num(n), spec, ...t.map(f)].join("\t"));
  };
  const BASES = [10, 2, 3, 16];
  const LOG_DOMAINS = [[1, 10], [1, 1e6], [0.3, 4000], [1e-9, 1e9], [5, 7], [2, 3], [1, 1024], [0.001, 0.002], [1e-300, 1e300], [123, 123456], [-1e5, -2], [-10, -1], [1, 1]];
  for (const base of BASES) {
    for (const [a, b] of withReversed(LOG_DOMAINS)) for (const n of [1, 2, 5, 10, 20]) add(base, a, b, n, LOG_SPECS[(lines.length / 3) % LOG_SPECS.length]);
  }
  for (let i = 0; i < 800; i++) {
    const base = r.pick(BASES);
    const sign = r.unit() < 0.2 ? -1 : 1;
    const a = sign * r.mag(-12, 12);
    const b = a * 10 ** (r.unit() * r.int(0, 15));
    const n = r.int(1, 30);
    if (r.unit() < 0.5) add(base, a, b, n, r.pick(LOG_SPECS));
    else add(base, b, a, n, r.pick(LOG_SPECS));
  }
  write("log.txt", [
    "d3-scale 4.0.2, scaleLog().base(base).domain([a, b]): `ticks base a b count : ticks(count)`, `nice base a b : nice().domain()`,",
    "and tab-separated `format base a b count specifier labels…`, tickFormat(count, specifier) over ticks(count) (auto is d3's default).",
    "Written by scripts/fixtures.mjs (REQ-REF-010); numbers as JavaScript's String(x), -0 as -0.",
  ], lines);
}

// ---- zone-ticks.txt ---------------------------------------------------------------
{
  const self = fileURLToPath(import.meta.url);
  const lines = [];
  for (const zone of ZONES) {
    const text = execFileSync(process.execPath, [self, "--zone", zone], { env: { ...process.env, TZ: zone }, encoding: "utf8", maxBuffer: 1 << 28 });
    lines.push(...text.trimEnd().split("\n"));
  }
  write("zone-ticks.txt", [
    "d3-scale 4.0.2's scaleTime() run in a child Node process with TZ=<zone>, one block a zone (REQ-TZ-030).",
    "`zone name table` opens a block: the zone table (zone.schema.json) built from Intl over its span, as the shell builds it.",
    "`ticks a b count : scaleTime().domain([a, b]).ticks(count)` and `nice a b count : .nice(count).domain()` in epoch ms;",
    "tab-separated `label t scaleTime().tickFormat()(new Date(t))`.",
    "Written by scripts/fixtures.mjs (REQ-REF-010); numbers as JavaScript's String(x).",
  ], lines);
}

// ---- lttb.txt ------------------------------------------------------------------------
{
  // The reference as published, flot-downsample af872e0 (MIT): an IIFE over
  // jQuery that registers a plot plugin, run with a stand-in for jQuery.
  const plugins = [];
  const source = readFileSync(new URL("./fixtures/lttb-reference.js", import.meta.url), "utf8");
  new Function("jQuery", source)({ plot: { plugins } });
  const plot = { hooks: { processRawData: [] } };
  plugins[0].init(plot);
  const lttb = (data, threshold) => {
    const series = { data, downsample: { threshold } };
    plot.hooks.processRawData[0](plot, series);
    return series.data;
  };
  // The series, from SplitMix64 as tests/reference.rs makes them: a walk with
  // x steps of 0.5 to 1.5, or spikes over noise. Each point carries its index.
  const series = (kind, seed, n) => {
    const r = rng(seed);
    const data = [];
    let x = 0, y = 0;
    for (let i = 0; i < n; i++) {
      x += 0.5 + r.unit();
      if (kind === "walk") y += r.unit() - 0.5;
      else y = r.unit() < 0.02 ? 50 * r.unit() : r.unit();
      data.push([x, y, i]);
    }
    return data;
  };
  const lines = [];
  let seed = 100;
  for (const kind of ["walk", "spike"]) {
    for (const n of [1, 2, 3, 10, 1000, 10000, 100000]) {
      const data = series(kind, seed, n);
      for (const threshold of [0, 1, 2, 3, 5, 100, 640, 999, 5000]) {
        // Every index comes back for a threshold of 0 or n and more: kept short.
        if ((threshold === 0 || threshold >= n) && n > 10) continue;
        lines.push(`${kind} ${seed} ${n} ${threshold} : ${lttb(data, threshold).map((p) => p[2]).join(" ")}`);
      }
      seed++;
    }
  }
  write("lttb.txt", [
    "flot-downsample af872e0's largestTriangleThreeBuckets (scripts/fixtures/lttb-reference.js, MIT):",
    "`kind seed n threshold : the indices it keeps` over the series scripts/fixtures.mjs makes from SplitMix64.",
    "Written by scripts/fixtures.mjs (REQ-REF-010).",
  ], lines);
}
