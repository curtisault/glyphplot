// data.ts: the three data forms and the binary form, normalised to a
// descriptor and writers (REQ-DATA-120, REQ-DATA-050, REQ-MEM-020, REQ-I18N-050).
import { test } from "node:test";
import assert from "node:assert/strict";
import { prepare, describe, appendTo, materialise, parseRfc3339, stampIn, nfcDeep, inferColumn, rowsToColumns, emptyLike, MISSING_CODE } from "../src/data.ts";

const cols = (p) => p.descriptor.tables.data.columns;

test("columns shorthand: numbers f64, booleans bool, RFC 3339 timestamps, strings categories", () => {
  const p = prepare({ t: ["2026-10-18T00:00:00Z", "2026-10-18T00:00:01.5Z", null], close: [1, null, 3], up: [true, false, null], sym: ["a", "b", "a"] });
  assert.deepEqual(cols(p).map((c) => c.type), ["timestamp", "f64", "bool", "category"]);
  assert.equal(p.descriptor.tables.data.length, 3);
  assert.equal(p.descriptor.version, 1);
  const t = materialise(p.sources[0]);
  assert.equal(t[0], Date.UTC(2026, 9, 18));
  assert.equal(t[1], Date.UTC(2026, 9, 18) + 1500);
  assert.ok(Number.isNaN(t[2]));
  assert.deepEqual([...materialise(p.sources[2])].map((v) => (Number.isNaN(v) ? null : v)), [1, 0, null]);
  assert.deepEqual(cols(p)[3].categories, ["a", "b"]);
  assert.deepEqual([...materialise(p.sources[3])], [0, 1, 0]);
  assert.equal(p.sources[3].kind, 1);
  assert.ok(!("values" in cols(p)[1]), "numeric values never travel in the descriptor");
});

test("all missing is f64; mixed types are an error naming the column", () => {
  assert.equal(inferColumn("e", [null, null]).desc.type, "f64");
  assert.throws(() => prepare({ v: [1, "a"] }), /column `v`: expected values of one type/);
  assert.throws(() => prepare({ a: [1, 2], b: [1] }), /column `b` has 1 values, not 2/);
});

test("rows shorthand: keys in first-seen order, a missing key null", () => {
  assert.deepEqual(rowsToColumns([{ x: 1 }, { y: 2, x: 3 }]), { x: [1, 3], y: [null, 2] });
  const p = prepare([{ x: 1, y: 2 }, { x: 2 }]);
  assert.deepEqual(cols(p).map((c) => c.name), ["x", "y"]);
  assert.ok(Number.isNaN(materialise(p.sources[1])[1]));
});

test("JSON text is parsed with JSON.parse", () => {
  const p = prepare('{"x":[1,2],"y":[3,4]}');
  assert.deepEqual([...materialise(p.sources[1])], [3, 4]);
});

test("typed arrays: one copy into the reserved buffer, f64 inferred", () => {
  const x = new Float64Array([1, 2, 3]);
  const p = prepare({ x, y: new Int32Array([4, 5, 6]) });
  assert.deepEqual(cols(p).map((c) => c.type), ["f64", "f64"]);
  let sets = 0;
  const dst = new Float64Array(3);
  const spy = Object.assign(dst, { set: (src) => { sets++; Float64Array.prototype.set.call(dst, src); } });
  p.sources[0].write(spy);
  assert.equal(sets, 1);
  assert.deepEqual([...dst], [1, 2, 3]);
  assert.deepEqual([...materialise(p.sources[1])], [4, 5, 6]);
  const big = prepare({ v: new BigInt64Array([1n, -2n]) });
  assert.deepEqual([...materialise(big.sources[0])], [1, -2]);
});

