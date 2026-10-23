#!/usr/bin/env node
// The dataset manifest's checks and HYG's fetch (REQ-DATASET-010, 020, 030,
// ADR-137). `datasets/README.md` says what the manifest holds.
//
//   node scripts/datasets.mjs check        mise run datasets:check
//   node scripts/datasets.mjs fetch        mise run datasets:fetch
//   node scripts/datasets.mjs hash FILE…   a manifest stub for each file
//
// `check` reads no network. It fails when:
//   - an entry lacks a field, or two entries share an id or a path;
//   - a file under one of DATA_DIRS has no entry, or its size or SHA-256
//     differs from its entry's (REQ-DATASET-010);
//   - a committed dataset exceeds 1 MB (1,000,000 B), or a licence is
//     NonCommercial, NoDerivatives or proprietary, or a committed dataset's
//     licence is not on the allow-list: public domain, CC0, CC BY, ODC-By,
//     MIT, BSD (REQ-DATASET-020);
//   - a ShareAlike dataset is committed, or a file of a published crate
//     (`cargo package --list`) or of the npm package (`npm pack --dry-run
//     --json` in web/) is a ShareAlike dataset by path or by SHA-256
//     (REQ-DATASET-030);
//   - a build-time dataset's path is not gitignored, or is tracked;
//   - crates/glyphplot/tests/data/README.md and the manifest disagree on a
//     file, its source URL, its retrieval date or its licence.

import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, readdirSync, renameSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { gunzipSync } from "node:zlib";

/** The repository's root. */
export const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");

/** The manifest, from the root. */
export const MANIFEST = "datasets/manifest.json";

/**
 * The folders whose every file must be a dataset in the manifest, from the
 * root: `datasets/` (committed data the site and examples share),
 * `site/public/data/` (the site's data folder: Vite serves it at `data/`),
 * and the crate's test datasets.
 */
export const DATA_DIRS = ["datasets", "site/public/data", "crates/glyphplot/tests/data"];

/** Files in DATA_DIRS that are not data: the manifest, its notes, licence notices. */
export const NOT_DATA = /^(manifest\.json|README\.md|LICENSE(\.md|\.txt)?|NOTICE(\.md|\.txt)?)$/;

/** The largest committed dataset, bytes (REQ-DATASET-020: 1 MB, taken as 10^6 B). */
export const MAX_COMMITTED = 1_000_000;

/** Where a dataset lives. */
export const STORAGES = ["committed", "build", "live", "generated"];

/** The test datasets' README, checked against the manifest. */
export const TEST_README = "crates/glyphplot/tests/data/README.md";

const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");
const posix = (p) => p.split(sep).join("/");

/**
 * What a licence name is, against the allow-list (REQ-DATASET-020):
 * `allowed` (public domain, CC0, CC BY, ODC-By, MIT, BSD), `sharealike`
 * (allowed only where ShareAlike data may be: served, never committed or
 * packaged), `refused` with the reason (NonCommercial, NoDerivatives,
 * proprietary), or `unknown`.
 */
export function classify(licence) {
  const n = String(licence ?? "")
    .toLowerCase()
    .replace(/[-_/]+/g, " ")
    .replace(/\s+/g, " ")
    .trim();
  if (/\bnon ?commercial\b|\bnc\b/.test(n)) return { kind: "refused", why: "NonCommercial" };
  if (/\bno ?deriv(ative)?s?\b|\bnd\b/.test(n)) return { kind: "refused", why: "NoDerivatives" };
  if (/\bproprietary\b|all rights reserved|\brestricted\b/.test(n)) return { kind: "refused", why: "proprietary" };
  if (/\bshare ?alike\b|\bsa\b/.test(n)) return { kind: "sharealike", why: "ShareAlike" };
  if (/^(public domain|cc0|cc by|creative commons attribution|odc by|odc attribution|mit|bsd)\b/.test(n)) return { kind: "allowed" };
  return { kind: "unknown", why: "not on the allow-list (public domain, CC0, CC BY, ODC-By, MIT, BSD)" };
}

