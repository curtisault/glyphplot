// Throwaway (see ../README.md). Min and median over runs of each case's per-run median frame time.
import { readFileSync } from "node:fs";
const rows = readFileSync(process.argv[2] ?? "results.jsonl", "utf8").trim().split("\n").map((l) => JSON.parse(l));
const by = new Map();
for (const r of rows) {
  if (r.digits) { console.log("digits", JSON.stringify(r.digits)); continue; }
  const k = `${r.cols}x${r.rows} ${r.kind} ${r.model} ${r.renderer}${r.slots ? "/" + r.slots : ""}`;
  if (!by.has(k)) by.set(k, []);
  by.get(k).push(r);
}
const med = (a) => { const s = [...a].sort((x, y) => x - y); return s[Math.floor(s.length / 2)]; };
console.log("case | runs | frame ms: min of medians | median of medians | median p95 | atlas hit rate | clears/60 frames | distinct (glyph, colour) a frame");
for (const [k, rs] of by) {
  const m = rs.map((r) => r.median);
  const hit = rs[0].hitRate == null ? "—" : (100 * med(rs.map((r) => r.hitRate))).toFixed(1) + " %";
  console.log(`${k} | ${rs.length} | ${Math.min(...m).toFixed(1)} | ${med(m).toFixed(1)} | ${med(rs.map((r) => r.p95)).toFixed(1)} | ${hit} | ${rs[0].clears ?? "—"} | ${rs[0].distinctPerFrame}`);
}
