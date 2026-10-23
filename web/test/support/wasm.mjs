// What the wasm tests share (REQ-TEST-020): the build under test, its
// manifest, modules found with the shell's own compiled `moduleOf`, the
// fixtures written by `cargo run -p glyphplot --example fixtures -- write`,
// and a schema validator over spec/schemas.
//
// The build under test is <web>/pkg, played by the shell compiled in
// <web>/dist, which loads its modules from ../pkg as a page does. <web> is
// web/ by default, or the folder GLYPHPLOT_WEB names (relative to the
// repository root): `mise run test:web` runs every test against the release
// build in web/pkg, then with GLYPHPLOT_WEB=target/web-debug, a folder
// holding a copy of web/dist beside the debug build as pkg/. A debug build is
// one whose manifest has a `fallback` (REQ-MANIFEST-020); the panic tests run
// only there.
import { existsSync, mkdirSync, readFileSync, renameSync, rmSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

export const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");
const WEB = resolve(ROOT, process.env.GLYPHPLOT_WEB || "web");
export const PKG = join(WEB, "pkg");
const DIST = join(WEB, "dist");

if (!existsSync(join(PKG, "modules.js"))) {
  throw new Error(`no build in ${PKG}: run \`mise run build:wasm\` or \`mise run build:wasm:debug\``);
}
if (!existsSync(join(DIST, "wasm.js"))) {
  throw new Error(`no ${DIST}: run \`npm run build --prefix web\` (the tests use the shell's compiled code)`);
}

/** <web>/pkg/modules.js of the build under test. */
export const manifest = await import(pathToFileURL(join(PKG, "modules.js")).href);
/** Whether the build under test is the debug build (REQ-MANIFEST-020). */
export const DEBUG = manifest.fallback !== null;
/** The build's name in test titles. */
export const BUILD = DEBUG ? "debug" : "release";
/** `skip` for a test that runs only against the debug build. */
export const debugOnly = DEBUG ? false : "debug build only (REQ-TEST-020)";
/** `skip` for a test that runs only against the release build. */
export const releaseOnly = DEBUG ? "release build only" : false;

/** The shell's compiled modules (<web>/dist). */
export const shell = {
  wasm: await import(pathToFileURL(join(DIST, "wasm.js")).href),
  chart: await import(pathToFileURL(join(DIST, "chart.js")).href),
  data: await import(pathToFileURL(join(DIST, "data.js")).href),
  input: await import(pathToFileURL(join(DIST, "input.js")).href),
  stream: await import(pathToFileURL(join(DIST, "stream.js")).href),
  source: await import(pathToFileURL(join(DIST, "source.js")).href),
  arrow: await import(pathToFileURL(join(DIST, "arrow.js")).href),
};

/** The chart types of the build: the manifest's, the debug charts left out. */
export const types = manifest.charts.map((m) => m.slug);

/** The module the shell's `moduleOf` picks for `type` (REQ-TEST-020). */
export function moduleFor(type) {
  return shell.wasm.moduleOf(type, manifest);
}

const bytes = new Map();
function wasmBytes(name) {
  if (!bytes.has(name)) bytes.set(name, readFileSync(join(PKG, name, "glyphplot_bg.wasm")));
  return bytes.get(name);
}

let instances = 0;

/**
 * Module `name` of the build, instantiated. `fresh` gives an instance of its
 * own (a new copy of the glue, so a new memory); otherwise the shared one.
 * Returns `{name, glue, memory, exports, loaded}`; `loaded` is the shell's
 * `Loaded`, for its `Guarded`.
 */
export function instantiate(name, { fresh = false } = {}) {
  const url = pathToFileURL(join(PKG, name, "glyphplot.js")).href + (fresh ? `?instance=${++instances}` : "");
  return import(url).then((glue) => {
    const exports = glue.initSync({ module: new WebAssembly.Module(wasmBytes(name)) });
    glue.start?.();
    const stack = typeof exports.__wbindgen_add_to_stack_pointer === "function" ? exports.__wbindgen_add_to_stack_pointer : null;
    return { name, glue, memory: exports.memory, exports, loaded: { name, module: glue, memory: exports.memory, stack } };
  });
}

/** The module that plays `type`, found with the shell's `moduleOf`, instantiated. */
export function moduleOfType(type, opts) {
  const name = moduleFor(type);
  if (name === null) throw new Error(`the build plays no chart named "${type}"`);
  return instantiate(name, opts);
}

// ---------------------------------------------------------------- frames

/** The grid's views made after the frame, as the shell makes them (REQ-WASM-030). */
export function views(c, memory) {
  const cols = c.cols();
  const rows = c.rows();
  const n = cols * rows;
  const cp = c.cells_ptr();
  const kp = c.colors_ptr();
  const bp = c.backs_ptr();
  const buffer = memory.buffer;
  return { cols, rows, cells: new Uint32Array(buffer, cp, n), colors: new Uint8Array(buffer, kp, n), backs: new Uint8Array(buffer, bp, n) };
}

/** The grid's text from its cells: rows joined by "\n", WIDE_TAIL cells left out (REQ-GRID-060). */
export function textOf(g, wideTail = 0) {
  const lines = [];
  for (let r = 0; r < g.rows; r++) {
    let s = "";
    for (let c = 0; c < g.cols; c++) {
      const cp = g.cells[r * g.cols + c];
      if (cp !== wideTail) s += String.fromCodePoint(cp);
    }
    lines.push(s);
  }
  return lines.join("\n");
}

// ---------------------------------------------------------------- data

/** A two-column series `{x, y}` of `n` rows as one descriptor and Float64Arrays. */
export function series(n, y = (i) => Math.sin(i / 50)) {
  const xs = new Float64Array(n);
  const ys = new Float64Array(n);
  for (let i = 0; i < n; i++) {
    xs[i] = i;
    ys[i] = y(i);
  }
  return { xs, ys, descriptor: JSON.stringify({ version: 1, tables: { data: { length: n, columns: [{ name: "x", type: "f64" }, { name: "y", type: "f64" }] } } }) };
}

/** Commits `columns` (Float64Array or Uint32Array a column, null for none) through reserve_column, each view made after its reservation (REQ-DATA-060). */
export function commitColumns(c, memory, descriptor, columns) {
  columns.forEach((values, i) => {
    if (!values) return;
    const kind = values instanceof Uint32Array ? 1 : 0;
    const ptr = c.reserve_column(i, kind, values.length);
    const view = kind === 1 ? new Uint32Array(memory.buffer, ptr, values.length) : new Float64Array(memory.buffer, ptr, values.length);
    view.set(values);
  });
  return c.commit(descriptor);
}

// ---------------------------------------------------------------- fixtures

export const FIXTURES = process.env.GLYPHPLOT_FIXTURES ? resolve(ROOT, process.env.GLYPHPLOT_FIXTURES) : join(ROOT, "target/fixtures");

/**
 * The fixtures' index, written first if absent by `cargo run -p glyphplot
 * --example fixtures -- write` into a staging folder renamed into place, so
 * test files running at once never read a half-written set.
 */
export function fixtureIndex() {
  const index = join(FIXTURES, "index.json");
  if (!existsSync(index)) {
    const stage = `${FIXTURES}.stage.${process.pid}`;
    rmSync(stage, { recursive: true, force: true });
    mkdirSync(dirname(stage), { recursive: true });
    execFileSync("cargo", ["run", "-q", "--profile", "contract", "-p", "glyphplot", "--example", "fixtures", "--", "write", stage], { cwd: ROOT, stdio: ["ignore", "ignore", "inherit"] });
    try {
      renameSync(stage, FIXTURES);
    } catch {
      rmSync(stage, { recursive: true, force: true }); // another test file wrote it first
    }
  }
  return JSON.parse(readFileSync(index, "utf8"));
}

/** A little-endian f64 file as a Float64Array (copied, so aligned). */
function f64File(path) {
  const b = readFileSync(path);
  const out = new Float64Array(b.byteLength / 8);
  new Uint8Array(out.buffer).set(b);
  return out;
}

/**
 * One fixture as the boundary takes it: the descriptor's JSON (an i64
 * `origin` written as an exact integer) and one typed array a column
 * (category codes as Uint32Array; null for a string column), numbered as
 * reserve_column numbers them.
 */
export function fixtureData(fx) {
  const columns = [];
  const origins = [];
  const tables = {};
  for (const t of fx.tables) {
    tables[t.name] = {
      length: t.length,
      columns: t.columns.map((c) => {
        const d = { name: c.name, type: c.type };
        for (const k of ["role", "time_unit", "zone", "unit", "categories", "values"]) if (c[k] !== undefined) d[k] = c[k];
        if (c.origin !== undefined) {
          d.origin = `@origin${origins.length}@`;
          origins.push(c.origin);
        }
        if (c.file) {
          const v = f64File(join(FIXTURES, c.file));
          columns.push(c.type === "category" ? Uint32Array.from(v) : v);
        } else columns.push(null);
        return d;
      }),
    };
  }
  const matrices = {};
  for (const m of fx.matrices) {
    matrices[m.name] = { rows: m.rows, cols: m.cols, row_labels: m.row_labels, col_labels: m.col_labels };
    columns.push(f64File(join(FIXTURES, m.file)));
  }
  let descriptor = JSON.stringify(fx.matrices.length ? { version: 1, tables, matrices } : { version: 1, tables });
  origins.forEach((o, i) => (descriptor = descriptor.replace(`"@origin${i}@"`, o)));
  return { descriptor, columns };
}

// ---------------------------------------------------------------- schemas

export const SCHEMAS = join(ROOT, "spec/schemas");

let ajv = null;
/** An Ajv 2020-12 validator holding every schema in spec/schemas by `$id`, `date-time` checked as RFC 3339. */
export async function schemas() {
  if (ajv) return ajv;
  const { default: Ajv2020 } = await import("ajv/dist/2020.js");
  const { readdirSync } = await import("node:fs");
  ajv = new Ajv2020({ strict: false, allErrors: true });
  ajv.addFormat("date-time", (s) => shell.data.parseRfc3339(s) !== null);
  for (const f of readdirSync(SCHEMAS).filter((f) => f.endsWith(".schema.json"))) ajv.addSchema(JSON.parse(readFileSync(join(SCHEMAS, f), "utf8")));
  return ajv;
}

/** The validator of `<name>.schema.json`. */
export async function schema(name) {
  const a = await schemas();
  const v = a.getSchema(`https://github.com/curtisault/glyphplot/spec/schemas/${name}.schema.json`);
  if (!v) throw new Error(`no schema ${name}.schema.json`);
  return v;
}

/** Throws with ajv's errors when `value` does not validate. */
export function check(validate, value, what) {
  if (!validate(value)) throw new Error(`${what}: ${JSON.stringify(validate.errors.slice(0, 5))}`);
}

// ---------------------------------------------------------------- timing

/** The median of `xs`. */
export function median(xs) {
  const s = [...xs].sort((a, b) => a - b);
  const m = s.length >> 1;
  return s.length % 2 ? s[m] : (s[m - 1] + s[m]) / 2;
}
