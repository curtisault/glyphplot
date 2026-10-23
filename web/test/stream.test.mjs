// The shell's streaming and `src` parts without wasm: the tail's ring rules
// (REQ-STREAM-020, 030, 040), its staging layout (the boundary's
// `staging(n)`), the CSV and NDJSON readers and the format by Content-Type
// or extension (REQ-DATA-150), and the Arrow row count and a file's stream
// part, over the tests' own Arrow writer.
import { test } from "node:test";
import assert from "node:assert/strict";
import { prepare, materialise } from "../src/data.ts";
import { Tail, ageIn, xColumnOf, DEFAULT_RETAIN_ROWS } from "../src/stream.ts";
import { csvRows, parseCsv, parseNdjson, formatOf } from "../src/source.ts";
import { arrowRows, arrowStreamOf } from "../src/arrow.ts";
import { arrowFile, arrowStream, walk } from "./support/arrow.mjs";

const col = (t, i) => [...materialise(t.prepared().sources[i])];

test("a tail appends rows, rows and typed columns, typed as the table's columns (REQ-STREAM-010)", () => {
  const t = Tail.from(prepare({ x: [0, 1], y: [5, 6] }));
  t.push({ x: 2, y: 7 });
  t.push([{ x: 3, y: 8 }, { x: 4 }]);
  t.push({ x: Float64Array.of(5, 6), y: Float64Array.of(10, 11) });
  assert.equal(t.length, 7);
  assert.deepEqual(col(t, 0), [0, 1, 2, 3, 4, 5, 6]);
  assert.deepEqual(col(t, 1).map((v) => (Number.isNaN(v) ? null : v)), [5, 6, 7, 8, null, 10, 11]);
  assert.equal(t.prepared().descriptor.tables.data.length, 7);
  assert.throws(() => t.push({ x: 7, nope: 1 }), /no column `nope`/);
  assert.throws(() => t.push({ x: [1, 2], y: [1] }), /has 1 values, not 2/);
  assert.equal(t.length, 7, "a bad batch holds nothing");
});

test("an equal x replaces the last row; a lesser x is dropped and counted as late (REQ-STREAM-040)", () => {
  const t = Tail.from(prepare({ x: [0, 1, 2], y: [1, 1, 1] }));
  t.push({ x: 2, y: 9 });
  t.push([{ x: 1, y: 5 }, { x: 3, y: 3 }, { x: 3, y: 4 }, { x: 0.5, y: 0 }]);
  assert.deepEqual(col(t, 0), [0, 1, 2, 3]);
  assert.deepEqual(col(t, 1), [1, 1, 9, 4]);
  assert.deepEqual(t.takeCounts(), { late: 2, dropped: 0 });
  assert.deepEqual(t.takeCounts(), { late: 0, dropped: 0 }, "taken once");
  assert.equal(t.newestX, 3);
});

test("retain.rows trims the oldest rows and counts them dropped; retain.age keeps the first x ≥ newest − age (REQ-STREAM-020, 030)", () => {
  const t = Tail.from(prepare({ x: [0, 1, 2], y: [0, 0, 0] }), { rows: 4 });
  assert.equal(t.capacity, 4);
  for (let i = 3; i < 10; i++) t.push({ x: i, y: i });
  assert.deepEqual(col(t, 0), [6, 7, 8, 9]);
  assert.deepEqual(t.takeCounts(), { late: 0, dropped: 6 });
  const a = Tail.from(prepare({ x: [0, 1, 2, 3], y: [0, 0, 0, 0] }), { age: 2.5 });
  a.push({ x: 5, y: 1 });
  assert.deepEqual(col(a, 0), [3, 5], "5 − 2.5 = 2.5: 3 is the first kept");
  assert.equal(Tail.from(prepare({ x: [1], y: [1] })).capacity, DEFAULT_RETAIN_ROWS);
  // A duration on a timestamp column, in its unit.
  assert.equal(ageIn("90s", { name: "t", type: "timestamp" }), 90_000);
  assert.equal(ageIn("2min", { name: "t", type: "timestamp", time_unit: "s" }), 120);
  assert.equal(ageIn("1h", { name: "x", type: "f64" }), null, "a duration needs a timestamp x");
  const ts = Tail.from(prepare({ t: [new Date(0), new Date(60_000)], y: [1, 2] }), { age: "90s" });
  ts.push({ t: new Date(150_000), y: 3 });
  assert.deepEqual(col(ts, 0), [60_000, 150_000]);
  assert.deepEqual(ts.timeSpan(), [60_000, 150_000]);
});

test("a ring at capacity keeps the newest rows through many pushes, compacting in place", () => {
  const t = Tail.from(prepare({ x: Float64Array.from({ length: 1000 }, (_, i) => i), y: new Float64Array(1000) }), { rows: 1000 });
  for (let i = 1000; i < 20_000; i++) t.push({ x: i, y: -i });
  assert.equal(t.length, 1000);
  const x = col(t, 0);
  assert.equal(x[0], 19_000);
  assert.equal(x[999], 19_999);
  assert.equal(col(t, 1)[999], -19_999);
  assert.equal(t.takeCounts().dropped, 19_000);
});

