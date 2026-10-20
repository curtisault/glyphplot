// mise run size --charts (scripts/size.mjs hands over to this): what each
// chart module is made of (REQ-SIZE-050).
//
// For each family it builds the module as build:wasm does, but with the
// `size-names` profile (release with its symbols kept) and wasm-opt run with
// `-g`, so the optimised module keeps its name section (see `build` below).
// Its stripped size is within 1 % of web/pkg/<family>'s (`mise run size`
// builds that first; `breakdown` says why they differ at all), and the gzip
// -9 size is then the shipped module's.
//
// Each function's body in the code section is attributed by its demangled
// name in the name section (wasm-ld writes demangled names; the `[hash]` of a
// crate is dropped), first rule that matches:
//   core        an export of the boundary (`chart_commit`, `resolve_palette`):
//               wasm-bindgen's wrapper of a glyphplot_wasm function, with
//               whatever LLVM and wasm-opt inlined into it;
//   serde       any name that mentions serde, serde_json, serde_core or zmij
//               (serde_json's float writer): the libraries and the code their
//               derives and generics instantiate for the core's types;
//   chart       glyphplot::charts::<family>::…, the family's own code;
//   core        any other glyphplot:: or glyphplot_wasm:: name, including
//               std generics instantiated for the core's types
//               (drop_in_place<glyphplot::…>, Vec<glyphplot::…>);
//   rest        everything else: std, core, alloc, dlmalloc, wasm-bindgen,
//               compiler_builtins.
// The data section (string literals, tables) and the other sections (types,
// imports, exports, the function table) are listed on their own lines: they
// carry no names.
//
// Gzipped shares are apportioned: each row's raw bytes over the named
// module's raw bytes (stripped), times the shipped module's gzip -9 size. Code and data compress
// differently, so a row's gzipped share is an estimate of its part of the
// module's gzipped bytes, not the size it would have alone.
//
//   node scripts/size.mjs --charts             every family, the 15 largest functions and owners
//   node scripts/size.mjs --charts --top 40    the 40 largest
//   node scripts/size.mjs --charts --no-build  read target/size-names/<family> as last built
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(fileURLToPath(new URL("..", import.meta.url)));
const gzipped = (bytes) => execFileSync("gzip", ["-9"], { input: bytes, maxBuffer: 1 << 30 }).length;
const n = (x) => Math.round(x).toLocaleString("en-US");
const pct = (x) => `${(100 * x).toFixed(1)}%`;

// --- the WebAssembly binary format: sections, the code section's bodies and
// the name section's function names (WebAssembly Core Specification 2.0,
// 5.5 Modules; Appendix 7.4 Custom Sections, "Name Section").

/** A reader over `bytes` from `at`. */
function reader(bytes, at = 0) {
  const r = {
    at,
    byte: () => bytes[r.at++],
    u32() {
      let result = 0;
      let shift = 0;
      for (;;) {
        const b = bytes[r.at++];
        result += (b & 0x7f) * 2 ** shift;
        if ((b & 0x80) === 0) return result;
        shift += 7;
      }
    },
    name() {
      const len = r.u32();
      const s = new TextDecoder().decode(bytes.subarray(r.at, r.at + len));
      r.at += len;
      return s;
    },
  };
  return r;
}

/** The module's sections: id, the custom section's name, and the byte ranges of the whole section and its content. */
function sections(bytes) {
  if (bytes.length < 8 || bytes[0] !== 0 || bytes[1] !== 0x61 || bytes[2] !== 0x73 || bytes[3] !== 0x6d) throw new Error("not a wasm module");
  const out = [];
  const r = reader(bytes, 8);
  while (r.at < bytes.length) {
    const start = r.at;
    const id = r.byte();
    const size = r.u32();
    const content = r.at;
    let name = null;
    if (id === 0) name = reader(bytes, content).name();
    out.push({ id, name, start, content, end: content + size });
    r.at = content + size;
  }
  return out;
}

/** The number of imported functions: function indices start after them. */
function importedFunctions(bytes, sec) {
  if (!sec) return 0;
  const r = reader(bytes, sec.content);
  let count = 0;
  for (let i = 0, k = r.u32(); i < k; i++) {
    r.name();
    r.name();
    const kind = r.byte();
    if (kind === 0) {
      r.u32();
      count++;
    } else if (kind === 1) {
      r.byte(); // reftype
      const flags = r.byte();
      r.u32();
      if (flags & 1) r.u32();
    } else if (kind === 2) {
      const flags = r.byte();
      r.u32();
      if (flags & 1) r.u32();
    } else if (kind === 3) {
      r.byte();
      r.byte();
    } else if (kind === 4) {
      r.byte();
      r.u32();
    } else throw new Error(`import kind ${kind}`);
  }
  return count;
}

