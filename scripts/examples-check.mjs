// `mise run examples:check` (REQ-SITE-080, REQ-SITE-090, REQ-SITE-110,
// REQ-SITE-050): the gallery's coverage, from site/examples/manifest.json,
// the example modules and web/pkg/modules.js (the built chart types and
// modes). It fails, naming each problem, when
//
// - a built chart type has fewer than three examples;
// - a built mode, or a feature of the plan's list built at this milestone,
//   has none;
// - an example names a feature not on the list, a type that is not built, or
//   a mode its type does not have;
// - an example names a dataset missing from datasets/manifest.json;
// - an example in the manifest has no module, a module has no manifest
//   entry, or index.ts lists them in another order;
// - generated data has no seed, or a seed has no generated data (REQ-SITE-110);
//   a live dataset has no `feed` naming its source and licence (REQ-SITE-050);
// - the first two examples are not the two live candlesticks (REQ-SITE-040);
// - the gallery holds fewer than 50 examples at v0.7.0, 85 from v0.12.0
//   (REQ-SITE-090).
//
// Examples of a later milestone than GLYPHPLOT_MILESTONE (default v0.7.0)
// are listed but not counted. Prints the counts by type, mode, feature and
// domain. Reads files only.
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const url = (p) => pathToFileURL(join(ROOT, p)).href;
const MILESTONE = process.env.GLYPHPLOT_MILESTONE ?? "v0.7.0";

/**
 * The plan's "Features the gallery must cover" (docs/IMPLEMENTATION_PLAN.md),
 * in its words, with the milestone each is built in. `null`: in the core but
 * reachable from no chart yet, so not yet required.
 */
const FEATURES = [
  ["streaming", "v0.6.0"],
  ["pause", "v0.6.0"],
  ["follow the live end", "v0.6.0"],
  ["late and duplicate rows", "v0.6.0"],
  ["retention", "v0.6.0"],
  ["linking", "v0.5.0"],
  ["brushing to drive another chart", "v0.5.0"],
  ["the view in a URL", "v0.3.0"],
  ["the event log", "v0.3.0"],
  ["panes", "v0.5.0"], // the candles' volume pane
  ["dual y axes", null], // the core lays them out (REQ-AXIS-010); no chart reads `axes` (SPEC §5.4, Phase 4 "partly covered")
  ["annotations (lines, bands, labels, markers)", "v0.5.0"],
  ["log, symlog and index time scales", "v0.5.0"], // the index time scale (candles `xScale`); log and symlog are reachable from no chart yet
  ["time zones", "v0.5.0"],
  ["locales", "v0.3.0"],
  ["one ink", "v0.3.0"],
  ["paper", "v0.3.0"],
  ["CSS custom-property themes", "v0.3.0"],
  ["a theme object", "v0.3.0"],
  ["sizing and resize", "v0.3.0"],
  ["a million points", "v0.3.0"],
  ["data from CSV, JSON and Arrow `src`", "v0.6.0"],
  ["typed arrays through the JS API", "v0.3.0"],
  ["plain HTML with no build", "v0.3.0"],
  ["React 19 JSX", null], // no JSX typings and no React in the site before packaging (v0.11.0)
  ["keyboard-only use", "v0.3.0"],
  ["touch scrubbing", "v0.3.0"],
  ["the screen-reader table", "v0.3.0"],
  ["reduced motion", "v0.3.0"],
  ["exports", "v0.8.0"],
  ["the terminal", "v0.8.0"],
  ["server-rendered first frames", "v0.8.0"],
  ["domain packs", "v0.10.0"],
];

/** The first two examples (REQ-SITE-040, ADR-136). */
const FIRST = ["live-market", "wikimedia-edits"];
/** Files in site/examples/ that are not examples. */
const NOT_EXAMPLES = new Set(["api.ts", "index.ts", "element.ts"]);

const ver = (v) => (v ?? "v999.0.0").replace(/^v/, "").split(".").map(Number);
const atOrBefore = (a, b) => {
  const [x, y] = [ver(a), ver(b)];
  for (let i = 0; i < 3; i++) if (x[i] !== y[i]) return x[i] < y[i];
  return true;
};
const builtAt = (from) => from !== null && atOrBefore(from, MILESTONE);

const problems = [];
const fail = (m) => problems.push(m);

// ---------------------------------------------------------------- inputs

if (!existsSync(join(ROOT, "web/pkg/modules.js"))) {
  console.error("examples:check: no web/pkg/modules.js: run `mise run build:wasm`");
  process.exit(1);
}
const built = await import(url("web/pkg/modules.js"));
const types = new Map(built.charts.map((m) => [m.slug, m.modes]));
const manifest = JSON.parse(readFileSync(join(ROOT, "site/examples/manifest.json"), "utf8"));
const { examples } = await import(url("site/examples/index.ts"));

const datasetsFrom = "datasets/manifest.json";
if (!existsSync(join(ROOT, datasetsFrom))) {
  console.error(`examples:check: no ${datasetsFrom}`);
  process.exit(1);
}
/** datasets/manifest.json's entries by id (REQ-DATASET-010). */
const DATASETS = new Map(JSON.parse(readFileSync(join(ROOT, datasetsFrom), "utf8")).datasets.map((d) => [d.id, d]));

// ---------------------------------------------------------------- modules and entries