const REQUIRED = ["id", "title", "publisher", "url", "licence", "licenceUrl", "retrieved", "storage"];

/** Problems with the entries' fields alone, as strings. */
export function checkFields(manifest) {
  const out = [];
  if (!manifest || !Array.isArray(manifest.datasets)) return [`${MANIFEST}: no "datasets" array`];
  const ids = new Set();
  const paths = new Map();
  for (const [i, d] of manifest.datasets.entries()) {
    const at = `datasets[${i}]${d?.id ? ` (${d.id})` : ""}`;
    for (const k of REQUIRED) if (typeof d?.[k] !== "string" || d[k].trim() === "") out.push(`${at}: no ${k}`);
    if (typeof d?.id === "string") {
      if (ids.has(d.id)) out.push(`${at}: the id ${d.id} is used twice`);
      ids.add(d.id);
    }
    if (typeof d?.retrieved === "string" && !/^\d{4}-\d{2}-\d{2}$/.test(d.retrieved)) out.push(`${at}: retrieved is not a YYYY-MM-DD date`);
    for (const k of ["url", "licenceUrl"]) if (typeof d?.[k] === "string" && !/^https?:\/\//.test(d[k])) out.push(`${at}: ${k} is not an http(s) URL`);
    if (!STORAGES.includes(d?.storage)) out.push(`${at}: storage is not one of ${STORAGES.join(", ")}`);
    const hasFile = d?.storage === "committed" || d?.storage === "build";
    if (hasFile) {
      if (!Array.isArray(d.paths) || d.paths.length === 0 || d.paths.some((p) => typeof p !== "string" || p === "")) out.push(`${at}: a ${d.storage} dataset needs paths`);
      if (!Number.isInteger(d.size) || d.size < 0) out.push(`${at}: size is not a whole number of bytes`);
      if (typeof d.sha256 !== "string" || !/^[0-9a-f]{64}$/.test(d.sha256)) out.push(`${at}: sha256 is not 64 lower-case hex digits`);
    } else if (d && (d.paths?.length || d.size != null || d.sha256 != null)) {
      out.push(`${at}: a ${d.storage} dataset has no file: paths, size and sha256 must be absent or null`);
    }
    if (d?.storage === "build" && !d.fetch && !d.derivedFrom) out.push(`${at}: a build dataset needs fetch (a URL) or derivedFrom (an id)`);
    for (const p of d?.paths ?? []) {
      if (typeof p !== "string") continue;
      if (p.startsWith("/") || p.split("/").includes("..")) out.push(`${at}: path ${p} is not relative to the repository`);
      if (paths.has(p)) out.push(`${at}: path ${p} is also ${paths.get(p)}'s`);
      paths.set(p, d.id);
    }
  }
  for (const d of manifest.datasets) {
    if (d?.derivedFrom !== undefined && !ids.has(d.derivedFrom)) out.push(`${d.id}: derivedFrom ${d.derivedFrom} is no entry's id`);
  }
  return out;
}

/** Licence and size problems (REQ-DATASET-020) and committed ShareAlike (REQ-DATASET-030). */
export function checkLicences(manifest) {
  const out = [];
  for (const d of manifest.datasets) {
    const v = classify(d.licence);
    if (v.kind === "refused") out.push(`${d.id}: licence "${d.licence}" is ${v.why}: such data is never committed or served (REQ-DATASET-020)`);
    else if (d.storage === "committed" && v.kind === "sharealike") out.push(`${d.id}: licence "${d.licence}" is ShareAlike: it is never committed (REQ-DATASET-030); fetch it at build time`);
    else if (d.storage === "committed" && v.kind === "unknown") out.push(`${d.id}: licence "${d.licence}" is ${v.why} (REQ-DATASET-020)`);
    else if (d.storage === "build" && v.kind === "unknown") out.push(`${d.id}: licence "${d.licence}" is ${v.why}`);
    else if (d.storage === "live" && v.kind === "unknown" && !d.note) out.push(`${d.id}: licence "${d.licence}" is ${v.why}; a live feed needs a note saying why it may be shown`);
    if (d.storage === "committed" && Number.isInteger(d.size) && d.size > MAX_COMMITTED) {
      out.push(`${d.id}: ${d.size} B is over the ${MAX_COMMITTED} B (1 MB) a committed dataset may be (REQ-DATASET-020); commit an extract`);
    }
  }
  return out;
}

/** Every file under `dir` (absolute), recursively, as paths from `root`. */
function walk(root, dir) {
  if (!existsSync(dir)) return [];
  const out = [];
  for (const e of readdirSync(dir, { withFileTypes: true })) {
    const p = join(dir, e.name);
    if (e.isDirectory()) out.push(...walk(root, p));
    else if (e.isFile()) out.push(posix(relative(root, p)));
  }
  return out;
}

/**
 * The files on disk against the manifest (REQ-DATASET-010): every file in
 * `dataDirs` has an entry; every entry's file present has its size and
 * SHA-256; committed files are present and not ignored; build files are
 * ignored and untracked. `git` answers `ignored(path)` and `tracked(path)`.
 * Returns problems and notes.
 */
export function checkFiles(manifest, { root = ROOT, dataDirs = DATA_DIRS, git = gitOf(root) } = {}) {
  const out = [];
  const notes = [];
  const byPath = new Map();
  for (const d of manifest.datasets) for (const p of d.paths ?? []) byPath.set(p, d);
  for (const dir of dataDirs) {
    for (const p of walk(root, join(root, dir))) {
      const name = p.split("/").pop();
      if (NOT_DATA.test(name)) continue;
      if (!byPath.has(p)) out.push(`${p}: no entry in ${MANIFEST} (REQ-DATASET-010); \`node scripts/datasets.mjs hash ${p}\` prints one to fill in`);
    }
  }
  for (const d of manifest.datasets) {
    for (const p of d.paths ?? []) {
      const abs = join(root, p);
      if (!existsSync(abs)) {
        if (d.storage === "committed") out.push(`${d.id}: ${p} is missing`);
        else notes.push(`${d.id}: ${p} not fetched (mise run datasets:fetch)`);
      } else {
        const bytes = readFileSync(abs);
        const h = sha256(bytes);
        if (bytes.length !== d.size) out.push(`${d.id}: ${p} is ${bytes.length} B, the manifest says ${d.size} B`);
        if (h !== d.sha256) out.push(`${d.id}: ${p} has SHA-256 ${h}, the manifest says ${d.sha256} (REQ-DATASET-010)`);
      }
      if (d.storage === "committed" && git.ignored(p)) out.push(`${d.id}: ${p} is committed but gitignored`);
      if (d.storage === "build") {
        if (!git.ignored(p)) out.push(`${d.id}: ${p} is fetched at build time but not gitignored: it must never be committed (ADR-137)`);
        if (git.tracked(p)) out.push(`${d.id}: ${p} is fetched at build time but tracked by git`);
      }
    }
  }
  return { problems: out, notes };
}

/** `ignored` and `tracked` from git, read-only. */
export function gitOf(root) {
  const run = (args) => {
    try {
      execFileSync("git", args, { cwd: root, stdio: ["ignore", "pipe", "ignore"] });
      return true;
    } catch {
      return false;
    }
  };
  return {
    ignored: (p) => run(["check-ignore", "-q", "--no-index", p]),
    tracked: (p) => run(["ls-files", "--error-unmatch", "--", p]),
  };
}

/**
 * The test datasets' README against the manifest: each file it lists has an
 * entry whose URL, retrieval date and licence its row states, and each entry
 * with a path beside the README has a row.
 */
export function checkReadme(manifest, { root = ROOT, readme = TEST_README } = {}) {
  const out = [];
  const abs = join(root, readme);
  if (!existsSync(abs)) return [`${readme} is missing`];
  const dir = readme.split("/").slice(0, -1).join("/");
  const rows = new Map();
  for (const line of readFileSync(abs, "utf8").split("\n")) {
    const m = /^\| `([^`]+)` \|(.*)$/.exec(line);
    if (m) rows.set(`${dir}/${m[1]}`, m[2]);
  }
  const byPath = new Map();
  for (const d of manifest.datasets) for (const p of d.paths ?? []) byPath.set(p, d);
  for (const [p, row] of rows) {
    const d = byPath.get(p);
    if (!d) {
      out.push(`${readme}: ${p} has no entry in ${MANIFEST}`);
      continue;
    }
    const cells = row.replace(/\|\s*$/, "").split(" | ").map((c) => c.trim());
    if (!row.includes(d.url)) out.push(`${readme}: ${p}'s row does not give the manifest's URL ${d.url}`);
    if (!cells.includes(d.retrieved)) out.push(`${readme}: ${p}'s row does not give the manifest's retrieval date ${d.retrieved}`);
    if (!cells.some((c) => c.toLowerCase().startsWith(d.licence.toLowerCase()))) out.push(`${readme}: ${p}'s licence cell does not start with the manifest's "${d.licence}"`);
  }
  for (const [p, d] of byPath) {
    if (p.startsWith(`${dir}/`) && !p.slice(dir.length + 1).includes("/") && !rows.has(p)) out.push(`${readme}: no row for ${p} (${d.id})`);
  }
  return out;
}

/** The published crates and the npm package, each with its files as repository paths. */
export function listPackages(root = ROOT) {
  const out = [];
  const meta = JSON.parse(execFileSync("cargo", ["metadata", "--no-deps", "--format-version", "1"], { cwd: root, encoding: "utf8", maxBuffer: 64 << 20 }));
  for (const pkg of meta.packages) {
    if (Array.isArray(pkg.publish) && pkg.publish.length === 0) continue; // publish = false
    const dir = dirname(pkg.manifest_path);
    const list = execFileSync("cargo", ["package", "--list", "--allow-dirty", "-p", pkg.name], { cwd: root, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"], maxBuffer: 64 << 20 });
    const files = list
      .split("\n")
      .filter((l) => l.trim() !== "")
      .map((l) => posix(relative(root, join(dir, l.trim()))));
    out.push({ name: `crate ${pkg.name}`, files });
  }
  const web = join(root, "web");
  const npm = JSON.parse(execFileSync("npm", ["pack", "--dry-run", "--json", "--ignore-scripts"], { cwd: web, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"], maxBuffer: 64 << 20 }));
  for (const p of npm) out.push({ name: `npm ${p.name}`, files: p.files.map((f) => posix(relative(root, join(web, f.path)))) });
  return out;
}

/**
 * Package contents against the manifest (REQ-DATASET-030): a file that is a
 * ShareAlike dataset, by path or by SHA-256, is a problem; every dataset a
 * package holds is noted.
 */
export function checkPackages(manifest, packages, { root = ROOT } = {}) {
  const problems = [];
  const notes = [];
  const byPath = new Map();
  const byHash = new Map();
  for (const d of manifest.datasets) {
    for (const p of d.paths ?? []) byPath.set(p, d);
    if (d.sha256) byHash.set(d.sha256, d);
  }
  for (const pkg of packages) {
    const held = new Set();
    for (const f of pkg.files) {
      let d = byPath.get(f);
      if (!d) {
        const abs = join(root, f);
        if (existsSync(abs) && statSync(abs).isFile()) d = byHash.get(sha256(readFileSync(abs)));
      }
      if (!d) continue;
      held.add(d.id);
      if (classify(d.licence).kind === "sharealike") problems.push(`${pkg.name}: holds ${f}, ${d.id} under "${d.licence}": ShareAlike data is never in a published package (REQ-DATASET-030)`);
    }
    notes.push(`${pkg.name}: ${pkg.files.length} files, datasets: ${held.size ? [...held].join(", ") : "none"}`);
  }
  return { problems, notes };
}

/** Reads the manifest. */
export function readManifest(root = ROOT) {
  return JSON.parse(readFileSync(join(root, MANIFEST), "utf8"));
}

/** Every check; returns problems and notes. `packages` defaults to listing them. */
export function checkAll({ root = ROOT, manifest = readManifest(root), packages, git, dataDirs } = {}) {
  const fields = checkFields(manifest);
  if (fields.length > 0) return { problems: fields, notes: [] };
  const files = checkFiles(manifest, { root, ...(git ? { git } : {}), ...(dataDirs ? { dataDirs } : {}) });
  const pk = checkPackages(manifest, packages ?? listPackages(root), { root });
  return {
    problems: [...checkLicences(manifest), ...files.problems, ...checkReadme(manifest, { root }), ...pk.problems],
    notes: [...files.notes, ...pk.notes],
  };
}

// ---------------------------------------------------------------- fetch

/** Splits one CSV line (RFC 4180 quoting, no newlines inside fields). */
export function csvFields(line) {
  const out = [];
  let cur = "";
  let quoted = false;
  for (let i = 0; i < line.length; i++) {
    const ch = line[i];
    if (quoted) {
      if (ch === '"' && line[i + 1] === '"') {
        cur += '"';
        i++;
      } else if (ch === '"') quoted = false;
      else cur += ch;
    } else if (ch === '"') quoted = true;
    else if (ch === ",") {
      out.push(cur);
      cur = "";
    } else cur += ch;
  }
  out.push(cur);
  return out;
}

const csvCell = (v) => (/[",\n]/.test(v) ? `"${v.replace(/"/g, '""')}"` : v);