/** Each defined function's body size, by function index. */
function bodies(bytes, sec, imported) {
  const r = reader(bytes, sec.content);
  const out = [];
  for (let i = 0, k = r.u32(); i < k; i++) {
    const before = r.at;
    const size = r.u32();
    r.at += size;
    out.push({ index: imported + i, bytes: r.at - before });
  }
  return out;
}

/** Function names by index, from the name section's subsection 1. */
function functionNames(bytes, sec) {
  const names = new Map();
  if (!sec) return names;
  const r = reader(bytes, sec.content);
  r.name(); // "name"
  while (r.at < sec.end) {
    const id = r.byte();
    const size = r.u32();
    const end = r.at + size;
    if (id === 1) {
      for (let i = 0, k = r.u32(); i < k; i++) {
        const index = r.u32();
        names.set(index, r.name());
      }
    }
    r.at = end;
  }
  return names;
}

/** The names of the exported functions. */
function exportedFunctions(bytes, sec) {
  const out = new Set();
  if (!sec) return out;
  const r = reader(bytes, sec.content);
  for (let i = 0, k = r.u32(); i < k; i++) {
    const name = r.name();
    const kind = r.byte();
    r.u32();
    if (kind === 0) out.add(name);
  }
  return out;
}

/** The module without its custom sections: its code and data. */
function stripped(bytes, secs = sections(bytes)) {
  const parts = [bytes.subarray(0, 8), ...secs.filter((s) => s.id !== 0).map((s) => bytes.subarray(s.start, s.end))];
  return Buffer.concat(parts);
}

// --- attribution.

/** A demangled name without its crate hashes. */
const clean = (name) => name.replace(/\[[0-9a-f]{16}\]/g, "");

/** The owner of the boundary's exports (`chart_commit`, `resolve_palette`): wasm-bindgen's wrapper around a glyphplot_wasm function, inlined into it. */
const EXPORTS = "glyphplot_wasm exports (with what was inlined into them)";

/** Whether `name` is wasm-bindgen's own (`__wbindgen_*`, `__wbg_*`) or a compiler intrinsic. */
const intrinsic = (name) => name.startsWith("__") || name.startsWith("_ZN") || /^[a-z_0-9]+$/.test(name);

/** The bucket a function belongs to, by the rules at the top of this file. */
function bucket(name, family, exported) {
  if (exported.has(name) && !name.startsWith("__")) return "core";
  if (/\bserde(_json|_core|_derive)?::|\bzmij::/.test(name)) return "serde";
  if (name.includes(`glyphplot::charts::${family}::`)) return "chart";
  if (/\bglyphplot(_wasm)?::/.test(name)) return "core";
  return "rest";
}

/** The owner a function's bytes are grouped under for the top list: its first `crate::module` path. */
function owner(name, exported) {
  if (exported.has(name) && !name.startsWith("__")) return EXPORTS;
  const m = name.match(/\b(glyphplot_wasm|glyphplot|serde_json|serde_core|serde|zmij|core|alloc|std|dlmalloc|wasm_bindgen|compiler_builtins|unicode_width)::([a-z_0-9]+)(?:::([a-z_0-9]+))?/);
  if (!m) return intrinsic(name) ? "(wasm-bindgen and compiler intrinsics)" : "(other)";
  const [, krate, a, b] = m;
  // charts::<family>::<file> and core's num/fmt by their third segment; else two.
  if ((krate === "glyphplot" && a === "charts") || (krate === "core" && (a === "num" || a === "fmt")) || krate === "serde_json") return b ? `${krate}::${a}::${b}` : `${krate}::${a}`;
  return `${krate}::${a}`;
}

const BUCKETS = [
  ["chart", "the chart's own code (glyphplot::charts::<family>)"],
  ["core", "the core (glyphplot, glyphplot_wasm)"],
  ["serde", "serde and serde_json (with derives and zmij)"],
  ["rest", "the rest (std, core, alloc, dlmalloc, wasm-bindgen)"],
  ["data", "data section (strings, tables)"],
  ["other", "types, imports, exports, function table"],
];

/** How far the shipped module may be from the named one, as a share of its bytes, for its gzip size to stand. */
const CLOSE = 0.01;

/**
 * The breakdown of one module with a name section. Rows are bytes of the
 * named module stripped of its custom sections (`total`). `shipped`, the
 * release module, gives the gzip size when it is within 1 % of `total`
 * (`same`); otherwise the stripped named module does. The two are not byte
 * for byte the same: the size-names profile's crate hashes differ from
 * release's, which reorders a few functions and moves a few addresses, and
 * wasm-bindgen names three exports differently in a module with names (with
 * `opt-level = "s"` they were the same size to the byte, with `"z"` 109 B
 * apart).
 */