test("the staging area: the rows since the last take, as they came, column-major (the boundary's staging(n))", () => {
  const t = Tail.from(prepare({ x: [0], y: [0], s: ["a"] }));
  assert.equal(t.whole, false);
  t.push({ x: 1, y: 2, s: "a" });
  t.push([{ x: 1, y: 3, s: "a" }, { x: 0, y: 4, s: null }]);
  assert.equal(t.staged, 3);
  assert.equal(t.width, 3);
  const dst = new Float64Array(9);
  assert.equal(t.takeStaged(dst), 3);
  assert.deepEqual([...dst.subarray(0, 3)], [1, 1, 0], "x as appended, before the rules");
  assert.deepEqual([...dst.subarray(3, 6)], [2, 3, 4]);
  assert.deepEqual([...dst.subarray(6, 9)], [0, 0, 0xffffffff], "category codes, missing 4294967295");
  assert.equal(t.staged, 0);
  t.push({ x: 2, y: 1, s: "b" });
  assert.equal(t.whole, true, "a new label: the next write commits whole");
  assert.deepEqual(t.prepared().descriptor.tables.data.columns[2].categories, ["a", "b"]);
  t.committedWhole();
  assert.equal(t.whole, false);
});

test("the x column: encode, role, name, the first timestamp, the first number", () => {
  const c = (...columns) => ({ columns });
  assert.equal(xColumnOf(c({ name: "a", type: "f64" }, { name: "b", type: "f64" }), { x: "b" }), 1);
  assert.equal(xColumnOf(c({ name: "a", type: "f64" }, { name: "b", type: "f64", role: "x" }), {}), 1);
  assert.equal(xColumnOf(c({ name: "a", type: "f64" }, { name: "x", type: "f64" }), {}), 1);
  assert.equal(xColumnOf(c({ name: "v", type: "f64" }, { name: "t", type: "timestamp" }), {}), 1);
  assert.equal(xColumnOf(c({ name: "k", type: "category" }, { name: "v", type: "f64" }), {}), 1);
});

test("csv: header, quoted fields with commas, quotes and newlines, CRLF, BOM, typed by content, empty fields missing (REQ-DATA-150)", () => {
  const text = '﻿t,price,"name, full",ok\r\n2026-01-01T00:00:00Z,1.5,"say ""hi""",true\r\n2026-01-02T00:00:00Z,,"two\nlines",false\n2026-01-03T00:00:00Z,-2e3,plain,\n';
  assert.equal(csvRows(text).length, 4);
  const d = parseCsv(text);
  assert.deepEqual(Object.keys(d), ["t", "price", "name, full", "ok"]);
  assert.deepEqual(d.price, [1.5, null, -2000]);
  assert.deepEqual(d["name, full"], ['say "hi"', "two\nlines", "plain"]);
  assert.deepEqual(d.ok, [true, false, null]);
  assert.deepEqual(d.t, ["2026-01-01T00:00:00Z", "2026-01-02T00:00:00Z", "2026-01-03T00:00:00Z"]);
  const p = prepare(d);
  assert.deepEqual(p.descriptor.tables.data.columns.map((c) => c.type), ["timestamp", "f64", "category", "bool"], "the shorthand types the text");
  assert.deepEqual(parseCsv("a,b\n1,x\n2\n"), { a: [1, 2], b: ["x", null] }, "a short row is missing at its end");
  assert.deepEqual(parseCsv("a\n1\n\n2\n"), { a: [1, 2] }, "blank lines are not rows");
  assert.deepEqual(parseCsv("n\n0x10\n1\n"), { n: ["0x10", "1"] }, "only decimal numbers are numbers");
  assert.throws(() => parseCsv("a,b\n1,2,3\n"), /row 2 has 3 fields/);
  assert.throws(() => parseCsv("a,a\n1,2\n"), /named twice/);
  assert.throws(() => parseCsv('a\n"open\n'), /not closed/);
  assert.throws(() => parseCsv(""), /no header/);
});

test("ndjson: one object a line, blank lines skipped (REQ-DATA-150)", () => {
  assert.deepEqual(parseNdjson('{"x":1,"y":2}\n\n{"x":2}\r\n'), [{ x: 1, y: 2 }, { x: 2 }]);
  assert.throws(() => parseNdjson('{"x":1}\n[1]\n'), /line 2 is not an object/);
  assert.throws(() => parseNdjson("{nope}\n"), /line 1/);
});

test("the format from Content-Type, else the extension (REQ-DATA-150)", () => {
  assert.equal(formatOf("text/csv; charset=utf-8", "/a.json"), "csv", "the Content-Type wins");
  assert.equal(formatOf("application/vnd.apache.arrow.stream", "/feed"), "arrow");
  assert.equal(formatOf("application/x-ndjson", "/feed"), "ndjson");
  assert.equal(formatOf("application/geo+json", "/feed"), "json");
  assert.equal(formatOf("application/octet-stream", "/data/walk.arrows?v=2"), "arrow", "an unhelpful type: the extension");
  assert.equal(formatOf(null, "https://example.org/x/walk.JSONL"), "ndjson");
  assert.equal(formatOf("text/plain", "walk.csv"), "csv");
  assert.equal(formatOf("text/plain", "/walk"), null);
});

test("arrowRows counts an Arrow stream's rows and refuses other bytes", () => {
  const w = walk(37);
  const bytes = arrowStream({ t: { type: "timestamp", values: w.t }, y: { type: "f64", values: w.y } });
  assert.equal(arrowRows(bytes), 37);
  assert.throws(() => arrowRows(new TextEncoder().encode("t,y\n1,2\n")), /not an Arrow IPC stream/);
  assert.throws(() => arrowRows(Uint8Array.of(0xff, 0xff, 0xff, 0xff, 0, 0, 0, 0)), /no schema/);
});

test("an Arrow file's rows are its stream's, and the stream part is what set_arrow is sent (REQ-DATA-150)", () => {
  const w = walk(37);
  const cols = { t: { type: "timestamp", values: w.t }, y: { type: "f64", values: w.y } };
  const stream = arrowStream(cols);
  const file = arrowFile(cols);
  assert.equal(arrowRows(file), 37);
  assert.deepEqual(arrowStreamOf(file).subarray(0, stream.length), stream);
  assert.equal(arrowStreamOf(stream), stream, "a stream is sent as it is");
});