/** HYG's columns the subset keeps. */
export const HYG_COLUMNS = ["id", "ra", "dec", "mag", "ci", "proper", "con"];
/** The faintest magnitude the subset keeps (naked eye). */
export const HYG_MAG = 6.5;

/**
 * The naked-eye subset of an HYG CSV: stars with `mag` ≤ 6.5 other than the
 * Sun (id 0), in the release's order, columns {@link HYG_COLUMNS}, values as
 * the release writes them; quoted only where a value needs it.
 */
export function hygSubset(csv) {
  const lines = csv.split(/\r?\n/);
  const head = csvFields(lines[0] ?? "");
  const idx = HYG_COLUMNS.map((c) => {
    const i = head.indexOf(c);
    if (i < 0) throw new Error(`HYG has no column ${c}`);
    return i;
  });
  const mag = head.indexOf("mag");
  const rows = [HYG_COLUMNS.join(",")];
  for (const line of lines.slice(1)) {
    if (line === "") continue;
    const f = csvFields(line);
    const m = Number(f[mag]);
    if (f[idx[0]] === "0" || f[mag] === "" || !(m <= HYG_MAG)) continue;
    rows.push(idx.map((i) => csvCell(f[i] ?? "")).join(","));
  }
  return `${rows.join("\n")}\n`;
}

