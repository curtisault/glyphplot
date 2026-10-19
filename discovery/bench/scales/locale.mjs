// Derives a d3-shaped number and time locale from Intl, as the shell would,
// and times it (5 runs of 200). Prints the object for de-DE and its JSON size.
//   node locale.mjs
function derive(tag) {
  const nf = new Intl.NumberFormat(tag);
  const parts = nf.formatToParts(-1234567.5);
  const get = (t) => parts.find((p) => p.type === t)?.value ?? "";
  const cur = new Intl.NumberFormat(tag, { style: "currency", currency: "USD" }).formatToParts(1);
  const ci = cur.findIndex((p) => p.type === "currency");
  const before = ci < cur.findIndex((p) => p.type === "integer");
  const sym = cur[ci].value;
  const month = (o) => Array.from({ length: 12 }, (_, m) => new Intl.DateTimeFormat(tag, { month: o, timeZone: "UTC" }).format(Date.UTC(2026, m, 15)));
  const day = (o) => Array.from({ length: 7 }, (_, d) => new Intl.DateTimeFormat(tag, { weekday: o, timeZone: "UTC" }).format(Date.UTC(2026, 9, 18 + d)));
  const hc = new Intl.DateTimeFormat(tag, { hour: "numeric" }).resolvedOptions().hourCycle;
  return {
    decimal: get("decimal") || ".", thousands: get("group"), grouping: [3], minus: get("minusSign") || "-",
    currency: before ? [sym, ""] : ["", " " + sym], hourCycle: hc,
    months: month("long"), shortMonths: month("short"), days: day("long"), shortDays: day("short"),
  };
}
const runs = [];
for (let r = 0; r < 5; r++) { const s = performance.now(); for (let i = 0; i < 200; i++) derive(["de-DE", "en-US", "ja-JP", "ar-EG"][i % 4]); runs.push((performance.now() - s) / 200); }
runs.sort((a, b) => a - b);
console.log(`derive a locale: min ${runs[0].toFixed(3)} ms median ${runs[2].toFixed(3)} ms`);
const de = derive("de-DE");
console.log(JSON.stringify(de), JSON.stringify(de).length, "bytes");
console.log(JSON.stringify(derive("ja-JP")));
