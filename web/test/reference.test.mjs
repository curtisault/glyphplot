// The core's ports in wasm against the d3 captures in
// crates/glyphplot/tests/fixtures (REQ-TICK-010, REQ-TICK-020, REQ-REF-010),
// and the fixtures generated in wasm against their native digests
// (REQ-FIX-010), each case compared bitwise as tests/reference.rs compares it.
//
// No release export reaches ticks, formats or fixtures, so these run against
// the debug build through two debug-only exports (crates/wasm/src/debug.rs):
//
// - `debug_reference(file, line) -> string`: the core's answer to one case
//   line of tests/fixtures/<file>, written as the file writes its expected
//   part (numbers as JavaScript's String(x), separated as in the file); a
//   `zone` line of zone-ticks.txt sets the zone the lines after it are
//   answered in and answers with the zone's name, so a file's lines go to it
//   in order;
// - `debug_fixture_digest(name) -> string`: the digest of the fixture the
//   module generates, as `{:#018x}` (fixtures::digest).
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { BUILD, ROOT, DEBUG, debugOnly, types, moduleOfType, fixtureIndex } from "./support/wasm.mjs";

const CAPTURES = join(ROOT, "crates/glyphplot/tests/fixtures");
const { glue } = await moduleOfType(types[0]);
const reference = (file, line) => glue.debug_reference(file, line);
const digest = (name) => glue.debug_fixture_digest(name);

const cases = (file) => readFileSync(join(CAPTURES, file), "utf8").split("\n").filter((l) => l !== "" && !l.startsWith("#"));

/** The expected part of a case line, and whether it is numbers. */
function expected(file, line) {
  if (file === "zone-ticks.txt" && line.startsWith("zone ")) return { text: line.split(" ")[1], numbers: false };
  if (file === "format.txt" || file === "time-format.txt" || line.includes("\t")) {
    const f = line.split("\t");
    const at = file === "log.txt" ? { format: 6 }[f[0]] : { fixed: 3, exponential: 3, precision: 3, string: 2, format: 3, tick: 5, label: 2, strftime: 3 }[f[0]];
    assert.ok(at !== undefined, `unknown case ${f[0]}`);
    return { text: f.slice(at).join("\t"), numbers: false };
  }
  const i = line.indexOf(" :");
  return { text: line.slice(i + 2).trim(), numbers: true };
}

/** Bitwise equality of two lists of numbers written as String(x), NaN equal to NaN. */
function same(a, b) {
  const x = a.split(" ").filter(Boolean).map(Number);
  const y = b.split(" ").filter(Boolean).map(Number);
  return x.length === y.length && x.every((v, i) => Object.is(v, y[i]) || (Number.isNaN(v) && Number.isNaN(y[i])));
}

for (const [file, reqs] of [
  ["ticks.txt", "REQ-TICK-010"],
  ["nice.txt", "REQ-SCALE-030"],
  ["time-ticks.txt", "REQ-TICK-020"],
  ["format.txt", "REQ-FMT-010"],
  ["time-format.txt", "REQ-FMT-030"],
  ["log.txt", "REQ-SCALE-040, REQ-TICK-060"],
  ["zone-ticks.txt", "REQ-TZ-030"],
  ["lttb.txt", "REQ-SAMPLE-060"],
]) {
  test(`${BUILD}: ${file} in wasm equals the d3 capture bitwise (${reqs}, REQ-REF-010)`, { skip: debugOnly && "the reference exports are debug-only" }, () => {
    const bad = [];
    const lines = cases(file);
    for (const line of lines) {
      const want = expected(file, line);
      const got = reference(file, line);
      if (want.numbers ? !same(got, want.text) : got !== want.text) bad.push(`${line}\n  got ${JSON.stringify(got)}`);
    }
    assert.ok(lines.length > 0);
    assert.equal(bad.length, 0, `${bad.length} of ${lines.length} cases differ:\n${bad.slice(0, 20).join("\n")}`);
  });
}

test(`${BUILD}: every fixture generated in wasm has its native digest (REQ-FIX-010)`, { skip: debugOnly && "the reference exports are debug-only" }, () => {
  const index = fixtureIndex();
  assert.equal(index.fixtures.length, 23, "the plan's 23 fixtures");
  for (const f of index.fixtures) assert.equal(digest(f.name), f.digest, f.name);
});

test(`${BUILD}: the reference exports are the debug build's alone, and refuse what they do not know (REQ-WASM-060)`, () => {
  if (!DEBUG) {
    assert.equal(glue.debug_reference, undefined);
    assert.equal(glue.debug_fixture_digest, undefined);
    return;
  }
  assert.throws(() => reference("nope.txt", "1 2 3 : 4"), /no capture file/);
  assert.throws(() => reference("ticks.txt", "spin 0 1 2 : 3"), /unknown case/);
  assert.throws(() => reference("log.txt", "spin 10 1 10 : 1"), /unknown case/);
  assert.throws(() => reference("lttb.txt", "spin 1 2 3 : 0"), /unknown case/);
  assert.throws(() => digest("no-such-fixture"), /no fixture named/);
});