/** The transforms `derivedFrom` entries name, by entry id: the parent's bytes to the child's. */
export const TRANSFORMS = {
  "hyg-v44-mag6.5": (gz) => Buffer.from(hygSubset(gunzipSync(gz).toString("utf8")), "utf8"),
};

/** The notice written beside ShareAlike data the site serves. */
function notice(d, parent) {
  return [
    `# ${d.title}`,
    "",
    `${d.attribution ?? d.publisher}`,
    "",
    `Made from ${parent ? `${parent.title}, ${parent.fetch ?? parent.url}` : d.url}, retrieved ${d.retrieved}.${d.transform ? ` ${d.transform}` : ""}`,
    "",
    `Licensed under ${d.licence}: ${d.licenceUrl}. This file and the data beside it are not under glyphplot's MIT licence.`,
    "",
  ].join("\n");
}

const writeAtomic = (abs, bytes) => {
  mkdirSync(dirname(abs), { recursive: true });
  const tmp = `${abs}.part`;
  writeFileSync(tmp, bytes);
  renameSync(tmp, abs);
};

/** Whether `p` holds `d`'s bytes. */
const holds = (root, p, d) => {
  const abs = join(root, p);
  if (!existsSync(abs)) return false;
  const b = readFileSync(abs);
  return b.length === d.size && sha256(b) === d.sha256;
};