const files = readdirSync(join(ROOT, "site/examples")).filter((f) => f.endsWith(".ts") && !NOT_EXAMPLES.has(f));
const moduleSlugs = new Set();
for (const f of files) {
  const mod = await import(url(`site/examples/${f}`));
  const slug = f.replace(/\.ts$/, "");
  if (!mod.example) fail(`site/examples/${f} exports no \`example\``);
  else if (mod.example.slug !== slug) fail(`site/examples/${f}: its example's slug is "${mod.example.slug}", not "${slug}"`);
  moduleSlugs.add(slug);
}
const seen = new Set();
for (const e of manifest) {
  if (seen.has(e.slug)) fail(`manifest: "${e.slug}" is listed twice`);
  seen.add(e.slug);
  if (!/^[a-z0-9]+(-[a-z0-9]+)*$/.test(e.slug)) fail(`manifest: "${e.slug}" is not kebab-case`);
  if (!moduleSlugs.has(e.slug)) fail(`manifest: "${e.slug}" has no module site/examples/${e.slug}.ts`);
}
for (const s of moduleSlugs) if (!seen.has(s)) fail(`site/examples/${s}.ts has no manifest entry`);
const order = examples.map((x) => x.slug);
if (order.join() !== manifest.map((e) => e.slug).join()) fail("index.ts and manifest.json list the examples in different orders, or different examples");
if (manifest[0]?.slug !== FIRST[0] || manifest[1]?.slug !== FIRST[1]) fail(`the first two examples must be ${FIRST.join(" and ")} (REQ-SITE-040)`);

// ---------------------------------------------------------------- each entry

const KEYS = ["slug", "title", "what", "types", "modes", "domain", "features", "datasets", "milestone", "feed"];
const featureFrom = new Map(FEATURES);
const counted = manifest.filter((e) => atOrBefore(e.milestone, MILESTONE));
for (const e of manifest) {
  for (const k of KEYS) if (!(k in e)) fail(`${e.slug}: no \`${k}\``);
  if (!/^v\d+\.\d+\.\d+$/.test(e.milestone ?? "")) fail(`${e.slug}: milestone "${e.milestone}" is not vX.Y.Z`);
  for (const f of e.features ?? []) if (!featureFrom.has(f)) fail(`${e.slug}: "${f}" is not a feature of the plan's list`);
  for (const d of e.datasets ?? []) if (!DATASETS.has(d)) fail(`${e.slug}: dataset "${d}" is not in ${datasetsFrom}`);
  if ((e.datasets ?? []).length === 0) fail(`${e.slug}: names no dataset`);
  const ex = examples.find((x) => x.slug === e.slug);
  const synthetic = (e.datasets ?? []).some((d) => DATASETS.get(d)?.storage === "generated");
  if (ex && synthetic && ex.seed === null) fail(`${e.slug}: generated data with no seed (REQ-SITE-110)`);
  if (ex && !synthetic && ex.seed !== null) fail(`${e.slug}: a seed but no "synthetic" dataset (REQ-SITE-110)`);
  const live = (e.datasets ?? []).some((d) => DATASETS.get(d)?.storage === "live");
  if (live && !(e.feed && e.feed.source && e.feed.url && e.feed.licence)) fail(`${e.slug}: a live feed without its source, URL and licence (REQ-SITE-050)`);
  if (!atOrBefore(e.milestone, MILESTONE)) continue;
  for (const t of e.types ?? []) if (!types.has(t)) fail(`${e.slug}: type "${t}" is not built (web/pkg/modules.js)`);
  for (const m of e.modes ?? []) if (!(e.types ?? []).some((t) => (types.get(t) ?? []).includes(m))) fail(`${e.slug}: mode "${m}" is not a mode of ${(e.types ?? []).join(" or ")}`);
  for (const f of e.features ?? []) if (featureFrom.has(f) && !builtAt(featureFrom.get(f))) fail(`${e.slug}: "${f}" is not built at ${MILESTONE}; move the example to its milestone`);
}

// ---------------------------------------------------------------- coverage

const tally = (key) => {
  const n = new Map();
  for (const e of counted) for (const v of [e[key]].flat()) n.set(v, (n.get(v) ?? 0) + 1);
  return n;
};
const byType = tally("types");
for (const t of types.keys()) if ((byType.get(t) ?? 0) < 3) fail(`type ${t}: ${byType.get(t) ?? 0} examples, fewer than 3`);
const byMode = new Map();
for (const [t, modes] of types) {
  for (const m of modes) {
    const n = counted.filter((e) => e.types.includes(t) && e.modes.includes(m)).length;
    byMode.set(`${t}/${m}`, n);
    if (n === 0) fail(`mode ${t}/${m}: no example`);
  }
}
const byFeature = tally("features");
for (const [f, from] of FEATURES) if (builtAt(from) && !byFeature.get(f)) fail(`feature "${f}" (built ${from}): no example`);
const floor = atOrBefore("v0.12.0", MILESTONE) ? 85 : 50;
if (counted.length < floor) fail(`${counted.length} examples at ${MILESTONE}, fewer than ${floor} (REQ-SITE-090)`);

// ---------------------------------------------------------------- report

const line = (m) => [...m].map(([k, v]) => `${k} ${v}`).join(", ");
console.log(`examples:check at ${MILESTONE}: ${counted.length} examples counted (${manifest.length} listed); datasets from ${datasetsFrom}`);
console.log(`  by type: ${line(byType)}`);
console.log(`  by mode: ${line(byMode)}`);
console.log(`  by feature: ${FEATURES.map(([f, from]) => `${f} ${byFeature.get(f) ?? 0}${builtAt(from) ? "" : " (not built)"}`).join("; ")}`);
console.log(`  by domain: ${line(tally("domain"))}`);
console.log(`  by dataset: ${line(tally("datasets"))}`);
if (problems.length > 0) {
  console.error(`examples:check: ${problems.length} problem(s):\n  ${problems.join("\n  ")}`);
  process.exit(1);
}
console.log("examples:check: ok");