export function breakdown(bytes, family, shipped) {
  const secs = sections(bytes);
  const code = secs.find((s) => s.id === 10);
  if (!code) throw new Error("no code section");
  const names = functionNames(bytes, secs.find((s) => s.id === 0 && s.name === "name"));
  if (names.size === 0) throw new Error("no name section: build with the size-names profile and wasm-opt -g");
  const imported = importedFunctions(bytes, secs.find((s) => s.id === 2));
  const exported = exportedFunctions(bytes, secs.find((s) => s.id === 7));
  const plain = stripped(bytes, secs);
  const total = plain.length;
  const same = shipped !== undefined && Math.abs(shipped.length - total) <= CLOSE * total;
  const gzTotal = gzipped(same ? shipped : plain);
  const fns = bodies(bytes, code, imported).map((f) => {
    const name = clean(names.get(f.index) ?? `(function ${f.index})`);
    return { ...f, name, bucket: bucket(name, family, exported), owner: owner(name, exported) };
  });
  const sums = Object.fromEntries(BUCKETS.map(([k]) => [k, 0]));
  for (const f of fns) sums[f.bucket] += f.bytes;
  const data = secs.find((s) => s.id === 11);
  sums.data = data ? data.end - data.start : 0;
  sums.other = total - Object.values(sums).reduce((a, b) => a + b, 0);
  const owners = new Map();
  for (const f of fns) {
    const o = owners.get(f.owner) ?? { owner: f.owner, bytes: 0, count: 0, bucket: f.bucket };
    o.bytes += f.bytes;
    o.count++;
    owners.set(f.owner, o);
  }
  return { same, total, shipped: shipped?.length, gz: gzTotal, sums, fns: fns.sort((a, b) => b.bytes - a.bytes), owners: [...owners.values()].sort((a, b) => b.bytes - a.bytes) };
}

/** Rows as aligned text, the first column left-aligned. */
function table(rows) {
  const widths = rows[0].map((_, i) => Math.max(...rows.map((r) => String(r[i]).length)));
  return rows.map((r) => r.map((c, i) => (i === 0 || i === r.length - 1 ? String(c).padEnd(widths[i]) : String(c).padStart(widths[i]))).join("  ").trimEnd()).join("\n");
}

/** The breakdown as the text `size --charts` prints. */
export function report(family, b, top) {
  const share = (bytes) => (bytes / b.total) * b.gz;
  const lines = [];
  const of = b.same ? `web/pkg/${family}, ${n(b.shipped)} B raw` : "the named build";
  lines.push(`== ${family}: ${n(b.total)} B raw in the named build; ${n(b.gz)} B gzip -9 (${of})`);
  lines.push(table([["share", "raw B", "raw", "gz B (apportioned)", "what"], ...BUCKETS.map(([k, what]) => [k, n(b.sums[k]), pct(b.sums[k] / b.total), n(share(b.sums[k])), what])]));
  lines.push("");
  lines.push(`the ${top} largest owners (first crate::module path of a name):`);
  lines.push(table([["owner", "fns", "raw B", "gz B", "bucket"], ...b.owners.slice(0, top).map((o) => [o.owner, o.count, n(o.bytes), n(share(o.bytes)), o.bucket])]));
  lines.push("");
  lines.push(`the ${top} largest functions:`);
  const short = (s) => (s.length > 150 ? s.slice(0, 147) + "..." : s);
  lines.push(table([["raw B", "gz B", "bucket", "function"], ...b.fns.slice(0, top).map((f) => [n(f.bytes), n(share(f.bytes)), f.bucket, short(f.name)])]));
  return lines.join("\n");
}

// --- the build.

/** A tool in wasm-pack's cache: `<dir>/<bin>` for the newest `<dir>` named `prefix…`. */
function cached(prefix, bin) {
  const caches = [join(homedir(), "Library/Caches/.wasm-pack"), join(process.env.XDG_CACHE_HOME ?? join(homedir(), ".cache"), ".wasm-pack")];
  for (const dir of caches) {
    if (!existsSync(dir)) continue;
    const found = readdirSync(dir)
      .filter((d) => d.startsWith(prefix))
      .map((d) => join(dir, d, bin))
      .filter((p) => existsSync(p));
    if (found.length) return found.at(-1);
  }
  return null;
}

/** wasm-opt: $WASM_OPT, else the one wasm-pack fetched for build:wasm. */
function wasmOpt() {
  const found = process.env.WASM_OPT ?? cached("wasm-opt-", "bin/wasm-opt");
  if (!found) throw new Error("size --charts: no wasm-opt; set WASM_OPT, or run `mise run build:wasm` once so wasm-pack fetches it");
  return found;
}