/**
 * Fetches every build dataset with a `fetch` URL whose file is absent or
 * differs, checks its SHA-256 against the manifest, then makes each
 * `derivedFrom` dataset from its parent and checks it too. Skips what is
 * present with the right hash.
 */
export async function fetchAll({ root = ROOT, manifest = readManifest(root), log = console.log } = {}) {
  const byId = new Map(manifest.datasets.map((d) => [d.id, d]));
  for (const d of manifest.datasets.filter((d) => d.storage === "build" && d.fetch)) {
    const p = d.paths[0];
    if (holds(root, p, d)) {
      log(`${d.id}: ${p} present, SHA-256 matches; skipped`);
      continue;
    }
    log(`${d.id}: fetching ${d.fetch}`);
    const res = await fetch(d.fetch);
    if (!res.ok) throw new Error(`${d.id}: ${d.fetch} answered ${res.status}`);
    const bytes = Buffer.from(await res.arrayBuffer());
    const h = sha256(bytes);
    if (h !== d.sha256) throw new Error(`${d.id}: fetched ${bytes.length} B with SHA-256 ${h}, the manifest pins ${d.sha256}`);
    for (const q of d.paths) writeAtomic(join(root, q), bytes);
    log(`${d.id}: ${bytes.length} B, SHA-256 matches, written to ${d.paths.join(", ")}`);
  }
  for (const d of manifest.datasets.filter((d) => d.storage === "build" && d.derivedFrom)) {
    const parent = byId.get(d.derivedFrom);
    const f = TRANSFORMS[d.id];
    if (!f) throw new Error(`${d.id}: no transform in scripts/datasets.mjs TRANSFORMS`);
    const noticePath = d.notice ? join(root, d.notice) : null;
    if (d.paths.every((p) => holds(root, p, d)) && (!noticePath || existsSync(noticePath))) {
      log(`${d.id}: present, SHA-256 matches; skipped`);
      continue;
    }
    const src = parent.paths[0];
    if (!holds(root, src, parent)) throw new Error(`${d.id}: its parent ${parent.id} (${src}) is missing or differs`);
    const bytes = f(readFileSync(join(root, src)));
    const h = sha256(bytes);
    if (h !== d.sha256 || bytes.length !== d.size) throw new Error(`${d.id}: made ${bytes.length} B with SHA-256 ${h}, the manifest pins ${d.size} B and ${d.sha256}`);
    for (const q of d.paths) writeAtomic(join(root, q), bytes);
    if (noticePath) writeAtomic(noticePath, Buffer.from(notice(d, parent), "utf8"));
    log(`${d.id}: ${bytes.length} B from ${parent.id}, SHA-256 matches, written to ${[...d.paths, ...(d.notice ? [d.notice] : [])].join(", ")}`);
  }
}