test("the full form: types, units, strings kept in the descriptor, lengths checked", () => {
  const p = prepare({
    tables: {
      prices: {
        sorted: "t",
        columns: [
          { name: "t", type: "timestamp", time_unit: "s", values: [0, "1970-01-01T00:00:01.5Z"], zone: "America/New_York" },
          { name: "sym", type: "category", categories: ["A", "B"], values: [1, null] },
          { name: "note", type: "string", values: ["café", null] },
          { name: "ok", type: "bool", values: [true, false] },
        ],
      },
    },
    matrices: { m: { rows: 1, cols: 2, values: [1, null], col_labels: ["a", "b"] } },
  });
  const t = p.descriptor.tables.prices;
  assert.equal(t.length, 2);
  assert.equal(t.sorted, "t");
  assert.deepEqual([...materialise(p.sources[0])], [0, 1.5], "values in the column's time_unit");
  assert.deepEqual([...materialise(p.sources[1])], [1, MISSING_CODE]);
  assert.equal(p.sources[2], null);
  assert.deepEqual(t.columns[2].values, ["café", null], "label text NFC-normalised");
  assert.deepEqual([...materialise(p.sources[4])].map((v) => (Number.isNaN(v) ? null : v)), [1, null], "matrix values after the tables' columns");
  assert.throws(() => materialise(prepare({ tables: { a: { columns: [{ name: "c", type: "category", categories: ["x"], values: [3] }] } } }).sources[0]), /category index below 1/);
  assert.throws(() => materialise(prepare({ tables: { a: { columns: [{ name: "b", type: "bool", values: [1] }] } } }).sources[0]), /a boolean or null/);
  assert.throws(() => prepare({ tables: {}, extra: 1 }), /unknown field `extra`/);
  assert.throws(() => prepare({ tables: { a: { columns: [{ name: "x", type: "f64" }] } } }), /no `length`/);
});

test("the binary form: a descriptor without values and typed columns by name", () => {
  const x = new Float64Array([1, 2]);
  const p = prepare({ descriptor: { tables: { d: { columns: [{ name: "x", type: "f64" }, { name: "y", type: "f64" }] } } }, columns: { x, "d/y": new Float32Array([0.5, 1.5]) } });
  assert.equal(p.descriptor.tables.d.length, 2);
  assert.deepEqual([...materialise(p.sources[1])], [0.5, 1.5]);
});

test("describe returns the descriptor and lengths, no values (REQ-EL-070)", () => {
  const p = prepare({ tables: { a: { columns: [{ name: "s", type: "string", values: ["x"] }, { name: "v", type: "f64", values: [1] }] } } });
  const d = describe(p);
  assert.equal(d.tables.a.length, 1);
  assert.ok(d.tables.a.columns.every((c) => !("values" in c)));
});

test("RFC 3339 as the core reads it", () => {
  assert.deepEqual(parseRfc3339("1970-01-01T00:00:00Z"), [0, 0]);
  assert.deepEqual(parseRfc3339("1970-01-01T01:00:00+01:00"), [0, 0]);
  assert.deepEqual(parseRfc3339("2026-10-18T14:52:00.250Z"), [Date.UTC(2026, 9, 18, 14, 52) / 1000, 250000000]);
  assert.deepEqual(parseRfc3339("2016-12-31T23:59:60Z"), [Date.UTC(2017, 0, 1) / 1000, 0], "a leap second reads as the next second");
  assert.equal(parseRfc3339("2026-02-29T00:00:00Z"), null);
  assert.equal(parseRfc3339("2026-10-18 00:00:00Z"), null);
  assert.equal(parseRfc3339("2026-10-18"), null);
  assert.equal(stampIn("1970-01-01T00:00:00.000001Z", "ns"), 1000);
  assert.equal(stampIn("1970-01-01T00:00:01Z", "us"), 1e6);
});

test("appends join the first table, typed as its columns, new labels added", () => {
  const p = prepare({ t: [1, 2], sym: ["a", "b"] });
  const q = appendTo(p, [{ t: 3, sym: "c" }, { t: 4, sym: "a" }]);
  assert.equal(q.descriptor.tables.data.length, 4);
  assert.deepEqual([...materialise(q.sources[0])], [1, 2, 3, 4]);
  assert.deepEqual(q.descriptor.tables.data.columns[1].categories, ["a", "b", "c"]);
  assert.deepEqual([...materialise(q.sources[1])], [0, 1, 2, 0]);
  const r = appendTo(q, { t: new Float64Array([5]) });
  assert.deepEqual([...materialise(r.sources[0])], [1, 2, 3, 4, 5]);
  assert.equal(materialise(r.sources[1])[4], MISSING_CODE);
  assert.throws(() => appendTo(p, { nope: [1] }), /no column `nope`/);
});

test("nfcDeep normalises strings, keeps keys and typed arrays", () => {
  const f = new Float64Array(1);
  const o = nfcDeep({ "é": ["é"], f });
  assert.deepEqual(Object.keys(o), ["e\u0301", "f"]);
  assert.equal(o["é"][0], "é");
  assert.equal(o.f, f);
});

test("emptyLike keeps the first table's columns", () => {
  const e = emptyLike(describe(prepare({ a: [1], b: ["x"] })));
  assert.equal(e.tables.data.length, 0);
  assert.deepEqual(e.tables.data.columns.map((c) => c.name), ["a", "b"]);
});
