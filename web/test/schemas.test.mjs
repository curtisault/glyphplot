// spec/schemas in Node (REQ-DATA-010 and the other schemas' requirements):
// every schema compiles under JSON Schema 2020-12 with its `$id` naming its
// file; every examples/<name>[.<what>].json validates against
// <name>.schema.json and every examples/invalid/<name>.<what>.json fails it,
// each schema having one of each (as spec/schemas/validate.py checks); and
// what the build under test writes validates: each type's meta_json and
// default options, charts_json, view_json, table_json, the manifest,
// budgets.json and sizes.json (REQ-MANIFEST-010, REQ-CHART-020).
// The validator is ajv 8 (draft 2020-12), `date-time` checked as RFC 3339.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { BUILD, PKG, SCHEMAS, manifest, types, moduleOfType, series, commitColumns, schemas, schema, check, fixtureIndex, fixtureData } from "./support/wasm.mjs";

const BASE = "https://github.com/curtisault/glyphplot/spec/schemas/";
const read = (p) => JSON.parse(readFileSync(p, "utf8"));
const names = readdirSync(SCHEMAS).filter((f) => f.endsWith(".schema.json")).map((f) => f.slice(0, -".schema.json".length));

test("every schema compiles under 2020-12 and its $id names its file", async () => {
  const ajv = await schemas();
  for (const n of names) {
    const s = read(join(SCHEMAS, `${n}.schema.json`));
    assert.equal(s.$schema, "https://json-schema.org/draft/2020-12/schema", n);
    assert.equal(s.$id, `${BASE}${n}.schema.json`, n);
    assert.equal(typeof ajv.getSchema(s.$id), "function", `${n} compiles`);
  }
});

const valid = readdirSync(join(SCHEMAS, "examples")).filter((f) => f.endsWith(".json"));
const invalid = readdirSync(join(SCHEMAS, "examples/invalid")).filter((f) => f.endsWith(".json"));

test("every schema has a valid and an invalid example", () => {
  for (const n of names) {
    assert.ok(valid.some((f) => f.split(".")[0] === n), `${n}: no valid example`);
    assert.ok(invalid.some((f) => f.split(".")[0] === n), `${n}: no invalid example`);
  }
  for (const f of [...valid, ...invalid]) assert.ok(names.includes(f.split(".")[0]), `${f}: no schema named ${f.split(".")[0]}.schema.json`);
});

for (const f of valid) {
  test(`examples/${f} validates (${f.startsWith("data.") ? "REQ-DATA-010" : f.split(".")[0] + ".schema.json"})`, async () => {
    check(await schema(f.split(".")[0]), read(join(SCHEMAS, "examples", f)), f);
  });
}

for (const f of invalid) {
  test(`examples/invalid/${f} fails`, async () => {
    const validate = await schema(f.split(".")[0]);
    assert.equal(validate(read(join(SCHEMAS, "examples/invalid", f))), false, `${f} validated, but it must fail`);
  });
}

/** The shape of table_json, as web/src/api.ts's TableJson states it (spec/schemas has no table schema). */
const TABLE = {
  type: "object",
  required: ["caption", "columns", "rows", "total"],
  additionalProperties: false,
  properties: {
    caption: { type: "string" },
    columns: { type: "array", items: { type: "string" } },
    rows: { type: "array", items: { type: "array", items: { type: ["string", "null"] } } },
    total: { type: "integer", minimum: 0 },
    domain: { type: "array", items: { type: "number" }, minItems: 2, maxItems: 2 },
  },
};

test(`${BUILD}: the manifest, budgets.json and sizes.json validate (REQ-MANIFEST-010)`, async () => {
  const { charts, modules, fallback, budgets } = manifest;
  check(await schema("manifest"), { charts, modules, fallback, budgets }, "web/pkg/modules.js");
  check(await schema("budgets"), read(join(PKG, "budgets.json")), "budgets.json");
  check(await schema("sizes"), read(join(PKG, "sizes.json")), "sizes.json");
  assert.equal(typeof manifest.imports, "object");
  for (const [name, f] of Object.entries(manifest.imports)) assert.equal(typeof f, "function", name);
  assert.equal(typeof manifest.wasm, "object");
  for (const [name, f] of Object.entries(manifest.wasm)) assert.ok(f() instanceof URL, name);
});

test(`${BUILD}: each module's charts_json, meta_json, default options, view_json and table_json validate (REQ-CHART-020, REQ-VIEW-010, REQ-A11Y-020)`, async () => {
  const ajv = await schemas();
  const table = ajv.compile(TABLE);
  const [meta, options, view] = await Promise.all([schema("meta"), schema("options"), schema("view")]);
  for (const type of types) {
    const { glue, memory } = await moduleOfType(type);
    const all = JSON.parse(glue.charts_json());
    assert.ok(Array.isArray(all));
    for (const m of all) check(meta, m, `charts_json: ${m.slug}`);
    const c = new glue.Chart(type, "{}");
    const m = JSON.parse(c.meta_json());
    check(meta, m, `${type} meta_json`);
    if (m.options !== null) check(options, m.options, `${type} default options`);
    check(view, JSON.parse(c.view_json()), `${type} view_json before data`);
    check(table, JSON.parse(c.table_json(10)), `${type} table_json before data`);
    // A series chart gets 1,000 points; another its first fixture (candles
    // ohlcv-days, bar categorical-12), whose table lists what is in view.
    const takesSeries = m.shapes.includes("series");
    if (takesSeries) {
      const s = series(1000);
      commitColumns(c, memory, s.descriptor, [s.xs, s.ys]);
    } else {
      const f = fixtureIndex().fixtures.find((x) => m.shapes.includes(x.shape));
      assert.ok(f, `${type}: a fixture of its shapes`);
      const { descriptor, columns } = fixtureData(f);
      commitColumns(c, memory, descriptor, columns);
    }
    c.frame(0, false);
    check(view, JSON.parse(c.view_json()), `${type} view_json`);
    for (const n of [0, 2, 10, 5000]) {
      const t = JSON.parse(c.table_json(n));
      check(table, t, `${type} table_json(${n})`);
      assert.ok(t.rows.length <= Math.max(n, 0), `${type} table_json(${n}) holds at most ${n} rows`);
      if (takesSeries) {
        assert.equal(t.total, 1000);
        assert.deepEqual(t.domain, [0, 999], `${type} table_json(${n}): the fitted x domain in data units`);
      } else {
        assert.ok(t.total > 0 && t.rows.length <= t.total, `${type} table_json(${n}): ${t.total} rows in view`);
        assert.ok(t.domain[0] < t.domain[1], `${type} table_json(${n}): an x domain in data units`);
      }
    }
    c.input(4, 40.5, 12.5, 0, -3, 0, 2, 0);
    c.input(0, 30.5, 12.5, 0, 0, 0, 0, 0);
    c.input(9, 0, 0, 12, 0, 0, 0, 0);
    c.frame(0, false);
    check(view, JSON.parse(c.view_json()), `${type} view_json after a zoom, a hover and Enter`);
    check(table, JSON.parse(c.table_json(10)), `${type} table_json after a zoom`);
    c.free();
  }
});