// ---------------------------------------------------------------- main

async function main(argv) {
  const [cmd, ...rest] = argv;
  if (cmd === "check") {
    const { problems, notes } = checkAll();
    for (const n of notes) console.log(`  ${n}`);
    if (problems.length > 0) {
      console.error(`datasets: ${problems.length} problem${problems.length === 1 ? "" : "s"}:`);
      for (const p of problems) console.error(`  ${p}`);
      process.exit(1);
    }
    console.log(`datasets: ${readManifest().datasets.length} entries in ${MANIFEST}, every check passed`);
  } else if (cmd === "fetch") {
    await fetchAll();
  } else if (cmd === "hash") {
    const stubs = rest.map((p) => {
      const bytes = readFileSync(resolve(p));
      const rel = posix(relative(ROOT, resolve(p)));
      return { id: "", title: "", publisher: "", url: "", licence: "", licenceUrl: "", retrieved: new Date().toISOString().slice(0, 10), storage: "committed", paths: [rel], size: bytes.length, sha256: sha256(bytes) };
    });
    console.log(JSON.stringify(stubs, null, 2));
  } else {
    console.error("usage: node scripts/datasets.mjs check | fetch | hash FILE…");
    process.exit(2);
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main(process.argv.slice(2)).catch((e) => {
    console.error(`datasets: ${e.message}`);
    process.exit(1);
  });
}
