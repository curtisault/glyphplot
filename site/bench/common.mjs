// What the browser benches share: the load line every table starts and ends
// with (REQ-BENCH-020), the fixtures, and the release build's checks.
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, renameSync, rmSync } from "node:fs";
import { loadavg, cpus } from "node:os";
import { dirname, join } from "node:path";
import { FIXTURES, REPO, SITE_DIST } from "./serve.mjs";

/** `uptime`'s three load averages (1, 5 and 15 minutes), from os.loadavg(), which reads the same counters. */
export function load() {
  return loadavg()
    .map((l) => l.toFixed(2))
    .join(" ");
}

/** The machine line: CPUs and the load. */
export function machine() {
  const c = cpus();
  return `${c[0]?.model ?? "cpu"} × ${c.length}, load averages ${load()} (1/5/15 min, as uptime prints)`;
}

/** Fails with a hint when site/dist lacks a page: the benches run on the release build `mise run build:site` makes. */
export function needBuild(page) {
  if (!existsSync(join(SITE_DIST, page))) {
    console.error(`bench: ${join("site/dist", page)} is missing; \`mise run build:site\` builds the release site first (the mise task does it)`);
    process.exit(2);
  }
  const modules = join(REPO, "web/pkg/modules.js");
  if (existsSync(modules) && /export const fallback = "/.test(readFileSync(modules, "utf8"))) {
    console.error("bench: web/pkg holds the debug build; the benches time the release build (`mise run build:wasm`)");
    process.exit(2);
  }
}

/** target/fixtures, written as the Node tests write it (a staging folder renamed into place) when absent. */
export function ensureFixtures() {
  if (existsSync(join(FIXTURES, "index.json"))) return;
  const stage = `${FIXTURES}.stage.${process.pid}`;
  rmSync(stage, { recursive: true, force: true });
  mkdirSync(dirname(stage), { recursive: true });
  console.log("bench: writing the fixtures into target/fixtures");
  execFileSync("cargo", ["run", "-q", "--profile", "contract", "-p", "glyphplot", "--example", "fixtures", "--", "write", stage], { cwd: REPO, stdio: ["ignore", "ignore", "inherit"] });
  try {
    renameSync(stage, FIXTURES);
  } catch {
    rmSync(stage, { recursive: true, force: true });
  }
}

/** Mean and maximum of `xs`. */
export function stats(xs) {
  let sum = 0;
  let max = 0;
  for (const v of xs) {
    sum += v;
    if (v > max) max = v;
  }
  return { avg: xs.length ? sum / xs.length : 0, worst: max };
}

/** Median of `xs`. */
export function median(xs) {
  const s = [...xs].sort((a, b) => a - b);
  const m = Math.floor(s.length / 2);
  return s.length % 2 ? s[m] : (s[m - 1] + s[m]) / 2;
}

/** A plain text table: a header row, a rule, the rows; columns padded, numbers right-aligned. */
export function table(head, rows) {
  const w = head.map((h, i) => Math.max(h.length, ...rows.map((r) => String(r[i]).length)));
  const num = head.map((_, i) => rows.every((r) => /^[\d.,\s<>—-]*$/.test(String(r[i]))));
  const line = (r) => r.map((v, i) => (num[i] ? String(v).padStart(w[i]) : String(v).padEnd(w[i]))).join("  ").trimEnd();
  return [line(head), w.map((n) => "-".repeat(n)).join("  "), ...rows.map(line)].join("\n");
}