/** wasm-bindgen of the version Cargo.lock pins: $WASM_BINDGEN, else the one wasm-pack fetched. */
function wasmBindgen() {
  const lock = readFileSync(join(root, "Cargo.lock"), "utf8");
  const version = lock.match(/name = "wasm-bindgen"\nversion = "([^"]+)"/)?.[1];
  const found = process.env.WASM_BINDGEN ?? (version && cached(`wasm-bindgen-cargo-install-${version}`, "wasm-bindgen"));
  if (!found) throw new Error(`size --charts: no wasm-bindgen ${version}; set WASM_BINDGEN, or run \`mise run build:wasm\` once so wasm-pack fetches it`);
  return found;
}

/** The release profile's wasm-opt flags, from crates/wasm/Cargo.toml (REQ-BUILD-050). */
function optFlags() {
  const toml = readFileSync(join(root, "crates/wasm/Cargo.toml"), "utf8");
  const m = toml.match(/\[package\.metadata\.wasm-pack\.profile\.release\][^[]*?wasm-opt\s*=\s*\[([^\]]*)\]/s);
  if (!m) throw new Error("size --charts: crates/wasm/Cargo.toml names no release wasm-opt flags");
  return [...m[1].matchAll(/"([^"]*)"/g)].map((x) => x[1]);
}

/** The families, as build:wasm builds them. */
function families() {
  return execFileSync("cargo", ["run", "-q", "-p", "glyphplot", "--example", "manifest", "--", "families"], { cwd: root, encoding: "utf8" }).split(/\s+/).filter(Boolean);
}

/**
 * One family's optimised module with its names, into
 * target/size-names/<family>/named.wasm, by build:wasm's steps with the names
 * kept: cargo with the size-names profile; the custom sections the release
 * profile's `strip` drops dropped here too, keeping `name` (debug info,
 * `producers` and `target_features` go: wasm-bindgen reads
 * `target_features`, and would otherwise emit reference types and multivalue
 * shims the release module does not have); wasm-bindgen as wasm-pack runs
 * it; wasm-opt with the release flags and `-g`.
 */
function build(family) {
  const out = join(root, "target/size-names", family);
  execFileSync(
    "cargo",
    ["build", "-q", "-p", "glyphplot-wasm", "--lib", "--target", "wasm32-unknown-unknown", "--profile", "size-names", "--no-default-features", "--features", family],
    { cwd: root, stdio: ["ignore", "inherit", "inherit"] },
  );
  const linked = readFileSync(join(root, "target/wasm32-unknown-unknown/size-names/glyphplot_wasm.wasm"));
  const keep = sections(linked).filter((s) => s.id !== 0 || s.name === "name" || s.name === "__wasm_bindgen_unstable");
  mkdirSync(out, { recursive: true });
  writeFileSync(join(out, "linked.wasm"), Buffer.concat([linked.subarray(0, 8), ...keep.map((s) => linked.subarray(s.start, s.end))]));
  execFileSync(wasmBindgen(), [join(out, "linked.wasm"), "--out-dir", out, "--typescript", "--target", "web", "--out-name", "glyphplot"], { cwd: root, stdio: "inherit" });
  execFileSync(wasmOpt(), [...optFlags(), "-g", join(out, "glyphplot_bg.wasm"), "-o", join(out, "named.wasm")], { cwd: root, stdio: "inherit" });
  return join(out, "named.wasm");
}

export function main(argv) {
  const at = argv.indexOf("--top");
  const top = at >= 0 ? Number(argv[at + 1]) : 15;
  if (!Number.isInteger(top) || top < 1) {
    console.error("size --charts: --top takes a whole number");
    return 2;
  }
  const noBuild = argv.includes("--no-build");
  for (const family of families()) {
    const path = noBuild ? join(root, "target/size-names", family, "named.wasm") : build(family);
    if (!existsSync(path)) {
      console.error(`size --charts: ${path} is missing; run without --no-build`);
      return 1;
    }
    // The release module (`mise run size` builds it first) gives the gzip size when it is within 1 % of the named one.
    const release = join(root, "web/pkg", family, "glyphplot_bg.wasm");
    const b = breakdown(readFileSync(path), family, existsSync(release) ? readFileSync(release) : undefined);
    if (!b.same) console.log(`size --charts: ${family}: note: web/pkg/${family} is missing or over 1 % from the named build; its gzip is the named build's`);
    console.log(report(family, b, top));
    console.log("");
  }
  console.log("gz B is each row's raw share of the module times its gzip -9 size (apportioned, not compressed alone).");
  return 0;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) process.exit(main(process.argv.slice(2)));
