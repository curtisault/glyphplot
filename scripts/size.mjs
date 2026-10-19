// mise run size: each module's wasm and JS glue, raw and gzip -9, and the
// total; fails when a module's wasm passes its budget, warns past half of it
// (REQ-SIZE-010 to 040). Budgets come from web/pkg/budgets.json, and
// WASM_GZ_LIMIT (bytes) overrides every one.
//
//   node scripts/size.mjs                     the table and the gate
//   node scripts/size.mjs --write             only web/pkg/sizes.json (build:wasm)
//   node scripts/size.mjs --write --pkg DIR   the same for a staged build in DIR
import { appendFileSync, existsSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { pathToFileURL } from "node:url";
import { resolve } from "node:path";

const at = process.argv.indexOf("--pkg");
const dir = at >= 0 ? process.argv[at + 1] : undefined;
if (at >= 0 && !dir) {
  console.error("size: --pkg needs a folder");
  process.exit(2);
}
const pkg = dir ? pathToFileURL(resolve(dir) + "/") : new URL("../web/pkg/", import.meta.url);
const read = (path) => readFileSync(new URL(path, pkg));
// gzip -9 of stdin, as `gzip -9 < file | wc -c` counts it (no name in the header).
const gzipped = (bytes) => execFileSync("gzip", ["-9"], { input: bytes, maxBuffer: 1 << 30 }).length;

if (!existsSync(new URL("budgets.json", pkg))) {
  console.error("size: web/pkg/budgets.json is missing; run `mise run build:wasm`");
  process.exit(1);
}
const budgets = JSON.parse(read("budgets.json"));
const modules = readdirSync(pkg, { withFileTypes: true })
  .filter((d) => d.isDirectory() && existsSync(new URL(`${d.name}/glyphplot_bg.wasm`, pkg)))
  .map((d) => d.name)
  .sort();

const rows = modules.map((name) => {
  const wasm = read(`${name}/glyphplot_bg.wasm`);
  const js = read(`${name}/glyphplot.js`);
  return { name, wasm: wasm.length, wasmGz: gzipped(wasm), js: js.length, jsGz: gzipped(js) };
});

if (process.argv.includes("--write")) {
  writeFileSync(new URL("sizes.json", pkg), JSON.stringify(Object.fromEntries(rows.map((r) => [r.name, r.wasmGz])), null, 2) + "\n");
  process.exit(0);
}

const override = process.env.WASM_GZ_LIMIT;
if (override !== undefined && !/^\d+$/.test(override)) {
  console.error(`size: WASM_GZ_LIMIT must be a whole number of bytes, not ${JSON.stringify(override)}`);
  process.exit(1);
}
const limitOf = (name) => (override !== undefined ? Number(override) : budgets[name]?.gzip);

const n = (x) => x.toLocaleString("en-US");
const lines = [];
const out = (line) => (console.log(line), lines.push(line));
const cols = ["module", "wasm", "wasm gz", "js", "js gz", "limit"];
const table = [cols, ...rows.map((r) => [r.name, n(r.wasm), n(r.wasmGz), n(r.js), n(r.jsGz), n(limitOf(r.name) ?? 0)])];
const sum = (k) => rows.reduce((a, r) => a + r[k], 0);
table.push(["total", n(sum("wasm")), n(sum("wasmGz")), n(sum("js")), n(sum("jsGz")), ""]);
const widths = cols.map((_, i) => Math.max(...table.map((row) => row[i].length)));
for (const row of table) out(row.map((cell, i) => (i === 0 ? cell.padEnd(widths[i]) : cell.padStart(widths[i]))).join("  ").trimEnd());

const problems = [];
for (const r of rows) {
  const limit = limitOf(r.name);
  if (limit === undefined) problems.push(`size: ${r.name} has no budget in web/pkg/budgets.json`);
  else if (r.wasmGz > limit) problems.push(`size: ${r.name} is over budget: ${n(r.wasmGz)} B gzipped, limit ${n(limit)} B`);
  else if (r.wasmGz > limit / 2) out(`size: warning: ${r.name} is past half its budget: ${n(r.wasmGz)} B gzipped, limit ${n(limit)} B`);
}
if (rows.length === 0) problems.push("size: web/pkg holds no module; run `mise run build:wasm`");
problems.forEach((p) => (console.error(p), lines.push(p)));

if (process.env.GITHUB_STEP_SUMMARY) appendFileSync(process.env.GITHUB_STEP_SUMMARY, "```\n" + lines.join("\n") + "\n```\n");
process.exit(problems.length ? 1 : 0);
