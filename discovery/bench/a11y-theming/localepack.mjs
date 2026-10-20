// What a "locale pack" built by the shell from Intl would hold, and what it costs:
// the number symbols and digits, and month, weekday and day-period names, read with
// formatToParts once per locale and handed to wasm as JSON. Also times Intl formatting
// of tick labels. Throwaway (see README.md).
// Run: mise exec -- node discovery/bench/a11y-theming/localepack.mjs
const pack = (loc) => {
  const nf = new Intl.NumberFormat(loc);
  const parts = nf.formatToParts(-1234567.891);
  const get = (t) => parts.filter((p) => p.type === t).map((p) => p.value);
  const digits = new Intl.NumberFormat(loc, { useGrouping: false }).format(9876543210);
  const pct = new Intl.NumberFormat(loc, { style: "percent" }).formatToParts(0.5).map((p) => (p.type === "integer" ? "#" : p.value)).join("");
  const dtf = (o) => new Intl.DateTimeFormat(loc, { timeZone: "UTC", ...o });
  const months = (w) => Array.from({ length: 12 }, (_, m) => dtf({ month: w }).format(Date.UTC(2026, m, 15)));
  const days = (w) => Array.from({ length: 7 }, (_, d) => dtf({ weekday: w }).format(Date.UTC(2026, 9, 18 + d)));
  const order = dtf({ year: "numeric", month: "short", day: "numeric" }).formatToParts(Date.UTC(2026, 9, 18)).map((p) => (p.type === "literal" ? p.value : `{${p.type}}`)).join("");
  const hm = dtf({ hour: "numeric", minute: "2-digit" }).formatToParts(Date.UTC(2026, 9, 18, 14, 5)).map((p) => (p.type === "literal" ? p.value : `{${p.type}}`)).join("");
  return {
    locale: nf.resolvedOptions().locale,
    numberingSystem: nf.resolvedOptions().numberingSystem,
    minus: get("minusSign").join(""),
    group: get("group")[0] ?? "",
    decimal: get("decimal")[0] ?? "",
    groupSizes: get("integer").map((s) => s.length),
    digits: [...digits].reverse().join(""),
    percent: pct,
    monthsShort: months("short"),
    monthsLong: months("long"),
    weekdaysShort: days("short"),
    date: order,
    time: hm,
    dir: new Intl.Locale(loc).getTextInfo?.().direction ?? new Intl.Locale(loc).textInfo?.direction,
  };
};
const locs = ["en-US", "de-DE", "fr-FR", "en-IN", "ar-EG", "fa-IR", "he-IL", "ja-JP", "zh-CN", "hi-IN-u-nu-deva", "th-TH"];
let t0 = performance.now();
const packs = locs.map(pack);
const buildMs = (performance.now() - t0) / locs.length;
for (const p of packs) console.log(JSON.stringify(p));
const sizes = packs.map((p) => new TextEncoder().encode(JSON.stringify(p)).length);
console.log(`\npack JSON bytes: min ${Math.min(...sizes)} max ${Math.max(...sizes)}; build ${buildMs.toFixed(2)} ms a locale (first build, cold)`);
// tick labels: 20 labels a frame with a cached formatter, many frames
for (const loc of ["en-US", "ar-EG"]) {
  const nf = new Intl.NumberFormat(loc, { maximumFractionDigits: 2 });
  const vals = Array.from({ length: 20 }, (_, i) => i * 12.5 - 100);
  for (let r = 0; r < 3; r++) {
    t0 = performance.now();
    let n = 0;
    for (let f = 0; f < 10000; f++) for (const v of vals) n += nf.format(v + f).length;
    const us = ((performance.now() - t0) * 1000) / 10000;
    console.log(`${loc} run ${r + 1}: 20 labels a frame cost ${us.toFixed(1)} µs (cached NumberFormat, 10000 frames)`);
  }
  t0 = performance.now();
  for (let i = 0; i < 1000; i++) new Intl.NumberFormat(loc).format(i);
  console.log(`${loc}: a new NumberFormat per label costs ${((performance.now() - t0)).toFixed(1)} µs each (1000)`);
}
