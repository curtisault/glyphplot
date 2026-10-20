// shape.ts: the shape a chart took its data as, decided as the core's
// `Data::shape` decides it, so a type change carries data only to a chart
// that takes that shape (REQ-EL-050, REQ-DATA-110).
import { test } from "node:test";
import assert from "node:assert/strict";
import { prepare } from "../src/data.ts";
import { shapeOf, encodeOf, SHAPE_CHANNELS } from "../src/shape.ts";

const d = (input) => prepare(input).descriptor;

test("the first shape whose channels resolve, by name, role or encode", () => {
  const ohlc = d({ x: [1, 2], open: [1, 2], high: [2, 3], low: [0, 1], close: [1, 2] });
  assert.equal(shapeOf(ohlc, ["series", "ohlcv"]), "ohlcv");
  assert.equal(shapeOf(d({ x: [1, 2], y: [3, 4] }), ["ohlcv", "series"]), "series");
  // encode names the columns a channel takes.
  const prices = d({ t: [1, 2], price: [3, 4] });
  assert.equal(shapeOf(prices, ["points", "series"], { x: "t", y: "price" }), "points");
  assert.equal(shapeOf(prices, ["points", "series"], { x: "t", y: ["price", "missing"] }), "points", "no shape fits: the first of the kind");
  // A role resolves its channel.
  const roles = { version: 1, tables: { data: { columns: [{ name: "when", type: "f64", role: "x", values: [1] }, { name: "v", type: "f64", role: "y", values: [2] }] } } };
  assert.equal(shapeOf(d(roles), ["ohlcv", "series"]), "series");
});

test("without every channel, the first shape of the data's kind; null for none", () => {
  const xa = d({ x: [0, 1], a: [1, 2] });
  assert.equal(shapeOf(xa, ["series"]), "series", "the line chart takes {x, a} as series");
  assert.equal(shapeOf(xa, ["matrix"]), null, "a table is no matrix");
  assert.equal(shapeOf(xa, ["graph"]), null);
  const m = { version: 1, tables: {}, matrices: { m: { rows: 1, cols: 1 } } };
  assert.equal(shapeOf(m, ["series", "matrix"]), "matrix");
});

test("every shape of meta.schema.json has channels; encodeOf reads the option", () => {
  assert.deepEqual(Object.keys(SHAPE_CHANNELS).sort(), ["bins", "counts", "ephemeris", "geo", "graph", "matrix", "ohlcv", "orbits", "points", "series", "sky", "values"]);
  assert.deepEqual(encodeOf({ encode: { x: "t" } }), { x: "t" });
  assert.deepEqual(encodeOf({ encode: [1] }), {});
  assert.deepEqual(encodeOf({}), {});
});
